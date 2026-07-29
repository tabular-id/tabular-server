use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;

use crate::{
    AppState,
    error::{AppError, Result},
    models::{JwtClaims, PushHistoryRequest, QueryHistory},
};

#[derive(Debug, Deserialize)]
pub struct ListHistoryQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub since: Option<String>, // ISO-8601 — for incremental sync
}

/// GET /api/v1/history
pub async fn list_history(
    claims: JwtClaims,
    State(app): State<AppState>,
    Query(q): Query<ListHistoryQuery>,
) -> Result<Json<serde_json::Value>> {
    let limit = q.limit.unwrap_or(500);
    let offset = q.offset.unwrap_or(0);

    let rows: Vec<QueryHistory> = if let Some(since) = &q.since {
        sqlx::query_as(
            "SELECT id, user_id, connection_name, query_text, executed_at
             FROM query_history
             WHERE user_id = ? AND executed_at > ?
             ORDER BY executed_at DESC
             LIMIT ? OFFSET ?"
        )
        .bind(&claims.sub)
        .bind(since)
        .bind(limit)
        .bind(offset)
        .fetch_all(&app.db)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id, user_id, connection_name, query_text, executed_at
             FROM query_history
             WHERE user_id = ?
             ORDER BY executed_at DESC
             LIMIT ? OFFSET ?"
        )
        .bind(&claims.sub)
        .bind(limit)
        .bind(offset)
        .fetch_all(&app.db)
        .await?
    };

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM query_history WHERE user_id = ?"
    )
    .bind(&claims.sub)
    .fetch_one(&app.db)
    .await?;

    Ok(Json(serde_json::json!({
        "success": true,
        "data": rows,
        "total": total,
        "limit": limit,
        "offset": offset,
    })))
}

/// POST /api/v1/history — batch push from client
pub async fn push_history(
    claims: JwtClaims,
    State(app): State<AppState>,
    Json(req): Json<PushHistoryRequest>,
) -> Result<Json<serde_json::Value>> {
    if req.items.is_empty() {
        return Ok(Json(serde_json::json!({ "success": true, "inserted": 0 })));
    }

    let mut inserted = 0u64;
    for item in &req.items {
        // Dedup by (user_id, query_text, connection_name, executed_at)
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM query_history
             WHERE user_id = ? AND query_text = ? AND connection_name = ? AND executed_at = ?
             LIMIT 1"
        )
        .bind(&claims.sub)
        .bind(&item.query_text)
        .bind(&item.connection_name)
        .bind(&item.executed_at)
        .fetch_optional(&app.db)
        .await?;

        if existing.is_none() {
            let id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO query_history (id, user_id, connection_name, query_text, executed_at)
                 VALUES (?, ?, ?, ?, ?)"
            )
            .bind(&id)
            .bind(&claims.sub)
            .bind(&item.connection_name)
            .bind(&item.query_text)
            .bind(&item.executed_at)
            .execute(&app.db)
            .await?;
            inserted += 1;
        }
    }

    // Keep only latest 2000 entries per user
    sqlx::query(
        "DELETE FROM query_history WHERE user_id = ? AND id NOT IN (
             SELECT id FROM (
                 SELECT id FROM query_history WHERE user_id = ?
                 ORDER BY executed_at DESC LIMIT 2000
             ) AS t
         )"
    )
    .bind(&claims.sub)
    .bind(&claims.sub)
    .execute(&app.db)
    .await?;

    Ok(Json(serde_json::json!({ "success": true, "inserted": inserted })))
}

/// DELETE /api/v1/history/{id}
pub async fn delete_history_item(
    claims: JwtClaims,
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let res = sqlx::query(
        "DELETE FROM query_history WHERE id = ? AND user_id = ?"
    )
    .bind(&id)
    .bind(&claims.sub)
    .execute(&app.db)
    .await?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("History item {} not found", id)));
    }

    Ok(Json(serde_json::json!({ "success": true })))
}

/// DELETE /api/v1/history — clear all for user
pub async fn clear_history(
    claims: JwtClaims,
    State(app): State<AppState>,
) -> Result<Json<serde_json::Value>> {
    sqlx::query("DELETE FROM query_history WHERE user_id = ?")
        .bind(&claims.sub)
        .execute(&app.db)
        .await?;

    Ok(Json(serde_json::json!({ "success": true })))
}
