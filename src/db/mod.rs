use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;
use std::time::Duration;

pub mod schema {
    pub const SCHEMA: &str = include_str!("schema.sql");
}

pub async fn create_pool(database_url: &str) -> anyhow::Result<MySqlPool> {
    let pool = MySqlPoolOptions::new()
        .max_connections(20)
        .min_connections(2)
        .acquire_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(300))
        .connect(database_url)
        .await?;

    tracing::info!("✅ MySQL connection pool established");
    Ok(pool)
}

/// Run the embedded schema SQL (idempotent CREATE IF NOT EXISTS statements).
/// Each statement is executed separately to handle MySQL limitations.
pub async fn run_migrations(pool: &MySqlPool) -> anyhow::Result<()> {
    tracing::info!("🔄 Running schema migrations...");

    // Split by the statement delimiter and run each one
    let schema_sql = schema::SCHEMA;
    let statements: Vec<&str> = schema_sql
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && !s.starts_with("--"))
        .collect();

    for stmt in statements {
        if stmt.trim().is_empty() {
            continue;
        }
        sqlx::query(stmt).execute(pool).await.map_err(|e| {
            tracing::error!("Migration error for statement: {}\nError: {}", &stmt[..50.min(stmt.len())], e);
            e
        })?;
    }

    // Ensure redirect_port column exists in existing tables (ignore error if already exists)
    let _ = sqlx::query("ALTER TABLE oauth_states ADD COLUMN redirect_port INT DEFAULT NULL").execute(pool).await;

    tracing::info!("✅ Schema migrations complete");
    Ok(())
}
