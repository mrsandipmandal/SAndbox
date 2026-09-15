use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub email: String,
    pub api_key: String,
    #[sqlx(default)]
    pub ed25519_public_key: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Package {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Version {
    pub id: i64,
    pub package_id: i64,
    pub version: String,
    pub file_hash: String,
    pub file_size: i64,
    pub checksum: String,
    #[sqlx(default)]
    pub signature: Option<String>,
    pub yanked: bool,
    pub published_by: i64,
    pub published_at: DateTime<Utc>,
}

// API request/response types

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub api_key: String,
    pub username: String,
}

#[derive(Debug, Deserialize)]
pub struct PublishRequest {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub entry_point: Option<String>,
    pub dependencies: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct PackageInfo {
    pub name: String,
    pub description: Option<String>,
    pub latest_version: Option<String>,
    pub versions: Vec<VersionInfo>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct VersionInfo {
    pub version: String,
    pub checksum: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    pub yanked: bool,
    pub published_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<Vec<DependencyInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signed_by: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DependencyInfo {
    pub name: String,
    pub spec: String,
}

#[derive(Debug, Deserialize)]
pub struct RegisterKeyRequest {
    pub public_key: String,
}

#[derive(Debug, Serialize)]
pub struct KeyInfo {
    pub username: String,
    pub public_key: String,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct SearchResults {
    pub packages: Vec<PackageInfo>,
    pub total: u32,
    pub page: u32,
    pub per_page: u32,
}
