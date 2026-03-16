use shared::types::cache::*;

#[test]
fn cache_strategy_display_variants_are_non_empty() {
    let strategies = [
        CacheStrategy::Yes,
        CacheStrategy::No,
        CacheStrategy::Explicit,
    ];
    for s in &strategies {
        let out = format!("{}", s);
        assert!(!out.is_empty());
    }
}

#[test]
fn cache_strategy_clone_and_copy() {
    let a = CacheStrategy::Yes;
    let b = a; // Verifies Copy trait
    let c = a.clone(); // Verifies Clone trait
    let _ = (b, c);
}

#[test]
fn cache_strategy_deserializes_from_string() {
    let json = r#""Yes""#;
    let s: CacheStrategy = serde_json::from_str(json).unwrap();
    assert!(matches!(s, CacheStrategy::Yes));
}
