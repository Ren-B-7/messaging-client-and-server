use std::{env, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context;
use tracing::{debug, error, info, warn};

use tokio::net::TcpListener;
use tokio::sync::Semaphore;

use hyper::server::conn::http1;
use hyper_util::{
    rt::{TokioIo, TokioTimer},
    service::TowerToHyperService,
};

use tower::ServiceBuilder;
use tower::load_shed::LoadShedLayer;
use tower_http::compression::{CompressionLayer, CompressionLevel};

use server::{
    AppState, create_cors_layer,
    database::create,
    handlers::{admin::AdminService, user::UserService},
    tower_middle::tower_timeout_handler::TimeoutLayer,
};

use shared::config;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        error!("Usage: {} <config_path>", args[0]);
        return Err("Missing config path argument".into());
    }

    let config_path = args[1].clone();

    let db = create::open_database("messaging.db").await?;
    let app_config = config::load_config(&config_path).context("Failed to load configuration")?;

    let jwt_secret = app_config.auth.resolved_jwt_secret().unwrap();

    let web_dir = app_config.paths.web_dir.clone();
    let icons = app_config.paths.icons.clone();

    // ── max_connections: enforced via a shared semaphore ────────────────────
    //
    // Previously `max_connections` was validated in config but never used.
    // We now create a `Semaphore` with that many permits.  Each accepted
    // connection acquires one permit; when all permits are taken new
    // connections block in `listener.accept()` until a slot frees up.
    //
    // The semaphore is shared between the user and admin servers so that the
    // total across both listeners never exceeds `max_connections`.  If you
    // want independent caps, create two separate semaphores.
    let max_conn = app_config.server.max_connections;
    info!(
        "Connection limit: {} (shared across both servers)",
        max_conn
    );
    let connection_semaphore = Arc::new(Semaphore::new(max_conn));

    let live_config = config::LiveConfig::new(app_config);

    let user_router = Arc::new(server::build_user_router_with_config(
        Some(web_dir.clone()),
        Some(icons.clone()),
    ));
    let admin_router = Arc::new(server::build_admin_router_with_config(
        Some(web_dir),
        Some(icons),
    ));

    let state = AppState::new(live_config, db, jwt_secret, user_router, admin_router);

    // ── Rate limiters ─────────────────────────────────────────────────────
    //
    // Previously both servers shared AppState.rate_limiter.  That meant heavy
    // user traffic could exhaust the token-bucket budget and rate-limit the
    // admin interface during incidents — exactly when the admin is most needed.
    //
    // Now the admin server gets its own dedicated rate limiter with a much
    // more permissive budget (600 req/s, burst 1200).  The user server keeps
    // the original AppState limiter (100 req/s, burst 200).
    let admin_rate_limiter = server::RateLimiter::new(600, 1200);

    let (user_port, admin_port) = {
        let cfg = state.config.read().await;
        (
            cfg.server.port_client.unwrap_or(1337),
            cfg.server.port_admin.unwrap_or(1338),
        )
    };

    let cors_origins = state.config.read().await.auth.cors_origins.clone();

    let connection_timeouts = vec![Duration::from_secs(5), Duration::from_secs(2)];
    let user_timeout = connection_timeouts.clone();
    let admin_timeout = connection_timeouts.clone();

    let user_sock: SocketAddr = ([127, 0, 0, 1], user_port).into();
    let admin_sock: SocketAddr = ([127, 0, 0, 1], admin_port).into();

    let user_state = state.clone();
    let admin_state = state.clone();

    let rate_limiter_cleanup = state.clone().rate_limiter;
    let admin_rate_limiter_cleanup = admin_rate_limiter.clone();
    let db_cleanup = state.clone().db;
    let sse_manager_clone = state.clone().sse_manager;
    let metrics_clone = state.clone().metrics;
    let timeout = Duration::from_secs(state.config.read().await.server.timeout);

    // ── Background task: rate limiter + DB cleanup ───────────────────────────
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;

            rate_limiter_cleanup.cleanup().await;
            admin_rate_limiter_cleanup.cleanup().await;
            info!("Rate limiter cleanup completed");

            match server::login::cleanup_expired_sessions(&db_cleanup).await {
                Ok(n) if n > 0 => info!("Cleaned up {} expired sessions", n),
                Err(e) => error!("Session cleanup failed: {}", e),
                _ => {}
            }

            match server::password::cleanup_expired_reset_tokens(&db_cleanup).await {
                Ok(n) if n > 0 => info!("Cleaned up {} expired reset tokens", n),
                Err(e) => error!("Reset token cleanup failed: {}", e),
                _ => {}
            }
        }
    });

    // ── Background task: metrics snapshot + SSE cleanup ──────────────────────
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            sse_manager_clone.cleanup().await;
            info!("SSE manager cleanup completed");

            let snapshot = metrics_clone.snapshot().await;
            info!("{}", snapshot.format());
        }
    });

    // ── Background task: SIGHUP → hot-reload config ──────────────────────────
    {
        let reload_handle = state.config.clone();
        let path_for_reload = config_path.clone();
        tokio::spawn(async move {
            let mut signal =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup()) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("Could not install SIGHUP handler: {}", e);
                        return;
                    }
                };

            loop {
                signal.recv().await;
                info!(
                    "SIGHUP received — reloading config from {}",
                    path_for_reload
                );
                match config::load_config(&path_for_reload) {
                    Ok(new_cfg) => {
                        reload_handle.reload(new_cfg).await;
                        info!("Config hot-reloaded successfully");
                    }
                    Err(e) => error!("Config reload failed, keeping current config: {}", e),
                }
            }
        });
    }

    // ── Background task: SIGTERM → graceful shutdown ─────────────────────────
    {
        tokio::spawn(async move {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut signal) => {
                    signal.recv().await;
                    info!("SIGTERM received — shutting down");
                    std::process::exit(0);
                }
                Err(e) => warn!("Could not install SIGTERM handler: {}", e),
            }
        });
    }

    let user_cors = create_cors_layer(&cors_origins);
    let admin_cors = create_cors_layer(&cors_origins);

    // ── User server ──────────────────────────────────────────────────────────
    let user_sem = Arc::clone(&connection_semaphore);
    let user_server = async move {
        let listener = TcpListener::bind(user_sock)
            .await
            .context(format!("Failed to bind to {}", user_sock))
            .unwrap();

        info!("User server listening on http://{}", user_sock);

        loop {
            // Acquire a connection permit before accepting.  This is what
            // actually enforces max_connections.  If no permit is available
            // the task yields until one frees up — no connection is dropped,
            // it just waits.
            let permit = Arc::clone(&user_sem)
                .acquire_owned()
                .await
                .expect("semaphore closed");

            let (stream, addr) = listener.accept().await.unwrap();

            let user_tower_service = UserService::new(
                user_state.clone(),
                addr,
                Arc::clone(&user_state.user_router),
            );

            let tower_service = ServiceBuilder::new()
                .layer(LoadShedLayer::new())
                .layer(CompressionLayer::new().quality(CompressionLevel::Default))
                .layer(server::IpFilterLayer::new(user_state.ip_filter.clone()))
                .layer(server::RateLimiterLayer::new(
                    user_state.rate_limiter.clone(),
                ))
                .layer(TimeoutLayer::new(timeout))
                .layer(server::MetricsLayer::new(user_state.metrics.clone()))
                .layer(user_cors.clone())
                .service(user_tower_service);

            let final_service = TowerToHyperService::new(tower_service);
            let io = TokioIo::new(stream);
            let timeout = user_timeout.clone();

            tokio::task::spawn(async move {
                // `permit` is moved into this task and dropped when it ends,
                // which releases the semaphore slot and allows the next accept.
                let _permit = permit;

                let conn = http1::Builder::new()
                    .timer(TokioTimer::new())
                    .header_read_timeout(Duration::from_secs(2))
                    .serve_connection(io, final_service);
                tokio::pin!(conn);

                let mut completed = false;
                for (iter, sleep) in timeout.iter().enumerate() {
                    tokio::select! {
                        res = conn.as_mut() => {
                            match res {
                                Ok(()) => debug!("Connection closed: {}", addr),
                                Err(e) => warn!("Error serving connection {:?}", e),
                            };
                            completed = true;
                            break;
                        }
                        _ = tokio::time::sleep(*sleep) => {
                            info!(
                                "iter = {} timeout elapsed, calling graceful_shutdown",
                                iter
                            );
                            conn.as_mut().graceful_shutdown();
                        }
                    }
                }

                if !completed {
                    info!("Holding long-lived connection open: {}", addr);
                    if let Err(e) = conn.await {
                        warn!("Long-lived connection {} closed with error: {:?}", addr, e);
                    }
                }
            });
        }
    };

    // ── Admin server ─────────────────────────────────────────────────────────
    let admin_sem = Arc::clone(&connection_semaphore);
    let admin_server = async move {
        let listener = TcpListener::bind(admin_sock)
            .await
            .context(format!("Failed to bind to {}", admin_sock))
            .unwrap();

        info!("Admin server listening on http://{}", admin_sock);

        loop {
            let permit = Arc::clone(&admin_sem)
                .acquire_owned()
                .await
                .expect("semaphore closed");

            let (stream, addr) = listener.accept().await.unwrap();

            let admin_tower_service = AdminService::new(
                admin_state.clone(),
                addr,
                Arc::clone(&admin_state.admin_router),
            );

            // Admin server uses its own dedicated rate limiter so that user
            // traffic cannot starve admin operations during incidents.
            let tower_service = ServiceBuilder::new()
                .layer(LoadShedLayer::new())
                .layer(CompressionLayer::new().quality(CompressionLevel::Default))
                .layer(server::IpFilterLayer::new(admin_state.ip_filter.clone()))
                .layer(server::RateLimiterLayer::new(admin_rate_limiter.clone()))
                .layer(TimeoutLayer::new(timeout))
                .layer(server::MetricsLayer::new(admin_state.metrics.clone()))
                .layer(admin_cors.clone())
                .service(admin_tower_service);

            let final_service = TowerToHyperService::new(tower_service);
            let io = TokioIo::new(stream);
            let timeout = admin_timeout.clone();

            tokio::task::spawn(async move {
                let _permit = permit;

                let conn = http1::Builder::new()
                    .timer(TokioTimer::new())
                    .header_read_timeout(Duration::from_secs(2))
                    .serve_connection(io, final_service);
                tokio::pin!(conn);

                for (iter, sleep) in timeout.iter().enumerate() {
                    tokio::select! {
                        res = conn.as_mut() => {
                            match res {
                                Ok(()) => debug!("Connection closed: {}", addr),
                                Err(e) => warn!("Error serving connection {:?}", e),
                            };
                            break;
                        }
                        _ = tokio::time::sleep(*sleep) => {
                            info!(
                                "iter = {} timeout elapsed, calling graceful_shutdown",
                                iter
                            );
                            conn.as_mut().graceful_shutdown();
                        }
                    }
                }
            });
        }
    };

    tokio::join!(user_server, admin_server);
    info!("Both servers closed!");

    Ok(())
}
