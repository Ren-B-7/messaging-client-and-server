use anyhow::{Context, Result};
use bytes::Bytes;
use http_body_util::{BodyExt, combinators::BoxBody};
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use std::collections::HashMap;
use std::convert::Infallible;
use tracing::info;

use crate::AppState;
use crate::handlers::http::utils::{deliver_error_json, deliver_success_json};

// ---------------------------------------------------------------------------
// Auth helper (token → user_id)
// ---------------------------------------------------------------------------

async fn extract_user(
    req: &Request<Incoming>,
    state: &AppState,
) -> std::result::Result<i64, ()> {
    use crate::database::login as db_login;

    let token = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .or_else(|| {
            req.headers()
                .get("cookie")
                .and_then(|h| h.to_str().ok())
                .and_then(|cookies| {
                    cookies
                        .split(';')
                        .find(|c| c.trim().starts_with("auth_id="))
                        .and_then(|c| c.split('=').nth(1))
                })
        })
        .map(|s| s.to_string())
        .ok_or(())?;

    db_login::validate_session(&state.db, token)
        .await
        .ok()
        .flatten()
        .ok_or(())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// GET /api/groups — list all groups the authenticated user belongs to
pub async fn handle_get_groups(
    req: Request<Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::groups as db_groups;

    let user_id = match extract_user(&req, &state).await {
        Ok(id) => id,
        Err(_) => {
            return deliver_error_json(
                "UNAUTHORIZED",
                "Authentication required",
                StatusCode::UNAUTHORIZED,
            );
        }
    };

    info!("Fetching groups for user {}", user_id);

    let groups = db_groups::get_user_groups(&state.db, user_id)
        .await
        .context("Failed to fetch groups")?;

    let groups_json: Vec<serde_json::Value> = groups
        .into_iter()
        .map(|g| {
            serde_json::json!({
                "id":          g.id,
                "name":        g.name,
                "description": g.description,
                "created_by":  g.created_by,
                "created_at":  g.created_at,
            })
        })
        .collect();

    deliver_success_json(
        Some(serde_json::json!({ "groups": groups_json })),
        None,
        StatusCode::OK,
    )
}

/// POST /api/groups — create a new group
pub async fn handle_create_group(
    req: Request<Incoming>,
    state: AppState,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::groups as db_groups;

    // Auth must happen before the body is consumed
    let creator_id = match extract_user(&req, &state).await {
        Ok(id) => id,
        Err(_) => {
            return deliver_error_json(
                "UNAUTHORIZED",
                "Authentication required",
                StatusCode::UNAUTHORIZED,
            );
        }
    };

    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let name = params
        .get("name")
        .ok_or_else(|| anyhow::anyhow!("Missing group name"))?
        .clone();

    if name.trim().is_empty() {
        return deliver_error_json(
            "INVALID_INPUT",
            "Group name cannot be empty",
            StatusCode::BAD_REQUEST,
        );
    }

    let description: Option<String> = params.get("description").map(|s| s.to_string());

    let group_id = db_groups::create_group(
        &state.db,
        db_groups::NewGroup {
            name: name.clone(),
            created_by: creator_id,
            description: description.clone(),
        },
    )
    .await
    .context("Failed to create group")?;

    info!("Group {} created by user {}", group_id, creator_id);

    deliver_success_json(
        Some(serde_json::json!({
            "group_id":    group_id,
            "name":        name,
            "description": description,
        })),
        Some("Group created successfully"),
        StatusCode::CREATED,
    )
}

/// GET /api/groups/:id/members — list members of a group
pub async fn handle_get_members(
    _req: Request<Incoming>,
    state: AppState,
    group_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::groups as db_groups;

    info!("Fetching members for group {}", group_id);

    let members = db_groups::get_group_members(&state.db, group_id)
        .await
        .context("Failed to fetch group members")?;

    let members_json: Vec<serde_json::Value> = members
        .into_iter()
        .map(|m| {
            serde_json::json!({
                "user_id":   m.user_id,
                "group_id":  m.group_id,
                "role":      m.role,
                "joined_at": m.joined_at,
            })
        })
        .collect();

    deliver_success_json(
        Some(serde_json::json!({
            "group_id": group_id,
            "members":  members_json,
        })),
        None,
        StatusCode::OK,
    )
}

/// POST /api/groups/:id/members — add a member to a group
pub async fn handle_add_member(
    req: Request<Incoming>,
    state: AppState,
    group_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::groups as db_groups;

    info!("Adding member to group {}", group_id);

    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let user_id: i64 = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid or missing user_id"))?;

    let role = params
        .get("role")
        .cloned()
        .unwrap_or_else(|| "member".to_string());

    db_groups::add_group_member(&state.db, group_id, user_id, role)
        .await
        .context("Failed to add group member")?;

    deliver_success_json(
        Some(serde_json::json!({
            "group_id": group_id,
            "user_id":  user_id,
        })),
        Some("Member added successfully"),
        StatusCode::OK,
    )
}

/// DELETE /api/groups/:id/members — remove a member from a group
pub async fn handle_remove_member(
    req: Request<Incoming>,
    state: AppState,
    group_id: i64,
) -> Result<Response<BoxBody<Bytes, Infallible>>> {
    use crate::database::groups as db_groups;

    info!("Removing member from group {}", group_id);

    let body = req
        .collect()
        .await
        .context("Failed to read request body")?
        .to_bytes();

    let params: HashMap<String, String> =
        form_urlencoded::parse(body.as_ref()).into_owned().collect();

    let user_id: i64 = params
        .get("user_id")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| anyhow::anyhow!("Invalid or missing user_id"))?;

    let removed = db_groups::remove_group_member(&state.db, group_id, user_id)
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
            "group_id": group_id,
            "user_id":  user_id,
        })),
        Some("Member removed successfully"),
        StatusCode::OK,
    )
}
