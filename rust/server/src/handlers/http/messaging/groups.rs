use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use std::convert::Infallible;
use tracing::info;

use crate::AppState;
use crate::handlers::http::utils::{deliver_error_json, deliver_success_json};
use shared::types::jwt::JwtClaims;

// ---------------------------------------------------------------------------
// Handlers
//
// Auth is performed by the router BEFORE these handlers are called.
//   - GET handlers receive `claims: JwtClaims`  (light auth — JWT only)
//   - POST / DELETE handlers receive `user_id: i64`  (hard auth — JWT + DB + IP)
//
// No handler calls any auth function internally.
// ---------------------------------------------------------------------------

/// GET /api/groups — list groups the authenticated user belongs to.
///
/// Light-auth route: claims are pre-verified (JWT signature + expiry only).
pub async fn handle_get_groups(
    _req: Request<Incoming>,
    state: AppState,
    claims: JwtClaims,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::groups as db_groups;

    let user_id = claims.user_id;
    info!("Fetching groups for user {}", user_id);

    let groups = db_groups::get_user_groups(&state.db, user_id)
        .await
        .context("Failed to fetch groups")?;

    // Only return proper group chats from this endpoint, not DMs.
    let groups_json: Vec<serde_json::Value> = groups
        .into_iter()
        .filter(|g| g.chat_type == "group")
        .map(|g| {
            serde_json::json!({
                "id":          g.id,
                "name":        g.name,
                "description": g.description,
                "created_by":  g.created_by,
                "created_at":  g.created_at,
                "chat_type":   g.chat_type,
            })
        })
        .collect();

    deliver_success_json(
        Some(serde_json::json!({ "groups": groups_json })),
        None,
        StatusCode::OK,
    )
}

/// POST /api/groups — create a new group chat.
///
/// Hard-auth route: `user_id` is pre-verified (JWT + DB session lookup + IP).
pub async fn handle_create_group(
    req: Request<Incoming>,
    state: AppState,
    user_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::groups as db_groups;

    info!("User {} creating a new group", user_id);

    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params: serde_json::Value =
        serde_json::from_slice(&body).context("Failed to parse JSON request body")?;

    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing or invalid group name"))?
        .to_string();

    if name.trim().is_empty() {
        return deliver_error_json(
            "INVALID_INPUT",
            "Group name cannot be empty",
            StatusCode::BAD_REQUEST,
        );
    }

    let description: Option<String> = params
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let chat_id = db_groups::create_group(
        &state.db,
        db_groups::NewGroup {
            name: name.clone(),
            created_by: user_id,
            description: description.clone(),
            chat_type: "group".to_string(),
        },
    )
    .await
    .context("Failed to create group")?;

    info!("Group {} created by user {}", chat_id, user_id);

    deliver_success_json(
        Some(serde_json::json!({
            "chat_id":    chat_id,
            "name":        name,
            "description": description,
            "chat_type":   "group",
        })),
        Some("Group created successfully"),
        StatusCode::CREATED,
    )
}

/// GET /api/groups/:id/members — list members of a group.
///
/// Light-auth route: claims are pre-verified (JWT signature + expiry only).
pub async fn handle_get_members(
    _req: Request<Incoming>,
    state: AppState,
    _claims: JwtClaims,
    chat_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::groups as db_groups;

    info!("Fetching members for group {}", chat_id);

    let members = db_groups::get_group_members(&state.db, chat_id)
        .await
        .context("Failed to fetch group members")?;

    let members_json: Vec<serde_json::Value> = members
        .into_iter()
        .map(|m| {
            serde_json::json!({
                "user_id":   m.user_id,
                "chat_id":  m.chat_id,
                "role":      m.role,
                "joined_at": m.joined_at,
            })
        })
        .collect();

    deliver_success_json(
        Some(serde_json::json!({
            "chat_id": chat_id,
            "members":  members_json,
        })),
        None,
        StatusCode::OK,
    )
}

/// POST /api/groups/:id/members — add a member to a group.
///
/// Hard-auth route: `user_id` is pre-verified (JWT + DB session lookup + IP).
pub async fn handle_add_member(
    req: Request<Incoming>,
    state: AppState,
    _user_id: i64,
    chat_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::groups as db_groups;

    info!("Adding member to group {}", chat_id);

    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params: serde_json::Value =
        serde_json::from_slice(&body).context("Failed to parse JSON request body")?;

    let target_user_id: i64 = params
        .get("user_id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| anyhow::anyhow!("Invalid or missing user_id"))?;

    // For group chats new members join as regular members; the caller can
    // promote them separately via update_member_role if needed.
    // For direct chats this endpoint shouldn't normally be called, but if it
    // is the role will default to "member" — the client should use the DM
    // creation flow instead.
    let role = params
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("member")
        .to_string();

    db_groups::add_group_member(&state.db, chat_id, target_user_id, role)
        .await
        .context("Failed to add group member")?;

    deliver_success_json(
        Some(serde_json::json!({
            "chat_id": chat_id,
            "user_id":  target_user_id,
        })),
        Some("Member added successfully"),
        StatusCode::OK,
    )
}

/// DELETE /api/groups/:id/members — remove a member from a group.
///
/// Hard-auth route: `user_id` is pre-verified (JWT + DB session lookup + IP).
pub async fn handle_remove_member(
    req: Request<Incoming>,
    state: AppState,
    _user_id: i64,
    chat_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::groups as db_groups;

    info!("Removing member from group {}", chat_id);

    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params: serde_json::Value =
        serde_json::from_slice(&body).context("Failed to parse JSON request body")?;

    let target_user_id: i64 = params
        .get("user_id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| anyhow::anyhow!("Invalid or missing user_id"))?;

    let removed = db_groups::remove_group_member(&state.db, chat_id, target_user_id)
        .await
        .context("Failed to remove group member")?;

    if !removed {
        return deliver_error_json(
            "NOT_FOUND",
            "User is not a member of this group",
            StatusCode::NOT_FOUND,
        );
    }

    deliver_success_json(
        Some(serde_json::json!({
            "chat_id": chat_id,
            "user_id":  target_user_id,
        })),
        Some("Member removed successfully"),
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    #[test]
    fn participants_csv_parses_to_vec() {
        let raw = "1,2, 3, 42";
        let ids: Vec<i64> = raw
            .split(',')
            .filter_map(|p| p.trim().parse().ok())
            .collect();
        assert_eq!(ids, vec![1, 2, 3, 42]);
    }

    #[test]
    fn participants_empty_string_gives_empty_vec() {
        let raw = "";
        let ids: Vec<i64> = raw
            .split(',')
            .filter_map(|p| p.trim().parse::<i64>().ok())
            .collect();
        assert!(ids.is_empty());
    }

    #[test]
    fn role_defaults_to_member() {
        let params: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let role = params
            .get("role")
            .cloned()
            .unwrap_or_else(|| "member".to_string());
        assert_eq!(role, "member");
    }

    #[test]
    fn user_id_parses_from_string() {
        let s = "42";
        let id: Option<i64> = s.parse::<i64>().ok();
        assert_eq!(id, Some(42));
    }

    #[test]
    fn invalid_user_id_gives_none() {
        let s = "not_a_number";
        let id: Option<i64> = s.parse::<i64>().ok();
        assert!(id.is_none());
    }
}
