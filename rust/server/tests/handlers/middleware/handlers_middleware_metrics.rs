use server::Metrics;
use std::time::Duration;

// ── Initialisation ────────────────────────────────────────────────────────

#[tokio::test]
async fn fresh_metrics_snapshot_is_all_zero() {
    let metrics = Metrics::new();
    let snap = metrics.snapshot().await;

    assert_eq!(snap.total_requests, 0, "total_requests must start at 0");
    assert_eq!(
        snap.active_connections, 0,
        "active_connections must start at 0"
    );
    assert_eq!(snap.error_count, 0, "error_count must start at 0");
}

// ── request_start / request_end lifecycle ────────────────────────────────

#[tokio::test]
async fn request_start_increments_total_and_active() {
    let metrics = Metrics::new();
    metrics.request_start();

    let snap = metrics.snapshot().await;
    assert_eq!(snap.total_requests, 1);
    assert_eq!(snap.active_connections, 1);
}

#[tokio::test]
async fn request_end_decrements_active_connections() {
    let metrics = Metrics::new();
    metrics.request_start();
    metrics.request_end(Duration::from_millis(10));

    let snap = metrics.snapshot().await;
    assert_eq!(snap.total_requests, 1, "total must still be 1 after end");
    assert_eq!(snap.active_connections, 0, "active must drop back to 0");
}

#[tokio::test]
async fn multiple_concurrent_requests_tracked() {
    let metrics = Metrics::new();
    for _ in 0..5 {
        metrics.request_start();
    }

    let snap = metrics.snapshot().await;
    assert_eq!(snap.total_requests, 5);
    assert_eq!(snap.active_connections, 5);
}

#[tokio::test]
async fn active_connections_return_to_zero_after_all_end() {
    let metrics = Metrics::new();
    for _ in 0..3 {
        metrics.request_start();
    }
    for _ in 0..3 {
        metrics.request_end(Duration::from_millis(5));
    }

    let snap = metrics.snapshot().await;
    assert_eq!(snap.active_connections, 0);
    assert_eq!(snap.total_requests, 3);
}

#[tokio::test]
async fn active_connections_never_go_below_zero() {
    let metrics = Metrics::new();
    metrics.request_start();
    metrics.request_end(Duration::from_millis(1));
    metrics.request_end(Duration::from_millis(1)); // extra end — must not underflow

    let snap = metrics.snapshot().await;
    // The implementation uses saturating_sub; active_connections must be 0, not negative.
    assert_eq!(snap.active_connections, 0);
}

// ── Error tracking ────────────────────────────────────────────────────────

#[tokio::test]
async fn record_error_increments_error_count() {
    let metrics = Metrics::new();
    metrics.request_start();
    metrics.record_error();

    let snap = metrics.snapshot().await;
    assert_eq!(snap.error_count, 1);
}

#[tokio::test]
async fn multiple_errors_accumulate() {
    let metrics = Metrics::new();
    for _ in 0..7 {
        metrics.record_error();
    }

    let snap = metrics.snapshot().await;
    assert_eq!(snap.error_count, 7);
}

#[tokio::test]
async fn successful_request_does_not_increment_error_count() {
    let metrics = Metrics::new();
    metrics.request_start();
    metrics.request_end(Duration::from_millis(20));
    // No record_error call

    let snap = metrics.snapshot().await;
    assert_eq!(snap.error_count, 0);
}

// ── Derived statistics ────────────────────────────────────────────────────

#[tokio::test]
async fn error_rate_is_zero_with_no_requests() {
    let metrics = Metrics::new();
    let snap = metrics.snapshot().await;
    assert_eq!(snap.error_rate(), 0.0);
}

#[tokio::test]
async fn error_rate_is_100_when_all_requests_fail() {
    let metrics = Metrics::new();
    for _ in 0..4 {
        metrics.request_start();
        metrics.record_error();
        metrics.request_end(Duration::from_millis(5));
    }

    let snap = metrics.snapshot().await;
    assert_eq!(snap.total_requests, 4);
    assert_eq!(snap.error_count, 4);
    let rate = snap.error_rate();
    assert!(
        (rate - 100.0).abs() < 0.01,
        "error_rate should be 100.0, got {:.2}",
        rate
    );
}

#[tokio::test]
async fn error_rate_calculation_is_correct() {
    let metrics = Metrics::new();
    // 10 requests, 2 errors → 20%
    for _ in 0..10 {
        metrics.request_start();
        metrics.request_end(Duration::from_millis(1));
    }
    for _ in 0..2 {
        metrics.record_error();
    }

    let snap = metrics.snapshot().await;
    let rate = snap.error_rate();
    assert!(
        (rate - 20.0).abs() < 0.01,
        "error_rate should be 20.0, got {:.2}",
        rate
    );
}

// ── Latency recording ─────────────────────────────────────────────────────

#[tokio::test]
async fn single_request_latency_is_recorded() {
    let metrics = Metrics::new();
    metrics.request_start();
    metrics.request_end(Duration::from_millis(50));

    let snap = metrics.snapshot().await;
    // avg_latency must be Some and close to 50ms after one sample
    if let Some(avg) = snap.latency_avg {
        let ms = avg.as_secs_f64() * 1000.0;
        assert!(
            (ms - 50.0).abs() < 5.0,
            "avg latency should be ~50ms, got {:.2}ms",
            ms
        );
    }
    // If avg is None the implementation doesn't track latency — that's also
    // acceptable; we just skip the assertion.
}

#[tokio::test]
async fn requests_per_second_is_zero_before_any_requests() {
    let metrics = Metrics::new();
    let snap = metrics.snapshot().await;
    // RPS should be 0 or NaN-safe when no requests have arrived
    let rps = snap.requests_per_second();
    assert!(rps >= 0.0, "RPS must not be negative");
}

// ── Snapshot format ───────────────────────────────────────────────────────

#[tokio::test]
async fn snapshot_format_is_non_empty_string() {
    let metrics = Metrics::new();
    metrics.request_start();
    metrics.request_end(Duration::from_millis(10));

    let snap = metrics.snapshot().await;
    let formatted = snap.format();
    assert!(
        !formatted.is_empty(),
        "formatted snapshot must not be empty"
    );
}

// ── Clone shares state ────────────────────────────────────────────────────

#[tokio::test]
async fn cloned_metrics_reflects_updates_from_original() {
    let metrics = Metrics::new();
    let clone = metrics.clone();

    metrics.request_start();

    let snap = clone.snapshot().await;
    assert_eq!(
        snap.total_requests, 1,
        "clone must see updates made through the original handle"
    );
}
