use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Simple token bucket rate limiter per IP address
#[derive(Clone, Debug)]
pub struct RateLimiter {
    inner: Arc<RateLimiterInner>,
}

#[derive(Debug)]
struct RateLimiterInner {
    /// Storage for each IP's bucket
    buckets: RwLock<HashMap<IpAddr, TokenBucket>>,
    /// Maximum tokens in bucket
    capacity: usize,
    /// Tokens added per second
    refill_rate: usize,
    /// How long to keep empty buckets
    cleanup_duration: Duration,
}

#[derive(Debug, Clone)]
struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `requests_per_second` - How many requests per second to allow
    /// * `burst` - How many requests can burst at once
    pub fn new(requests_per_second: usize, burst: usize) -> Self {
        Self {
            inner: Arc::new(RateLimiterInner {
                buckets: RwLock::new(HashMap::new()),
                capacity: burst,
                refill_rate: requests_per_second,
                cleanup_duration: Duration::from_secs(60),
            }),
        }
    }

    /// Check if a request from this IP is allowed
    /// Returns true if allowed, false if rate limited
    pub async fn check(&self, ip: IpAddr) -> bool {
        let mut buckets = self.inner.buckets.write().await;

        let bucket = buckets.entry(ip).or_insert_with(|| TokenBucket {
            tokens: self.inner.capacity as f64,
            last_refill: Instant::now(),
        });

        // Refill tokens based on time elapsed
        let now = Instant::now();
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        let tokens_to_add = elapsed * self.inner.refill_rate as f64;

        bucket.tokens = (bucket.tokens + tokens_to_add).min(self.inner.capacity as f64);
        bucket.last_refill = now;

        // Try to consume a token
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Remove old buckets that haven't been used recently
    pub async fn cleanup(&self) {
        let mut buckets = self.inner.buckets.write().await;
        let now = Instant::now();

        buckets.retain(|_, bucket| {
            now.duration_since(bucket.last_refill) < self.inner.cleanup_duration
        });
    }

    /// Get current rate limiter statistics
    pub async fn stats(&self) -> RateLimiterStats {
        let buckets = self.inner.buckets.read().await;

        let total_ips = buckets.len();
        let rate_limited = buckets.values().filter(|b| b.tokens < 1.0).count();

        RateLimiterStats {
            total_ips,
            rate_limited,
            capacity: self.inner.capacity,
            refill_rate: self.inner.refill_rate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimiterStats {
    pub total_ips: usize,
    pub rate_limited: usize,
    pub capacity: usize,
    pub refill_rate: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(10, 5); // 10 req/sec, burst of 5
        let ip: IpAddr = "192.168.1.1".parse().unwrap();

        // First 5 requests should succeed (burst)
        for _ in 0..5 {
            assert!(limiter.check(ip).await);
        }

        // 6th request should fail (no tokens left)
        assert!(!limiter.check(ip).await);

        // Wait for refill
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Should have ~2 tokens now (200ms * 10 req/sec)
        assert!(limiter.check(ip).await);
        assert!(limiter.check(ip).await);
        assert!(!limiter.check(ip).await);
    }
}
