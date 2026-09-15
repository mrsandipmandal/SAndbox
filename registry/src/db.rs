use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;

pub async fn init_db(db_path: &str) -> anyhow::Result<SqlitePool> {
    // Ensure parent directory exists
    if let Some(parent) = Path::new(db_path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Use ?mode=rwc to create DB if it doesn't exist
    let conn_str = if db_path.contains('?') {
        db_path.to_string()
    } else {
        format!("{}?mode=rwc", db_path)
    };
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect(&conn_str)
        .await?;

    // Run migrations
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT UNIQUE NOT NULL,
            email TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            api_key TEXT UNIQUE NOT NULL,
            ed25519_public_key TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS packages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT UNIQUE NOT NULL,
            description TEXT,
            readme TEXT,
            owner_id INTEGER NOT NULL REFERENCES users(id),
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS versions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            package_id INTEGER NOT NULL REFERENCES packages(id),
            version TEXT NOT NULL,
            file_hash TEXT NOT NULL,
            file_size INTEGER NOT NULL DEFAULT 0,
            checksum TEXT NOT NULL,
            signature TEXT,
            yanked INTEGER NOT NULL DEFAULT 0,
            published_by INTEGER NOT NULL REFERENCES users(id),
            published_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(package_id, version)
        );

        CREATE TABLE IF NOT EXISTS dependencies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            version_id INTEGER NOT NULL REFERENCES versions(id) ON DELETE CASCADE,
            dep_name TEXT NOT NULL,
            dep_spec TEXT NOT NULL,
            UNIQUE(version_id, dep_name)
        );

        CREATE INDEX IF NOT EXISTS idx_packages_name ON packages(name);
        CREATE INDEX IF NOT EXISTS idx_versions_package_id ON versions(package_id);
        CREATE INDEX IF NOT EXISTS idx_users_api_key ON users(api_key);
        CREATE INDEX IF NOT EXISTS idx_dependencies_version_id ON dependencies(version_id);
        "#,
    )
    .execute(&pool)
    .await?;

    tracing::info!("Database initialized at {}", db_path);
    Ok(pool)
}
