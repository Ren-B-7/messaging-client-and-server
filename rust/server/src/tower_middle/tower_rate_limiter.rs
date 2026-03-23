use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::{Request, Response, StatusCode};
use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

use crate::tower_middle::security::RateLimiter;

/// Tower layer for per-IP rate limiting.
///
/// Rate-limited connections receive a proper JSON 429 response body instead
/// of the previous empty body produced by `ResBody::default()`.
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

/// The actual service that performs rate limiting.
#[derive(Clone)]
pub struct RateLimiterService<S> {
    inner: S,
    limiter: RateLimiter,
}

fn json_error_body(code: &'static str, message: &'static str) -> BoxBody<Bytes, Infallible> {
    let json = format!(
        r#"{{"status":"error","code":"{}","message":"{}"}}"#,
        code, message
    );
    Full::new(Bytes::from(json)).boxed()
}

impl<S, ReqBody> Service<Request<ReqBody>> for RateLimiterService<S>
where
    S: Service<Request<ReqBody>, Response = Response<BoxBody<Bytes, Infallible>>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let client_ip = req.extensions().get::<SocketAddr>().map(|addr| addr.ip());
        let limiter = self.limiter.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            if let Some(ip) = client_ip
                && !limiter.check(ip).await
            {
                tracing::warn!("Connection from {} rate limited", ip);

                return Ok(Response::builder()
                    .status(StatusCode::TOO_MANY_REQUESTS)
                    .header("content-type", "application/json")
                    .header("retry-after", "1")
                    .body(json_error_body(
                        "RATE_LIMITED",
                        "Too many requests — please slow down",
                    ))
                    .unwrap());
            }

            inner.call(req).await
        })
    }
}
