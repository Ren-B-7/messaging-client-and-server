use shared::types::{server_config::*, server_stats::*};
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
            uploads_dir: "/uploads".into(),
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
    assert_eq!(stats.database.total_users, 10);
}

#[test]
fn server_stats_serializes_to_json() {
    let db = DatabaseInfo::empty("x.db");
    let stats = ServerStats::build(&test_config(), db, 0);
    let json = serde_json::to_value(&stats).unwrap();
    assert!(json.get("server").is_some());
}
