pub mod hyper_to_tower_adapter;
/// Tower middleware module
///
/// This module contains Tower-based middleware layers for:
/// - IP filtering
/// - Rate limiting  
/// - Metrics tracking
///
/// These replace the manual security checks in the connection loop.
pub mod tower_ip_filter;
pub mod tower_metrics;
pub mod tower_rate_limiter;

pub use hyper_to_tower_adapter::HyperToTowerAdapter;
pub use tower_ip_filter::{IpFilterLayer, IpFilterService};
pub use tower_metrics::{DetailedMetricsService, MetricsLayer, MetricsService};
pub use tower_rate_limiter::{RateLimiterLayer, RateLimiterService};
