use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use tracing::{error, info};

use tokio::net::TcpListener;
use tower::ServiceBuilder;

// CORS and Compression imports
use hyper::Method;
use hyper::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::service::TowerToHyperService;
use tower::load_shed::LoadShedLayer;
use tower_http::compression::{CompressionLayer, CompressionLevel};
use tower_http::cors::CorsLayer;

mod database;
mod handlers;
mod tower_middle;

use shared::config::{self, Config};
use tokio_rusqlite::Connection;

use handlers::{admin::AdminService, user::UserService};
use tower_middle::security::{IpFilter, Metrics, RateLimiter};

// Import Tower middleware
use tower_middle::{IpFilterLayer, MetricsLayer, RateLimiterLayer, TimeoutLayer};

use crate::database::create;

/// Shared application state
#[derive(Clone, Debug)]
pub struct AppState {
    pub db: Arc<Connection>,
    pub config: Arc<Config>,
    pub ip_filter: IpFilter,
    pub rate_limiter: RateLimiter,
    pub metrics: Metrics,
    pub timeout: Duration,
}

impl AppState {
    fn new(config: Config, db: Connection) -> Self {
        let rate_limiter = RateLimiter::new(100, 200);

        Self {
            db: Arc::new(db),
            config: Arc::new(config),
            ip_filter: IpFilter::new(),
            rate_limiter,
            metrics: Metrics::new(),
            timeout: Duration::new(10, 0),
        }
    }
}

/// Create CORS layer based on environment
fn create_cors_layer() -> CorsLayer {
    if cfg!(debug_assertions) {
        // Development: permissive CORS
        info!("Using permissive CORS (development mode)");
        CorsLayer::permissive()
    } else {
        // Production: restrictive CORS
        info!("Using restrictive CORS (production mode)");
        CorsLayer::new()
            // Allow multiple origins if needed
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

    let db: Connection = database::create::open_database("messaging.db").await?;
    let config = config::load_config(&args[1]).context("Failed to load configuration")?;
    let state = AppState::new(config, db);

    info!("IP filter configured");

    let user_port = state.config.server.port_client.unwrap_or(1337) as u16;
    let admin_port = state.config.server.port_admin.unwrap_or(1338) as u16;

    let user_sock: SocketAddr = ([127, 0, 0, 1], user_port).into();
    let admin_sock: SocketAddr = ([127, 0, 0, 1], admin_port).into();

    // Clone state for each server
    let user_state = state.clone();
    let admin_state = state.clone();
    let metrics_state = state.clone();

    // Background tasks
    let cleanup_state = user_state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            cleanup_state.rate_limiter.cleanup().await;
            info!("Rate limiter cleanup completed");
        }
    });

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let snapshot = metrics_state.metrics.snapshot().await;
            info!("{}", snapshot.format());
        }
    });

    let user_server = async move {
        let listener = TcpListener::bind(user_sock)
            .await
            .context(format!("Failed to bind to {}", user_sock))
            .unwrap();

        info!("User server listening on http://{}", user_sock);

        loop {
            let (stream, addr) = listener.accept().await.unwrap();

            let user_tower_service = UserService::new(user_state.clone(), user_sock);

            // Order matters! Outer layers run last on request, first on response
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

            tokio::task::spawn(async move {
                let conn = http1::Builder::new()
                    .timer(TokioTimer::new())
                    .header_read_timeout(Duration::from_secs(2))
                    .serve_connection(io, final_service);

                if let Err(err) = conn.await {
                    error!("Connection error for {}: {:?}", addr, err);
                }
            });
        }
    };

    let admin_server = async move {
        let listener = TcpListener::bind(admin_sock)
            .await
            .context(format!("Failed to bind to {}", admin_sock))
            .unwrap();

        info!("Admin server listening on http://{}", admin_sock);

        loop {
            let (stream, addr) = listener.accept().await.unwrap();

            // Step 1: Create Hyper service
            let admin_tower_service = AdminService::new(admin_state.clone(), admin_sock);

            let tower_service = ServiceBuilder::new()
                .layer(LoadShedLayer::new())
                .layer(IpFilterLayer::new(admin_state.ip_filter.clone()))
                .layer(RateLimiterLayer::new(admin_state.rate_limiter.clone()))
                .layer(TimeoutLayer::new(Duration::from_secs(10)))
                .layer(MetricsLayer::new(admin_state.metrics.clone()))
                .layer(create_cors_layer())
                .layer(CompressionLayer::new().quality(CompressionLevel::Default))
                .service(admin_tower_service);

            // Step 4: Convert Tower → Hyper
            let final_service = TowerToHyperService::new(tower_service);

            let io = TokioIo::new(stream);

            tokio::task::spawn(async move {
                let conn = http1::Builder::new()
                    .timer(TokioTimer::new())
                    .header_read_timeout(Duration::from_secs(2))
                    .serve_connection(io, final_service);

                if let Err(err) = conn.await {
                    error!("Admin connection error for {}: {:?}", addr, err);
                }
            });
        }
    };

    tokio::join!(user_server, admin_server);
    info!("Both servers closed!");

    Ok(())
}
