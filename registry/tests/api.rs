use reqwest::{multipart, Client, StatusCode};
use serde_json::{json, Value};
use sqlx::SqlitePool;

/// Helper: spin up a test server and return its base URL.
async fn spawn_server() -> (String, SqlitePool) {
    // Use a unique in-memory DB per test to isolate state.
    let pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to create in-memory db");

    // Run migrations.
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
        "#,
    )
    .execute(&pool)
    .await
    .expect("failed to run migrations");

    let app = sandbox_registry::build_app(pool.clone());

    // Bind to port 0 (OS picks a free port).
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind");
    let addr = listener.local_addr().expect("failed to get addr");

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server failed");
    });

    (format!("http://{}", addr), pool)
}

/// Helper: register a user and return the API key.
async fn register_user(client: &Client, base: &str, suffix: &str) -> String {
    let resp = client
        .post(format!("{}/api/v1/users/register", base))
        .json(&json!({
            "username": format!("user_{}", suffix),
            "email": format!("{}@test.com", suffix),
            "password": "testpass123",
        }))
        .send()
        .await
        .expect("register request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("invalid json");
    body["api_key"].as_str().unwrap().to_string()
}

/// Helper: publish a package and return the response body.
async fn publish_package(
    client: &Client,
    base: &str,
    api_key: &str,
    name: &str,
    version: &str,
    description: &str,
    content: &[u8],
) -> Value {
    let form = multipart::Form::new()
        .text("name", name.to_string())
        .text("version", version.to_string())
        .text("description", description.to_string())
        .text("content", String::from_utf8_lossy(content).to_string());

    let resp = client
        .post(format!("{}/api/v1/packages", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .expect("publish request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    resp.json().await.expect("invalid json")
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_health_endpoint() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    let resp = client
        .get(format!("{}/health", base))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.text().await.unwrap(), "OK");
}

#[tokio::test]
async fn test_dashboard_endpoint() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    let resp = client
        .get(format!("{}/", base))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let html = resp.text().await.unwrap();
    assert!(html.contains("Sandbox Registry"));
    assert!(html.contains("package"));
}

#[tokio::test]
async fn test_register_user() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    let api_key = register_user(&client, &base, "alice").await;
    assert!(!api_key.is_empty());
    // API key should be a hex string (64 chars for 32 bytes)
    assert_eq!(api_key.len(), 64);
}

#[tokio::test]
async fn test_register_duplicate_user() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    register_user(&client, &base, "bob").await;

    // Second registration with same username should fail.
    let resp = client
        .post(format!("{}/api/v1/users/register", base))
        .json(&json!({
            "username": "user_bob",
            "email": "bob2@test.com",
            "password": "pass",
        }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_register_missing_fields() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    let resp = client
        .post(format!("{}/api/v1/users/register", base))
        .json(&json!({
            "username": "",
            "email": "a@b.com",
            "password": "pass",
        }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_login_success() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    register_user(&client, &base, "charlie").await;

    let resp = client
        .post(format!("{}/api/v1/auth/login", base))
        .json(&json!({
            "username": "user_charlie",
            "password": "testpass123",
        }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("invalid json");
    assert_eq!(body["username"].as_str().unwrap(), "user_charlie");
    assert!(!body["api_key"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_login_wrong_password() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    register_user(&client, &base, "dave").await;

    let resp = client
        .post(format!("{}/api/v1/auth/login", base))
        .json(&json!({
            "username": "user_dave",
            "password": "wrongpass",
        }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_login_nonexistent_user() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    let resp = client
        .post(format!("{}/api/v1/auth/login", base))
        .json(&json!({
            "username": "nobody",
            "password": "pass",
        }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_publish_package() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "eve").await;

    let result = publish_package(
        &client,
        &base,
        &api_key,
        "my-lib",
        "1.0.0",
        "A test library",
        b"fn main() { print(\"hello\") }",
    )
    .await;

    assert!(result["ok"].as_bool().unwrap());
    assert_eq!(result["package"].as_str().unwrap(), "my-lib");
    assert_eq!(result["version"].as_str().unwrap(), "1.0.0");
    assert!(result["checksum"].as_str().unwrap().starts_with("sha256:"));
    assert!(result["size"].as_i64().unwrap() > 0);
}

#[tokio::test]
async fn test_publish_requires_auth() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    let form = multipart::Form::new()
        .text("name", "pkg")
        .text("version", "1.0.0")
        .text("content", "data");

    let resp = client
        .post(format!("{}/api/v1/packages", base))
        .multipart(form)
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_publish_duplicate_version() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "frank").await;

    publish_package(
        &client,
        &base,
        &api_key,
        "dup-pkg",
        "1.0.0",
        "First version",
        b"v1",
    )
    .await;

    // Publishing same version again should fail.
    let form = multipart::Form::new()
        .text("name", "dup-pkg")
        .text("version", "1.0.0")
        .text("content", "v1 again");

    let resp = client
        .post(format!("{}/api/v1/packages", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_publish_different_versions() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "grace").await;

    publish_package(
        &client,
        &base,
        &api_key,
        "multi",
        "1.0.0",
        "v1",
        b"v1 content",
    )
    .await;

    publish_package(
        &client,
        &base,
        &api_key,
        "multi",
        "2.0.0",
        "v2",
        b"v2 content",
    )
    .await;

    let resp = client
        .get(format!("{}/api/v1/packages/multi", base))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("invalid json");
    assert_eq!(body["latest_version"].as_str().unwrap(), "2.0.0");
    assert_eq!(body["versions"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_search_packages_empty() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    let resp = client
        .get(format!("{}/api/v1/packages", base))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("invalid json");
    assert_eq!(body["packages"].as_array().unwrap().len(), 0);
    assert_eq!(body["total"].as_i64().unwrap(), 0);
}

#[tokio::test]
async fn test_search_packages_with_results() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "searcher").await;

    publish_package(
        &client,
        &base,
        &api_key,
        "alpha-lib",
        "1.0.0",
        "Alpha utilities",
        b"alpha",
    )
    .await;

    publish_package(
        &client,
        &base,
        &api_key,
        "beta-lib",
        "1.0.0",
        "Beta helpers",
        b"beta",
    )
    .await;

    // Search all
    let resp = client
        .get(format!("{}/api/v1/packages", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    assert_eq!(body["total"].as_i64().unwrap(), 2);

    // Search by name
    let resp = client
        .get(format!("{}/api/v1/packages?q=alpha", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    assert_eq!(body["total"].as_i64().unwrap(), 1);
    assert_eq!(
        body["packages"].as_array().unwrap()[0]["name"]
            .as_str()
            .unwrap(),
        "alpha-lib"
    );

    // Search by description
    let resp = client
        .get(format!("{}/api/v1/packages?q=helpers", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    assert_eq!(body["total"].as_i64().unwrap(), 1);
    assert_eq!(
        body["packages"].as_array().unwrap()[0]["name"]
            .as_str()
            .unwrap(),
        "beta-lib"
    );
}

#[tokio::test]
async fn test_get_package_info() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "info").await;

    publish_package(
        &client,
        &base,
        &api_key,
        "info-pkg",
        "3.2.1",
        "Some description",
        b"content",
    )
    .await;

    let resp = client
        .get(format!("{}/api/v1/packages/info-pkg", base))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("invalid json");
    assert_eq!(body["name"].as_str().unwrap(), "info-pkg");
    assert_eq!(body["description"].as_str().unwrap(), "Some description");
    assert_eq!(body["latest_version"].as_str().unwrap(), "3.2.1");
    assert_eq!(body["versions"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_get_package_not_found() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    let resp = client
        .get(format!("{}/api/v1/packages/nonexistent", base))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_download_package() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "downloader").await;

    let content = b"fn main() { print(\"download test\") }";
    publish_package(
        &client,
        &base,
        &api_key,
        "dl-pkg",
        "1.0.0",
        "Download test",
        content,
    )
    .await;

    let resp = client
        .get(format!("{}/api/v1/packages/dl-pkg/1.0.0/download", base))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("octet-stream"));
    assert!(resp
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("dl-pkg-1.0.0.sb"));

    let body = resp.bytes().await.expect("failed to read body");
    assert_eq!(body.as_ref(), content);
}

#[tokio::test]
async fn test_download_package_not_found() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    let resp = client
        .get(format!("{}/api/v1/packages/nope/1.0.0/download", base))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_yank_version() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "yanker").await;

    publish_package(
        &client,
        &base,
        &api_key,
        "yank-pkg",
        "1.0.0",
        "Yank test",
        b"yank me",
    )
    .await;

    let resp = client
        .post(format!("{}/api/v1/packages/yank-pkg/1.0.0/yank", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("invalid json");
    assert!(body["ok"].as_bool().unwrap());

    // Verify the version is yanked in package info.
    let resp = client
        .get(format!("{}/api/v1/packages/yank-pkg", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    assert!(body["versions"].as_array().unwrap()[0]["yanked"]
        .as_bool()
        .unwrap());
    // Yanked versions shouldn't appear as latest.
    assert!(body["latest_version"].is_null());
}

#[tokio::test]
async fn test_yank_requires_auth() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    let resp = client
        .post(format!("{}/api/v1/packages/nope/1.0.0/yank", base))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_yank_requires_ownership() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let owner_key = register_user(&client, &base, "owner").await;
    let other_key = register_user(&client, &base, "other").await;

    publish_package(
        &client,
        &base,
        &owner_key,
        "owned-pkg",
        "1.0.0",
        "Owned",
        b"owned",
    )
    .await;

    // Non-owner tries to yank.
    let resp = client
        .post(format!("{}/api/v1/packages/owned-pkg/1.0.0/yank", base))
        .header("Authorization", format!("Bearer {}", other_key))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_delete_package() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "deleter").await;

    publish_package(
        &client,
        &base,
        &api_key,
        "del-pkg",
        "1.0.0",
        "Delete me",
        b"delete",
    )
    .await;

    let resp = client
        .delete(format!("{}/api/v1/packages/del-pkg", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("invalid json");
    assert!(body["ok"].as_bool().unwrap());

    // Verify it's gone.
    let resp = client
        .get(format!("{}/api/v1/packages/del-pkg", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_delete_requires_auth() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    let resp = client
        .delete(format!("{}/api/v1/packages/nope", base))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_delete_requires_ownership() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let owner_key = register_user(&client, &base, "delowner").await;
    let other_key = register_user(&client, &base, "delother").await;

    publish_package(
        &client,
        &base,
        &owner_key,
        "del-owned",
        "1.0.0",
        "Owned",
        b"owned",
    )
    .await;

    let resp = client
        .delete(format!("{}/api/v1/packages/del-owned", base))
        .header("Authorization", format!("Bearer {}", other_key))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_full_lifecycle() {
    // End-to-end: register → publish → search → info → download → yank → delete
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    // 1. Register
    let api_key = register_user(&client, &base, "lifecycle").await;

    // 2. Publish v1
    publish_package(
        &client,
        &base,
        &api_key,
        "lifecycle-pkg",
        "1.0.0",
        "Lifecycle test",
        b"version 1",
    )
    .await;

    // 3. Publish v2
    publish_package(
        &client,
        &base,
        &api_key,
        "lifecycle-pkg",
        "2.0.0",
        "Lifecycle test v2",
        b"version 2",
    )
    .await;

    // 4. Search finds it
    let resp = client
        .get(format!("{}/api/v1/packages?q=lifecycle", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    assert_eq!(body["total"].as_i64().unwrap(), 1);

    // 5. Info shows both versions
    let resp = client
        .get(format!("{}/api/v1/packages/lifecycle-pkg", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    assert_eq!(body["latest_version"].as_str().unwrap(), "2.0.0");
    assert_eq!(body["versions"].as_array().unwrap().len(), 2);

    // 6. Download v1
    let resp = client
        .get(format!(
            "{}/api/v1/packages/lifecycle-pkg/1.0.0/download",
            base
        ))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.bytes().await.unwrap().as_ref(), b"version 1");

    // 7. Download v2
    let resp = client
        .get(format!(
            "{}/api/v1/packages/lifecycle-pkg/2.0.0/download",
            base
        ))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.bytes().await.unwrap().as_ref(), b"version 2");

    // 8. Yank v1
    let resp = client
        .post(format!("{}/api/v1/packages/lifecycle-pkg/1.0.0/yank", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);

    // 9. Latest is still v2
    let resp = client
        .get(format!("{}/api/v1/packages/lifecycle-pkg", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    assert_eq!(body["latest_version"].as_str().unwrap(), "2.0.0");
    // v1 is yanked
    let v1 = body["versions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["version"].as_str() == Some("1.0.0"))
        .unwrap();
    assert!(v1["yanked"].as_bool().unwrap());

    // 10. Delete entire package
    let resp = client
        .delete(format!("{}/api/v1/packages/lifecycle-pkg", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);

    // 11. Gone
    let resp = client
        .get(format!("{}/api/v1/packages/lifecycle-pkg", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_pagination() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "pager").await;

    for i in 0..5 {
        publish_package(
            &client,
            &base,
            &api_key,
            &format!("page-pkg-{}", i),
            "1.0.0",
            &format!("Package {}", i),
            b"data",
        )
        .await;
    }

    // Page 1 with per_page=2
    let resp = client
        .get(format!("{}/api/v1/packages?page=1&per_page=2", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    assert_eq!(body["total"].as_i64().unwrap(), 5);
    assert_eq!(body["page"].as_i64().unwrap(), 1);
    assert_eq!(body["per_page"].as_i64().unwrap(), 2);
    assert_eq!(body["packages"].as_array().unwrap().len(), 2);

    // Page 3 with per_page=2 (should have 1 item)
    let resp = client
        .get(format!("{}/api/v1/packages?page=3&per_page=2", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    assert_eq!(body["packages"].as_array().unwrap().len(), 1);
}

// ── Rate Limiting Tests ─────────────────────────────────────────────────────

/// Spawn a server with custom rate limit config.
async fn spawn_server_with_rate_limit(max_requests: u64, window_secs: u64) -> String {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to create in-memory db");

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
        "#,
    )
    .execute(&pool)
    .await
    .expect("failed to run migrations");

    let config = sandbox_registry::ratelimit::RateLimitConfig {
        max_requests,
        window_secs,
        max_upload_bytes: 50 * 1024 * 1024,
    };
    let app = sandbox_registry::build_app_with_config(pool, config);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind");
    let addr = listener.local_addr().expect("failed to get addr");

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server failed");
    });

    format!("http://{}", addr)
}

#[tokio::test]
async fn test_rate_limit_enforced() {
    // 3 requests per 60-second window.
    let base = spawn_server_with_rate_limit(3, 60).await;
    let client = Client::new();

    // First 3 should succeed.
    for i in 0..3 {
        let resp = client
            .get(format!("{}/health", base))
            .send()
            .await
            .expect("request failed");
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "request {} should succeed",
            i
        );
    }

    // 4th should be rate-limited.
    let resp = client
        .get(format!("{}/health", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(
        resp.headers().contains_key("retry-after"),
        "should have Retry-After header"
    );
}

#[tokio::test]
async fn test_rate_limit_different_paths_share_window() {
    // 3 requests per 60 seconds, shared across all paths.
    let base = spawn_server_with_rate_limit(3, 60).await;
    let client = Client::new();

    // Use up the limit on different paths.
    let _ = client.get(format!("{}/health", base)).send().await;
    let _ = client.get(format!("{}/api/v1/packages", base)).send().await;
    let _ = client
        .get(format!("{}/api/v1/packages?", base))
        .send()
        .await;

    // 4th request on any path should be rate-limited.
    let resp = client
        .get(format!("{}/health", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_rate_limit_does_not_affect_different_ips() {
    // 1 request per 60 seconds.
    let base = spawn_server_with_rate_limit(1, 60).await;
    let client = Client::new();

    // First request from default IP succeeds.
    let resp = client
        .get(format!("{}/health", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);

    // Second request from same IP is rate-limited.
    let resp = client
        .get(format!("{}/health", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);

    // Request from different IP (via X-Forwarded-For) succeeds.
    let resp = client
        .get(format!("{}/health", base))
        .header("x-forwarded-for", "10.0.0.99")
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);
}

// ── Package Size Limit Tests ────────────────────────────────────────────────

/// Spawn a server with a custom upload size limit.
async fn spawn_server_with_size_limit(max_upload_bytes: usize) -> (String, SqlitePool) {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("failed to create in-memory db");

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
        "#,
    )
    .execute(&pool)
    .await
    .expect("failed to run migrations");

    let config = sandbox_registry::ratelimit::RateLimitConfig {
        max_requests: 1000,
        window_secs: 60,
        max_upload_bytes,
    };
    let app = sandbox_registry::build_app_with_config(pool.clone(), config);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind");
    let addr = listener.local_addr().expect("failed to get addr");

    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server failed");
    });

    (format!("http://{}", addr), pool)
}

#[tokio::test]
async fn test_publish_rejects_oversized_package() {
    // 1 KB limit.
    let (base, _pool) = spawn_server_with_size_limit(1024).await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "sizer").await;

    // Create content larger than 1 KB.
    let large_content = vec![b'x'; 2048];
    let form = multipart::Form::new()
        .text("name", "big-pkg")
        .text("version", "1.0.0")
        .text(
            "content",
            String::from_utf8_lossy(&large_content).to_string(),
        );

    let resp = client
        .post(format!("{}/api/v1/packages", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .expect("request failed");

    let status = resp.status();
    let body_text = resp.text().await.unwrap_or_default();
    assert_eq!(
        status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "expected 413, got {} body: {}",
        status,
        body_text
    );
}

#[tokio::test]
async fn test_publish_allows_small_package() {
    // 1 KB limit.
    let (base, _pool) = spawn_server_with_size_limit(1024).await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "smallpkg").await;

    // Create content smaller than 1 KB.
    let small_content = vec![b'x'; 512];
    let form = multipart::Form::new()
        .text("name", "small-pkg")
        .text("version", "1.0.0")
        .text(
            "content",
            String::from_utf8_lossy(&small_content).to_string(),
        );

    let resp = client
        .post(format!("{}/api/v1/packages", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("invalid json");
    assert!(body["ok"].as_bool().unwrap());
}

#[tokio::test]
async fn test_publish_rejects_exact_boundary_package() {
    // 1 KB limit, upload exactly 1024 bytes.
    let (base, _pool) = spawn_server_with_size_limit(1024).await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "boundary").await;

    let content = vec![b'x'; 1024];
    let form = multipart::Form::new()
        .text("name", "boundary-pkg")
        .text("version", "1.0.0")
        .text("content", String::from_utf8_lossy(&content).to_string());

    let resp = client
        .post(format!("{}/api/v1/packages", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .expect("request failed");

    // Exactly at the limit should be allowed.
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_health_not_rate_limited_during_burst() {
    // Even with a 1-request limit, health should... actually still be limited.
    // This tests that rate limiting applies uniformly.
    let base = spawn_server_with_rate_limit(1, 60).await;
    let client = Client::new();

    let resp = client
        .get(format!("{}/health", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = client
        .get(format!("{}/health", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

// ── Transitive Dependency Tests ─────────────────────────────────────────────

/// Helper: publish a package with dependencies.
#[allow(clippy::too_many_arguments)] // test scaffolding mirrors the multipart form
async fn publish_package_with_deps(
    client: &Client,
    base: &str,
    api_key: &str,
    name: &str,
    version: &str,
    description: &str,
    content: &[u8],
    deps: &[(&str, &str)],
) -> Value {
    let form = multipart::Form::new()
        .text("name", name.to_string())
        .text("version", version.to_string())
        .text("description", description.to_string())
        .text("content", String::from_utf8_lossy(content).to_string())
        .text(
            "deps",
            serde_json::to_string(
                &deps
                    .iter()
                    .map(|(n, s)| json!({"name": n, "spec": s}))
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_default(),
        );

    let resp = client
        .post(format!("{}/api/v1/packages", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .expect("publish request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    resp.json().await.expect("invalid json")
}

#[tokio::test]
async fn test_publish_with_dependencies() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "depuser").await;

    publish_package_with_deps(
        &client,
        &base,
        &api_key,
        "my-app",
        "1.0.0",
        "App with deps",
        b"fn main() {}",
        &[("serde", "^1.0"), ("tokio", "^0.2")],
    )
    .await;

    // Verify deps endpoint returns the dependencies
    let resp = client
        .get(format!("{}/api/v1/packages/my-app/1.0.0/deps", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("invalid json");
    assert_eq!(body["package"].as_str().unwrap(), "my-app");
    assert_eq!(body["version"].as_str().unwrap(), "1.0.0");
    let deps = body["dependencies"].as_array().unwrap();
    assert_eq!(deps.len(), 2);
    // Should be sorted by name
    assert_eq!(deps[0]["name"].as_str().unwrap(), "serde");
    assert_eq!(deps[0]["spec"].as_str().unwrap(), "^1.0");
    assert_eq!(deps[1]["name"].as_str().unwrap(), "tokio");
    assert_eq!(deps[1]["spec"].as_str().unwrap(), "^0.2");
}

#[tokio::test]
async fn test_publish_without_dependencies() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "nodepuser").await;

    publish_package(
        &client,
        &base,
        &api_key,
        "standalone",
        "1.0.0",
        "No deps",
        b"fn main() {}",
    )
    .await;

    // Deps endpoint returns empty array
    let resp = client
        .get(format!("{}/api/v1/packages/standalone/1.0.0/deps", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("invalid json");
    let deps = body["dependencies"].as_array().unwrap();
    assert_eq!(deps.len(), 0);
}

#[tokio::test]
async fn test_version_deps_in_package_info() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "infodep").await;

    publish_package_with_deps(
        &client,
        &base,
        &api_key,
        "dep-info-pkg",
        "1.0.0",
        "Has deps",
        b"code",
        &[("regex", "^1.5")],
    )
    .await;

    // The package info response should include deps in versions
    let resp = client
        .get(format!("{}/api/v1/packages/dep-info-pkg", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("invalid json");
    let versions = body["versions"].as_array().unwrap();
    assert_eq!(versions.len(), 1);
    let deps = versions[0]["dependencies"].as_array().unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0]["name"].as_str().unwrap(), "regex");
    assert_eq!(deps[0]["spec"].as_str().unwrap(), "^1.5");
}

#[tokio::test]
async fn test_package_without_deps_omits_field() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "nodepinf").await;

    publish_package(
        &client,
        &base,
        &api_key,
        "no-dep-pkg",
        "1.0.0",
        "No deps",
        b"code",
    )
    .await;

    let resp = client
        .get(format!("{}/api/v1/packages/no-dep-pkg", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    let version = &body["versions"].as_array().unwrap()[0];
    // Should not have dependencies field (or it should be null)
    assert!(version.get("dependencies").is_none() || version["dependencies"].is_null());
}

#[tokio::test]
async fn test_deps_endpoint_not_found() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    // Package not found
    let resp = client
        .get(format!("{}/api/v1/packages/nonexistent/1.0.0/deps", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_transitive_chain_in_registry() {
    // A -> B -> C: when we publish all three with deps declared,
    // the deps endpoints should reflect the full chain.
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "chainuser").await;

    // Publish C (no deps)
    publish_package(
        &client,
        &base,
        &api_key,
        "lib-c",
        "1.0.0",
        "Leaf library",
        b"fn c() {}",
    )
    .await;

    // Publish B -> depends on C
    publish_package_with_deps(
        &client,
        &base,
        &api_key,
        "lib-b",
        "1.0.0",
        "Mid library",
        b"fn b() {}",
        &[("lib-c", "^1.0")],
    )
    .await;

    // Publish A -> depends on B
    publish_package_with_deps(
        &client,
        &base,
        &api_key,
        "app-a",
        "1.0.0",
        "Top app",
        b"fn main() {}",
        &[("lib-b", "^1.0")],
    )
    .await;

    // Verify A's deps
    let resp = client
        .get(format!("{}/api/v1/packages/app-a/1.0.0/deps", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    let deps = body["dependencies"].as_array().unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0]["name"].as_str().unwrap(), "lib-b");

    // Verify B's deps
    let resp = client
        .get(format!("{}/api/v1/packages/lib-b/1.0.0/deps", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    let deps = body["dependencies"].as_array().unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0]["name"].as_str().unwrap(), "lib-c");

    // Verify C has no deps
    let resp = client
        .get(format!("{}/api/v1/packages/lib-c/1.0.0/deps", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    let deps = body["dependencies"].as_array().unwrap();
    assert_eq!(deps.len(), 0);
}

// ── Package Detail Page Tests ───────────────────────────────────────────────

#[tokio::test]
async fn test_package_detail_page() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "detailuser").await;

    publish_package(
        &client,
        &base,
        &api_key,
        "cool-pkg",
        "1.2.0",
        "A cool package",
        b"fn main() {}",
    )
    .await;

    let resp = client
        .get(format!("{}/packages/cool-pkg", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let html = resp.text().await.unwrap();
    assert!(html.contains("cool-pkg"));
    assert!(html.contains("A cool package"));
    assert!(html.contains("1.2.0"));
    assert!(html.contains("sandbox add "));
}

#[tokio::test]
async fn test_package_detail_page_with_readme() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "readmeuser").await;

    let readme_content =
        "# My Package\n\nThis is a **great** package.\n\n## Features\n\n- Fast\n- Safe";

    let form = multipart::Form::new()
        .text("name", "readme-pkg")
        .text("version", "1.0.0")
        .text("description", "Has README")
        .text("readme", readme_content)
        .text("content", "code here");

    let resp = client
        .post(format!("{}/api/v1/packages", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .expect("publish request failed");
    assert_eq!(resp.status(), StatusCode::OK);

    let resp = client
        .get(format!("{}/packages/readme-pkg", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let html = resp.text().await.unwrap();
    assert!(html.contains("readme-pkg"));
    assert!(html.contains("Has README"));
    // README should be in the rendered HTML (as JSON data for client-side rendering)
    assert!(html.contains("This is a **great** package."));
}

#[tokio::test]
async fn test_package_detail_page_not_found() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    let resp = client
        .get(format!("{}/packages/nonexistent", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let html = resp.text().await.unwrap();
    assert!(html.contains("Not Found"));
}

#[tokio::test]
async fn test_package_detail_page_with_deps() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "detaildepuser").await;

    publish_package_with_deps(
        &client,
        &base,
        &api_key,
        "dep-pkg",
        "2.0.0",
        "Package with deps",
        b"code",
        &[("serde", "^1.0"), ("tokio", "^0.2")],
    )
    .await;

    let resp = client
        .get(format!("{}/packages/dep-pkg", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let html = resp.text().await.unwrap();
    assert!(html.contains("dep-pkg"));
    assert!(html.contains("2.0.0"));
    // Deps should be in the JSON data
    assert!(html.contains("serde"));
    assert!(html.contains("tokio"));
}

// ── Package Signing Tests ────────────────────────────────────────────────────

/// Generate a test ed25519 keypair, return (signing_key_hex, verifying_key_hex).
fn generate_test_keypair() -> (String, String) {
    use ed25519_dalek::SigningKey;
    use rand::rngs::OsRng;
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    (
        hex::encode(signing_key.to_bytes()),
        hex::encode(verifying_key.as_bytes()),
    )
}

/// Sign data with a hex-encoded private key, return hex-encoded signature.
fn sign_with_key(data: &[u8], private_key_hex: &str) -> String {
    use ed25519_dalek::{Signer, SigningKey};
    let key_bytes = hex::decode(private_key_hex).unwrap();
    let arr: [u8; 32] = key_bytes.try_into().unwrap();
    let signing_key = SigningKey::from_bytes(&arr);
    let sig = signing_key.sign(data);
    hex::encode(sig.to_bytes())
}

#[tokio::test]
async fn test_register_public_key() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "keyreg").await;
    let (_, public_hex) = generate_test_keypair();

    let resp = client
        .post(format!("{}/api/v1/users/keys", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&json!({ "public_key": public_hex }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("invalid json");
    assert!(body["ok"].as_bool().unwrap());
    assert_eq!(body["public_key"].as_str().unwrap(), public_hex);
}

#[tokio::test]
async fn test_register_invalid_key() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "badkey").await;

    // Too short
    let resp = client
        .post(format!("{}/api/v1/users/keys", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&json!({ "public_key": "aabb" }))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // Invalid hex
    let resp = client
        .post(format!("{}/api/v1/users/keys", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&json!({ "public_key": "not-hex-at-all!" }))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_register_key_requires_auth() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let (_, public_hex) = generate_test_keypair();

    let resp = client
        .post(format!("{}/api/v1/users/keys", base))
        .json(&json!({ "public_key": public_hex }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_user_key() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "keyget").await;
    let (_, public_hex) = generate_test_keypair();

    // Register the key
    client
        .post(format!("{}/api/v1/users/keys", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&json!({ "public_key": public_hex }))
        .send()
        .await
        .expect("request failed");

    // Get the key
    let resp = client
        .get(format!("{}/api/v1/users/user_keyget/keys", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("invalid json");
    assert_eq!(body["public_key"].as_str().unwrap(), public_hex);
}

#[tokio::test]
async fn test_publish_with_signature() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "signer").await;
    let (private_hex, public_hex) = generate_test_keypair();

    // Register the public key
    client
        .post(format!("{}/api/v1/users/keys", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&json!({ "public_key": public_hex }))
        .send()
        .await
        .expect("request failed");

    let content = b"fn main() { print(\"signed package\") }";

    // Compute checksum for signing
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content);
    let file_hash = hex::encode(hasher.finalize());
    let checksum = format!("sha256:{}", file_hash);

    // Sign the checksum
    let signature = sign_with_key(checksum.as_bytes(), &private_hex);

    let form = multipart::Form::new()
        .text("name", "signed-pkg")
        .text("version", "1.0.0")
        .text("description", "Signed package")
        .text("content", String::from_utf8_lossy(content).to_string())
        .text("signature", signature.clone());

    let resp = client
        .post(format!("{}/api/v1/packages", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .expect("publish request failed");
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify the signature via API
    let resp = client
        .get(format!("{}/api/v1/packages/signed-pkg/1.0.0/verify", base))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.expect("invalid json");
    assert!(body["valid"].as_bool().unwrap());
    assert_eq!(body["signed_by"].as_str().unwrap(), "user_signer");
    assert_eq!(body["public_key"].as_str().unwrap(), public_hex);
}

#[tokio::test]
async fn test_verify_unsigned_package() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "unsigner").await;

    publish_package(
        &client,
        &base,
        &api_key,
        "unsigned-pkg",
        "1.0.0",
        "Not signed",
        b"code",
    )
    .await;
    let resp = client
        .get(format!(
            "{}/api/v1/packages/unsigned-pkg/1.0.0/verify",
            base
        ))
        .send()
        .await
        .expect("request failed");
    // Unsigned packages report signed=false with 200 so `sandbox pkg verify`
    // (which parses this JSON) can print its graceful NOT-signed warning.
    assert_eq!(resp.status(), StatusCode::OK);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["signed"], json!(false));
    assert_eq!(body["valid"], json!(false));
    assert_eq!(body["signed_by"], Value::Null);
}

#[tokio::test]
async fn test_verify_bad_signature() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "badsigner").await;
    let (_, public_hex) = generate_test_keypair();

    // Register the key
    client
        .post(format!("{}/api/v1/users/keys", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&json!({ "public_key": public_hex }))
        .send()
        .await
        .expect("request failed");

    // Publish with a wrong signature (random bytes)
    let bad_sig = "a".repeat(128); // 64 bytes in hex
    let form = multipart::Form::new()
        .text("name", "badsig-pkg")
        .text("version", "1.0.0")
        .text("description", "Bad sig")
        .text("content", "code")
        .text("signature", bad_sig);

    let resp = client
        .post(format!("{}/api/v1/packages", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .expect("publish request failed");
    assert_eq!(resp.status(), StatusCode::OK);

    // Verify should fail
    let resp = client
        .get(format!("{}/api/v1/packages/badsig-pkg/1.0.0/verify", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    assert!(!body["valid"].as_bool().unwrap());
}

#[tokio::test]
async fn test_signature_shown_in_package_info() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();
    let api_key = register_user(&client, &base, "siginfo").await;
    let (private_hex, public_hex) = generate_test_keypair();

    client
        .post(format!("{}/api/v1/users/keys", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&json!({ "public_key": public_hex }))
        .send()
        .await
        .expect("request failed");

    let content = b"signed content";
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content);
    let checksum = format!("sha256:{}", hex::encode(hasher.finalize()));
    let signature = sign_with_key(checksum.as_bytes(), &private_hex);

    let form = multipart::Form::new()
        .text("name", "siginfo-pkg")
        .text("version", "1.0.0")
        .text("content", String::from_utf8_lossy(content).to_string())
        .text("signature", signature);

    client
        .post(format!("{}/api/v1/packages", base))
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .expect("request failed");

    // Package info should include signature and signed_by
    let resp = client
        .get(format!("{}/api/v1/packages/siginfo-pkg", base))
        .send()
        .await
        .expect("request failed");
    let body: Value = resp.json().await.expect("invalid json");
    let versions = body["versions"].as_array().unwrap();
    assert_eq!(versions.len(), 1);
    assert!(versions[0]["signature"].as_str().is_some());
    assert_eq!(versions[0]["signed_by"].as_str().unwrap(), "user_siginfo");
}

#[tokio::test]
async fn test_playground_routes() {
    let (base, _pool) = spawn_server().await;
    let client = Client::new();

    // Page
    let resp = client
        .get(format!("{}/playground", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let html = resp.text().await.unwrap();
    assert!(html.contains("Sandbox Playground"));
    assert!(html.contains("/playground/playground.js"));

    // JS asset
    let resp = client
        .get(format!("{}/playground/playground.js", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert!(resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("application/javascript"));
    let js = resp.text().await.unwrap();
    assert!(js.contains("/playground/compiler.wasm"));
    assert!(js.contains("/api/v1/packages"));

    // Compiler artifact must be a well-formed wasm module
    let resp = client
        .get(format!("{}/playground/compiler.wasm", base))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "application/wasm"
    );
    let bytes = resp.bytes().await.unwrap();
    assert!(bytes.len() > 8);
    assert_eq!(&bytes[0..4], b"\0asm", "compiler.wasm missing wasm magic");
}
