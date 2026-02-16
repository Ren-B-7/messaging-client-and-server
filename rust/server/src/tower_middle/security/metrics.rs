use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Simple metrics tracking for HTTP server
#[derive(Clone, Debug)]
pub struct Metrics {
    inner: Arc<MetricsInner>,
}

#[derive(Debug)]
struct MetricsInner {
    /// Total requests served
    total_requests: AtomicU64,
    /// Requests currently being processed
    active_connections: AtomicUsize,
    /// Total requests that returned errors
    error_count: AtomicU64,
    /// Total bytes sent
    bytes_sent: AtomicU64,
    /// Total bytes received
    bytes_received: AtomicU64,
    /// Requests blocked by rate limiter
    rate_limited: AtomicU64,
    /// Requests blocked by IP filter
    ip_blocked: AtomicU64,
    /// Request latencies (circular buffer)
    latencies: Arc<RwLock<LatencyTracker>>,
    /// When metrics started
    start_time: Instant,
}

#[derive(Debug)]
struct LatencyTracker {
    buffer: Vec<Duration>,
    index: usize,
    capacity: usize,
}

impl LatencyTracker {
    fn new(capacity: usize) -> Self {
        Self {
            buffer: Vec::with_capacity(capacity),
            index: 0,
            capacity,
        }
    }

    fn record(&mut self, duration: Duration) {
        if self.buffer.len() < self.capacity {
            self.buffer.push(duration);
        } else {
            self.buffer[self.index] = duration;
            self.index = (self.index + 1) % self.capacity;
        }
    }

    fn percentile(&self, p: f64) -> Option<Duration> {
        if self.buffer.is_empty() {
            return None;
        }

        let mut sorted = self.buffer.clone();
        sorted.sort();

        let index = ((p / 100.0) * sorted.len() as f64) as usize;
        sorted.get(index.min(sorted.len() - 1)).copied()
    }

    fn average(&self) -> Option<Duration> {
        if self.buffer.is_empty() {
            return None;
        }

        let sum: Duration = self.buffer.iter().sum();
        Some(sum / self.buffer.len() as u32)
    }
}

impl Metrics {
    /// Create a new metrics tracker
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MetricsInner {
                total_requests: AtomicU64::new(0),
                active_connections: AtomicUsize::new(0),
                error_count: AtomicU64::new(0),
                bytes_sent: AtomicU64::new(0),
                bytes_received: AtomicU64::new(0),
                rate_limited: AtomicU64::new(0),
                ip_blocked: AtomicU64::new(0),
                latencies: Arc::new(RwLock::new(LatencyTracker::new(1000))),
                start_time: Instant::now(),
            }),
        }
    }

    /// Record a request start
    pub fn request_start(&self) {
        self.inner.total_requests.fetch_add(1, Ordering::Relaxed);
        self.inner
            .active_connections
            .fetch_add(1, Ordering::Relaxed);
    }
    pub fn request_end(&self, duration: Duration) {
        self.inner
            .active_connections
            .fetch_sub(1, Ordering::Relaxed);

        let latencies = Arc::clone(&self.inner.latencies);

        tokio::spawn(async move {
            let mut lat = latencies.write().await;
            lat.record(duration);
        });
    }

    /// Record an error
    pub fn record_error(&self) {
        self.inner.error_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record bytes sent
    pub fn record_bytes_sent(&self, bytes: u64) {
        self.inner.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record bytes received
    pub fn record_bytes_received(&self, bytes: u64) {
        self.inner
            .bytes_received
            .fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record a rate limited request
    pub fn record_rate_limited(&self) {
        self.inner.rate_limited.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an IP blocked request
    pub fn record_ip_blocked(&self) {
        self.inner.ip_blocked.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current snapshot of metrics
    pub async fn snapshot(&self) -> MetricsSnapshot {
        let latencies = self.inner.latencies.read().await;

        MetricsSnapshot {
            total_requests: self.inner.total_requests.load(Ordering::Relaxed),
            active_connections: self.inner.active_connections.load(Ordering::Relaxed),
            error_count: self.inner.error_count.load(Ordering::Relaxed),
            bytes_sent: self.inner.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.inner.bytes_received.load(Ordering::Relaxed),
            rate_limited: self.inner.rate_limited.load(Ordering::Relaxed),
            ip_blocked: self.inner.ip_blocked.load(Ordering::Relaxed),
            uptime: self.inner.start_time.elapsed(),
            latency_avg: latencies.average(),
            latency_p50: latencies.percentile(50.0),
            latency_p95: latencies.percentile(95.0),
            latency_p99: latencies.percentile(99.0),
        }
    }

    /// Get metrics in Prometheus format
    pub async fn prometheus(&self) -> String {
        let snap = self.snapshot().await;

        format!(
            "# HELP http_requests_total Total HTTP requests\n\
             # TYPE http_requests_total counter\n\
             http_requests_total {}\n\
             \n\
             # HELP http_active_connections Currently active connections\n\
             # TYPE http_active_connections gauge\n\
             http_active_connections {}\n\
             \n\
             # HELP http_errors_total Total HTTP errors\n\
             # TYPE http_errors_total counter\n\
             http_errors_total {}\n\
             \n\
             # HELP http_bytes_sent_total Total bytes sent\n\
             # TYPE http_bytes_sent_total counter\n\
             http_bytes_sent_total {}\n\
             \n\
             # HELP http_bytes_received_total Total bytes received\n\
             # TYPE http_bytes_received_total counter\n\
             http_bytes_received_total {}\n\
             \n\
             # HELP http_rate_limited_total Total rate limited requests\n\
             # TYPE http_rate_limited_total counter\n\
             http_rate_limited_total {}\n\
             \n\
             # HELP http_ip_blocked_total Total IP blocked requests\n\
             # TYPE http_ip_blocked_total counter\n\
             http_ip_blocked_total {}\n\
             \n\
             # HELP http_request_duration_seconds Request duration\n\
             # TYPE http_request_duration_seconds summary\n\
             http_request_duration_seconds{{quantile=\"0.5\"}} {}\n\
             http_request_duration_seconds{{quantile=\"0.95\"}} {}\n\
             http_request_duration_seconds{{quantile=\"0.99\"}} {}\n\
             http_request_duration_seconds_sum {}\n\
             http_request_duration_seconds_count {}\n",
            snap.total_requests,
            snap.active_connections,
            snap.error_count,
            snap.bytes_sent,
            snap.bytes_received,
            snap.rate_limited,
            snap.ip_blocked,
            snap.latency_p50.map(|d| d.as_secs_f64()).unwrap_or(0.0),
            snap.latency_p95.map(|d| d.as_secs_f64()).unwrap_or(0.0),
            snap.latency_p99.map(|d| d.as_secs_f64()).unwrap_or(0.0),
            snap.latency_avg
                .map(|d| d.as_secs_f64() * snap.total_requests as f64)
                .unwrap_or(0.0),
            snap.total_requests,
        )
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub total_requests: u64,
    pub active_connections: usize,
    pub error_count: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub rate_limited: u64,
    pub ip_blocked: u64,
    pub uptime: Duration,
    pub latency_avg: Option<Duration>,
    pub latency_p50: Option<Duration>,
    pub latency_p95: Option<Duration>,
    pub latency_p99: Option<Duration>,
}

impl MetricsSnapshot {
    /// Calculate requests per second
    pub fn requests_per_second(&self) -> f64 {
        if self.uptime.as_secs() == 0 {
            return 0.0;
        }
        self.total_requests as f64 / self.uptime.as_secs_f64()
    }

    /// Calculate error rate
    pub fn error_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        (self.error_count as f64 / self.total_requests as f64) * 100.0
    }

    /// Format for human-readable display
    pub fn format(&self) -> String {
        format!(
            "Uptime: {:.2}s | Requests: {} ({:.2}/sec) | Active: {} | Errors: {} ({:.2}%) | \
             Rate Limited: {} | IP Blocked: {} | Avg Latency: {:.2}ms | P95: {:.2}ms | P99: {:.2}ms",
            self.uptime.as_secs_f64(),
            self.total_requests,
            self.requests_per_second(),
            self.active_connections,
            self.error_count,
            self.error_rate(),
            self.rate_limited,
            self.ip_blocked,
            self.latency_avg
                .map(|d| d.as_secs_f64() * 1000.0)
                .unwrap_or(0.0),
            self.latency_p95
                .map(|d| d.as_secs_f64() * 1000.0)
                .unwrap_or(0.0),
            self.latency_p99
                .map(|d| d.as_secs_f64() * 1000.0)
                .unwrap_or(0.0),
        )
    }
}
