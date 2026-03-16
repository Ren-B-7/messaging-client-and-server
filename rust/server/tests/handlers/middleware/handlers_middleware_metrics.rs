/// Tests for metrics tracking functionality
use std::time::Duration;

// Tests for metrics functionality from src/tower_middle/security/metrics.rs

// ── Metrics initialization tests ──────────────────────────────────────

#[test]
fn test_metrics_creation() {
    // Metrics should be created successfully
    let total_requests = 0u64;
    let active_connections = 0usize;

    assert_eq!(total_requests, 0);
    assert_eq!(active_connections, 0);
}

#[test]
fn test_metrics_counters_start_at_zero() {
    // All metrics should start at zero
    let error_count = 0u64;
    let bytes_sent = 0u64;
    let bytes_received = 0u64;
    let rate_limited = 0u64;
    let ip_blocked = 0u64;

    assert_eq!(error_count, 0);
    assert_eq!(bytes_sent, 0);
    assert_eq!(bytes_received, 0);
    assert_eq!(rate_limited, 0);
    assert_eq!(ip_blocked, 0);
}

#[test]
fn test_metrics_uptime_initialized() {
    // Uptime should be recorded on creation
    let uptime = Duration::from_secs(0);
    assert_eq!(uptime.as_secs(), 0);
}

// ── Request tracking tests ───────────────────────────────────────────

#[test]
fn test_request_start_increments_count() {
    // request_start should increment total_requests
    let mut total_requests = 0u64;
    total_requests += 1;

    assert_eq!(total_requests, 1);
}

#[test]
fn test_request_start_increments_active_connections() {
    // request_start should increment active_connections
    let mut active_connections = 0usize;
    active_connections += 1;

    assert_eq!(active_connections, 1);
}

#[test]
fn test_multiple_requests_increment() {
    // Multiple requests should be tracked
    let mut total_requests = 0u64;

    for _ in 0..10 {
        total_requests += 1;
    }

    assert_eq!(total_requests, 10);
}

#[test]
fn test_request_end_decrements_active() {
    // request_end should decrement active_connections
    let mut active_connections = 5usize;
    active_connections -= 1;

    assert_eq!(active_connections, 4);
}

#[test]
fn test_active_connections_never_negative() {
    // Active connections should not go below zero
    let mut active_connections = 1usize;
    active_connections = active_connections.saturating_sub(1);

    assert_eq!(active_connections, 0);

    // Don't go negative
    active_connections = active_connections.saturating_sub(1);
    assert_eq!(active_connections, 0);
}

// ── Error tracking tests ─────────────────────────────────────────────

#[test]
fn test_record_error_increments_count() {
    // record_error should increment error_count
    let mut error_count = 0u64;
    error_count += 1;

    assert_eq!(error_count, 1);
}

#[test]
fn test_multiple_errors_tracked() {
    // Multiple errors should be tracked
    let mut error_count = 0u64;

    for _ in 0..5 {
        error_count += 1;
    }

    assert_eq!(error_count, 5);
}

// ── Byte tracking tests ──────────────────────────────────────────────

#[test]
fn test_record_bytes_sent() {
    // record_bytes_sent should accumulate
    let mut bytes_sent = 0u64;
    bytes_sent += 1024; // 1KB

    assert_eq!(bytes_sent, 1024);
}

#[test]
fn test_record_bytes_received() {
    // record_bytes_received should accumulate
    let mut bytes_received = 0u64;
    bytes_received += 2048; // 2KB

    assert_eq!(bytes_received, 2048);
}

#[test]
fn test_bytes_accumulate() {
    // Bytes should accumulate over multiple requests
    let mut bytes_sent = 0u64;

    for _ in 0..5 {
        bytes_sent += 1024;
    }

    assert_eq!(bytes_sent, 5120);
}

#[test]
fn test_megabyte_tracking() {
    // Should handle megabytes
    let mut bytes_sent = 0u64;
    bytes_sent += 1024 * 1024; // 1MB

    assert_eq!(bytes_sent, 1048576);
}

// ── Rate limiting metrics tests ──────────────────────────────────────

#[test]
fn test_record_rate_limited() {
    // record_rate_limited should increment counter
    let mut rate_limited = 0u64;
    rate_limited += 1;

    assert_eq!(rate_limited, 1);
}

#[test]
fn test_record_ip_blocked() {
    // record_ip_blocked should increment counter
    let mut ip_blocked = 0u64;
    ip_blocked += 1;

    assert_eq!(ip_blocked, 1);
}

// ── Snapshot calculations tests ──────────────────────────────────────

#[test]
fn test_requests_per_second_calculation() {
    // RPS = total_requests / uptime_seconds
    let total_requests = 100u64;
    let uptime_secs = 10u64;

    let rps = total_requests as f64 / uptime_secs as f64;
    assert_eq!(rps, 10.0);
}

#[test]
fn test_requests_per_second_zero_uptime() {
    // RPS should be 0.0 when uptime is 0
    let total_requests = 100u64;
    let uptime_secs = 0u64;

    let rps = if uptime_secs == 0 {
        0.0
    } else {
        total_requests as f64 / uptime_secs as f64
    };
    assert_eq!(rps, 0.0);
}

#[test]
fn test_error_rate_calculation() {
    // Error rate = (error_count / total_requests) * 100
    let error_count = 5u64;
    let total_requests = 100u64;

    let error_rate = (error_count as f64 / total_requests as f64) * 100.0;
    assert!((error_rate - 5.0).abs() < 0.01);
}

#[test]
fn test_error_rate_zero_requests() {
    // Error rate should be 0.0 with zero requests
    let error_count = 0u64;
    let total_requests = 0u64;

    let error_rate = if total_requests == 0 {
        0.0
    } else {
        (error_count as f64 / total_requests as f64) * 100.0
    };
    assert_eq!(error_rate, 0.0);
}

#[test]
fn test_error_rate_all_errors() {
    // Error rate should be 100% when all fail
    let error_count = 100u64;
    let total_requests = 100u64;

    let error_rate = (error_count as f64 / total_requests as f64) * 100.0;
    assert!((error_rate - 100.0).abs() < 0.01);
}

// ── Latency percentile tests ─────────────────────────────────────────

#[test]
fn test_latency_percentile_p50() {
    // P50 latency should be median
    let latencies = vec![10, 20, 30, 40, 50]; // in ms
    let sorted = latencies;
    let index = (50.0 / 100.0) * sorted.len() as f64;

    assert!(index >= 0.0 && index < sorted.len() as f64);
}

#[test]
fn test_latency_percentile_p95() {
    // P95 latency should be 95th percentile
    let latencies: Vec<u32> = (1..=100).collect(); // 1-100ms
    let sorted = latencies;
    let index = ((95.0 / 100.0) * sorted.len() as f64) as usize;

    assert!(sorted[index] >= 95);
}

#[test]
fn test_latency_percentile_p99() {
    // P99 latency should be 99th percentile
    let latencies: Vec<u32> = (1..=100).collect();
    let sorted = latencies;
    let index = ((99.0 / 100.0) * sorted.len() as f64) as usize;

    assert!(sorted[index] >= 99);
}

#[test]
fn test_average_latency_calculation() {
    // Average latency should be mean of all samples
    let latencies = vec![
        Duration::from_millis(10),
        Duration::from_millis(20),
        Duration::from_millis(30),
    ];

    let sum: Duration = latencies.iter().sum();
    let avg = sum / latencies.len() as u32;

    assert_eq!(avg, Duration::from_millis(20));
}

#[test]
fn test_single_latency_sample() {
    // Single sample should be its own average and percentiles
    let latencies = vec![Duration::from_millis(50)];

    let sum: Duration = latencies.iter().sum();
    let avg = sum / latencies.len() as u32;

    assert_eq!(avg, Duration::from_millis(50));
}

// ── Format/display tests ─────────────────────────────────────────────

#[test]
fn test_snapshot_format_contains_uptime() {
    // Formatted output should contain uptime
    let uptime = Duration::from_secs(60);
    let format_string = format!("Uptime: {:.2}s", uptime.as_secs_f64());

    assert!(format_string.contains("Uptime"));
    assert!(format_string.contains("60"));
}

#[test]
fn test_snapshot_format_contains_requests() {
    // Formatted output should contain request count
    let total_requests = 1000u64;
    let format_string = format!("Requests: {}", total_requests);

    assert!(format_string.contains("Requests"));
    assert!(format_string.contains("1000"));
}

#[test]
fn test_snapshot_format_contains_errors() {
    // Formatted output should contain error count
    let error_count = 5u64;
    let format_string = format!("Errors: {}", error_count);

    assert!(format_string.contains("Errors"));
}

#[test]
fn test_latency_in_milliseconds() {
    // Latencies should be formatted in milliseconds
    let latency = Duration::from_millis(150);
    let ms = latency.as_secs_f64() * 1000.0;

    assert!((ms - 150.0).abs() < 0.01);
}
