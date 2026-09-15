use sandbox_registry::ratelimit::RateLimitConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let db_path =
        std::env::var("REGISTRY_DB").unwrap_or_else(|_| "registry-data/registry.db".into());
    let addr = std::env::var("REGISTRY_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".into());

    let pool = sandbox_registry::db::init_db(&db_path).await?;
    let config = RateLimitConfig::from_env();
    let app = sandbox_registry::build_app_with_config(pool, config.clone());

    tracing::info!("Sandbox Registry starting on {}", addr);
    tracing::info!(
        "Rate limit: {} requests per {}s",
        config.max_requests,
        config.window_secs
    );
    tracing::info!(
        "Max upload size: {} MB",
        config.max_upload_bytes / (1024 * 1024)
    );
    tracing::info!("Dashboard: http://{}", addr);
    tracing::info!("API: http://{}/api/v1/", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
