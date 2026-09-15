use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Instant;

use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;
use tower::{Layer, Service};

// ── Configuration ───────────────────────────────────────────────────────────

/// Rate-limit config read from environment variables.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Max requests allowed in the window.
    pub max_requests: u64,
    /// Window duration in seconds.
    pub window_secs: u64,
    /// Max upload size in bytes (used for publish endpoint).
    pub max_upload_bytes: usize,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 60,
            window_secs: 60,
            max_upload_bytes: 50 * 1024 * 1024, // 50 MB
        }
    }
}

impl RateLimitConfig {
    pub fn from_env() -> Self {
        let max_requests = std::env::var("REGISTRY_RATE_LIMIT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        let window_secs = std::env::var("REGISTRY_RATE_WINDOW")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60);

        let max_upload_mb = std::env::var("REGISTRY_MAX_UPLOAD_MB")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(50);

        Self {
            max_requests,
            window_secs,
            max_upload_bytes: max_upload_mb * 1024 * 1024,
        }
    }
}

// ── Sliding-window counter ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct WindowEntry {
    timestamps: Vec<Instant>,
}

#[derive(Clone)]
struct Limiter {
    windows: Arc<Mutex<HashMap<String, WindowEntry>>>,
    max_requests: u64,
    window_secs: u64,
}

impl Limiter {
    fn new(max_requests: u64, window_secs: u64) -> Self {
        Self {
            windows: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window_secs,
        }
    }

    /// Returns `true` if the request is allowed, `false` if rate-limited.
    fn check(&self, key: &str) -> bool {
        let mut windows = self.windows.lock().unwrap();
        let now = Instant::now();
        let window_start = now - std::time::Duration::from_secs(self.window_secs);

        let entry = windows
            .entry(key.to_string())
            .or_insert_with(|| WindowEntry {
                timestamps: Vec::new(),
            });

        // Evict timestamps outside the window.
        entry.timestamps.retain(|t| *t > window_start);

        if entry.timestamps.len() as u64 >= self.max_requests {
            return false;
        }

        entry.timestamps.push(now);
        true
    }
}

// ── Tower Layer / Service ───────────────────────────────────────────────────

#[derive(Clone)]
pub struct RateLimitLayer {
    limiter: Limiter,
}

impl RateLimitLayer {
    pub fn new(config: &RateLimitConfig) -> Self {
        Self {
            limiter: Limiter::new(config.max_requests, config.window_secs),
        }
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            limiter: self.limiter.clone(),
        }
    }
}

#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    limiter: Limiter,
}

impl<S, ReqBody> Service<axum::http::Request<ReqBody>> for RateLimitService<S>
where
    S: Service<axum::http::Request<ReqBody>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: axum::http::Request<ReqBody>) -> Self::Future {
        let key = extract_client_ip(&req);
        let allowed = self.limiter.check(&key);

        if !allowed {
            let mut response = Response::new(axum::body::Body::empty());
            *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            if let Ok(val) = HeaderValue::from_str("60") {
                response.headers_mut().insert("retry-after", val);
            }
            return Box::pin(async { Ok(response) });
        }

        let mut inner = self.clone();
        Box::pin(async move { inner.inner.call(req).await })
    }
}

fn extract_client_ip<B>(req: &axum::http::Request<B>) -> String {
    // Check X-Forwarded-For first (for reverse proxies).
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(s) = forwarded.to_str() {
            if let Some(first) = s.split(',').next() {
                let ip = first.trim();
                if !ip.is_empty() {
                    return ip.to_string();
                }
            }
        }
    }

    // Check X-Real-IP.
    if let Some(real_ip) = req.headers().get("x-real-ip") {
        if let Ok(s) = real_ip.to_str() {
            return s.to_string();
        }
    }

    // Fall back to socket address if available.
    req.extensions()
        .get::<std::net::SocketAddr>()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_within_window() {
        let limiter = Limiter::new(3, 10);
        assert!(limiter.check("127.0.0.1"));
        assert!(limiter.check("127.0.0.1"));
        assert!(limiter.check("127.0.0.1"));
    }

    #[test]
    fn blocks_when_exceeded() {
        let limiter = Limiter::new(2, 10);
        assert!(limiter.check("10.0.0.1"));
        assert!(limiter.check("10.0.0.1"));
        assert!(!limiter.check("10.0.0.1"));
    }

    #[test]
    fn different_ips_are_independent() {
        let limiter = Limiter::new(1, 10);
        assert!(limiter.check("1.1.1.1"));
        assert!(!limiter.check("1.1.1.1"));
        assert!(limiter.check("2.2.2.2"));
    }

    #[test]
    fn window_expiry_allows_new_requests() {
        let limiter = Limiter::new(1, 0); // 0-second window = instant expiry
        assert!(limiter.check("test"));
        // Next check should see the window has expired.
        assert!(limiter.check("test"));
    }

    #[test]
    fn config_from_env_defaults() {
        let config = RateLimitConfig::default();
        assert_eq!(config.max_requests, 60);
        assert_eq!(config.window_secs, 60);
        assert_eq!(config.max_upload_bytes, 50 * 1024 * 1024);
    }
}
