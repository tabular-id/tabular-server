use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ─── User ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub provider: String,
    pub provider_id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
}

impl From<User> for UserResponse {
    fn from(u: User) -> Self {
        UserResponse {
            id: u.id,
            email: u.email,
            display_name: u.display_name,
            avatar_url: u.avatar_url,
        }
    }
}

// ─── Session ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    pub device_info: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ─── Connection ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbConnection {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub db_type: String,
    pub encrypted_config: String,
    pub color_tag: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateConnectionRequest {
    pub name: String,
    pub db_type: String,
    pub encrypted_config: String,
    pub color_tag: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateConnectionRequest {
    pub name: Option<String>,
    pub encrypted_config: Option<String>,
    pub color_tag: Option<String>,
}

// ─── History ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct QueryHistory {
    pub id: String,
    pub user_id: String,
    pub connection_name: String,
    pub query_text: String,
    pub executed_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct PushHistoryRequest {
    pub items: Vec<HistoryItem>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HistoryItem {
    pub connection_name: String,
    pub query_text: String,
    pub executed_at: String, // ISO-8601
}

// ─── Saved Query ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SavedQuery {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub folder_path: String,
    pub query_text: String,
    pub connection_name: Option<String>,
    pub client_checksum: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateQueryRequest {
    pub name: String,
    pub folder_path: Option<String>,
    pub query_text: String,
    pub connection_name: Option<String>,
    pub client_checksum: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateQueryRequest {
    pub name: Option<String>,
    pub folder_path: Option<String>,
    pub query_text: Option<String>,
    pub connection_name: Option<String>,
    pub client_checksum: Option<String>,
}

// ─── Collab Room ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CollabRoom {
    pub id: String,
    pub owner_id: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRoomRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct InviteMemberRequest {
    pub email: String,
    pub role: Option<String>, // 'editor' | 'viewer', defaults to 'editor'
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RoomMember {
    pub user_id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub role: String,
}

// ─── OAuth State ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct OAuthState {
    pub state: String,
    pub provider: String,
    pub code_verifier: Option<String>,
    pub expires_at: DateTime<Utc>,
}

// ─── JWT Claims ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct JwtClaims {
    pub sub: String,        // user_id
    pub email: String,
    pub exp: usize,         // expiry timestamp
    pub iat: usize,         // issued at
}

// ─── API Responses ───────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct AuthTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,    // seconds
    pub user: UserResponse,
}

#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub success: bool,
    pub data: T,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        ApiResponse { success: true, data }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T: Serialize> {
    pub success: bool,
    pub data: Vec<T>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}
