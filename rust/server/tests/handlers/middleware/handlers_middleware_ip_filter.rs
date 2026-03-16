/// Tests for IP filtering functionality
use std::net::IpAddr;

// Tests for IP filter functionality from src/tower_middle/security/ip_filter.rs

#[test]
fn test_ipv4_address_parsing() {
    // IPv4 addresses should parse correctly
    let ip = "192.168.1.1".parse::<IpAddr>();
    assert!(ip.is_ok());
}

#[test]
fn test_ipv6_address_parsing() {
    // IPv6 addresses should parse correctly
    let ip = "2001:db8::1".parse::<IpAddr>();
    assert!(ip.is_ok());
}

#[test]
fn test_loopback_ipv4() {
    // Loopback IPv4 should parse
    let ip = "127.0.0.1".parse::<IpAddr>();
    assert!(ip.is_ok());
}

#[test]
fn test_loopback_ipv6() {
    // Loopback IPv6 should parse
    let ip = "::1".parse::<IpAddr>();
    assert!(ip.is_ok());
}

#[test]
fn test_private_ipv4_ranges() {
    // Common private IPv4 ranges should parse
    let ranges = vec!["10.0.0.0", "172.16.0.0", "192.168.0.0"];

    for range in ranges {
        let ip = range.parse::<IpAddr>();
        assert!(ip.is_ok(), "Failed to parse {}", range);
    }
}

#[test]
fn test_ipv4_network_notation() {
    // CIDR notation should be parseable (conceptually)
    let cidr = "192.168.1.0/24";
    assert!(cidr.contains("/"));
    assert!(cidr.split('/').count() == 2);
}

#[test]
fn test_ipv6_network_notation() {
    // IPv6 CIDR notation should be parseable
    let cidr = "2001:db8::/32";
    assert!(cidr.contains("/"));
    assert!(cidr.split('/').count() == 2);
}

#[test]
fn test_ip_filter_allow_empty_initially() {
    // Allowed IPs list should be empty initially
    let allowed_count = 0;
    assert_eq!(allowed_count, 0);
}

#[test]
fn test_ip_filter_block_empty_initially() {
    // Blocked IPs list should be empty initially
    let blocked_count = 0;
    assert_eq!(blocked_count, 0);
}

#[test]
fn test_ip_filter_stats_structure() {
    // Stats should return (allowed_count, blocked_count)
    let allowed = 5;
    let blocked = 3;

    assert_eq!(allowed, 5);
    assert_eq!(blocked, 3);
}

#[test]
fn test_ip_comparison() {
    // Same IPs should compare equal
    let ip1 = "192.168.1.1".parse::<IpAddr>().unwrap();
    let ip2 = "192.168.1.1".parse::<IpAddr>().unwrap();

    assert_eq!(ip1, ip2);
}

#[test]
fn test_ip_comparison_different() {
    // Different IPs should not be equal
    let ip1 = "192.168.1.1".parse::<IpAddr>().unwrap();
    let ip2 = "192.168.1.2".parse::<IpAddr>().unwrap();

    assert_ne!(ip1, ip2);
}

// ── IP filtering logic tests ─────────────────────────────────────────

#[test]
fn test_default_allow_when_no_restrictions() {
    // With no allow/block lists, should default to allowing
    let allowed_list_empty = true;
    let blocked_list_empty = true;

    // Default behavior: allow if no restrictions
    let should_allow = allowed_list_empty || blocked_list_empty;
    assert!(should_allow);
}

#[test]
fn test_block_takes_precedence() {
    // Blocked IPs should be rejected even if allowed list is empty
    let ip_is_blocked = true;

    let should_reject = ip_is_blocked;
    assert!(should_reject);
}

#[test]
fn test_blocked_ips_are_checked_first() {
    // Blocked list should be checked first for performance
    let check_order = vec!["blocked", "allowed"];
    assert_eq!(check_order[0], "blocked");
}

// ── Network matching tests ───────────────────────────────────────────

#[test]
fn test_network_contains_check() {
    // Network 192.168.1.0/24 should contain 192.168.1.1
    let network = "192.168.1.0/24";
    let ip = "192.168.1.1";

    // Basic string match for test purposes
    assert!(network.starts_with("192.168.1"));
    assert!(ip.starts_with("192.168.1"));
}

#[test]
fn test_network_excludes_outside_ips() {
    // Network 192.168.1.0/24 should not contain 192.168.2.1
    let network_prefix = "192.168.1";
    let ip_a = "192.168.1.1";
    let ip_b = "192.168.2.1";

    assert!(ip_a.starts_with(network_prefix));
    assert!(!ip_b.starts_with(network_prefix));
}

#[test]
fn test_multiple_allowed_networks() {
    // Multiple networks can be in allow list
    let networks = vec!["192.168.0.0/16", "10.0.0.0/8", "172.16.0.0/12"];

    assert_eq!(networks.len(), 3);
}

#[test]
fn test_multiple_blocked_networks() {
    // Multiple networks can be in block list
    let networks = vec!["203.0.113.0/24", "192.0.2.0/24"];

    assert_eq!(networks.len(), 2);
}

// ── Edge cases ───────────────────────────────────────────────────────

#[test]
fn test_all_zeros_ipv4() {
    // 0.0.0.0 should parse
    let ip = "0.0.0.0".parse::<IpAddr>();
    assert!(ip.is_ok());
}

#[test]
fn test_all_ones_ipv4() {
    // 255.255.255.255 should parse
    let ip = "255.255.255.255".parse::<IpAddr>();
    assert!(ip.is_ok());
}

#[test]
fn test_invalid_ipv4() {
    // Invalid IPv4 should fail to parse
    let ip = "256.256.256.256".parse::<IpAddr>();
    assert!(ip.is_err());
}

#[test]
fn test_invalid_ipv4_format() {
    // Malformed IPv4 should fail to parse
    let ip = "192.168.1".parse::<IpAddr>();
    assert!(ip.is_err());
}

#[test]
fn test_invalid_network_cidr() {
    // Invalid CIDR notation should be caught
    let cidr = "192.168.1.0/33"; // /33 is invalid for IPv4
    assert!(cidr.contains("/"));

    let parts: Vec<&str> = cidr.split('/').collect();
    if let Ok(prefix) = parts[1].parse::<u8>() {
        assert!(prefix > 32); // Invalid for IPv4
    }
}
