use axum::{
    extract::{Query, State},
    response::Redirect,
    Json,
};
use chrono::Utc;
use serde::Deserialize;
use sqlx::MySqlPool;

use crate::{
    AppState,
    auth::jwt,
    error::{AppError, Result},
    models::{ApiResponse, AuthTokenResponse, OAuthState, User, UserResponse},
};

// ─── OAuth provider info ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthProvider {
    Google,
    GitHub,
}

impl OAuthProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            OAuthProvider::Google => "google",
            OAuthProvider::GitHub => "github",
        }
    }
}

// ─── Request/Response types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct OAuthCallbackQuery {
    pub code: String,
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshTokenRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

// ─── OAuth login redirect ─────────────────────────────────────────────────────

/// GET /api/v1/auth/login/google
/// GET /api/v1/auth/login/github
pub async fn handle_login_redirect(
    State(state): State<AppState>,
    provider: OAuthProvider,
) -> Result<Redirect> {
    // Generate CSRF state nonce
    let state_token: String = (0..32)
        .map(|_| rand::random::<u8>())
        .map(|b| format!("{:02x}", b))
        .collect();

    // Store state in DB with 10-minute expiry
    let expires_at = Utc::now() + chrono::Duration::minutes(10);
    sqlx::query(
        "INSERT INTO oauth_states (state, provider, expires_at) VALUES (?, ?, ?)"
    )
    .bind(&state_token)
    .bind(provider.as_str())
    .bind(&expires_at)
    .execute(&state.db)
    .await?;

    // Build provider authorization URL
    let auth_url = match provider {
        OAuthProvider::Google => {
            format!(
                "https://accounts.google.com/o/oauth2/v2/auth\
                 ?client_id={}\
                 &redirect_uri={}\
                 &response_type=code\
                 &scope=openid%20email%20profile\
                 &state={}\
                 &access_type=offline\
                 &prompt=consent",
                state.config.google_client_id,
                urlencoding::encode(&state.config.google_redirect_uri),
                state_token,
            )
        }
        OAuthProvider::GitHub => {
            format!(
                "https://github.com/login/oauth/authorize\
                 ?client_id={}\
                 &redirect_uri={}\
                 &scope=read:user%20user:email\
                 &state={}",
                state.config.github_client_id,
                urlencoding::encode(&state.config.github_redirect_uri),
                state_token,
            )
        }
    };

    Ok(Redirect::temporary(&auth_url))
}

/// GET /api/v1/auth/login/google  (route adapter)
pub async fn login_google(State(app): State<AppState>) -> Result<Redirect> {
    handle_login_redirect(State(app), OAuthProvider::Google).await
}

/// GET /api/v1/auth/login/github  (route adapter)
pub async fn login_github(State(app): State<AppState>) -> Result<Redirect> {
    handle_login_redirect(State(app), OAuthProvider::GitHub).await
}

// ─── OAuth callback ──────────────────────────────────────────────────────────

/// GET /api/v1/auth/callback/google
pub async fn callback_google(
    State(app): State<AppState>,
    Query(q): Query<OAuthCallbackQuery>,
) -> Result<Json<ApiResponse<AuthTokenResponse>>> {
    handle_callback(app, q, OAuthProvider::Google).await
}

/// GET /api/v1/auth/callback/github
pub async fn callback_github(
    State(app): State<AppState>,
    Query(q): Query<OAuthCallbackQuery>,
) -> Result<Json<ApiResponse<AuthTokenResponse>>> {
    handle_callback(app, q, OAuthProvider::GitHub).await
}

async fn handle_callback(
    app: AppState,
    query: OAuthCallbackQuery,
    provider: OAuthProvider,
) -> Result<Json<ApiResponse<AuthTokenResponse>>> {
    // Validate state nonce
    let stored_state: Option<OAuthState> = sqlx::query_as(
        "SELECT state, provider, code_verifier, expires_at FROM oauth_states
         WHERE state = ? AND provider = ? AND expires_at > NOW()"
    )
    .bind(&query.state)
    .bind(provider.as_str())
    .fetch_optional(&app.db)
    .await?;

    let _state_record = stored_state
        .ok_or_else(|| AppError::BadRequest("Invalid or expired OAuth state".to_string()))?;

    // Consume the state nonce (prevent replay)
    sqlx::query("DELETE FROM oauth_states WHERE state = ?")
        .bind(&query.state)
        .execute(&app.db)
        .await?;

    // Exchange code for access token
    let (user_email, user_name, provider_id, avatar_url) = match provider {
        OAuthProvider::Google => exchange_google_code(&app, &query.code).await?,
        OAuthProvider::GitHub => exchange_github_code(&app, &query.code).await?,
    };

    // Upsert user
    let user = upsert_user(&app.db, provider.as_str(), &provider_id, &user_email, &user_name, &avatar_url).await?;

    // Generate tokens
    let access_token = jwt::generate_access_token(
        &user.id,
        &user.email,
        &app.config.jwt_secret,
        app.config.jwt_access_expiry_minutes,
    )?;
    let refresh_token = jwt::generate_refresh_token();

    // Store session
    let session_id = uuid::Uuid::new_v4().to_string();
    let expires_at = Utc::now() + chrono::Duration::days(app.config.jwt_refresh_expiry_days);
    sqlx::query(
        "INSERT INTO sessions (id, user_id, refresh_token, expires_at) VALUES (?, ?, ?, ?)"
    )
    .bind(&session_id)
    .bind(&user.id)
    .bind(&refresh_token)
    .bind(&expires_at)
    .execute(&app.db)
    .await?;

    Ok(Json(ApiResponse::ok(AuthTokenResponse {
        access_token,
        refresh_token,
        expires_in: app.config.jwt_access_expiry_minutes * 60,
        user: UserResponse::from(user),
    })))
}

// ─── Token refresh ─────────────────────────────────────────────────────────

/// POST /api/v1/auth/refresh
pub async fn refresh_token(
    State(app): State<AppState>,
    Json(req): Json<RefreshTokenRequest>,
) -> Result<Json<ApiResponse<AuthTokenResponse>>> {
    // Find session
    let row = sqlx::query_as::<_, (String, String, String, chrono::DateTime<Utc>)>(
        "SELECT s.id, s.user_id, u.email, s.expires_at
         FROM sessions s JOIN users u ON s.user_id = u.id
         WHERE s.refresh_token = ? AND s.expires_at > NOW()"
    )
    .bind(&req.refresh_token)
    .fetch_optional(&app.db)
    .await?;

    let (session_id, user_id, email, _expires_at) = row
        .ok_or_else(|| AppError::Unauthorized)?;

    // Rotate refresh token
    let new_refresh_token = jwt::generate_refresh_token();
    let new_expires_at = Utc::now() + chrono::Duration::days(app.config.jwt_refresh_expiry_days);

    sqlx::query(
        "UPDATE sessions SET refresh_token = ?, expires_at = ? WHERE id = ?"
    )
    .bind(&new_refresh_token)
    .bind(&new_expires_at)
    .bind(&session_id)
    .execute(&app.db)
    .await?;

    let access_token = jwt::generate_access_token(
        &user_id,
        &email,
        &app.config.jwt_secret,
        app.config.jwt_access_expiry_minutes,
    )?;

    // Fetch full user
    let user: User = sqlx::query_as(
        "SELECT id, provider, provider_id, email, display_name, avatar_url, created_at, updated_at
         FROM users WHERE id = ?"
    )
    .bind(&user_id)
    .fetch_one(&app.db)
    .await?;

    Ok(Json(ApiResponse::ok(AuthTokenResponse {
        access_token,
        refresh_token: new_refresh_token,
        expires_in: app.config.jwt_access_expiry_minutes * 60,
        user: UserResponse::from(user),
    })))
}

/// POST /api/v1/auth/logout
pub async fn logout(
    State(app): State<AppState>,
    Json(req): Json<LogoutRequest>,
) -> Result<Json<serde_json::Value>> {
    sqlx::query("DELETE FROM sessions WHERE refresh_token = ?")
        .bind(&req.refresh_token)
        .execute(&app.db)
        .await?;

    Ok(Json(serde_json::json!({ "success": true, "message": "Logged out" })))
}

// ─── Google token exchange ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    id_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    sub: String,
    email: String,
    name: Option<String>,
    picture: Option<String>,
}

async fn exchange_google_code(
    app: &AppState,
    code: &str,
) -> Result<(String, Option<String>, String, Option<String>)> {
    let client = reqwest::Client::new();

    // Exchange code for tokens
    let token_res: GoogleTokenResponse = client
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", app.config.google_client_id.as_str()),
            ("client_secret", app.config.google_client_secret.as_str()),
            ("redirect_uri", app.config.google_redirect_uri.as_str()),
            ("code", code),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| AppError::OAuthError(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::OAuthError(format!("Google token parse error: {}", e)))?;

    // Get user info
    let user_info: GoogleUserInfo = client
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(&token_res.access_token)
        .send()
        .await
        .map_err(|e| AppError::OAuthError(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::OAuthError(format!("Google userinfo parse error: {}", e)))?;

    Ok((user_info.email, user_info.name, user_info.sub, user_info.picture))
}

// ─── GitHub token exchange ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct GitHubTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct GitHubUser {
    id: i64,
    name: Option<String>,
    avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubEmail {
    email: String,
    primary: bool,
    verified: bool,
}

async fn exchange_github_code(
    app: &AppState,
    code: &str,
) -> Result<(String, Option<String>, String, Option<String>)> {
    let client = reqwest::Client::builder()
        .user_agent("tabular-server/0.1")
        .build()
        .unwrap();

    // Exchange code for token
    let token_res: GitHubTokenResponse = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&[
            ("client_id", app.config.github_client_id.as_str()),
            ("client_secret", app.config.github_client_secret.as_str()),
            ("redirect_uri", app.config.github_redirect_uri.as_str()),
            ("code", code),
        ])
        .send()
        .await
        .map_err(|e| AppError::OAuthError(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::OAuthError(format!("GitHub token parse error: {}", e)))?;

    // Get user info
    let gh_user: GitHubUser = client
        .get("https://api.github.com/user")
        .bearer_auth(&token_res.access_token)
        .send()
        .await
        .map_err(|e| AppError::OAuthError(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::OAuthError(format!("GitHub user parse error: {}", e)))?;

    // Get primary email
    let emails: Vec<GitHubEmail> = client
        .get("https://api.github.com/user/emails")
        .bearer_auth(&token_res.access_token)
        .send()
        .await
        .map_err(|e| AppError::OAuthError(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::OAuthError(format!("GitHub emails parse error: {}", e)))?;

    let primary_email = emails
        .into_iter()
        .find(|e| e.primary && e.verified)
        .map(|e| e.email)
        .ok_or_else(|| AppError::OAuthError("No verified primary email on GitHub account".to_string()))?;

    Ok((
        primary_email,
        gh_user.name,
        gh_user.id.to_string(),
        gh_user.avatar_url,
    ))
}

// ─── Upsert user ─────────────────────────────────────────────────────────────

async fn upsert_user(
    pool: &MySqlPool,
    provider: &str,
    provider_id: &str,
    email: &str,
    display_name: &Option<String>,
    avatar_url: &Option<String>,
) -> Result<User> {
    let user_id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO users (id, provider, provider_id, email, display_name, avatar_url)
         VALUES (?, ?, ?, ?, ?, ?)
         ON DUPLICATE KEY UPDATE
             email = VALUES(email),
             display_name = VALUES(display_name),
             avatar_url = VALUES(avatar_url),
             updated_at = CURRENT_TIMESTAMP"
    )
    .bind(&user_id)
    .bind(provider)
    .bind(provider_id)
    .bind(email)
    .bind(display_name)
    .bind(avatar_url)
    .execute(pool)
    .await?;

    let user: User = sqlx::query_as(
        "SELECT id, provider, provider_id, email, display_name, avatar_url, created_at, updated_at
         FROM users WHERE provider = ? AND provider_id = ?"
    )
    .bind(provider)
    .bind(provider_id)
    .fetch_one(pool)
    .await?;

    Ok(user)
}

// ─── Helper for URL encoding ──────────────────────────────────────────────────

mod urlencoding {
    pub fn encode(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                    c.to_string()
                }
                _ => format!("%{:02X}", c as u8),
            })
            .collect()
    }
}
