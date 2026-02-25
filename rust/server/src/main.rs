use std::{env, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::Context;
use tracing::{debug, error, info, warn};

use tokio::net::TcpListener;
use tokio_rusqlite::Connection;

use hyper::{
    Method,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderValue},
    server::conn::http1,
};
use hyper_util::{
    rt::{TokioIo, TokioTimer},
    service::TowerToHyperService,
};

use tower::ServiceBuilder;
use tower::load_shed::LoadShedLayer;
use tower_http::{
    compression::{CompressionLayer, CompressionLevel},
    cors::CorsLayer,
};

mod database;
mod handlers;
mod tower_middle;

use handlers::{admin::AdminService, sse::SseManager, user::UserService};
use tower_middle::{
    IpFilterLayer, MetricsLayer, RateLimiterLayer, TimeoutLayer,
    security::{IpFilter, Metrics, RateLimiter},
};

use shared::config::{self, LiveConfig};

/// Shared application state.
///
/// `config` is a `LiveConfig` — a cheaply-cloneable `Arc<RwLock<AppConfig>>`
/// wrapper. Every clone of `AppState` shares the **same** underlying config,
/// so an admin-triggered reload or SIGHUP is visible everywhere immediately.
///
/// `jwt_secret` is intentionally **not** inside `LiveConfig` / the hot-reload
/// path.  Changing the secret invalidates every live session instantly —
/// that is a deliberate operator action that requires a server restart, not
/// something that should happen silently on SIGHUP.
#[derive(Clone, Debug)]
pub struct AppState {
    pub db: Arc<Connection>,
    pub config: LiveConfig,
    pub ip_filter: IpFilter,
    pub rate_limiter: RateLimiter,
    pub metrics: Metrics,
    pub timeout: Duration,
    pub sse_manager: Arc<SseManager>,
    /// HMAC key used to sign and verify JWTs.  Shared via `Arc` so cloning
    /// `AppState` is cheap.  Treat this like a password — load from env/config
    /// and never log it.
    pub jwt_secret: Arc<String>,
}

impl AppState {
    fn new(config: LiveConfig, db: Connection, jwt_secret: String) -> Self {
        let rate_limiter = RateLimiter::new(100, 200);

        Self {
            db: Arc::new(db),
            config,
            ip_filter: IpFilter::new(),
            rate_limiter,
            metrics: Metrics::new(),
            timeout: Duration::new(10, 0),
            sse_manager: Arc::new(SseManager::new()),
            jwt_secret: Arc::new(jwt_secret),
        }
    }
}

/// Create CORS layer based on environment
fn create_cors_layer() -> CorsLayer {
    if cfg!(debug_assertions) {
        info!("Using permissive CORS (development mode)");
        CorsLayer::permissive()
    } else {
        info!("Using restrictive CORS (production mode)");
        CorsLayer::new()
            .allow_origin([
                "http://127.0.0.1:1337".parse::<HeaderValue>().unwrap(),
                "http://127.0.0.1:1338".parse::<HeaderValue>().unwrap(),
            ])
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::DELETE,
                Method::HEAD,
                Method::OPTIONS,
            ])
            .allow_headers([AUTHORIZATION, CONTENT_TYPE, ACCEPT])
            .allow_credentials(true)
            .max_age(Duration::from_secs(3600))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        error!("Usage: {} <config_path>", args[0]);
        return Err("Missing config path argument".into());
    }

    let config_path = args[1].clone();

    let db: Connection = database::create::open_database("messaging.db").await?;
    let app_config = config::load_config(&config_path).context("Failed to load configuration")?;

    // Resolve the JWT secret — env var takes priority over the config field.
    // Presence and minimum length are already enforced by load_config's
    // validate_config, so unwrap() is safe here.
    let jwt_secret = app_config.auth.resolved_jwt_secret().unwrap();

    let live_config = LiveConfig::new(app_config);
    let state = AppState::new(live_config, db, jwt_secret);

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

    let cleanup_state = state.clone();
    let sse_state = state.clone();
    let metrics_state = state.clone();

    // Rate limiter cleanup
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            cleanup_state.rate_limiter.cleanup().await;
            info!("Rate limiter cleanup completed");
        }
    });

    // SSE cleanup
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            sse_state.sse_manager.cleanup().await;
            debug!("SSE manager cleanup completed");
        }
    });

    // Metric snapshot
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let snapshot = metrics_state.metrics.snapshot().await;
            info!("{}", snapshot.format());
        }
    });

    // ── Background task: SIGHUP → hot-reload config ─────────────────────────
    //
    // Send `kill -HUP <pid>` to reload the config file without restarting.
    // Ports and jwt_secret are NOT re-read after a reload.
    {
        let reload_handle = state.config.clone();
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
                info!("SIGHUP received — reloading config from {}", config_path);
                match config::load_config(&config_path) {
                    Ok(new_cfg) => {
                        reload_handle.reload(new_cfg).await;
                        info!("Config hot-reloaded successfully");
                    }
                    Err(e) => error!("Config reload failed, keeping current config: {}", e),
                }
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
            let (stream, _addr) = listener.accept().await.unwrap();

            let user_tower_service = UserService::new(user_state.clone(), user_sock).await;

            // Order matters: outer layers run last on request, first on response
            let tower_service = ServiceBuilder::new()
                .layer(LoadShedLayer::new())
                .layer(IpFilterLayer::new(user_state.ip_filter.clone()))
                .layer(RateLimiterLayer::new(user_state.rate_limiter.clone()))
                .layer(TimeoutLayer::new(Duration::from_secs(10)))
                .layer(MetricsLayer::new(user_state.metrics.clone()))
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

                for (iter, sleep) in timeout.iter().enumerate() {
                    tokio::select! {
                        res = conn.as_mut() => {
                            match res {
                                Ok(()) => debug!("Connection closed"),
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

    // ── Admin server ─────────────────────────────────────────────────────────
    let admin_server = async move {
        let listener = TcpListener::bind(admin_sock)
            .await
            .context(format!("Failed to bind to {}", admin_sock))
            .unwrap();

        info!("Admin server listening on http://{}", admin_sock);

        loop {
            let (stream, _addr) = listener.accept().await.unwrap();

            let admin_tower_service = AdminService::new(admin_state.clone(), admin_sock).await;

            let tower_service = ServiceBuilder::new()
                .layer(LoadShedLayer::new())
                .layer(IpFilterLayer::new(admin_state.ip_filter.clone()))
                .layer(RateLimiterLayer::new(admin_state.rate_limiter.clone()))
                .layer(TimeoutLayer::new(Duration::from_secs(10)))
                .layer(MetricsLayer::new(admin_state.metrics.clone()))
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
                                Ok(()) => debug!("Connection closed"),
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
