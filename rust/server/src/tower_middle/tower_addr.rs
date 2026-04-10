use hyper::Request;
use std::net::SocketAddr;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Tower layer for inserting the remote SocketAddr into request extensions.
/// 
/// This is required for downstream middlewares (like IpFilter or RateLimiter)
/// to be able to identify the client IP.
#[derive(Clone)]
pub struct AddAddrLayer {
    addr: SocketAddr,
}

impl AddAddrLayer {
    pub fn new(addr: SocketAddr) -> Self {
        Self { addr }
    }
}

impl<S> Layer<S> for AddAddrLayer {
    type Service = AddAddrService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AddAddrService {
            inner,
            addr: self.addr,
        }
    }
}

/// The actual service that inserts the SocketAddr.
#[derive(Clone)]
pub struct AddAddrService<S> {
    inner: S,
    addr: SocketAddr,
}

impl<S, ReqBody> Service<Request<ReqBody>> for AddAddrService<S>
where
    S: Service<Request<ReqBody>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        req.extensions_mut().insert(self.addr);
        self.inner.call(req)
    }
}
