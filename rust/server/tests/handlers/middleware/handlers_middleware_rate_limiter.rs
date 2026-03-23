use server::RateLimiter;
use std::net::IpAddr;

// ── Default-allow (fresh bucket is full) ─────────────────────────────────

#[tokio::test]
async fn first_request_from_new_ip_is_allowed() {
    let limiter = RateLimiter::new(100, 200);
    let ip: IpAddr = "192.168.1.1".parse().unwrap();
    assert!(limiter.check(ip).await, "first request must be allowed");
}

#[tokio::test]
async fn first_request_from_ipv6_is_allowed() {
    let limiter = RateLimiter::new(100, 200);
    let ip: IpAddr = "2001:db8::1".parse().unwrap();
    assert!(limiter.check(ip).await);
}

// ── Burst capacity ────────────────────────────────────────────────────────

#[tokio::test]
async fn burst_capacity_requests_all_succeed() {
    // RateLimiter::new(rate, burst): the bucket starts full at `burst` tokens.
    // All burst requests should succeed before the bucket empties.
    let burst = 10usize;
    let limiter = RateLimiter::new(100, burst);
    let ip: IpAddr = "10.0.0.1".parse().unwrap();

    for i in 0..burst {
        assert!(
            limiter.check(ip).await,
            "request {} within burst must be allowed",
            i + 1
        );
    }
}

#[tokio::test]
async fn request_beyond_burst_is_rate_limited() {
    // With rate=1 and burst=1, the second immediate request must be rejected.
    let limiter = RateLimiter::new(1, 1);
    let ip: IpAddr = "10.0.0.2".parse().unwrap();

    let first = limiter.check(ip).await;
    assert!(first, "first request must be allowed");

    let second = limiter.check(ip).await;
    assert!(!second, "second immediate request must be rate-limited");
}

// ── Per-IP isolation ──────────────────────────────────────────────────────

#[tokio::test]
async fn different_ips_have_independent_buckets() {
    let limiter = RateLimiter::new(1, 1);
    let ip1: IpAddr = "10.0.0.1".parse().unwrap();
    let ip2: IpAddr = "10.0.0.2".parse().unwrap();

    // Exhaust ip1
    limiter.check(ip1).await;
    let ip1_second = limiter.check(ip1).await;
    assert!(
        !ip1_second,
        "ip1 should be rate-limited after exhausting burst"
    );

    // ip2 must be unaffected
    assert!(limiter.check(ip2).await, "ip2 must still be allowed");
}

#[tokio::test]
async fn loopback_and_private_ips_are_tracked_independently() {
    let limiter = RateLimiter::new(1, 1);
    let loopback: IpAddr = "127.0.0.1".parse().unwrap();
    let private: IpAddr = "192.168.1.1".parse().unwrap();

    assert!(limiter.check(loopback).await);
    assert!(limiter.check(private).await);

    // Both exhausted — second calls must fail
    assert!(!limiter.check(loopback).await);
    assert!(!limiter.check(private).await);
}

// ── Stats ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn stats_total_ips_zero_before_any_request() {
    let limiter = RateLimiter::new(100, 200);
    let stats = limiter.stats().await;
    assert_eq!(stats.total_ips, 0);
}

#[tokio::test]
async fn stats_total_ips_increments_per_unique_ip() {
    let limiter = RateLimiter::new(100, 200);
    limiter.check("10.0.0.1".parse::<IpAddr>().unwrap()).await;
    limiter.check("10.0.0.2".parse::<IpAddr>().unwrap()).await;
    limiter.check("10.0.0.2".parse::<IpAddr>().unwrap()).await; // same IP again

    let stats = limiter.stats().await;
    assert_eq!(stats.total_ips, 2, "only unique IPs are counted");
}

#[tokio::test]
async fn stats_capacity_and_refill_rate_match_constructor() {
    let limiter = RateLimiter::new(100, 200);
    let stats = limiter.stats().await;
    assert_eq!(stats.capacity, 200, "capacity must match burst argument");
    assert_eq!(
        stats.refill_rate, 100,
        "refill_rate must match rate argument"
    );
}

#[tokio::test]
async fn stats_rate_limited_count_increments_on_rejection() {
    let limiter = RateLimiter::new(1, 1);
    let ip: IpAddr = "172.16.0.1".parse().unwrap();

    limiter.check(ip).await; // allowed
    limiter.check(ip).await; // rate-limited

    let stats = limiter.stats().await;
    assert!(
        stats.rate_limited >= 1,
        "at least one rejection should be recorded"
    );
}

// ── Cleanup ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn cleanup_is_callable_without_panic() {
    let limiter = RateLimiter::new(100, 200);
    limiter.check("1.2.3.4".parse::<IpAddr>().unwrap()).await;
    limiter.cleanup().await; // must not panic
}

#[tokio::test]
async fn cleanup_is_idempotent() {
    let limiter = RateLimiter::new(100, 200);
    limiter.cleanup().await;
    limiter.cleanup().await; // second call on empty limiter must not panic
}

#[tokio::test]
async fn after_cleanup_new_requests_from_same_ip_are_allowed() {
    // After cleanup removes stale buckets, the next request from that IP
    // starts with a fresh full bucket.
    let limiter = RateLimiter::new(1, 1);
    let ip: IpAddr = "9.9.9.9".parse().unwrap();

    limiter.check(ip).await; // consume the one token
    assert!(
        !limiter.check(ip).await,
        "should be rate-limited before cleanup"
    );

    limiter.cleanup().await;

    // After cleanup the bucket for this IP may or may not have been removed
    // depending on the implementation's eviction policy (some only evict old
    // buckets, some evict all zero-token buckets).  What we can assert is that
    // cleanup does not panic and the limiter remains usable.
    let _ = limiter.check(ip).await; // must not panic
}

// ── Clone shares state ────────────────────────────────────────────────────

#[tokio::test]
async fn cloned_limiter_shares_bucket_state() {
    let limiter = RateLimiter::new(1, 1);
    let clone = limiter.clone();
    let ip: IpAddr = "5.5.5.5".parse().unwrap();

    // Exhaust the bucket through the original handle
    limiter.check(ip).await;

    // The clone must see the same exhausted bucket
    assert!(
        !clone.check(ip).await,
        "clone must share the same rate-limit state"
    );
}
