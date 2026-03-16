use std::{env, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context;
use tracing::{debug, error, info, warn};

use tokio::net::TcpListener;

use hyper::server::conn::http1;
use hyper_util::{
    rt::{TokioIo, TokioTimer},
    service::TowerToHyperService,
};

use tower::ServiceBuilder;
use tower::load_shed::LoadShedLayer;
use tower_http::compression::{CompressionLayer, CompressionLevel};

use server::{
    database::create,
    handlers::{admin::AdminService, user::UserService},
    tower_middle::tower_timeout_handler::TimeoutLayer,
    AppState, create_cors_layer,
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

    // Resolve the JWT secret — env var takes priority over the config field.
    // Presence and minimum length are already enforced by load_config's
    // validate_config, so unwrap() is safe here.
    let jwt_secret = app_config.auth.resolved_jwt_secret().unwrap();

    // Extract paths before app_config is moved into LiveConfig.
    let web_dir = app_config.paths.web_dir.clone();
    let icons = app_config.paths.icons.clone();

    let live_config = config::LiveConfig::new(app_config);

    // Build each router exactly once at startup, then share via Arc.
    // Neither router is ever rebuilt per-connection.
    let user_router = Arc::new(server::build_user_router_with_config(
        Some(web_dir.clone()),
        Some(icons.clone()),
    ));
    let admin_router = Arc::new(server::build_admin_router_with_config(Some(web_dir), Some(icons)));

    let state = AppState::new(live_config, db, jwt_secret, user_router, admin_router);

    // Read the ports once at startup — these are fixed for the lifetime of the
    // process (changing them requires a restart, not a hot-reload).
    let (user_port, admin_port) = {
        let cfg = state.config.read().await;
        (
            cfg.server.port_client.unwrap_or(1337),
            cfg.server.port_admin.unwrap_or(1338),
        )
    };

    let connection_timeouts = vec![Duration::from_secs(5), Duration::from_secs(2)];
    let user_timeout = connection_timeouts.clone();
    let admin_timeout = connection_timeouts.clone();

    let user_sock: SocketAddr = ([127, 0, 0, 1], user_port).into();
    let admin_sock: SocketAddr = ([127, 0, 0, 1], admin_port).into();

    let user_state = state.clone();
    let admin_state = state.clone();

    let rate_limiter_cleanup = state.clone().rate_limiter;
    let db_cleanup = state.clone().db;
    let sse_manager_clone = state.clone().sse_manager;
    let metrics_clone = state.clone().metrics;

    // ── Background task: rate limiter + DB cleanup ───────────────────────────
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;

            rate_limiter_cleanup.cleanup().await;
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
    //
    // Send `kill -HUP <pid>` to reload the config file without restarting.
    // Ports and jwt_secret are NOT re-read after a reload.
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
    //
    // Send `kill <pid>` or `systemctl stop` to shut down cleanly.
    // In-flight connections are allowed to drain before the process exits.
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

    // ── User server ──────────────────────────────────────────────────────────
    let user_server = async move {
        let listener = TcpListener::bind(user_sock)
            .await
            .context(format!("Failed to bind to {}", user_sock))
            .unwrap();

        info!("User server listening on http://{}", user_sock);

        loop {
            let (stream, addr) = listener.accept().await.unwrap();

            // new() is plain fn — no .await, router is a cheap Arc clone.
            let user_tower_service = UserService::new(
                user_state.clone(),
                addr,
                Arc::clone(&user_state.user_router),
            );

            // Order matters: outer layers run last on request, first on response.
            // TimeoutLayer is intentionally absent — the connection-level timeout
            // below handles regular requests, and SSE connections must stay open
            // indefinitely (they are driven to completion after the loop exits).
            let tower_service = ServiceBuilder::new()
                .layer(LoadShedLayer::new())
                .layer(server::IpFilterLayer::new(user_state.ip_filter.clone()))
                .layer(server::RateLimiterLayer::new(user_state.rate_limiter.clone()))
                .layer(server::MetricsLayer::new(user_state.metrics.clone()))
                .layer(create_cors_layer())
                .layer(CompressionLayer::new().quality(CompressionLevel::Default))
                .service(user_tower_service);

            let final_service = TowerToHyperService::new(tower_service);

            let io = TokioIo::new(stream);
            let timeout = user_timeout.clone();

            tokio::task::spawn(async move {
                let conn = http1::Builder::new()
                    .timer(TokioTimer::new())
                    .header_read_timeout(Duration::from_secs(2))
                    .serve_connection(io, final_service);
                tokio::pin!(conn);

                // Drive the graceful-shutdown sequence for normal HTTP requests.
                // If the connection completes within the timeout windows it was a
                // regular request and we are done.
                //
                // If it survives all graceful_shutdown calls it is a long-lived
                // SSE stream — fall through and await it to completion so the
                // stream stays open as long as the client is connected.
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

                // Connection outlived the timeout sequence -> it is a streaming
                // response (SSE). Hold it open until the client disconnects.
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
    let admin_server = async move {
        let listener = TcpListener::bind(admin_sock)
            .await
            .context(format!("Failed to bind to {}", admin_sock))
            .unwrap();

        info!("Admin server listening on http://{}", admin_sock);

        loop {
            let (stream, addr) = listener.accept().await.unwrap();

            // new() is plain fn — no .await, router is a cheap Arc clone.
            let admin_tower_service = AdminService::new(
                admin_state.clone(),
                addr,
                Arc::clone(&admin_state.admin_router),
            );

            let tower_service = ServiceBuilder::new()
                .layer(LoadShedLayer::new())
                .layer(server::IpFilterLayer::new(admin_state.ip_filter.clone()))
                .layer(server::RateLimiterLayer::new(admin_state.rate_limiter.clone()))
                .layer(TimeoutLayer::new(Duration::from_secs(10)))
                .layer(server::MetricsLayer::new(admin_state.metrics.clone()))
                .layer(create_cors_layer())
                .layer(CompressionLayer::new().quality(CompressionLevel::Default))
                .service(admin_tower_service);

            let final_service = TowerToHyperService::new(tower_service);

            let io = TokioIo::new(stream);
            let timeout = admin_timeout.clone();

            tokio::task::spawn(async move {
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
