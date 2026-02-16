use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use http_body_util::Full;
use hyper::{Request, Response, StatusCode};
use tokio::time;
use tower::{Layer, Service};

/// Tower layer for request timeouts
///
/// If the inner service does not respond within the configured
/// duration, a 408 Request Timeout response is returned.
#[derive(Clone)]
pub struct TimeoutLayer {
    duration: Duration,
}

impl TimeoutLayer {
    pub fn new(duration: Duration) -> Self {
        Self { duration }
    }
}

impl<S> Layer<S> for TimeoutLayer {
    type Service = TimeoutService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        TimeoutService {
            inner,
            duration: self.duration,
        }
    }
}

/// The actual timeout service
#[derive(Clone)]
pub struct TimeoutService<S> {
    inner: S,
    duration: Duration,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for TimeoutService<S>
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
        let duration = self.duration;
        let mut inner = self.inner.clone();

        Box::pin(async move {
            match time::timeout(duration, inner.call(req)).await {
                Ok(result) => result,
                Err(_) => {
                    tracing::warn!("Request timed out after {:?}", duration);

                    let response = Response::builder()
                        .status(StatusCode::REQUEST_TIMEOUT)
                        .header("content-type", "application/json")
                        .body(ResBody::default())
                        .unwrap();

                    Ok(response)
                }
            }
        })
    }
}
