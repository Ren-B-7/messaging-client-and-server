use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::pin;

use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};

// Error tracing
use anyhow::Context;
use tracing::{error, info};

mod database;
mod handlers;

use shared::config::{self, Config};

use handlers::{admin::AdminService, user::UserService};

/// Shared application state that will be passed to all handlers
/// Clone is cheap because Config is wrapped in Arc
#[derive(Clone, Debug)]
pub struct AppState {
    pub config: Arc<Config>,
}

impl AppState {
    fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing
    tracing_subscriber::fmt().init();

    // Read config file location from command line
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        error!("Usage: {} <config_path>", args[0]);
        return Err("Missing config path argument".into());
    }

    // Load config and wrap in AppState
    let config = config::load_config(&args[1]).context("Failed to load configuration")?;
    let state = AppState::new(config);

    // Use config values for socket addresses
    let user_port = state.config.server.port_client.unwrap_or(137) as u16;
    let admin_port = state.config.server.port_admin.unwrap_or(138) as u16;

    let user_sock: SocketAddr = ([127, 0, 0, 1], user_port).into();
    let admin_sock: SocketAddr = ([127, 0, 0, 1], admin_port).into();

    info!(
        "Listening on http://{} (user) and http://{} (admin)",
        user_sock, admin_sock
    );

    let connection_timeouts = vec![Duration::from_secs(30), Duration::from_secs(2)];

    // Clone state for each server
    let user_state = state.clone();
    let admin_state = state;

    let user_timeouts = connection_timeouts.clone();
    let admin_timeouts = connection_timeouts;

    let user_server = async move {
        let listener = TcpListener::bind(user_sock)
            .await
            .context(format!("Failed to bind to {}", user_sock))
            .unwrap();

        info!("User server listening on {}", user_sock);

        loop {
            let (stream, addr) = listener.accept().await.unwrap();
            let io = TokioIo::new(stream);
            let connection_timeouts_clone = user_timeouts.clone();

            // Create service instance for this connection
            let svc = UserService::new(user_state.clone(), addr);

            tokio::task::spawn(async move {
                let conn = http1::Builder::new()
                    .timer(TokioTimer::new())
                    .serve_connection(io, svc);
                pin!(conn);

                for sleep_duration in connection_timeouts_clone {
                    tokio::select! {
                        res = &mut conn => {
                            match res {
                                Ok(()) => info!("User connection from {} closed normally", addr),
                                Err(err) => error!(%err, "Connection error for {}:{}", addr.ip(), addr.port()),
                            }
                            return;
                        }
                        _ = tokio::time::sleep(sleep_duration) => {
                            info!("User connection timeout for {}, graceful shutdown", addr);
                            conn.as_mut().graceful_shutdown();
                        }
                    }
                }
            });
        }
    };

    let admin_server = async move {
        let listener = TcpListener::bind(admin_sock)
            .await
            .context(format!("Failed to bind to {}", admin_sock))
            .unwrap();

        info!("Admin server listening on {}", admin_sock);

        loop {
            let (stream, addr) = listener.accept().await.unwrap();
            let io = TokioIo::new(stream);
            let connection_timeouts_clone = admin_timeouts.clone();

            // Create service instance for this connection
            let svc = AdminService::new(admin_state.clone(), addr);

            tokio::task::spawn(async move {
                let conn = http1::Builder::new()
                    .timer(TokioTimer::new())
                    .serve_connection(io, svc);
                pin!(conn);

                for sleep_duration in connection_timeouts_clone {
                    tokio::select! {
                        res = &mut conn => {
                            match res {
                                Ok(()) => info!("Admin connection from {} closed normally", addr),
                                Err(err) => error!(%err, "Connection error for {}:{}", addr.ip(), addr.port()),
                            }
                            return;
                        }
                        _ = tokio::time::sleep(sleep_duration) => {
                            info!("Admin connection timeout for {}, graceful shutdown", addr);
                            conn.as_mut().graceful_shutdown();
                        }
                    }
                }
            });
        }
    };

    // Run both servers concurrently
    tokio::join!(user_server, admin_server);
    info!("Both servers closed!");

    Ok(())
}
