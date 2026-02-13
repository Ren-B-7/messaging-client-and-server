use bytes::Bytes;
use http_body_util::Empty;
use hyper::header::{HeaderValue, UPGRADE};
use hyper::upgrade::Upgraded;
use hyper::{Request, Response, StatusCode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use anyhow::{anyhow, Result};
use tracing::{debug, info, warn, error};

/// Check if a request contains an upgrade header
pub fn is_upgrade_request(req: &Request<hyper::body::Incoming>) -> bool {
    let is_upgrade = req.headers().contains_key(UPGRADE);
    if is_upgrade {
        debug!("Upgrade request detected");
    }
    is_upgrade
}

/// Get the upgrade protocol from the request headers
pub fn get_upgrade_protocol(req: &Request<hyper::body::Incoming>) -> Option<String> {
    req.headers()
        .get(UPGRADE)
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            debug!("Upgrade protocol: {}", s);
            s.to_string()
        })
}

/// Create a response accepting the upgrade to a specific protocol
pub fn accept_upgrade(protocol: &str) -> Response<Empty<Bytes>> {
    info!("Accepting upgrade to protocol: {}", protocol);
    let mut res = Response::new(Empty::new());
    *res.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
    res.headers_mut()
        .insert(UPGRADE, HeaderValue::from_str(protocol).unwrap());
    res
}

/// Create a response rejecting the upgrade
pub fn reject_upgrade() -> Response<Empty<Bytes>> {
    warn!("Upgrade request rejected");
    let mut res = Response::new(Empty::new());
    *res.status_mut() = StatusCode::BAD_REQUEST;
    res
}

/// Handle WebSocket upgrade
pub async fn handle_websocket_upgrade(
    mut req: Request<hyper::body::Incoming>,
) -> Result<Response<Empty<Bytes>>> {
    // Validate upgrade request
    if !is_upgrade_request(&req) {
        return Ok(reject_upgrade());
    }

    let protocol = get_upgrade_protocol(&req);
    if protocol.as_deref() != Some("websocket") {
        warn!("WebSocket upgrade requested but protocol mismatch: {:?}", protocol);
        return Ok(reject_upgrade());
    }

    // Spawn a task to handle the upgraded connection
    tokio::task::spawn(async move {
        match hyper::upgrade::on(&mut req).await {
            Ok(upgraded) => {
                info!("WebSocket upgrade successful");
                if let Err(e) = websocket_io(upgraded).await {
                    error!("WebSocket I/O error: {}", e);
                }
            }
            Err(e) => {
                error!("WebSocket upgrade failed: {}", e);
            }
        }
    });

    Ok(accept_upgrade("websocket"))
}

/// Handle I/O on the upgraded WebSocket connection
async fn websocket_io(upgraded: Upgraded) -> Result<()> {
    // Note: In your actual implementation, wrap upgraded with TokioIo from support
    // For example: let mut upgraded = TokioIo::new(upgraded);
    
    // Simple echo server for WebSocket frames
    // This is a simplified version - actual WebSocket implementation
    // requires proper frame handling
    
    debug!("WebSocket I/O handler started");
    
    // Placeholder - you'll need to implement proper WebSocket protocol
    Ok(())
}

/// Handle custom protocol upgrade
pub async fn handle_custom_upgrade(
    mut req: Request<hyper::body::Incoming>,
    protocol: &'static str,
    handler: fn(Upgraded) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>,
) -> Result<Response<Empty<Bytes>>> {
    if !is_upgrade_request(&req) {
        return Ok(reject_upgrade());
    }

    let req_protocol = get_upgrade_protocol(&req);
    if req_protocol.as_deref() != Some(protocol) {
        warn!("Custom upgrade requested for {} but got: {:?}", protocol, req_protocol);
        return Ok(reject_upgrade());
    }

    info!("Handling custom protocol upgrade: {}", protocol);

    tokio::task::spawn(async move {
        match hyper::upgrade::on(&mut req).await {
            Ok(upgraded) => {
                info!("Custom upgrade successful for protocol: {}", protocol);
                if let Err(e) = handler(upgraded).await {
                    error!("{} I/O error: {}", protocol, e);
                }
            }
            Err(e) => {
                error!("Custom upgrade failed for {}: {}", protocol, e);
            }
        }
    });

    Ok(accept_upgrade(protocol))
}
