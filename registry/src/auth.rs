use axum::http::StatusCode;
use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AuthenticatedUser {
    pub user_id: i64,
    pub username: String,
}

impl AuthenticatedUser {
    /// Extract user from Authorization header via direct DB lookup.
    pub async fn from_header(
        pool: &SqlitePool,
        headers: &axum::http::HeaderMap,
    ) -> Result<Self, StatusCode> {
        let api_key = extract_api_key(headers).ok_or(StatusCode::UNAUTHORIZED)?;

        let user =
            sqlx::query_as::<_, (i64, String)>("SELECT id, username FROM users WHERE api_key = ?")
                .bind(&api_key)
                .fetch_optional(pool)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
                .ok_or(StatusCode::UNAUTHORIZED)?;

        Ok(AuthenticatedUser {
            user_id: user.0,
            username: user.1,
        })
    }
}

fn extract_api_key(headers: &axum::http::HeaderMap) -> Option<String> {
    let header = headers.get("authorization")?;
    let value = header.to_str().ok()?;
    let key = value.strip_prefix("Bearer ")?;
    Some(key.to_string())
}

/// Hash a password using Argon2
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    use argon2::password_hash::SaltString;
    use argon2::{Argon2, PasswordHasher};

    let salt = SaltString::generate(&mut rand::thread_rng());
    let hash = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?
        .to_string();
    Ok(hash)
}

/// Verify a password against a hash
pub fn verify_password(password: &str, hash: &str) -> anyhow::Result<bool> {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    use argon2::Argon2;

    let parsed =
        PasswordHash::new(hash).map_err(|e| anyhow::anyhow!("Failed to parse hash: {}", e))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Generate a random API key
pub fn generate_api_key() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    hex::encode(bytes)
}
