use std::task::{Context, Poll};
use std::future::Future;
use std::pin::Pin;
use tower::Service as TowerService;
use hyper::service::Service as HyperService;
use hyper::{Request, Response};

/// Adapter to make Hyper services work with Tower middleware
///
/// Your UserService and AdminService implement hyper::service::Service,
/// but Tower middleware needs tower::Service. This adapter bridges them.
#[derive(Clone)]
pub struct HyperToTowerAdapter<S> {
    inner: S,
}

impl<S> HyperToTowerAdapter<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S, ReqBody, ResBody> TowerService<Request<ReqBody>> for HyperToTowerAdapter<S>
where
    S: HyperService<Request<ReqBody>, Response = Response<ResBody>> + Clone,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
    ResBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Hyper services are always ready
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        self.inner.call(req)
    }
}
