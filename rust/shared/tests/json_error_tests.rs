use shared::types::json_error::*;

#[test]
fn error_response_new_sets_status_to_error() {
    let e = ErrorResponse::new("NOT_FOUND", "resource missing");
    assert_eq!(e.status, "error");
    assert_eq!(e.code, "NOT_FOUND");
    assert_eq!(e.message, "resource missing");
}

#[test]
fn error_response_serializes_correctly() {
    let e = ErrorResponse::new("FORBIDDEN", "access denied");
    let json = serde_json::to_value(&e).unwrap();
    assert_eq!(json["status"], "error");
    assert_eq!(json["code"], "FORBIDDEN");
}
