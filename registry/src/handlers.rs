use axum::{
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use sha2::{Digest, Sha256};

use crate::auth::{generate_api_key, hash_password, verify_password, AuthenticatedUser};
use crate::models::*;
use crate::AppState;

// ─── Health ─────────────────────────────────────────────────────────────────

pub async fn health() -> &'static str {
    "OK"
}

// ─── HTML Dashboard ─────────────────────────────────────────────────────────

pub async fn dashboard() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

// ─── Playground (in-browser .sbx → wasm compiler) ───────────────────────────
//
// All artifacts live under static/playground/ so they are inside the
// registry/ Docker build context.

pub async fn playground_page() -> Html<&'static str> {
    Html(include_str!("../static/playground/playground.html"))
}

pub async fn playground_js() -> impl IntoResponse {
    (
        [(
            header::CONTENT_TYPE,
            "application/javascript; charset=utf-8".to_string(),
        )],
        include_str!("../static/playground/playground.js"),
    )
        .into_response()
}

pub async fn playground_wasm() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/wasm".to_string())],
        include_bytes!("../static/playground/compiler.wasm").as_slice(),
    )
        .into_response()
}

// ─── Package Detail Page (HTML) ─────────────────────────────────────────────

pub async fn package_page(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Html<String>, (StatusCode, String)> {
    let pkg = sqlx::query_as::<_, (i64, String, Option<String>, Option<String>, String)>(
        "SELECT id, name, description, readme, created_at FROM packages WHERE name = ?",
    )
    .bind(&name)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (pkg_id, pkg_name, description, readme, _created_at) = match pkg {
        Some(p) => p,
        None => {
            return Ok(Html(
                include_str!("../static/package.html")
                    .replace("{{PKG_JSON}}", "{}")
                    .replace("{{PACKAGE_NAME}}", "Not Found"),
            ));
        }
    };

    // Fetch owner username
    let owner: (String,) = sqlx::query_as(
        "SELECT u.username FROM users u JOIN packages p ON p.owner_id = u.id WHERE p.id = ?",
    )
    .bind(pkg_id)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Fetch latest version
    let latest_version: Option<String> = sqlx::query_as::<_, (String,)>(
        "SELECT version FROM versions WHERE package_id = ? AND yanked = 0 ORDER BY id DESC LIMIT 1",
    )
    .bind(pkg_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map(|v| v.0);

    // Fetch all versions with deps, signature, and publisher
    let version_rows = sqlx::query_as::<_, (i64, String, String, Option<String>, i64, i64, String)>(
        "SELECT v.id, v.version, v.checksum, v.signature, v.yanked, v.published_by, v.published_at
         FROM versions v WHERE v.package_id = ? ORDER BY v.published_at DESC",
    )
    .bind(pkg_id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut versions = Vec::new();
    for v in version_rows {
        let deps = fetch_deps_for_version(&state.pool, v.0).await?;
        let signer: Option<String> =
            sqlx::query_as::<_, (String,)>("SELECT username FROM users WHERE id = ?")
                .bind(v.5)
                .fetch_optional(&state.pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .map(|r| r.0);
        versions.push(serde_json::json!({
            "version": v.1,
            "checksum": v.2,
            "signature": v.3,
            "signed_by": signer,
            "yanked": v.4 != 0,
            "published_at": v.6,
            "dependencies": if deps.is_empty() { serde_json::Value::Null } else { serde_json::to_value(&deps).unwrap_or_default() },
        }));
    }

    let pkg_json = serde_json::json!({
        "name": pkg_name,
        "description": description,
        "readme": readme.unwrap_or_default(),
        "latest_version": latest_version,
        "owner": owner.0,
        "versions": versions,
    });

    let html = include_str!("../static/package.html")
        .replace("{{PKG_JSON}}", &pkg_json.to_string())
        .replace("{{PACKAGE_NAME}}", &name);

    Ok(Html(html))
}

// ─── User Registration ──────────────────────────────────────────────────────

pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    if req.username.is_empty() || req.password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Username and password required".into(),
        ));
    }

    let password_hash = hash_password(&req.password)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let api_key = generate_api_key();

    let _ = sqlx::query(
        "INSERT INTO users (username, email, password_hash, api_key) VALUES (?, ?, ?, ?)",
    )
    .bind(&req.username)
    .bind(&req.email)
    .bind(&password_hash)
    .bind(&api_key)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            (
                StatusCode::CONFLICT,
                "Username or email already exists".into(),
            )
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;

    tracing::info!("User registered: {}", req.username);

    Ok(Json(AuthResponse {
        api_key,
        username: req.username,
    }))
}

// ─── Login ──────────────────────────────────────────────────────────────────

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, (StatusCode, String)> {
    let user = sqlx::query_as::<_, (i64, String, String, String)>(
        "SELECT id, username, password_hash, api_key FROM users WHERE username = ?",
    )
    .bind(&req.username)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::UNAUTHORIZED, "Invalid credentials".into()))?;

    let valid = verify_password(&req.password, &user.2).unwrap_or(false);
    if !valid {
        return Err((StatusCode::UNAUTHORIZED, "Invalid credentials".into()));
    }

    Ok(Json(AuthResponse {
        api_key: user.3,
        username: user.1,
    }))
}

// ─── Publish Package ────────────────────────────────────────────────────────

pub async fn publish_package(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let auth = AuthenticatedUser::from_header(&state.pool, &headers)
        .await
        .map_err(|e| (e, "Unauthorized".to_string()))?;

    let mut name = String::new();
    let mut version = String::new();
    let mut description = None::<String>;
    let mut readme = None::<String>;
    let mut file_data: Option<Vec<u8>> = None;
    let mut deps_json = None::<String>;
    let mut signature = None::<String>;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        let field_name = field.name().unwrap_or("").to_string();
        match field_name.as_str() {
            "name" => {
                name = field
                    .text()
                    .await
                    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
            }
            "version" => {
                version = field
                    .text()
                    .await
                    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
            }
            "description" => {
                description = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?,
                );
            }
            "readme" => {
                readme = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?,
                );
            }
            "content" => {
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
                file_data = Some(data.to_vec());
            }
            "deps" => {
                deps_json = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?,
                );
            }
            "signature" => {
                signature = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?,
                );
            }
            _ => {
                field
                    .bytes()
                    .await
                    .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
            }
        }
    }

    if name.is_empty() || version.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Package name and version required".into(),
        ));
    }

    let content = file_data.ok_or((StatusCode::BAD_REQUEST, "Package content required".into()))?;

    // Enforce package size limit from config
    if content.len() > state.config.max_upload_bytes {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "Package too large: {} bytes (max {} bytes)",
                content.len(),
                state.config.max_upload_bytes
            ),
        ));
    }

    // Compute hashes
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let file_hash = hex::encode(hasher.finalize());
    let checksum = format!("sha256:{}", file_hash);
    let file_size = content.len() as i64;

    // Store package and version in a transaction
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Upsert package
    let package_id: i64 = if let Some(row) =
        sqlx::query_as::<_, (i64,)>("SELECT id FROM packages WHERE name = ?")
            .bind(&name)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        row.0
    } else {
        let result = sqlx::query(
            "INSERT INTO packages (name, description, readme, owner_id) VALUES (?, ?, ?, ?)",
        )
        .bind(&name)
        .bind(&description)
        .bind(&readme)
        .bind(auth.user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        result.last_insert_rowid()
    };

    // Update readme if provided on an existing package
    if let Some(ref r) = readme {
        sqlx::query("UPDATE packages SET readme = ? WHERE id = ?")
            .bind(r)
            .bind(package_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    // Check if user owns this package
    let owner: (i64,) = sqlx::query_as("SELECT owner_id FROM packages WHERE id = ?")
        .bind(package_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if owner.0 != auth.user_id {
        tx.rollback()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Err((StatusCode::FORBIDDEN, "You don't own this package".into()));
    }

    // Insert version
    let version_result = sqlx::query(
        "INSERT INTO versions (package_id, version, file_hash, file_size, checksum, signature, published_by) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(package_id)
    .bind(&version)
    .bind(&file_hash)
    .bind(file_size)
    .bind(&checksum)
    .bind(&signature)
    .bind(auth.user_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        if e.to_string().contains("UNIQUE") {
            (
                StatusCode::CONFLICT,
                format!("Version {} already exists", version),
            )
        } else {
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    })?;

    let version_id = version_result.last_insert_rowid();

    // Store dependencies if provided
    if let Some(ref deps_str) = deps_json {
        if let Ok(deps) = serde_json::from_str::<Vec<serde_json::Value>>(deps_str) {
            for dep in deps {
                if let (Some(dn), Some(ds)) = (
                    dep.get("name").and_then(|v| v.as_str()),
                    dep.get("spec").and_then(|v| v.as_str()),
                ) {
                    if !dn.is_empty() {
                        sqlx::query(
                            "INSERT INTO dependencies (version_id, dep_name, dep_spec) VALUES (?, ?, ?)",
                        )
                        .bind(version_id)
                        .bind(dn)
                        .bind(ds)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                    }
                }
            }
        }
    }

    // Update package timestamp
    sqlx::query("UPDATE packages SET updated_at = datetime('now') WHERE id = ?")
        .bind(package_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Store file on disk
    let storage_path = format!("registry-data/packages/{}/{}.sb", name, version);
    if let Some(parent) = std::path::Path::new(&storage_path).parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    std::fs::write(&storage_path, &content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!("Published {} v{} by {}", name, version, auth.username);

    Ok(Json(serde_json::json!({
        "ok": true,
        "package": name,
        "version": version,
        "checksum": checksum,
        "size": file_size,
    })))
}

// ─── Search Packages ────────────────────────────────────────────────────────

pub async fn search_packages(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResults>, (StatusCode, String)> {
    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(20).min(100);
    let offset = (page - 1) * per_page;

    let (packages, total) = if let Some(ref q) = query.q {
        let search = format!("%{}%", q);
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM packages WHERE name LIKE ? OR description LIKE ?")
                .bind(&search)
                .bind(&search)
                .fetch_one(&state.pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let rows = sqlx::query_as::<_, (i64, String, Option<String>, String, String, String)>(
            "SELECT p.id, p.name, p.description, p.created_at, p.updated_at, u.username
             FROM packages p JOIN users u ON p.owner_id = u.id
             WHERE p.name LIKE ? OR p.description LIKE ?
             ORDER BY p.updated_at DESC
             LIMIT ? OFFSET ?",
        )
        .bind(&search)
        .bind(&search)
        .bind(per_page as i64)
        .bind(offset as i64)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        (rows, count.0 as u32)
    } else {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM packages")
            .fetch_one(&state.pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let rows = sqlx::query_as::<_, (i64, String, Option<String>, String, String, String)>(
            "SELECT p.id, p.name, p.description, p.created_at, p.updated_at, u.username
             FROM packages p JOIN users u ON p.owner_id = u.id
             ORDER BY p.updated_at DESC
             LIMIT ? OFFSET ?",
        )
        .bind(per_page as i64)
        .bind(offset as i64)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        (rows, count.0 as u32)
    };

    let mut result_packages = Vec::new();
    for row in packages {
        let latest_version = sqlx::query_as::<_, (String,)>(
            "SELECT version FROM versions WHERE package_id = ? AND yanked = 0 ORDER BY id DESC LIMIT 1",
        )
        .bind(row.0)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(|v| v.0);

        let version_rows = sqlx::query_as::<_, (i64, String, String, Option<String>, i64, i64, String)>(
            "SELECT v.id, v.version, v.checksum, v.signature, v.yanked, v.published_by, v.published_at
             FROM versions v WHERE v.package_id = ? ORDER BY v.published_at DESC",
        )
        .bind(row.0)
        .fetch_all(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let mut versions = Vec::new();
        for v in version_rows {
            let deps = fetch_deps_for_version(&state.pool, v.0).await?;
            let signer: Option<String> =
                sqlx::query_as::<_, (String,)>("SELECT username FROM users WHERE id = ?")
                    .bind(v.5)
                    .fetch_optional(&state.pool)
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                    .map(|r| r.0);
            versions.push(VersionInfo {
                version: v.1,
                checksum: v.2,
                signature: v.3,
                yanked: v.4 != 0,
                published_at: v.6.parse().unwrap_or_default(),
                dependencies: if deps.is_empty() { None } else { Some(deps) },
                signed_by: signer,
            });
        }

        result_packages.push(PackageInfo {
            name: row.1,
            description: row.2,
            latest_version,
            versions,
            created_at: row.3.parse().unwrap_or_default(),
        });
    }

    Ok(Json(SearchResults {
        packages: result_packages,
        total,
        page,
        per_page,
    }))
}

// ─── Get Package Info ───────────────────────────────────────────────────────

pub async fn get_package(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<PackageInfo>, (StatusCode, String)> {
    let pkg = sqlx::query_as::<_, (i64, String, Option<String>, String)>(
        "SELECT id, name, description, created_at FROM packages WHERE name = ?",
    )
    .bind(&name)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Package not found".into()))?;

    let latest_version = sqlx::query_as::<_, (String,)>(
        "SELECT version FROM versions WHERE package_id = ? AND yanked = 0 ORDER BY id DESC LIMIT 1",
    )
    .bind(pkg.0)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map(|v| v.0);

    let version_rows = sqlx::query_as::<_, (i64, String, String, Option<String>, i64, i64, String)>(
        "SELECT v.id, v.version, v.checksum, v.signature, v.yanked, v.published_by, v.published_at
         FROM versions v WHERE v.package_id = ? ORDER BY v.published_at DESC",
    )
    .bind(pkg.0)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut versions = Vec::new();
    for v in version_rows {
        let deps = fetch_deps_for_version(&state.pool, v.0).await?;
        let signer: Option<String> =
            sqlx::query_as::<_, (String,)>("SELECT username FROM users WHERE id = ?")
                .bind(v.5)
                .fetch_optional(&state.pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
                .map(|r| r.0);
        versions.push(VersionInfo {
            version: v.1,
            checksum: v.2,
            signature: v.3,
            yanked: v.4 != 0,
            published_at: v.6.parse().unwrap_or_default(),
            dependencies: if deps.is_empty() { None } else { Some(deps) },
            signed_by: signer,
        });
    }

    Ok(Json(PackageInfo {
        name: pkg.1,
        description: pkg.2,
        latest_version,
        versions,
        created_at: pkg.3.parse().unwrap_or_default(),
    }))
}

// ─── Download Package ───────────────────────────────────────────────────────

pub async fn download_package(
    State(state): State<AppState>,
    Path((name, version)): Path<(String, String)>,
) -> Result<Response, (StatusCode, String)> {
    let pkg = sqlx::query_as::<_, (i64,)>("SELECT id FROM packages WHERE name = ?")
        .bind(&name)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Package not found".into()))?;

    let ver = sqlx::query_as::<_, (i64, String, i64)>(
        "SELECT id, file_hash, file_size FROM versions WHERE package_id = ? AND version = ?",
    )
    .bind(pkg.0)
    .bind(&version)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Version not found".into()))?;

    let file_path = format!("registry-data/packages/{}/{}.sb", name, version);
    let content = std::fs::read(&file_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Verify integrity
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let hash = hex::encode(hasher.finalize());
    if hash != ver.1 {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "File integrity check failed".into(),
        ));
    }

    let filename = format!("{}-{}.sb", name, version);
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{}\"", filename),
            ),
            (header::CONTENT_LENGTH, ver.2.to_string()),
        ],
        content,
    )
        .into_response())
}

// ─── Yank Version ───────────────────────────────────────────────────────────

pub async fn yank_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((name, version)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let auth = AuthenticatedUser::from_header(&state.pool, &headers)
        .await
        .map_err(|e| (e, "Unauthorized".to_string()))?;

    let pkg = sqlx::query_as::<_, (i64, i64)>("SELECT id, owner_id FROM packages WHERE name = ?")
        .bind(&name)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Package not found".into()))?;

    if pkg.1 != auth.user_id {
        return Err((StatusCode::FORBIDDEN, "Not package owner".into()));
    }

    let result = sqlx::query("UPDATE versions SET yanked = 1 WHERE package_id = ? AND version = ?")
        .bind(pkg.0)
        .bind(&version)
        .execute(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Version not found".into()));
    }

    tracing::info!("Yanked {} v{}", name, version);

    Ok(Json(serde_json::json!({
        "ok": true,
        "message": format!("Version {} yanked", version),
    })))
}

// ─── Delete Package ─────────────────────────────────────────────────────────

pub async fn delete_package(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let auth = AuthenticatedUser::from_header(&state.pool, &headers)
        .await
        .map_err(|e| (e, "Unauthorized".to_string()))?;

    let pkg = sqlx::query_as::<_, (i64, i64)>("SELECT id, owner_id FROM packages WHERE name = ?")
        .bind(&name)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Package not found".into()))?;

    if pkg.1 != auth.user_id {
        return Err((StatusCode::FORBIDDEN, "Not package owner".into()));
    }

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query("DELETE FROM versions WHERE package_id = ?")
        .bind(pkg.0)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    sqlx::query("DELETE FROM packages WHERE id = ?")
        .bind(pkg.0)
        .execute(&mut *tx)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tx.commit()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Clean up files
    let dir = format!("registry-data/packages/{}", name);
    let _ = std::fs::remove_dir_all(&dir);

    tracing::info!("Deleted package {}", name);

    Ok(Json(serde_json::json!({
        "ok": true,
        "message": format!("Package {} deleted", name),
    })))
}

// ─── Helper: fetch deps for a version ──────────────────────────────────────

async fn fetch_deps_for_version(
    pool: &sqlx::SqlitePool,
    version_id: i64,
) -> Result<Vec<crate::models::DependencyInfo>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT dep_name, dep_spec FROM dependencies WHERE version_id = ? ORDER BY dep_name",
    )
    .bind(version_id)
    .fetch_all(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|(name, spec)| crate::models::DependencyInfo { name, spec })
        .collect())
}

// ─── Get Package Dependencies ───────────────────────────────────────────────

/// Returns the dependencies declared by a specific package version.
pub async fn get_version_deps(
    State(state): State<AppState>,
    Path((name, version)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let pkg = sqlx::query_as::<_, (i64,)>("SELECT id FROM packages WHERE name = ?")
        .bind(&name)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Package not found".into()))?;

    let ver =
        sqlx::query_as::<_, (i64,)>("SELECT id FROM versions WHERE package_id = ? AND version = ?")
            .bind(pkg.0)
            .bind(&version)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
            .ok_or((StatusCode::NOT_FOUND, "Version not found".into()))?;

    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT dep_name, dep_spec FROM dependencies WHERE version_id = ? ORDER BY dep_name",
    )
    .bind(ver.0)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let deps: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(name, spec)| serde_json::json!({ "name": name, "spec": spec }))
        .collect();

    Ok(Json(serde_json::json!({
        "package": name,
        "version": version,
        "dependencies": deps,
    })))
}

// ─── Key Management ─────────────────────────────────────────────────────────

/// Register an ed25519 public key for the authenticated user.
pub async fn register_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RegisterKeyRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let auth = AuthenticatedUser::from_header(&state.pool, &headers)
        .await
        .map_err(|e| (e, "Unauthorized".to_string()))?;

    // Validate it's a valid hex-encoded 32-byte ed25519 public key
    let key_bytes = hex::decode(&req.public_key)
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid hex encoding".into()))?;
    if key_bytes.len() != 32 {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Public key must be 32 bytes (got {})", key_bytes.len()),
        ));
    }

    sqlx::query("UPDATE users SET ed25519_public_key = ? WHERE id = ?")
        .bind(&req.public_key)
        .bind(auth.user_id)
        .execute(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!("Registered ed25519 key for {}", auth.username);

    Ok(Json(serde_json::json!({
        "ok": true,
        "username": auth.username,
        "public_key": req.public_key,
    })))
}

/// Get the public key for a user.
pub async fn get_user_key(
    State(state): State<AppState>,
    Path(username): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let user = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT username, ed25519_public_key FROM users WHERE username = ?",
    )
    .bind(&username)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "User not found".into()))?;

    match user.1 {
        Some(key) => Ok(Json(serde_json::json!({
            "username": user.0,
            "public_key": key,
        }))),
        None => Err((StatusCode::NOT_FOUND, "No public key registered".into())),
    }
}

/// Verify a package version's signature.
pub async fn verify_signature(
    State(state): State<AppState>,
    Path((name, version)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let pkg = sqlx::query_as::<_, (i64,)>("SELECT id FROM packages WHERE name = ?")
        .bind(&name)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .ok_or((StatusCode::NOT_FOUND, "Package not found".into()))?;

    let ver = sqlx::query_as::<_, (String, Option<String>, i64)>(
        "SELECT checksum, signature, published_by FROM versions WHERE package_id = ? AND version = ?",
    )
    .bind(pkg.0)
    .bind(&version)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Version not found".into()))?;

    let signature = match ver.1 {
        Some(sig) => sig,
        None => {
            // Package is not signed — return JSON with signed=false
            return Ok(Json(serde_json::json!({
                "package": name,
                "version": version,
                "signed": false,
                "valid": false,
                "signed_by": null,
                "public_key": null,
            })));
        }
    };

    // Get the publisher's public key
    let user = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT username, ed25519_public_key FROM users WHERE id = ?",
    )
    .bind(ver.2)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let (username, public_key_hex) = user.ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Publisher not found".into(),
    ))?;

    let public_key_hex = match public_key_hex {
        Some(key) => key,
        None => {
            return Ok(Json(serde_json::json!({
                "package": name,
                "version": version,
                "signed": false,
                "valid": false,
                "signed_by": username,
                "public_key": null,
            })));
        }
    };

    // Decode key and signature
    let key_bytes = hex::decode(&public_key_hex).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Invalid public key format".into(),
        )
    })?;
    let sig_bytes = hex::decode(&signature).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Invalid signature format".into(),
        )
    })?;

    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    let public_key_arr: [u8; 32] = key_bytes.try_into().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Invalid key length".into(),
        )
    })?;
    let signature_arr: [u8; 64] = sig_bytes.try_into().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Invalid signature length".into(),
        )
    })?;

    let verifying_key = VerifyingKey::from_bytes(&public_key_arr).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Invalid public key: {}", e),
        )
    })?;
    let signature = Signature::from_bytes(&signature_arr);

    // Verify: the signature signs the checksum string itself (e.g. "sha256:abc123...")
    let valid = verifying_key.verify(ver.0.as_bytes(), &signature).is_ok();

    Ok(Json(serde_json::json!({
        "package": name,
        "version": version,
        "signed": true,
        "valid": valid,
        "signed_by": username,
        "public_key": public_key_hex,
    })))
}
