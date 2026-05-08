// -----------------------------------------------------------------------
// handlers/http/messaging/groups.rs
// -----------------------------------------------------------------------

use anyhow::Context;
use bytes::Bytes;
use form_urlencoded;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use std::convert::Infallible;
use tracing::info;

use crate::AppState;
use crate::database::{groups, utils};
use crate::handlers::http;
use shared::types::groups::*;
use shared::types::jwt::JwtClaims;

// ---------------------------------------------------------------------------
// GET /api/groups
// ---------------------------------------------------------------------------

pub async fn handle_get_groups(
    _req: Request<Incoming>,
    state: AppState,
    claims: JwtClaims,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    let user_id = claims.user_id;
    info!("Fetching groups for user {}", user_id);

    let groups_list = groups::get_user_groups(&state.db, user_id)
        .await
        .context("Failed to fetch groups")?;

    let groups_json: Vec<serde_json::Value> = groups_list
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

    http::utils::deliver_success_json(
        Some(serde_json::json!({ "groups": groups_json })),
        None,
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// POST /api/groups
// ---------------------------------------------------------------------------

pub async fn handle_create_group(
    req: Request<Incoming>,
    state: AppState,
    user_id: i64,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
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
        .replace('\0', "");

    if name.trim().is_empty() {
        return http::utils::deliver_error_json(
            "INVALID_INPUT",
            "Group name cannot be empty",
            StatusCode::BAD_REQUEST,
        );
    }

    if name.len() > 100 {
        return http::utils::deliver_error_json(
            "INVALID_INPUT",
            "Group name cannot exceed 100 characters",
            StatusCode::BAD_REQUEST,
        );
    }

    let description: Option<String> = params.get("description").and_then(|v| v.as_str()).map(|s| {
        let s = s.replace('\0', "");
        if s.len() > 500 {
            s.chars().take(500).collect()
        } else {
            s
        }
    });

    let chat_id = groups::create_group(
        &state.db,
        NewGroup {
            name: name.clone(),
            created_by: user_id,
            description: description.clone(),
            chat_type: "group".to_string(),
        },
    )
    .await
    .context("Failed to create group")?;

    info!("Group {} created by user {}", chat_id, user_id);

    http::utils::deliver_success_json(
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

// ---------------------------------------------------------------------------
// GET /api/groups/:id/members
// ---------------------------------------------------------------------------

/// List members of a group with their usernames.
pub async fn handle_get_members(
    _req: Request<Incoming>,
    state: AppState,
    _claims: JwtClaims,
    chat_id: i64,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Fetching members for group {}", chat_id);

    let rows: Vec<(i64, i64, i64, i64, String, String)> = sqlx::query_as(
        "SELECT gm.id, gm.chat_id, gm.user_id, gm.joined_at, gm.role,
                u.username
         FROM   chat_members gm
         JOIN   users u ON u.id = gm.user_id
         WHERE  gm.chat_id = ?
         ORDER  BY gm.joined_at ASC",
    )
    .bind(chat_id)
    .fetch_all(&state.db)
    .await
    .context("Failed to fetch group members")?;

    let members_json: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            serde_json::json!({
                "id":        r.0,
                "chat_id":   r.1,
                "user_id":   r.2,
                "joined_at": r.3,
                "role":      r.4,
                "username":  r.5,
            })
        })
        .collect();

    http::utils::deliver_success_json(
        Some(serde_json::json!({
            "chat_id": chat_id,
            "members":  members_json,
        })),
        None,
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// POST /api/groups/:id/members
// ---------------------------------------------------------------------------

pub async fn handle_add_member(
    req: Request<Incoming>,
    state: AppState,
    _user_id: i64,
    chat_id: i64,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("Adding member to group {}", chat_id);

    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();
    let params: serde_json::Value =
        serde_json::from_slice(&body).context("Failed to parse JSON request body")?;

    let target_user_id: i64 = if let Some(uid) = params.get("user_id").and_then(|v| v.as_i64()) {
        uid
    } else if let Some(username) = params.get("username").and_then(|v| v.as_str()) {
        match utils::get_user_by_username(&state.db, username.to_string()).await? {
            Some(user) => user.id,
            None => {
                return http::utils::deliver_error_json(
                    "NOT_FOUND",
                    &format!("User '{}' not found", username),
                    StatusCode::NOT_FOUND,
                );
            }
        }
    } else {
        return http::utils::deliver_error_json(
            "INVALID_INPUT",
            "Request must include either 'username' or 'user_id'",
            StatusCode::BAD_REQUEST,
        );
    };

    if groups::is_group_member(&state.db, chat_id, target_user_id)
        .await
        .unwrap_or(false)
    {
        return http::utils::deliver_error_json(
            "CONFLICT",
            "User is already a member of this group",
            StatusCode::CONFLICT,
        );
    }

    let role = match params
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("member")
    {
        "admin" => "admin",
        _ => "member",
    }
    .to_string();

    groups::add_group_member(&state.db, chat_id, target_user_id, role)
        .await
        .context("Failed to add group member")?;

    http::utils::deliver_success_json(
        Some(serde_json::json!({ "chat_id": chat_id, "user_id": target_user_id })),
        Some("Member added successfully"),
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// PATCH /api/groups/:id
// ---------------------------------------------------------------------------

pub async fn handle_rename_group(
    req: Request<Incoming>,
    state: AppState,
    user_id: i64,
    chat_id: i64,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("User {} renaming group {}", user_id, chat_id);

    if !groups::is_group_member(&state.db, chat_id, user_id)
        .await
        .unwrap_or(false)
    {
        return http::utils::deliver_error_json(
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
        .replace('\0', "")
        .trim()
        .to_string();

    if new_name.is_empty() {
        return http::utils::deliver_error_json(
            "INVALID_INPUT",
            "Group name cannot be empty",
            StatusCode::BAD_REQUEST,
        );
    }
    if new_name.len() > 100 {
        return http::utils::deliver_error_json(
            "INVALID_INPUT",
            "Group name cannot exceed 100 characters",
            StatusCode::BAD_REQUEST,
        );
    }

    groups::update_group_name(&state.db, chat_id, new_name.clone())
        .await
        .context("Failed to rename group")?;

    info!(
        "Group {} renamed to '{}' by user {}",
        chat_id, new_name, user_id
    );

    http::utils::deliver_success_json(
        Some(serde_json::json!({ "chat_id": chat_id, "name": new_name })),
        Some("Group renamed successfully"),
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// DELETE /api/groups/:id
// ---------------------------------------------------------------------------

pub async fn handle_delete_group(
    _req: Request<Incoming>,
    state: AppState,
    user_id: i64,
    chat_id: i64,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    info!("User {} deleting group {}", user_id, chat_id);

    let group = match groups::get_group(&state.db, chat_id).await? {
        Some(g) => g,
        None => {
            return http::utils::deliver_error_json(
                "NOT_FOUND",
                "Group not found",
                StatusCode::NOT_FOUND,
            );
        }
    };

    if group.chat_type != "group" {
        return http::utils::deliver_error_json(
            "INVALID_INPUT",
            "Cannot delete a direct message conversation",
            StatusCode::BAD_REQUEST,
        );
    }

    if group.created_by != user_id {
        return http::utils::deliver_error_json(
            "FORBIDDEN",
            "Only the group creator can delete this group",
            StatusCode::FORBIDDEN,
        );
    }

    groups::delete_group(&state.db, chat_id)
        .await
        .context("Failed to delete group")?;

    info!("Group {} deleted by user {}", chat_id, user_id);

    http::utils::deliver_success_json(
        Some(serde_json::json!({ "chat_id": chat_id })),
        Some("Group deleted successfully"),
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// GET /api/users/search
// ---------------------------------------------------------------------------

pub async fn handle_search_users(
    req: Request<Incoming>,
    state: AppState,
    claims: JwtClaims,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
    let query_str = req.uri().query().unwrap_or("");
    let q: String = form_urlencoded::parse(query_str.as_bytes())
        .find(|(k, _)| k == "q")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_default()
        .trim()
        .to_string();

    if q.is_empty() {
        return http::utils::deliver_success_json(
            Some(serde_json::json!({ "users": [] })),
            None,
            StatusCode::OK,
        );
    }
    if q.len() > 50 {
        return http::utils::deliver_error_json(
            "INVALID_INPUT",
            "Search query too long",
            StatusCode::BAD_REQUEST,
        );
    }

    let users = utils::search_users_by_username(&state.db, &q, 10)
        .await
        .context("Failed to search users")?;

    let users_json: Vec<serde_json::Value> = users
        .into_iter()
        .filter(|u| u.id != claims.user_id)
        .map(|u| serde_json::json!({ "user_id": u.id, "username": u.username }))
        .collect();

    http::utils::deliver_success_json(
        Some(serde_json::json!({ "users": users_json })),
        None,
        StatusCode::OK,
    )
}

// ---------------------------------------------------------------------------
// DELETE /api/groups/:id/members
// ---------------------------------------------------------------------------

pub async fn handle_remove_member(
    req: Request<Incoming>,
    state: AppState,
    _user_id: i64,
    chat_id: i64,
) -> anyhow::Result<Response<BoxBody<Bytes, Infallible>>> {
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

    let removed = groups::remove_group_member(&state.db, chat_id, target_user_id)
        .await
        .context("Failed to remove group member")?;

    if !removed {
        return http::utils::deliver_error_json(
            "NOT_FOUND",
            "User is not a member of this group",
            StatusCode::NOT_FOUND,
        );
    }

    http::utils::deliver_success_json(
        Some(serde_json::json!({ "chat_id": chat_id, "user_id": target_user_id })),
        Some("Member removed successfully"),
        StatusCode::OK,
    )
}
