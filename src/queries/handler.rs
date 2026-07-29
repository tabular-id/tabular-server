use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{ApiResponse, CreateQueryRequest, JwtClaims, SavedQuery, UpdateQueryRequest},
};

#[derive(Debug, Deserialize)]
pub struct ListQueriesQuery {
    pub folder: Option<String>,
}

/// GET /api/v1/queries
pub async fn list_queries(
    claims: JwtClaims,
    State(app): State<AppState>,
    Query(q): Query<ListQueriesQuery>,
) -> Result<Json<ApiResponse<Vec<SavedQuery>>>> {
    let queries: Vec<SavedQuery> = if let Some(folder) = &q.folder {
        sqlx::query_as(
            "SELECT id, user_id, name, folder_path, query_text, connection_name,
                    client_checksum, created_at, updated_at
             FROM saved_queries
             WHERE user_id = ? AND folder_path = ?
             ORDER BY updated_at DESC"
        )
        .bind(&claims.sub)
        .bind(folder)
        .fetch_all(&app.db)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, user_id, name, folder_path, query_text, connection_name,
                    client_checksum, created_at, updated_at
             FROM saved_queries
             WHERE user_id = ?
             ORDER BY folder_path, name"
        )
        .bind(&claims.sub)
        .fetch_all(&app.db)
        .await?
    };

    Ok(Json(ApiResponse::ok(queries)))
}

/// POST /api/v1/queries
pub async fn create_query(
    claims: JwtClaims,
    State(app): State<AppState>,
    Json(req): Json<CreateQueryRequest>,
) -> Result<Json<ApiResponse<SavedQuery>>> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("Query name is required".to_string()));
    }

    let id = uuid::Uuid::new_v4().to_string();
    let folder = req.folder_path.unwrap_or_else(|| "/".to_string());

    sqlx::query(
        "INSERT INTO saved_queries
         (id, user_id, name, folder_path, query_text, connection_name, client_checksum)
         VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(&claims.sub)
    .bind(&req.name)
    .bind(&folder)
    .bind(&req.query_text)
    .bind(&req.connection_name)
    .bind(&req.client_checksum)
    .execute(&app.db)
    .await?;

    let query = get_query_by_id(&app.db, &id, &claims.sub).await?;
    Ok(Json(ApiResponse::ok(query)))
}

/// PUT /api/v1/queries/{id}
pub async fn update_query(
    claims: JwtClaims,
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateQueryRequest>,
) -> Result<Json<ApiResponse<SavedQuery>>> {
    get_query_by_id(&app.db, &id, &claims.sub).await?; // verify ownership

    let mut updates = vec!["updated_at = NOW()"];
    if req.name.is_some()         { updates.push("name = ?") }
    if req.folder_path.is_some()  { updates.push("folder_path = ?") }
    if req.query_text.is_some()   { updates.push("query_text = ?") }
    if req.connection_name.is_some() { updates.push("connection_name = ?") }
    if req.client_checksum.is_some() { updates.push("client_checksum = ?") }

    let sql = format!(
        "UPDATE saved_queries SET {} WHERE id = ? AND user_id = ?",
        updates.join(", ")
    );

    let mut q = sqlx::query(&sql);
    if let Some(v) = &req.name          { q = q.bind(v); }
    if let Some(v) = &req.folder_path   { q = q.bind(v); }
    if let Some(v) = &req.query_text    { q = q.bind(v); }
    if let Some(v) = &req.connection_name { q = q.bind(v); }
    if let Some(v) = &req.client_checksum { q = q.bind(v); }
    q.bind(&id).bind(&claims.sub).execute(&app.db).await?;

    let query = get_query_by_id(&app.db, &id, &claims.sub).await?;
    Ok(Json(ApiResponse::ok(query)))
}

/// DELETE /api/v1/queries/{id}
pub async fn delete_query(
    claims: JwtClaims,
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let res = sqlx::query(
        "DELETE FROM saved_queries WHERE id = ? AND user_id = ?"
    )
    .bind(&id)
    .bind(&claims.sub)
    .execute(&app.db)
    .await?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Query {} not found", id)));
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

async fn get_query_by_id(pool: &sqlx::MySqlPool, id: &str, user_id: &str) -> Result<SavedQuery> {
    sqlx::query_as(
        "SELECT id, user_id, name, folder_path, query_text, connection_name,
                client_checksum, created_at, updated_at
         FROM saved_queries WHERE id = ? AND user_id = ?"
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Query {} not found", id)))
}
