use std::{sync::Arc, time::Duration};

use tracing::info;

use tokio_rusqlite::Connection;

use hyper::{
    Method,
    header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderValue},
};
use tower_http::cors::CorsLayer;

pub mod database;
pub mod handlers;
pub mod tower_middle;

// Re-export database modules
pub use database::{create, login, password};

// Re-export handlers
pub use handlers::{
    admin::{AdminService, build_admin_router_with_config},
    http::routes::Router,
    sse::sse::SseManager,
    user::{UserService, build_user_router_with_config},
};

// Re-export tower middleware
pub use tower_middle::{
    security::{IpFilter, Metrics, RateLimiter},
    tower_ip_filter::IpFilterLayer,
    tower_metrics::MetricsLayer,
    tower_rate_limiter::RateLimiterLayer,
    tower_timeout_handler::TimeoutLayer,
};

use shared::config::LiveConfig;

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
///
/// `started_at` is captured once at construction and never changes.  It is
/// used by the admin stats endpoint to compute accurate uptime.
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
    pub user_router: Arc<Router>,
    pub admin_router: Arc<Router>,
    /// Unix timestamp (seconds) of when this `AppState` was first constructed.
    /// Used by the admin stats endpoint to compute server uptime accurately.
    /// Previously this was passed as a hard-coded `0` to `ServerStats::build`,
    /// which caused uptime to always display the full Unix epoch duration.
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
        let rate_limiter = RateLimiter::new(100, 200);

        // Capture the process start time once.  All clones of AppState share
        // this value (it's Copy), so the admin dashboard always shows the
        // actual elapsed time since the server started — not time since the
        // last connection handler was spawned.
        let started_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        Self {
            db: Arc::new(db),
            config,
            ip_filter: IpFilter::new(),
            rate_limiter,
            metrics: Metrics::new(),
            timeout: Duration::new(10, 0),
            sse_manager: Arc::new(SseManager::new()),
            jwt_secret: Arc::new(jwt_secret),
            user_router,
            admin_router,
            started_at,
        }
    }
}

/// Create CORS layer based on environment
pub fn create_cors_layer() -> CorsLayer {
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
