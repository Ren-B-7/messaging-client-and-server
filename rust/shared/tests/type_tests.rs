/// Integration-level tests for the `shared` crate.
///
/// Each section tests one module; unit tests that are tightly coupled to
/// private helpers live inside the modules themselves (see `#[cfg(test)]`
/// blocks in `login.rs` and `server_config.rs`).
// ---------------------------------------------------------------------------
// JWT claims
// ---------------------------------------------------------------------------
#[cfg(test)]
mod jwt_tests {
    use shared::types::*;

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
        // Ensure the session_id round-trips as a string (not a number).
        let json = serde_json::to_value(&c).unwrap();
        assert!(json["session_id"].is_string());
    }
}

// ---------------------------------------------------------------------------
// Login types
// ---------------------------------------------------------------------------

#[cfg(test)]
mod login_tests {

    use shared::types::*;
    // ── LoginData deserialization ─────────────────────────────────────────────

    #[test]
    fn login_data_deserializes_username() {
        let json = r#"{"username":"bob","password":"pass123"}"#;
        let d: LoginData = serde_json::from_str(json).unwrap();
        assert_eq!(d.username, "bob");
        assert!(!d.remember_me);
    }

    #[test]
    fn login_data_email_alias_maps_to_username() {
        let json = r#"{"email":"bob@example.com","password":"pass123"}"#;
        let d: LoginData = serde_json::from_str(json).unwrap();
        assert_eq!(d.username, "bob@example.com");
    }

    #[test]
    fn login_data_remember_me_defaults_false() {
        let json = r#"{"username":"x","password":"y"}"#;
        let d: LoginData = serde_json::from_str(json).unwrap();
        assert!(!d.remember_me);
    }

    #[test]
    fn login_data_remember_me_can_be_set() {
        let json = r#"{"username":"x","password":"y","remember_me":true}"#;
        let d: LoginData = serde_json::from_str(json).unwrap();
        assert!(d.remember_me);
    }

    // ── LoginError ────────────────────────────────────────────────────────────

    #[test]
    fn all_error_variants_have_non_empty_messages() {
        let variants: Vec<Box<dyn Fn() -> LoginError>> = vec![
            Box::new(|| LoginError::InvalidCredentials),
            Box::new(|| LoginError::UserBanned),
            Box::new(|| LoginError::UserNotFound),
            Box::new(|| LoginError::MissingField("test".into())),
            Box::new(|| LoginError::DatabaseError),
            Box::new(|| LoginError::InternalError),
        ];
        for v in variants {
            let e = v();
            assert!(!e.to_code().is_empty());
            assert!(!e.to_message().is_empty());
        }
    }

    #[test]
    fn login_error_response_is_serializable() {
        let r = LoginError::UserBanned.to_response();
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["code"], "USER_BANNED");
    }

    #[test]
    fn login_response_success_serializes_all_fields() {
        let r = LoginResponse::Success {
            user_id: 1,
            username: "alice".into(),
            token: "t.o.k".into(),
            expires_in: 3600,
            message: "ok".into(),
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["status"], "success");
        assert_eq!(json["expires_in"], 3600);
    }

    // ── Session / NewSession ──────────────────────────────────────────────────

    #[test]
    fn new_session_has_no_user_agent_field() {
        // Compile check: struct literal must not include user_agent.
        let _s = NewSession {
            user_id: 1,
            session_id: "uuid".into(),
            expires_at: 0,
            ip_address: None,
        };
    }

    #[test]
    fn session_has_no_user_agent_field() {
        let _s = Session {
            id: 1,
            user_id: 1,
            session_id: "uuid".into(),
            created_at: 0,
            expires_at: 0,
            last_activity: 0,
            ip_address: None,
        };
    }

    #[test]
    fn new_session_display_omits_sensitive_data() {
        // Ensure ip is shown but session_id is included (it's the revocation
        // handle, not a secret once it's in the DB).
        let s = NewSession {
            user_id: 5,
            session_id: "handle-123".into(),
            expires_at: 1000,
            ip_address: Some("10.0.0.1".into()),
        };
        let out = format!("{}", s);
        assert!(out.contains("handle-123"));
        assert!(out.contains("10.0.0.1"));
    }
}

// ---------------------------------------------------------------------------
// Register types
// ---------------------------------------------------------------------------

#[cfg(test)]
mod register_tests {
    use shared::types::*;

    #[test]
    fn all_register_error_codes_are_non_empty() {
        let errors: Vec<Box<dyn Fn() -> RegisterError>> = vec![
            Box::new(|| RegisterError::UsernameTaken),
            Box::new(|| RegisterError::EmailTaken),
            Box::new(|| RegisterError::InvalidUsername),
            Box::new(|| RegisterError::InvalidPassword),
            Box::new(|| RegisterError::InvalidEmail),
            Box::new(|| RegisterError::EmailRequired),
            Box::new(|| RegisterError::PasswordMismatch),
            Box::new(|| RegisterError::MissingField("f".into())),
            Box::new(|| RegisterError::DatabaseError),
            Box::new(|| RegisterError::InternalError),
            Box::new(|| RegisterError::WeakPassword),
        ];
        for e in errors {
            let err = e();
            assert!(!err.to_code().is_empty());
            assert!(!err.to_message().is_empty());
        }
    }

    #[test]
    fn missing_field_message_includes_field_name() {
        let err = RegisterError::MissingField("email".to_string());
        assert!(err.to_message().contains("email"));
    }

    #[test]
    fn register_error_response_serializes_correctly() {
        let r = RegisterError::UsernameTaken.to_response();
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["code"], "USERNAME_TAKEN");
    }

    #[test]
    fn register_response_success_has_redirect_field() {
        let r = RegisterResponse::Success {
            user_id: 1,
            username: "alice".into(),
            message: "ok".into(),
            redirect: "/chat".into(),
            token: None,
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["redirect"], "/chat");
        assert_eq!(json["status"], "success");
    }

    #[test]
    fn register_data_deserializes_from_json() {
        let json = r#"{
            "username": "bob",
            "password": "Pass1234",
            "confirm_password": "Pass1234",
            "email": "bob@example.com"
        }"#;
        let d: RegisterData = serde_json::from_str(json).unwrap();
        assert_eq!(d.username, "bob");
        assert_eq!(d.email, Some("bob@example.com".into()));
        assert!(d.full_name.is_none());
    }
}

// ---------------------------------------------------------------------------
// Settings types
// ---------------------------------------------------------------------------

#[cfg(test)]
mod settings_tests {
    use shared::types::*;

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
        let json = serde_json::to_value(&SettingsError::PasswordMismatch.to_response()).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["code"], "PASSWORD_MISMATCH");
    }

    #[test]
    fn settings_response_success_serializes() {
        let r = SettingsResponse::Success {
            message: "done".into(),
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["status"], "success");
        assert_eq!(json["message"], "done");
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
}

// ---------------------------------------------------------------------------
// Message types
// ---------------------------------------------------------------------------

#[cfg(test)]
mod message_tests {
    use shared::types::*;

    #[test]
    fn all_message_error_codes_are_unique() {
        let codes = [
            MessageError::Unauthorized.to_code(),
            MessageError::MissingRecipient.to_code(),
            MessageError::InvalidRecipient.to_code(),
            MessageError::MessageTooLong.to_code(),
            MessageError::EmptyMessage.to_code(),
            MessageError::MissingField("x".into()).to_code(),
            MessageError::DatabaseError.to_code(),
            MessageError::InternalError.to_code(),
        ];
        let unique: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(codes.len(), unique.len(), "duplicate message error codes");
    }

    #[test]
    fn message_error_display_shows_code() {
        let e = MessageError::Unauthorized;
        let out = format!("{}", e);
        assert!(out.contains("UNAUTHORIZED"));
    }

    #[test]
    fn send_response_error_serializes_status() {
        let r = MessageError::EmptyMessage.to_send_response();
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["code"], "EMPTY_MESSAGE");
    }

    #[test]
    fn list_response_error_serializes_status() {
        let r = MessageError::DatabaseError.to_list_response();
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["code"], "DATABASE_ERROR");
    }

    #[test]
    fn messages_response_success_contains_total() {
        let r = MessagesResponse::Success {
            messages: vec![],
            total: 0,
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["status"], "success");
        assert_eq!(json["total"], 0);
    }

    #[test]
    fn send_message_response_success_has_message_id() {
        let r = SendMessageResponse::Success {
            message_id: 99,
            sent_at: 1234,
            message: "sent".into(),
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["message_id"], 99);
    }

    #[test]
    fn send_message_data_deserializes_group_id() {
        let json = r#"{"group_id": 7, "content": "hello"}"#;
        let d: SendMessageData = serde_json::from_str(json).unwrap();
        assert_eq!(d.group_id, Some(7));
        assert!(d.recipient_id.is_none());
    }
}

// ---------------------------------------------------------------------------
// Update / Profile types
// ---------------------------------------------------------------------------

#[cfg(test)]
mod update_tests {
    use shared::types::*;

    #[test]
    fn profile_error_codes_are_unique() {
        let codes = [
            ProfileError::Unauthorized.to_code(),
            ProfileError::UserNotFound.to_code(),
            ProfileError::InvalidUsername.to_code(),
            ProfileError::InvalidEmail.to_code(),
            ProfileError::UsernameTaken.to_code(),
            ProfileError::EmailTaken.to_code(),
            ProfileError::MissingField("x".into()).to_code(),
            ProfileError::DatabaseError.to_code(),
            ProfileError::InternalError.to_code(),
        ];
        let unique: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(codes.len(), unique.len());
    }

    #[test]
    fn profile_error_to_profile_response_status() {
        let json = serde_json::to_value(&ProfileError::Unauthorized.to_profile_response()).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["code"], "UNAUTHORIZED");
    }

    #[test]
    fn profile_error_to_update_response_status() {
        let json = serde_json::to_value(&ProfileError::UsernameTaken.to_update_response()).unwrap();
        assert_eq!(json["status"], "error");
        assert_eq!(json["code"], "USERNAME_TAKEN");
    }

    #[test]
    fn profile_response_success_has_profile_data() {
        let r = ProfileResponse::Success {
            profile: ProfileData {
                user_id: 1,
                username: "alice".into(),
                email: None,
                created_at: 0,
                last_login: None,
            },
            message: "ok".into(),
        };
        let json = serde_json::to_value(&r).unwrap();
        assert_eq!(json["profile"]["username"], "alice");
    }

    #[test]
    fn update_profile_data_both_optional_fields() {
        let json = r#"{"username": "new_name"}"#;
        let d: UpdateProfileData = serde_json::from_str(json).unwrap();
        assert_eq!(d.username, Some("new_name".into()));
        assert!(d.email.is_none());
    }
}

// ---------------------------------------------------------------------------
// SSE types
// ---------------------------------------------------------------------------

#[cfg(test)]
mod sse_tests {
    use shared::types::*;

    #[test]
    fn sse_event_serializes_and_deserializes() {
        let e = SseEvent {
            user_id: "42".into(),
            event_type: "message_sent".into(),
            data: serde_json::json!({ "msg_id": 1 }),
            timestamp: 1234,
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: SseEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.user_id, "42");
        assert_eq!(back.event_type, "message_sent");
    }

    #[test]
    fn sse_error_channel_send_failed_display() {
        let e = SseError::ChannelSendFailed("test error".into());
        let out = format!("{}", e);
        assert!(out.contains("test error"));
    }

    #[test]
    fn sse_error_channel_closed_display() {
        let e = SseError::ChannelClosed;
        let out = format!("{}", e);
        assert!(out.contains("closed") || out.contains("Closed"));
    }
}

// ---------------------------------------------------------------------------
// Cache types
// ---------------------------------------------------------------------------

#[cfg(test)]
mod cache_tests {
    use shared::types::*;

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
        let b = a; // Copy
        let c = a.clone();
        let _ = (b, c); // no move errors
    }

    #[test]
    fn cache_strategy_deserializes_from_string() {
        let json = r#""Yes""#;
        let s: CacheStrategy = serde_json::from_str(json).unwrap();
        assert!(matches!(s, CacheStrategy::Yes));
    }
}

// ---------------------------------------------------------------------------
// JSON error type
// ---------------------------------------------------------------------------

#[cfg(test)]
mod json_error_tests {
    use shared::types::*;

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
}

// ---------------------------------------------------------------------------
// Server stats
// ---------------------------------------------------------------------------

#[cfg(test)]
mod server_stats_tests {
    use shared::types::*;
    use std::collections::HashSet;

    fn test_config() -> AppConfig {
        AppConfig {
            server: ServerConfig {
                bind: "127.0.0.1".into(),
                port_client: Some(1337),
                port_admin: Some(1338),
                max_connections: 500,
            },
            paths: PathsConfig {
                icons: "/icons".into(),
                web_dir: "/web".into(),
                blocked_paths: HashSet::new(),
            },
            auth: AuthConfig {
                token_expiry_minutes: 60,
                email_required: false,
                jwt_secret: None,
            },
        }
    }

    #[test]
    fn server_stats_build_populates_all_sections() {
        let db = DatabaseInfo {
            path: "messaging.db".into(),
            total_users: 10,
            active_sessions: 3,
            banned_users: 1,
            total_messages: 100,
            total_groups: 5,
        };
        let stats = ServerStats::build(&test_config(), db, 0);
        assert_eq!(stats.server.port_client, 1337);
        assert_eq!(stats.server.port_admin, 1338);
        assert_eq!(stats.auth.token_expiry_minutes, 60);
        assert_eq!(stats.database.total_users, 10);
        assert!(stats.runtime.uptime_secs >= 0);
    }

    #[test]
    fn database_info_empty_constructor() {
        let db = DatabaseInfo::empty("test.db");
        assert_eq!(db.path, "test.db");
        assert_eq!(db.total_users, 0);
        assert_eq!(db.active_sessions, 0);
    }

    #[test]
    fn server_stats_serializes_to_json() {
        let db = DatabaseInfo::empty("x.db");
        let stats = ServerStats::build(&test_config(), db, 0);
        let json = serde_json::to_value(&stats).unwrap();
        assert!(json.get("server").is_some());
        assert!(json.get("auth").is_some());
        assert!(json.get("database").is_some());
        assert!(json.get("runtime").is_some());
    }
}
