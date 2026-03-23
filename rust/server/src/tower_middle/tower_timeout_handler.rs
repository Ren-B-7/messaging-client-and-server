use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::{Request, Response, StatusCode};
use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time;
use tower::{Layer, Service};
use tracing::warn;

/// Tower layer for request timeouts.
///
/// If the inner service does not respond within the configured duration, a
/// 408 Request Timeout response is returned with a JSON body instead of the
/// previous empty body produced by `ResBody::default()`.
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

/// The actual timeout service.
#[derive(Clone)]
pub struct TimeoutService<S> {
    inner: S,
    duration: Duration,
}

fn json_error_body(code: &'static str, message: &'static str) -> BoxBody<Bytes, Infallible> {
    let json = format!(
        r#"{{"status":"error","code":"{}","message":"{}"}}"#,
        code, message
    );
    Full::new(Bytes::from(json)).boxed()
}

impl<S, ReqBody> Service<Request<ReqBody>> for TimeoutService<S>
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
        let duration = self.duration;
        let mut inner = self.inner.clone();

        Box::pin(async move {
            match time::timeout(duration, inner.call(req)).await {
                Ok(result) => result,
                Err(_) => {
                    warn!("Request timed out after {:?}", duration);

                    Ok(Response::builder()
                        .status(StatusCode::REQUEST_TIMEOUT)
                        .header("content-type", "application/json")
                        .body(json_error_body(
                            "REQUEST_TIMEOUT",
                            "Request took too long — please try again",
                        ))
                        .unwrap())
                }
            }
        })
    }
}
