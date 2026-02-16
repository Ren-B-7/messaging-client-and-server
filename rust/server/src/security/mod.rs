/// Security module for HTTP server
/// Provides IP filtering, rate limiting, and metrics tracking

// Make submodules public so their types can be exported
pub mod ip_filter;
pub mod metrics;
pub mod rate_limiter;

// Re-export the main types for convenience
pub use ip_filter::IpFilter;
pub use metrics::{Metrics, MetricsSnapshot};
pub use rate_limiter::{RateLimiter, RateLimiterStats};
