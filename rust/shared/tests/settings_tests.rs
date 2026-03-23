use shared::types::settings::*;

#[test]
fn all_settings_error_codes_unique() {
    let codes = [
        SettingsError::Unauthorized.to_code(),
        SettingsError::InvalidCurrentPassword.to_code(),
        SettingsError::InvalidNewPassword.to_code(),
        SettingsError::PasswordMismatch.to_code(),
        SettingsError::PasswordTooWeak.to_code(),
        SettingsError::SamePassword.to_code(),
        SettingsError::MissingField("x".into()).to_code(),
        SettingsError::DatabaseError.to_code(),
        SettingsError::InternalError.to_code(),
    ];
    let unique: std::collections::HashSet<_> = codes.iter().collect();
    assert_eq!(codes.len(), unique.len(), "duplicate settings error codes");
}

#[test]
fn settings_error_to_response_is_error_status() {
    let json = serde_json::to_value(SettingsError::PasswordMismatch.to_response()).unwrap();
    assert_eq!(json["status"], "error");
    assert_eq!(json["code"], "PASSWORD_MISMATCH");
}

#[test]
fn change_password_data_deserializes() {
    let json = r#"{
        "current_password": "OldPass1",
        "new_password": "NewPass2",
        "confirm_password": "NewPass2"
    }"#;
    let d: ChangePasswordData = serde_json::from_str(json).unwrap();
    assert_eq!(d.current_password, "OldPass1");
    assert_eq!(d.new_password, d.confirm_password);
}
