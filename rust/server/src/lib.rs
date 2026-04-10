use std::{sync::Arc, time::Duration};

use hyper::{
    Method,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderValue},
};
use tokio_rusqlite::Connection;
use tower_http::cors::CorsLayer;
use tracing::info;

pub mod database;
pub mod handlers;
pub mod tower_middle;

pub use database::{create, login, password};
pub use handlers::{
    admin::{AdminService, build_admin_router_with_config},
    http::routes::Router,
    sse::sse_helper::SseManager,
    user::{UserService, build_user_router_with_config},
};
pub use tower_middle::{
    security::{IpFilter, Metrics, RateLimiter},
    tower_addr::AddAddrLayer,
    tower_ip_filter::IpFilterLayer,
    tower_metrics::MetricsLayer,
    tower_rate_limiter::RateLimiterLayer,
    tower_timeout_handler::TimeoutLayer,
};

use shared::config::LiveConfig;

/// Shared application state.
///
/// `started_at` is captured once at construction and never changes — used by
/// the admin stats endpoint to compute accurate server uptime.
///
/// `jwt_secret` is outside `LiveConfig` intentionally: rotating it on SIGHUP
/// would invalidate every live session immediately, so it requires a restart.
#[derive(Clone, Debug)]
pub struct AppState {
    pub db: Arc<Connection>,
    pub config: LiveConfig,
    pub ip_filter: IpFilter,
    pub rate_limiter: RateLimiter,
    pub metrics: Metrics,
    pub sse_manager: Arc<SseManager>,
    pub jwt_secret: Arc<String>,
    pub user_router: Arc<Router>,
    pub admin_router: Arc<Router>,
    pub started_at: i64,
}

impl AppState {
    pub fn new(
        config: LiveConfig,
        db: Connection,
        jwt_secret: String,
        user_router: Arc<Router>,
        admin_router: Arc<Router>,
    ) -> Self {
        let started_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Self {
            db: Arc::new(db),
            config,
            ip_filter: IpFilter::new(),
            rate_limiter: RateLimiter::new(100, 200),
            metrics: Metrics::new(),
            sse_manager: Arc::new(SseManager::new()),
            jwt_secret: Arc::new(jwt_secret),
            user_router,
            admin_router,
            started_at,
        }
    }
}

/// Build a CORS layer from a list of allowed origins loaded from config.
///
/// Previously hardcoded to `http://127.0.0.1:1337/1338`, which broke every
/// deployment behind a real domain. Now driven by `auth.cors_origins` in
/// config.toml, with a sensible localhost default.
///
/// Debug builds are always permissive regardless of the list.
pub fn create_cors_layer(allowed_origins: &[String]) -> CorsLayer {
    if cfg!(debug_assertions) {
        info!("Using permissive CORS (development mode)");
        return CorsLayer::permissive();
    }

    info!(
        "Using restrictive CORS (production mode) — {} origin(s)",
        allowed_origins.len()
    );

    let origins: Vec<HeaderValue> = allowed_origins
        .iter()
        .filter_map(|o| {
            o.parse::<HeaderValue>()
                .map_err(|e| tracing::warn!("Ignoring invalid CORS origin '{}': {}", o, e))
                .ok()
        })
        .collect();

    if origins.is_empty() {
        tracing::warn!(
            "No valid CORS origins configured — all cross-origin requests will be blocked. \
             Set auth.cors_origins in config.toml."
        );
    }

    CorsLayer::new()
        .allow_origin(origins)
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
