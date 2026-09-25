pub mod auth;
pub mod db;
pub mod handlers;
pub mod models;
pub mod ratelimit;

use axum::Router;
use sqlx::SqlitePool;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::ratelimit::{RateLimitConfig, RateLimitLayer};

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: RateLimitConfig,
}

/// Build the Axum application router with the given database pool and default config.
pub fn build_app(pool: SqlitePool) -> Router {
    build_app_with_config(pool, RateLimitConfig::from_env())
}

/// Build the Axum application router with explicit config.
pub fn build_app_with_config(pool: SqlitePool, config: RateLimitConfig) -> Router {
    let state = AppState {
        pool,
        config: config.clone(),
    };

    Router::new()
        .route("/", axum::routing::get(handlers::dashboard))
        .route("/playground", axum::routing::get(handlers::playground_page))
        .route(
            "/playground/playground.js",
            axum::routing::get(handlers::playground_js),
        )
        .route(
            "/playground/compiler.wasm",
            axum::routing::get(handlers::playground_wasm),
        )
        .route(
            "/packages/:name",
            axum::routing::get(handlers::package_page),
        )
        .route("/health", axum::routing::get(handlers::health))
        .route(
            "/api/v1/packages",
            axum::routing::get(handlers::search_packages),
        )
        .route(
            "/api/v1/packages/:name",
            axum::routing::get(handlers::get_package),
        )
        .route(
            "/api/v1/packages/:name/:version/download",
            axum::routing::get(handlers::download_package),
        )
        .route(
            "/api/v1/packages/:name/:version/deps",
            axum::routing::get(handlers::get_version_deps),
        )
        .route(
            "/api/v1/users/register",
            axum::routing::post(handlers::register),
        )
        .route("/api/v1/auth/login", axum::routing::post(handlers::login))
        .route(
            "/api/v1/packages",
            axum::routing::post(handlers::publish_package),
        )
        .route(
            "/api/v1/packages/:name",
            axum::routing::delete(handlers::delete_package),
        )
        .route(
            "/api/v1/packages/:name/:version/yank",
            axum::routing::post(handlers::yank_version),
        )
        .route(
            "/api/v1/users/keys",
            axum::routing::post(handlers::register_key),
        )
        .route(
            "/api/v1/users/:username/keys",
            axum::routing::get(handlers::get_user_key),
        )
        .route(
            "/api/v1/packages/:name/:version/verify",
            axum::routing::get(handlers::verify_signature),
        )
        .layer(RateLimitLayer::new(&config))
        .layer(TraceLayer::new_for_http())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}
