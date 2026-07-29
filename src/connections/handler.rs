use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{
        ApiResponse, CreateConnectionRequest, DbConnection, JwtClaims, UpdateConnectionRequest,
    },
};

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// GET /api/v1/connections
pub async fn list_connections(
    claims: JwtClaims,
    State(app): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<ApiResponse<Vec<DbConnection>>>> {
    let limit = q.limit.unwrap_or(200);
    let offset = q.offset.unwrap_or(0);

    let connections: Vec<DbConnection> = sqlx::query_as(
        "SELECT id, user_id, name, db_type, encrypted_config, color_tag, created_at, updated_at
         FROM connections WHERE user_id = ?
         ORDER BY updated_at DESC
         LIMIT ? OFFSET ?"
    )
    .bind(&claims.sub)
    .bind(limit)
    .bind(offset)
    .fetch_all(&app.db)
    .await?;

    Ok(Json(ApiResponse::ok(connections)))
}

/// POST /api/v1/connections
pub async fn create_connection(
    claims: JwtClaims,
    State(app): State<AppState>,
    Json(req): Json<CreateConnectionRequest>,
) -> Result<Json<ApiResponse<DbConnection>>> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("Connection name is required".to_string()));
    }

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO connections (id, user_id, name, db_type, encrypted_config, color_tag)
         VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&claims.sub)
    .bind(&req.name)
    .bind(&req.db_type)
    .bind(&req.encrypted_config)
    .bind(&req.color_tag)
    .execute(&app.db)
    .await?;

    let conn = get_connection_by_id(&app.db, &id, &claims.sub).await?;
    Ok(Json(ApiResponse::ok(conn)))
}

/// PUT /api/v1/connections/{id}
pub async fn update_connection(
    claims: JwtClaims,
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateConnectionRequest>,
) -> Result<Json<ApiResponse<DbConnection>>> {
    // Verify ownership
    get_connection_by_id(&app.db, &id, &claims.sub).await?;

    if let Some(name) = &req.name {
        sqlx::query("UPDATE connections SET name = ?, updated_at = NOW() WHERE id = ? AND user_id = ?")
            .bind(name)
            .bind(&id)
            .bind(&claims.sub)
            .execute(&app.db)
            .await?;
    }
    if let Some(config) = &req.encrypted_config {
        sqlx::query("UPDATE connections SET encrypted_config = ?, updated_at = NOW() WHERE id = ? AND user_id = ?")
            .bind(config)
            .bind(&id)
            .bind(&claims.sub)
            .execute(&app.db)
            .await?;
    }
    if let Some(color) = &req.color_tag {
        sqlx::query("UPDATE connections SET color_tag = ?, updated_at = NOW() WHERE id = ? AND user_id = ?")
            .bind(color)
            .bind(&id)
            .bind(&claims.sub)
            .execute(&app.db)
            .await?;
    }

    let conn = get_connection_by_id(&app.db, &id, &claims.sub).await?;
    Ok(Json(ApiResponse::ok(conn)))
}

/// DELETE /api/v1/connections/{id}
pub async fn delete_connection(
    claims: JwtClaims,
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let result = sqlx::query(
        "DELETE FROM connections WHERE id = ? AND user_id = ?"
    )
    .bind(&id)
    .bind(&claims.sub)
    .execute(&app.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Connection {} not found", id)));
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

async fn get_connection_by_id(
    pool: &sqlx::MySqlPool,
    id: &str,
    user_id: &str,
) -> Result<DbConnection> {
    sqlx::query_as(
        "SELECT id, user_id, name, db_type, encrypted_config, color_tag, created_at, updated_at
         FROM connections WHERE id = ? AND user_id = ?"
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Connection {} not found", id)))
}
