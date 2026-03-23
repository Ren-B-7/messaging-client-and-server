use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::{Request, Response, StatusCode};
use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

use crate::tower_middle::security::IpFilter;

/// Tower layer for IP filtering.
///
/// Wraps any service and checks the client IP against the filter before
/// allowing the request through.  Blocked connections receive a proper JSON
/// 403 response body instead of the previous empty body produced by
/// `ResBody::default()`.
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

/// The actual service that performs IP filtering.
#[derive(Clone)]
pub struct IpFilterService<S> {
    inner: S,
    filter: IpFilter,
}

/// Construct a JSON error body as a `BoxBody<Bytes, Infallible>`.
///
/// Used by the middleware layers to return machine-readable error responses
/// rather than the empty bodies produced by `ResBody::default()`.
fn json_error_body(code: &'static str, message: &'static str) -> BoxBody<Bytes, Infallible> {
    let json = format!(
        r#"{{"status":"error","code":"{}","message":"{}"}}"#,
        code, message
    );
    Full::new(Bytes::from(json)).boxed()
}

impl<S, ReqBody> Service<Request<ReqBody>> for IpFilterService<S>
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
        let filter = self.filter.clone();
        let mut inner = self.inner.clone();

        Box::pin(async move {
            if let Some(ip) = client_ip
                && !filter.is_allowed(ip).await
            {
                tracing::warn!("Connection from {} blocked by IP filter", ip);

                return Ok(Response::builder()
                    .status(StatusCode::FORBIDDEN)
                    .header("content-type", "application/json")
                    .body(json_error_body(
                        "IP_BLOCKED",
                        "Your IP address is not allowed",
                    ))
                    .unwrap());
            }

            inner.call(req).await
        })
    }
}
