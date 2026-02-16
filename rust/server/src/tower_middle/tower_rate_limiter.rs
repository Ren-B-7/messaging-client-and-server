use std::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;
use tower::{Layer, Service};
use hyper::{Request, Response, StatusCode};
use http_body_util::Full;
use bytes::Bytes;
use std::net::SocketAddr;

use crate::security::RateLimiter;

/// Tower layer for rate limiting
///
/// This wraps any service and rate limits requests per IP address
#[derive(Clone)]
pub struct RateLimiterLayer {
    limiter: RateLimiter,
}

impl RateLimiterLayer {
    pub fn new(limiter: RateLimiter) -> Self {
        Self { limiter }
    }
}

impl<S> Layer<S> for RateLimiterLayer {
    type Service = RateLimiterService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimiterService {
            inner,
            limiter: self.limiter.clone(),
        }
    }
}

/// The actual service that performs rate limiting
#[derive(Clone)]
pub struct RateLimiterService<S> {
    inner: S,
    limiter: RateLimiter,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for RateLimiterService<S>
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
        // Extract the client IP from extensions
        let client_ip = req.extensions().get::<SocketAddr>().map(|addr| addr.ip());

        let limiter = self.limiter.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // Check rate limit
            if let Some(ip) = client_ip {
                if !limiter.check(ip).await {
                    tracing::warn!("Connection from {} rate limited", ip);
                    
                    // Return 429 Too Many Requests
                    let response = Response::builder()
                        .status(StatusCode::TOO_MANY_REQUESTS)
                        .header("content-type", "application/json")
                        .header("retry-after", "1") // Retry after 1 second
                        .body(ResBody::default())
                        .unwrap();
                    
                    return Ok(response);
                }
            }

            // Rate limit passed, forward to inner service
            inner.call(req).await
        })
    }
}
