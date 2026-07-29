mod auth;
mod collab;
mod config;
mod connections;
mod db;
mod error;
mod history;
mod middleware;
mod models;
mod queries;

use axum::{
    Router,
    routing::{delete, get, post, put},
};
use std::{net::SocketAddr, sync::Arc};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::collab::room::RoomRegistry;
use crate::config::Config;

/// Shared application state passed to all handlers.
#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::MySqlPool,
    pub config: Arc<Config>,
    pub room_registry: RoomRegistry,
    pub http: reqwest::Client,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env
    dotenv::dotenv().ok();

    // Tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tabular_server=debug,tower_http=info".into()),
        )
        .init();

    // Config
    let config = Arc::new(Config::from_env()?);
    tracing::info!("🚀 Starting tabular-server on port {}", config.server_port);

    // Database
    let pool = db::create_pool(&config.database_url).await?;
    db::run_migrations(&pool).await?;

    // HTTP client (for OAuth requests)
    let http_client = reqwest::Client::builder()
        .user_agent("tabular-server/0.1")
        .build()?;

    // App state
    let state = AppState {
        db: pool,
        config: config.clone(),
        room_registry: RoomRegistry::new(),
        http: http_client,
    };

    // CORS
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Router
    let app = Router::new()
        // ── Health ──────────────────────────────────────────────────────────
        .route("/health", get(|| async { "ok" }))
        // ── Auth ────────────────────────────────────────────────────────────
        .route("/api/v1/auth/login/google",   get(auth::handler::login_google))
        .route("/api/v1/auth/login/github",   get(auth::handler::login_github))
        .route("/api/v1/auth/callback/google", get(auth::handler::callback_google))
        .route("/api/v1/auth/callback/github", get(auth::handler::callback_github))
        .route("/api/v1/auth/refresh",        post(auth::handler::refresh_token))
        .route("/api/v1/auth/logout",         post(auth::handler::logout))
        // ── Connections ─────────────────────────────────────────────────────
        .route("/api/v1/connections",
            get(connections::handler::list_connections)
            .post(connections::handler::create_connection)
        )
        .route("/api/v1/connections/{id}",
            put(connections::handler::update_connection)
            .delete(connections::handler::delete_connection)
        )
        // ── History ─────────────────────────────────────────────────────────
        .route("/api/v1/history",
            get(history::handler::list_history)
            .post(history::handler::push_history)
            .delete(history::handler::clear_history)
        )
        .route("/api/v1/history/{id}",
            delete(history::handler::delete_history_item)
        )
        // ── Saved Queries ────────────────────────────────────────────────────
        .route("/api/v1/queries",
            get(queries::handler::list_queries)
            .post(queries::handler::create_query)
        )
        .route("/api/v1/queries/{id}",
            put(queries::handler::update_query)
            .delete(queries::handler::delete_query)
        )
        // ── Collab Rooms ─────────────────────────────────────────────────────
        .route("/api/v1/collab/rooms",
            get(collab::handler::list_rooms)
            .post(collab::handler::create_room)
        )
        .route("/api/v1/collab/rooms/{room_id}",
            delete(collab::handler::delete_room)
        )
        .route("/api/v1/collab/rooms/{room_id}/members",
            get(collab::handler::list_members)
        )
        .route("/api/v1/collab/rooms/{room_id}/invite",
            post(collab::handler::invite_member)
        )
        // ── WebSocket ─────────────────────────────────────────────────────────
        .route("/ws/collab/{room_id}", get(collab::handler::ws_collab))
        // ── Middleware ────────────────────────────────────────────────────────
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.server_port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("✅ Listening on http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install Ctrl+C handler");
    tracing::info!("Shutdown signal received");
}
