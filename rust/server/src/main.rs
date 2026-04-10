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

    let state = AppState::new(
        live_config.clone(),
        db,
        jwt_secret,
        user_router,
        admin_router,
    );

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

    let cors_origins = state.config.read().await.auth.cors_origins.clone();

    let connection_timeouts = vec![Duration::from_secs(5), Duration::from_secs(2)];
    let user_timeout = connection_timeouts.clone();
    let admin_timeout = connection_timeouts.clone();

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

    let (shutdown_tx, _) = tokio::sync::broadcast::channel::<()>(1);
    let (restart_tx, _) = tokio::sync::broadcast::channel::<()>(1);

    // ── Background task: SIGTERM/SIGINT → graceful shutdown ──────────────────
    {
        let shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            let mut sigterm =
                match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("Could not install SIGTERM handler: {}", e);
                        return;
                    }
                };

            tokio::select! {
                _ = tokio::signal::ctrl_c() => info!("SIGINT received — starting graceful shutdown"),
                _ = sigterm.recv() => info!("SIGTERM received — starting graceful shutdown"),
            }

            let _ = shutdown_tx.send(());
        });
    }

    // ── Background task: Config watch → restart servers on port change ────────
    {
        let mut config_rx = live_config.subscribe();
        let restart_tx = restart_tx.clone();
        tokio::spawn(async move {
            let mut current_bind = config_rx.borrow().server.bind.clone();
            let mut current_user_port = config_rx.borrow().server.port_client;
            let mut current_admin_port = config_rx.borrow().server.port_admin;

            while config_rx.changed().await.is_ok() {
                let cfg = config_rx.borrow();
                let new_bind = &cfg.server.bind;
                let new_user_port = cfg.server.port_client;
                let new_admin_port = cfg.server.port_admin;

                if new_bind != &current_bind
                    || new_user_port != current_user_port
                    || new_admin_port != current_admin_port
                {
                    info!(
                        "Config change detected (bind/ports: {} {}:{} → {} {}:{}). Triggering server restart…",
                        current_bind,
                        current_user_port.unwrap_or(1337),
                        current_admin_port.unwrap_or(1338),
                        new_bind,
                        new_user_port.unwrap_or(1337),
                        new_admin_port.unwrap_or(1338)
                    );

                    current_bind = new_bind.clone();
                    current_user_port = new_user_port;
                    current_admin_port = new_admin_port;

                    let _ = restart_tx.send(());
                }
            }
        });
    }

    let user_cors = create_cors_layer(&cors_origins);
    let admin_cors = create_cors_layer(&cors_origins);

    // ── User server ──────────────────────────────────────────────────────────
    let user_sem = Arc::clone(&connection_semaphore);
    let user_state_loop = state.clone();
    let user_shutdown_tx = shutdown_tx.clone();
    let user_restart_tx = restart_tx.clone();
    let user_server = async move {
        loop {
            let mut user_shutdown_rx = user_shutdown_tx.subscribe();
            let mut user_loop_restart_rx = user_restart_tx.subscribe();

            let user_sock: SocketAddr = {
                let cfg = user_state_loop.config.read().await;
                let bind = cfg
                    .server
                    .bind
                    .parse::<std::net::IpAddr>()
                    .unwrap_or([127, 0, 0, 1].into());
                (bind, cfg.server.port_client.unwrap_or(1337)).into()
            };

            let listener = match TcpListener::bind(user_sock).await {
                Ok(l) => l,
                Err(e) => {
                    error!("Failed to bind User server to {}: {}", user_sock, e);
                    tokio::select! {
                        _ = user_loop_restart_rx.recv() => continue,
                        _ = user_shutdown_rx.recv() => break,
                    }
                }
            };

            info!("User server listening on http://{}", user_sock);

            loop {
                let (stream, addr) = tokio::select! {
                    res = listener.accept() => {
                        match res {
                            Ok(res) => res,
                            Err(e) => {
                                error!("User server accept error: {}", e);
                                tokio::time::sleep(Duration::from_millis(100)).await;
                                continue;
                            }
                        }
                    }
                    _ = user_shutdown_rx.recv() => {
                        info!("User server shutting down loop");
                        return;
                    }
                    _ = user_loop_restart_rx.recv() => {
                        info!("User server restarting due to config change");
                        break;
                    }
                };

                let permit = match user_sem.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => break,
                };

                let user_tower_service = UserService::new(
                    user_state_loop.clone(),
                    addr,
                    Arc::clone(&user_state_loop.user_router),
                );

                let tower_service = ServiceBuilder::new()
                    .layer(server::AddAddrLayer::new(addr))
                    .layer(LoadShedLayer::new())
                    .layer(CompressionLayer::new().quality(CompressionLevel::Default))
                    .layer(server::IpFilterLayer::new(
                        user_state_loop.ip_filter.clone(),
                    ))
                    .layer(server::RateLimiterLayer::new(
                        user_state_loop.rate_limiter.clone(),
                    ))
                    .layer(TimeoutLayer::new(timeout))
                    .layer(server::MetricsLayer::new(user_state_loop.metrics.clone()))
                    .layer(user_cors.clone())
                    .service(user_tower_service);

                let final_service = TowerToHyperService::new(tower_service);
                let io = TokioIo::new(stream);
                let timeout = user_timeout.clone();
                let mut shutdown_rx = user_shutdown_tx.subscribe();
                let mut restart_rx = user_restart_tx.subscribe();

                tokio::task::spawn(async move {
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
                            _ = shutdown_rx.recv() => {
                                info!("Shutdown received, calling graceful_shutdown for {}", addr);
                                conn.as_mut().graceful_shutdown();
                            }
                            _ = restart_rx.recv() => {
                                info!("Restart received, calling graceful_shutdown for {}", addr);
                                conn.as_mut().graceful_shutdown();
                            }
                        }
                    }

                    if !completed {
                        info!("Holding long-lived connection open: {}", addr);
                        tokio::select! {
                            res = conn.as_mut() => {
                                if let Err(e) = res {
                                    warn!("Long-lived connection {} closed with error: {:?}", addr, e);
                                }
                            }
                            _ = shutdown_rx.recv() => {
                                info!("Shutdown received, closing long-lived connection {}", addr);
                                conn.as_mut().graceful_shutdown();
                                let _ = conn.await;
                            }
                            _ = restart_rx.recv() => {
                                info!("Restart received, closing long-lived connection {}", addr);
                                conn.as_mut().graceful_shutdown();
                                let _ = conn.await;
                            }
                        }
                    }
                });
            }
        }
    };

    // ── Admin server ─────────────────────────────────────────────────────────
    let admin_sem = Arc::clone(&connection_semaphore);
    let admin_state_loop = state.clone();
    let admin_shutdown_tx = shutdown_tx.clone();
    let admin_restart_tx = restart_tx.clone();
    let admin_server = async move {
        loop {
            let mut admin_shutdown_rx = admin_shutdown_tx.subscribe();
            let mut admin_loop_restart_rx = admin_restart_tx.subscribe();

            let admin_sock: SocketAddr = {
                let cfg = admin_state_loop.config.read().await;
                let bind = cfg
                    .server
                    .bind
                    .parse::<std::net::IpAddr>()
                    .unwrap_or([127, 0, 0, 1].into());
                (bind, cfg.server.port_admin.unwrap_or(1338)).into()
            };

            let listener = match TcpListener::bind(admin_sock).await {
                Ok(l) => l,
                Err(e) => {
                    error!("Failed to bind Admin server to {}: {}", admin_sock, e);
                    tokio::select! {
                        _ = admin_loop_restart_rx.recv() => continue,
                        _ = admin_shutdown_rx.recv() => break,
                    }
                }
            };

            info!("Admin server listening on http://{}", admin_sock);

            loop {
                let (stream, addr) = tokio::select! {
                    res = listener.accept() => {
                        match res {
                            Ok(res) => res,
                            Err(e) => {
                                error!("Admin server accept error: {}", e);
                                tokio::time::sleep(Duration::from_millis(100)).await;
                                continue;
                            }
                        }
                    }
                    _ = admin_shutdown_rx.recv() => {
                        info!("Admin server shutting down loop");
                        return;
                    }
                    _ = admin_loop_restart_rx.recv() => {
                        info!("Admin server restarting due to config change");
                        break;
                    }
                };

                let permit = match admin_sem.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => break,
                };

                let admin_tower_service = AdminService::new(
                    admin_state_loop.clone(),
                    addr,
                    Arc::clone(&admin_state_loop.admin_router),
                );

                let tower_service = ServiceBuilder::new()
                    .layer(server::AddAddrLayer::new(addr))
                    .layer(LoadShedLayer::new())
                    .layer(CompressionLayer::new().quality(CompressionLevel::Default))
                    .layer(server::IpFilterLayer::new(
                        admin_state_loop.ip_filter.clone(),
                    ))
                    .layer(server::RateLimiterLayer::new(admin_rate_limiter.clone()))
                    .layer(TimeoutLayer::new(timeout))
                    .layer(server::MetricsLayer::new(admin_state_loop.metrics.clone()))
                    .layer(admin_cors.clone())
                    .service(admin_tower_service);

                let final_service = TowerToHyperService::new(tower_service);
                let io = TokioIo::new(stream);
                let timeout = admin_timeout.clone();
                let mut shutdown_rx = admin_shutdown_tx.subscribe();
                let mut restart_rx = admin_restart_tx.subscribe();

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
                            _ = shutdown_rx.recv() => {
                                info!("Shutdown received, calling graceful_shutdown for {}", addr);
                                conn.as_mut().graceful_shutdown();
                            }
                            _ = restart_rx.recv() => {
                                info!("Restart received, calling graceful_shutdown for {}", addr);
                                conn.as_mut().graceful_shutdown();
                            }
                        }
                    }
                });
            }
        }
    };

    tokio::join!(user_server, admin_server);
    info!("Both servers closed!");

    Ok(())
}
