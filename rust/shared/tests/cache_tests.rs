use shared::types::cache::*;

#[test]
fn cache_strategy_display_variants_are_non_empty() {
    let strategies = [
        CacheStrategy::LongTerm,
        CacheStrategy::ShortTerm,
        CacheStrategy::NoCache,
    ];
    for s in &strategies {
        let out = format!("{}", s);
        assert!(!out.is_empty());
    }
}

#[test]
fn cache_strategy_clone_and_copy() {
    let a = CacheStrategy::LongTerm;
    let b = a; // Verifies Copy trait
    let c = a.clone(); // Verifies Clone trait
    let _ = (b, c);
}

#[test]
fn cache_strategy_deserializes_from_string() {
    let json = r#""ShortTerm""#;
    let s: CacheStrategy = serde_json::from_str(json).unwrap();
    assert!(matches!(s, CacheStrategy::ShortTerm));
}
