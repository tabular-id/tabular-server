use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub server_port: u16,
    pub jwt_secret: String,
    pub jwt_access_expiry_minutes: i64,
    pub jwt_refresh_expiry_days: i64,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_redirect_uri: String,
    pub github_client_id: String,
    pub github_client_secret: String,
    pub github_redirect_uri: String,
    pub allowed_origins: Vec<String>,
    pub server_base_url: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let server_base_url = env::var("SERVER_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8420".to_string());
        let clean_base_url = server_base_url.trim_end_matches('/').to_string();

        let google_redirect_uri = env::var("GOOGLE_REDIRECT_URI")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("{}/api/v1/auth/callback/google", clean_base_url));

        let github_redirect_uri = env::var("GITHUB_REDIRECT_URI")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("{}/api/v1/auth/callback/github", clean_base_url));

        Ok(Config {
            database_url: required_env("DATABASE_URL")?,
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "8420".to_string())
                .parse()
                .unwrap_or(8420),
            jwt_secret: required_env("JWT_SECRET")?,
            jwt_access_expiry_minutes: env::var("JWT_ACCESS_EXPIRY_MINUTES")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .unwrap_or(60),
            jwt_refresh_expiry_days: env::var("JWT_REFRESH_EXPIRY_DAYS")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .unwrap_or(30),
            google_client_id: env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default(),
            google_redirect_uri,
            github_client_id: env::var("GITHUB_CLIENT_ID").unwrap_or_default(),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET").unwrap_or_default(),
            github_redirect_uri,
            allowed_origins: env::var("ALLOWED_ORIGINS")
                .unwrap_or_else(|_| "http://localhost:3000".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
            server_base_url: clean_base_url,
        })
    }
}

fn required_env(key: &str) -> anyhow::Result<String> {
    env::var(key).map_err(|_| anyhow::anyhow!("Missing required env var: {}", key))
}
