use anyhow::{Context, Result};
use bytes::Bytes;
use form_urlencoded;
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
    use crate::database::register as db_register;

    info!("Fetching members for group {}", chat_id);

    let members = db_groups::get_group_members(&state.db, chat_id)
        .await
        .context("Failed to fetch group members")?;

    // Resolve usernames for each member
    let mut members_json: Vec<serde_json::Value> = Vec::with_capacity(members.len());
    for m in members {
        let username = db_register::get_user_by_id(&state.db, m.user_id)
            .await
            .ok()
            .flatten()
            .map(|u| u.username)
            .unwrap_or_else(|| format!("user_{}", m.user_id));

        members_json.push(serde_json::json!({
            "user_id":   m.user_id,
            "username":  username,
            "chat_id":   m.chat_id,
            "role":      m.role,
            "joined_at": m.joined_at,
        }));
    }

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
/// Accepts `{ "username": "alice" }` or `{ "user_id": 42 }`.
///
/// Hard-auth route: `user_id` is pre-verified (JWT + DB session lookup + IP).
pub async fn handle_add_member(
    req: Request<Incoming>,
    state: AppState,
    _user_id: i64,
    chat_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::groups as db_groups;
    use crate::database::register as db_register;

    info!("Adding member to group {}", chat_id);

    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params: serde_json::Value =
        serde_json::from_slice(&body).context("Failed to parse JSON request body")?;

    // Resolve target user — accept either user_id or username
    let target_user_id: i64 = if let Some(uid) = params.get("user_id").and_then(|v| v.as_i64()) {
        uid
    } else if let Some(username) = params.get("username").and_then(|v| v.as_str()) {
        match db_register::get_user_by_username(&state.db, username.to_string()).await? {
            Some(user) => user.id,
            None => {
                return deliver_error_json(
                    "NOT_FOUND",
                    &format!("User '{}' not found", username),
                    StatusCode::NOT_FOUND,
                );
            }
        }
    } else {
        return deliver_error_json(
            "INVALID_INPUT",
            "Request must include either 'username' or 'user_id'",
            StatusCode::BAD_REQUEST,
        );
    };

    // Check if already a member
    let already_member = db_groups::is_group_member(&state.db, chat_id, target_user_id)
        .await
        .unwrap_or(false);

    if already_member {
        return deliver_error_json(
            "CONFLICT",
            "User is already a member of this group",
            StatusCode::CONFLICT,
        );
    }

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

/// PATCH /api/groups/:id — rename a group.
///
/// Body: `{ "name": "New Name" }`
///
/// Hard-auth route: `user_id` is pre-verified (JWT + DB session lookup + IP).
pub async fn handle_rename_group(
    req: Request<Incoming>,
    state: AppState,
    user_id: i64,
    chat_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::groups as db_groups;

    info!("User {} renaming group {}", user_id, chat_id);

    // Verify the caller is a member of the group
    let is_member = db_groups::is_group_member(&state.db, chat_id, user_id)
        .await
        .unwrap_or(false);

    if !is_member {
        return deliver_error_json(
            "FORBIDDEN",
            "You are not a member of this group",
            StatusCode::FORBIDDEN,
        );
    }

    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params: serde_json::Value =
        serde_json::from_slice(&body).context("Failed to parse JSON request body")?;

    let new_name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing or invalid group name"))?
        .trim()
        .to_string();

    if new_name.is_empty() {
        return deliver_error_json(
            "INVALID_INPUT",
            "Group name cannot be empty",
            StatusCode::BAD_REQUEST,
        );
    }

    if new_name.len() > 100 {
        return deliver_error_json(
            "INVALID_INPUT",
            "Group name cannot exceed 100 characters",
            StatusCode::BAD_REQUEST,
        );
    }

    db_groups::update_group_name(&state.db, chat_id, new_name.clone())
        .await
        .context("Failed to rename group")?;

    info!("Group {} renamed to '{}' by user {}", chat_id, new_name, user_id);

    deliver_success_json(
        Some(serde_json::json!({
            "chat_id": chat_id,
            "name":    new_name,
        })),
        Some("Group renamed successfully"),
        StatusCode::OK,
    )
}

/// DELETE /api/groups/:id — delete a group and all its messages.
///
/// Hard-auth route: `user_id` is pre-verified (JWT + DB session lookup + IP).
pub async fn handle_delete_group(
    _req: Request<Incoming>,
    state: AppState,
    user_id: i64,
    chat_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::groups as db_groups;

    info!("User {} deleting group {}", user_id, chat_id);

    // Fetch the group to verify it exists and is a group (not a DM)
    let group = match db_groups::get_group(&state.db, chat_id).await? {
        Some(g) => g,
        None => {
            return deliver_error_json("NOT_FOUND", "Group not found", StatusCode::NOT_FOUND);
        }
    };

    if group.chat_type != "group" {
        return deliver_error_json(
            "INVALID_INPUT",
            "Cannot delete a direct message conversation",
            StatusCode::BAD_REQUEST,
        );
    }

    // Only group creator or admins can delete the group
    if group.created_by != user_id {
        return deliver_error_json(
            "FORBIDDEN",
            "Only the group creator can delete this group",
            StatusCode::FORBIDDEN,
        );
    }

    db_groups::delete_group(&state.db, chat_id)
        .await
        .context("Failed to delete group")?;

    info!("Group {} deleted by user {}", chat_id, user_id);

    deliver_success_json(
        Some(serde_json::json!({ "chat_id": chat_id })),
        Some("Group deleted successfully"),
        StatusCode::OK,
    )
}

/// GET /api/users/search?q=alice — search for users by username prefix.
///
/// Light-auth route: claims are pre-verified (JWT signature + expiry only).
pub async fn handle_search_users(
    req: Request<Incoming>,
    state: AppState,
    claims: JwtClaims,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::register as db_register;

    let query_str = req.uri().query().unwrap_or("");
    let q: String = form_urlencoded::parse(query_str.as_bytes())
        .find(|(k, _)| k == "q")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default()
        .trim()
        .to_string();

    if q.is_empty() {
        return deliver_success_json(
            Some(serde_json::json!({ "users": [] })),
            None,
            StatusCode::OK,
        );
    }

    if q.len() > 50 {
        return deliver_error_json(
            "INVALID_INPUT",
            "Search query too long",
            StatusCode::BAD_REQUEST,
        );
    }

    let users = db_register::search_users_by_username(&state.db, &q, 10)
        .await
        .context("Failed to search users")?;

    let users_json: Vec<serde_json::Value> = users
        .into_iter()
        .filter(|u| u.id != claims.user_id) // exclude self
        .map(|u| {
            serde_json::json!({
                "user_id":  u.id,
                "username": u.username,
            })
        })
        .collect();

    deliver_success_json(
        Some(serde_json::json!({ "users": users_json })),
        None,
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
