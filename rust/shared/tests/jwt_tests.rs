use shared::types::jwt::*;

fn sample_claims() -> JwtClaims {
    JwtClaims {
        sub: "alice".to_string(),
        user_id: 42,
        session_id: "550e8400-e29b-41d4-a716-446655440000".to_string(),
        user_agent: "Mozilla/5.0 (X11; Linux x86_64)".to_string(),
        is_admin: false,
        exp: 9_999_999_999,
        iat: 1_700_000_000,
    }
}

#[test]
fn claims_serialize_and_deserialize_roundtrip() {
    let c = sample_claims();
    let json = serde_json::to_string(&c).unwrap();
    let back: JwtClaims = serde_json::from_str(&json).unwrap();
    assert_eq!(back.sub, c.sub);
    assert_eq!(back.user_id, c.user_id);
    assert_eq!(back.session_id, c.session_id);
    assert_eq!(back.user_agent, c.user_agent);
    assert_eq!(back.is_admin, c.is_admin);
    assert_eq!(back.exp, c.exp);
    assert_eq!(back.iat, c.iat);
}

#[test]
fn claims_json_contains_expected_keys() {
    let json = serde_json::to_value(&sample_claims()).unwrap();
    for key in &[
        "sub",
        "user_id",
        "session_id",
        "user_agent",
        "is_admin",
        "exp",
        "iat",
    ] {
        assert!(json.get(key).is_some(), "missing key: {}", key);
    }
}

#[test]
fn admin_claims_carry_is_admin_true() {
    let mut c = sample_claims();
    c.is_admin = true;
    let json = serde_json::to_value(&c).unwrap();
    assert_eq!(json["is_admin"], true);
}

#[test]
fn clone_produces_independent_copy() {
    let c1 = sample_claims();
    let mut c2 = c1.clone();
    c2.user_id = 99;
    assert_eq!(c1.user_id, 42);
    assert_eq!(c2.user_id, 99);
}

#[test]
fn session_id_is_a_string_field() {
    let c = sample_claims();
    let json = serde_json::to_value(&c).unwrap();
    assert!(json["session_id"].is_string());
}
