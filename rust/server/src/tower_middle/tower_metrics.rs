use hyper::{Request, Response};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;
use tower::{Layer, Service};

use crate::tower_middle::security::Metrics;

/// Tower layer for metrics tracking
///
/// This wraps any service and tracks request metrics
#[derive(Clone)]
pub struct MetricsLayer {
    metrics: Metrics,
}

impl MetricsLayer {
    pub fn new(metrics: Metrics) -> Self {
        Self { metrics }
    }
}

impl<S> Layer<S> for MetricsLayer {
    type Service = DetailedMetricsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DetailedMetricsService {
            inner,
            metrics: self.metrics.clone(),
        }
    }
}

/// Metrics layer that also tracks response status codes
#[derive(Clone)]
pub struct DetailedMetricsService<S> {
    inner: S,
    metrics: Metrics,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for DetailedMetricsService<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
    ResBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let metrics = self.metrics.clone();
        let mut inner = self.inner.clone();

        metrics.request_start();
        let start = Instant::now();

        Box::pin(async move {
            let result = inner.call(req).await;

            let duration = start.elapsed();
            metrics.request_end(duration);

            match &result {
                Ok(response) => {
                    // Track 4xx and 5xx as errors
                    if response.status().is_client_error() || response.status().is_server_error() {
                        metrics.record_error();
                    }
                }
                Err(_) => {
                    metrics.record_error();
                }
            }

            result
        })
    }
}
