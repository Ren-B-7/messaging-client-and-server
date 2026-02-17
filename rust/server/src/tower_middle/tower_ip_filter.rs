use hyper::{Request, Response, StatusCode};
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

use crate::tower_middle::security::IpFilter;

/// Tower layer for IP filtering
///
/// This wraps any service and checks the client IP against the filter
/// before allowing the request through.
#[derive(Clone)]
pub struct IpFilterLayer {
    filter: IpFilter,
}

impl IpFilterLayer {
    pub fn new(filter: IpFilter) -> Self {
        Self { filter }
    }
}

impl<S> Layer<S> for IpFilterLayer {
    type Service = IpFilterService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        IpFilterService {
            inner,
            filter: self.filter.clone(),
        }
    }
}

/// The actual service that performs IP filtering
#[derive(Clone)]
pub struct IpFilterService<S> {
    inner: S,
    filter: IpFilter,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for IpFilterService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
    ResBody: Default + Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        // Extract the client IP from extensions (set by connection handler)
        let client_ip = req.extensions().get::<SocketAddr>().map(|addr| addr.ip());

        let filter = self.filter.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Check if IP is allowed
            if let Some(ip) = client_ip {
                if !filter.is_allowed(ip).await {
                    tracing::warn!("Connection from {} blocked by IP filter", ip);

                    // Return 403 Forbidden
                    let response = Response::builder()
                        .status(StatusCode::FORBIDDEN)
                        .header("content-type", "application/json")
                        .body(ResBody::default())
                        .unwrap();

                    return Ok(response);
                }
            }

            // IP is allowed, forward to inner service
            inner.call(req).await
        })
    }
}

// Example usage in main.rs:
/*
use tower::ServiceBuilder;

let middleware_stack = ServiceBuilder::new()
    .layer(IpFilterLayer::new(state.ip_filter.clone()))
    .service(user_service);
*/
