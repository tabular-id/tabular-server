use axum::{
    Json,
    extract::{Path, Query, State, WebSocketUpgrade, ws::{Message, WebSocket}},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;

use crate::{
    AppState,
    auth::jwt,
    error::{AppError, Result},
    models::{ApiResponse, CollabRoom, CreateRoomRequest, InviteMemberRequest, JwtClaims, RoomMember},
};

// ─── REST — Room Management ──────────────────────────────────────────────────

/// GET /api/v1/collab/rooms
pub async fn list_rooms(
    claims: JwtClaims,
    State(app): State<AppState>,
) -> Result<Json<ApiResponse<Vec<CollabRoom>>>> {
    let rooms: Vec<CollabRoom> = sqlx::query_as(
        "SELECT r.id, r.owner_id, r.name, r.description, r.created_at, r.updated_at
         FROM collab_rooms r
         JOIN room_members m ON r.id = m.room_id
         WHERE m.user_id = ?
         ORDER BY r.updated_at DESC"
    )
    .bind(&claims.sub)
    .fetch_all(&app.db)
    .await?;

    Ok(Json(ApiResponse::ok(rooms)))
}

/// POST /api/v1/collab/rooms
pub async fn create_room(
    claims: JwtClaims,
    State(app): State<AppState>,
    Json(req): Json<CreateRoomRequest>,
) -> Result<Json<ApiResponse<CollabRoom>>> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("Room name is required".to_string()));
    }

    let room_id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO collab_rooms (id, owner_id, name, description) VALUES (?, ?, ?, ?)"
    )
    .bind(&room_id)
    .bind(&claims.sub)
    .bind(&req.name)
    .bind(&req.description)
    .execute(&app.db)
    .await?;

    // Add owner as member
    sqlx::query(
        "INSERT INTO room_members (room_id, user_id, role) VALUES (?, ?, 'owner')"
    )
    .bind(&room_id)
    .bind(&claims.sub)
    .execute(&app.db)
    .await?;

    let room = get_room_by_id(&app.db, &room_id).await?;
    Ok(Json(ApiResponse::ok(room)))
}

/// DELETE /api/v1/collab/rooms/{room_id}
pub async fn delete_room(
    claims: JwtClaims,
    State(app): State<AppState>,
    Path(room_id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let res = sqlx::query(
        "DELETE FROM collab_rooms WHERE id = ? AND owner_id = ?"
    )
    .bind(&room_id)
    .bind(&claims.sub)
    .execute(&app.db)
    .await?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Room {} not found or not owned by you", room_id)));
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

/// GET /api/v1/collab/rooms/{room_id}/members
pub async fn list_members(
    claims: JwtClaims,
    State(app): State<AppState>,
    Path(room_id): Path<String>,
) -> Result<Json<ApiResponse<Vec<RoomMember>>>> {
    // Verify membership
    verify_room_member(&app.db, &room_id, &claims.sub).await?;

    let members: Vec<RoomMember> = sqlx::query_as(
        "SELECT m.user_id, u.email, u.display_name, u.avatar_url, m.role
         FROM room_members m
         JOIN users u ON m.user_id = u.id
         WHERE m.room_id = ?"
    )
    .bind(&room_id)
    .fetch_all(&app.db)
    .await?;

    Ok(Json(ApiResponse::ok(members)))
}

/// POST /api/v1/collab/rooms/{room_id}/invite
pub async fn invite_member(
    claims: JwtClaims,
    State(app): State<AppState>,
    Path(room_id): Path<String>,
    Json(req): Json<InviteMemberRequest>,
) -> Result<Json<serde_json::Value>> {
    // Only owner can invite
    verify_room_owner(&app.db, &room_id, &claims.sub).await?;

    // Find user by email
    let invitee_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM users WHERE email = ?"
    )
    .bind(&req.email)
    .fetch_optional(&app.db)
    .await?;

    let invitee_id = invitee_id
        .ok_or_else(|| AppError::NotFound(format!("User {} not found", req.email)))?;

    let role = req.role.unwrap_or_else(|| "editor".to_string());

    sqlx::query(
        "INSERT INTO room_members (room_id, user_id, role)
         VALUES (?, ?, ?)
         ON DUPLICATE KEY UPDATE role = VALUES(role)"
    )
    .bind(&room_id)
    .bind(&invitee_id)
    .bind(&role)
    .execute(&app.db)
    .await?;

    Ok(Json(serde_json::json!({ "success": true })))
}

// ─── WebSocket — CRDT Collab ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: String, // JWT passed as query param for WS upgrade
}

/// GET /ws/collab/{room_id}?token=<jwt>
pub async fn ws_collab(
    State(app): State<AppState>,
    Path(room_id): Path<String>,
    Query(q): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Validate token before upgrading
    let claims = match jwt::verify_access_token(&q.token, &app.config.jwt_secret) {
        Ok(c) => c,
        Err(_) => {
            return axum::response::Response::builder()
                .status(401)
                .body(axum::body::Body::from("Unauthorized"))
                .unwrap();
        }
    };

    ws.on_upgrade(move |socket| handle_ws_connection(socket, app, room_id, claims))
}

async fn handle_ws_connection(
    socket: WebSocket,
    app: AppState,
    room_id: String,
    claims: JwtClaims,
) {
    // Verify room membership
    if verify_room_member(&app.db, &room_id, &claims.sub).await.is_err() {
        tracing::warn!("WS: user {} not a member of room {}", claims.sub, room_id);
        return;
    }

    let room = app.room_registry.get_or_create(&room_id);
    {
        let mut count = room.client_count.lock().unwrap();
        *count += 1;
    }
    tracing::info!("WS: user {} joined room {}", claims.sub, room_id);

    let (mut ws_tx, mut ws_rx) = socket.split();
    let mut broadcast_rx = room.tx.subscribe();

    // Send full doc state to new joiner (Yjs Sync Step 1 → Step 2)
    let state_update = room.encode_state();
    if !state_update.is_empty() {
        let mut msg = vec![0u8]; // message type 0 = sync update
        msg.extend_from_slice(&state_update);
        let _ = ws_tx.send(Message::Binary(msg.into())).await;
    }

    // Forward loop: broadcast → this WS client
    let mut forward_task = tokio::spawn(async move {
        loop {
            match broadcast_rx.recv().await {
                Ok(data) => {
                    if ws_tx.send(Message::Binary(data.into())).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // Receive loop: this WS client → apply update → broadcast
    let room_clone = room.clone();
    let mut receive_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            match msg {
                Message::Binary(data) => {
                    if data.is_empty() {
                        continue;
                    }
                    // Apply CRDT update to shared document
                    if let Err(e) = room_clone.apply_update(&data[1..]) {
                        tracing::warn!("WS: CRDT apply error: {}", e);
                        continue;
                    }
                    // Broadcast to all other clients
                    let _ = room_clone.tx.send(data.to_vec());
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = &mut forward_task => receive_task.abort(),
        _ = &mut receive_task => forward_task.abort(),
    }

    // Cleanup
    {
        let mut count = room.client_count.lock().unwrap();
        if *count > 0 {
            *count -= 1;
        }
    }
    app.room_registry.remove_if_empty(&room_id);
    tracing::info!("WS: user {} left room {}", claims.sub, room_id);
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

async fn get_room_by_id(pool: &sqlx::MySqlPool, id: &str) -> Result<CollabRoom> {
    sqlx::query_as(
        "SELECT id, owner_id, name, description, created_at, updated_at
         FROM collab_rooms WHERE id = ?"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Room {} not found", id)))
}

async fn verify_room_member(pool: &sqlx::MySqlPool, room_id: &str, user_id: &str) -> Result<()> {
    let exists: Option<String> = sqlx::query_scalar(
        "SELECT user_id FROM room_members WHERE room_id = ? AND user_id = ?"
    )
    .bind(room_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    exists
        .map(|_| ())
        .ok_or(AppError::Forbidden)
}

async fn verify_room_owner(pool: &sqlx::MySqlPool, room_id: &str, user_id: &str) -> Result<()> {
    let exists: Option<String> = sqlx::query_scalar(
        "SELECT user_id FROM room_members WHERE room_id = ? AND user_id = ? AND role = 'owner'"
    )
    .bind(room_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    exists
        .map(|_| ())
        .ok_or(AppError::Forbidden)
}
