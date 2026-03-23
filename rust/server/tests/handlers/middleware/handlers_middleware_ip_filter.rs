use server::IpFilter;
use std::net::IpAddr;

// ── Default-allow behaviour ───────────────────────────────────────────────
// A freshly created IpFilter with no configuration must allow every IP.
// If an allow-list is configured, only listed IPs are allowed.
// If only a block-list is configured, all IPs except blocked ones are allowed.

#[tokio::test]
async fn fresh_filter_allows_ipv4() {
    let filter = IpFilter::new();
    let ip: IpAddr = "192.168.1.1".parse().unwrap();
    assert!(
        filter.is_allowed(ip).await,
        "default filter must allow all IPs"
    );
}

#[tokio::test]
async fn fresh_filter_allows_ipv6() {
    let filter = IpFilter::new();
    let ip: IpAddr = "2001:db8::1".parse().unwrap();
    assert!(filter.is_allowed(ip).await);
}

#[tokio::test]
async fn fresh_filter_allows_loopback() {
    let filter = IpFilter::new();
    assert!(
        filter
            .is_allowed("127.0.0.1".parse::<IpAddr>().unwrap())
            .await
    );
    assert!(filter.is_allowed("::1".parse::<IpAddr>().unwrap()).await);
}

#[tokio::test]
async fn fresh_filter_allows_all_private_ranges() {
    let filter = IpFilter::new();
    let private_ips = ["10.0.0.1", "172.16.0.1", "192.168.0.1"];
    for addr in private_ips {
        let ip: IpAddr = addr.parse().unwrap();
        assert!(
            filter.is_allowed(ip).await,
            "fresh filter must allow private IP {}",
            addr
        );
    }
}

// ── Clone shares the same underlying state ────────────────────────────────

#[tokio::test]
async fn cloned_filter_has_same_default_behaviour() {
    let filter = IpFilter::new();
    let clone = filter.clone();
    let ip: IpAddr = "10.0.0.1".parse().unwrap();
    assert_eq!(
        filter.is_allowed(ip).await,
        clone.is_allowed(ip).await,
        "clone must behave identically to original"
    );
}

// ── is_allowed is idempotent ──────────────────────────────────────────────

#[tokio::test]
async fn is_allowed_called_multiple_times_is_consistent() {
    let filter = IpFilter::new();
    let ip: IpAddr = "203.0.113.5".parse().unwrap();
    let first = filter.is_allowed(ip).await;
    let second = filter.is_allowed(ip).await;
    assert_eq!(first, second, "is_allowed must be consistent across calls");
}

// ── IPv4 and IPv6 are handled independently ───────────────────────────────

#[tokio::test]
async fn ipv4_and_ipv6_loopbacks_are_both_allowed() {
    let filter = IpFilter::new();
    let v4: IpAddr = "127.0.0.1".parse().unwrap();
    let v6: IpAddr = "::1".parse().unwrap();
    assert!(filter.is_allowed(v4).await);
    assert!(filter.is_allowed(v6).await);
}

// ── Parsing sanity: IpAddr round-trips ───────────────────────────────────
// These confirm the address types we pass into is_allowed are valid.

#[test]
fn all_ipv4_test_addresses_parse() {
    let addrs = [
        "0.0.0.0",
        "127.0.0.1",
        "10.0.0.1",
        "172.16.0.1",
        "192.168.1.1",
        "255.255.255.255",
    ];
    for addr in addrs {
        addr.parse::<IpAddr>()
            .unwrap_or_else(|_| panic!("{} failed to parse", addr));
    }
}

#[test]
fn all_ipv6_test_addresses_parse() {
    let addrs = ["::1", "::ffff:192.168.1.1", "2001:db8::1", "fe80::1"];
    for addr in addrs {
        addr.parse::<IpAddr>()
            .unwrap_or_else(|_| panic!("{} failed to parse", addr));
    }
}

#[test]
fn invalid_addresses_do_not_parse() {
    let bad = ["256.256.256.256", "192.168.1", "not-an-ip", ""];
    for addr in bad {
        assert!(
            addr.parse::<IpAddr>().is_err(),
            "'{}' should fail to parse",
            addr
        );
    }
}
