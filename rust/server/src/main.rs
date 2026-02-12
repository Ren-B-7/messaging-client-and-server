use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioIo, TokioTimer};
use std::fmt;
use std::net::SocketAddr;
use tokio::net::TcpListener;

// Error tracing
use anyhow::{Context, Result};
use tracing::{error, info};

use super::handlers::{admin, user};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt().init();
    let user_sock: SocketAddr = ([127, 0, 0, 1], 1337).into();
    let admin_sock: SocketAddr = ([127, 0, 0, 1], 1338).into();

    // plan on using one port as the admin login and one port as the actual connection port, which
    // means that admins will get treated entirely differently and regular accounts and admin will
    // be split

    info!(
        "Listening on http://{} and (admin) http://{}",
        user_sock, admin_sock
    );

    let user_serv = async move {
        let listener = TcpListener::bind(user_sock)
            .await
            .context(format!("Failed to bind to {}", user_sock))
            .unwrap();
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let io = TokioIo::new(stream);
            tokio::task::spawn(async move {
                // Handle the connection from the client using HTTP1 and pass any
                // HTTP requests received on that connection to the `index1` function
                if let Err(err) = http1::Builder::new()
                    .timer(TokioTimer::new())
                    .serve_connection(io, service_fn())
                    .await
                {
                    println!("Error serving connection: {:?}", err);
                }
            });
        }
    };

    let admin_serv = async move {
        let listener = TcpListener::bind(admin_sock).await.unwrap();
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let io = TokioIo::new(stream);
            tokio::task::spawn(async move {
                if let Err(err) = http1::Builder::new()
                    .timer(TokioTimer::new())
                    .serve_connection(io, service_fn())
                    .await
                {
                    println!("Error serving connection: {:?}", err);
                }
            });
        }
    };

    // Run both servers concurrently
    tokio::join!(user_serv, admin_serv);
    info!("Both servers closed!");

    Ok(())
}
