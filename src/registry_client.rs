use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Default registry URL
const DEFAULT_REGISTRY: &str = "https://registry.sandbox.dev";

/// Get the registry URL from env or default
fn registry_url() -> String {
    std::env::var("SANDBOX_REGISTRY").unwrap_or_else(|_| DEFAULT_REGISTRY.to_string())
}

/// Config file path (~/.sandbox/config.toml)
fn config_path() -> PathBuf {
    dirs().join("config.toml")
}

fn dirs() -> PathBuf {
    if let Some(home) = dirs_next::home_dir() {
        home.join(".sandbox")
    } else {
        PathBuf::from(".sandbox")
    }
}

/// Read saved API key from config
pub fn get_api_key() -> Result<String> {
    let config_path = config_path();
    if !config_path.exists() {
        return Err(anyhow!(
            "Not logged in. Run `sandbox pkg login` first."
        ));
    }
    let content = fs::read_to_string(&config_path)?;
    let config: HashMap<String, String> = toml::from_str(&content)?;
    config
        .get("api_key")
        .cloned()
        .ok_or_else(|| anyhow!("No API key found. Run `sandbox pkg login`."))
}

/// Save API key to config
fn save_api_key(api_key: &str) -> Result<()> {
    let config_path = config_path();
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = format!("api_key = \"{}\"\n", api_key);
    fs::write(&config_path, content)?;
    Ok(())
}

/// Simple HTTP GET request using stdlib only
fn http_get(url: &str) -> Result<String> {
    let url = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(|| anyhow!("Invalid URL: {}", url))?;

    let (host_port, path) = match url.split_once('/') {
        Some((hp, p)) => (hp, format!("/{}", p)),
        None => (url, "/".to_string()),
    };

    let (host, port) = match host_port.split_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().unwrap_or(443)),
        None => (host_port, 443),
    };

    let addr = format!("{}:{}", host, port);
    let mut stream = std::net::TcpStream::connect(&addr)
        .map_err(|e| anyhow!("Cannot connect to {}: {}", addr, e))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .ok();

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: sandbox-cli/2.0\r\nConnection: close\r\n\r\n",
        path, host
    );
    stream.write_all(request.as_bytes())?;

    use std::io::Read;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf)?;
    let resp = String::from_utf8_lossy(&buf);

    let status = resp
        .lines()
        .next()
        .unwrap_or("HTTP/1.1 000")
        .split_whitespace()
        .nth(1)
        .unwrap_or("000");

    if status != "200" {
        return Err(anyhow!("HTTP {}: registry returned error", status));
    }

    let body = resp
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or("")
        .to_string();

    Ok(body)
}

/// Simple HTTP POST with JSON body
fn http_post_json(url: &str, body: &str, api_key: Option<&str>) -> Result<(u16, String)> {
    let url = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(|| anyhow!("Invalid URL: {}", url))?;

    let (host_port, path) = match url.split_once('/') {
        Some((hp, p)) => (hp, format!("/{}", p)),
        None => (url, "/".to_string()),
    };

    let (host, port) = match host_port.split_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().unwrap_or(443)),
        None => (host_port, 443),
    };

    let addr = format!("{}:{}", host, port);
    let mut stream = std::net::TcpStream::connect(&addr)
        .map_err(|e| anyhow!("Cannot connect to {}: {}", addr, e))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .ok();

    let mut headers = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nUser-Agent: sandbox-cli/2.0\r\nConnection: close",
        path,
        host,
        body.len()
    );

    if let Some(key) = api_key {
        headers.push_str(&format!("\r\nAuthorization: Bearer {}", key));
    }
    headers.push_str("\r\n\r\n");

    let request = format!("{}{}", headers, body);
    stream.write_all(request.as_bytes())?;

    use std::io::Read;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf)?;
    let resp = String::from_utf8_lossy(&buf);

    let status_line = resp
        .lines()
        .next()
        .unwrap_or("HTTP/1.1 000")
        .to_string();
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("000")
        .parse::<u16>()
        .unwrap_or(0);

    let resp_body = resp
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or("")
        .to_string();

    Ok((status_code, resp_body))
}

/// Multipart POST for publishing packages
fn http_post_multipart(url: &str, fields: &HashMap<String, String>, file_field: &str, file_data: &[u8], file_name: &str, api_key: &str) -> Result<(u16, String)> {
    let boundary = format!("----SandboxBoundary{:x}", rand::random::<u64>());

    let mut body = Vec::new();

    // Add text fields
    for (name, value) in fields {
        write!(body, "--{}\r\n", boundary)?;
        write!(body, "Content-Disposition: form-data; name=\"{}\"\r\n\r\n", name)?;
        write!(body, "{}\r\n", value)?;
    }

    // Add file field
    write!(body, "--{}\r\n", boundary)?;
    write!(
        body,
        "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
        file_field, file_name
    )?;
    write!(body, "Content-Type: application/octet-stream\r\n\r\n")?;
    body.extend_from_slice(file_data);
    write!(body, "\r\n")?;
    write!(body, "--{}--\r\n", boundary)?;

    let url_stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(|| anyhow!("Invalid URL"))?;

    let (host_port, path) = match url_stripped.split_once('/') {
        Some((hp, p)) => (hp, format!("/{}", p)),
        None => (url_stripped, "/".to_string()),
    };

    let (host, port) = match host_port.split_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().unwrap_or(443)),
        None => (host_port, 443),
    };

    let addr = format!("{}:{}", host, port);
    let mut stream = std::net::TcpStream::connect(&addr)
        .map_err(|e| anyhow!("Cannot connect to {}: {}", addr, e))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(30)))
        .ok();

    let headers = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: multipart/form-data; boundary={}\r\nContent-Length: {}\r\nAuthorization: Bearer {}\r\nUser-Agent: sandbox-cli/2.0\r\nConnection: close\r\n\r\n",
        path, host, boundary, body.len(), api_key
    );

    stream.write_all(headers.as_bytes())?;
    stream.write_all(&body)?;

    use std::io::Read;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf)?;
    let resp = String::from_utf8_lossy(&buf);

    let status_code = resp
        .lines()
        .next()
        .unwrap_or("HTTP/1.1 000")
        .split_whitespace()
        .nth(1)
        .unwrap_or("000")
        .parse::<u16>()
        .unwrap_or(0);

    let resp_body = resp
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or("")
        .to_string();

    Ok((status_code, resp_body))
}

// ── Package Fetch/Download (used by `sandbox install`) ──

/// Get package info from the registry.
pub fn fetch_package_info(name: &str) -> Result<serde_json::Value> {
    let registry = registry_url();
    let url = format!("{}/api/v1/packages/{}", registry, name);
    let body = http_get(&url)?;
    let val: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| anyhow!("Invalid registry response for {}: {}", name, e))?;
    Ok(val)
}

/// Download a specific package version from the registry.
pub fn download_package_bytes(name: &str, version: &str) -> Result<Vec<u8>> {
    let registry = registry_url();
    let url = format!("{}/api/v1/packages/{}/{}/download", registry, name, version);

    // Parse URL for raw TCP request
    let url_stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(|| anyhow!("Invalid URL"))?;

    let (host_port, path) = match url_stripped.split_once('/') {
        Some((hp, p)) => (hp, format!("/{}", p)),
        None => (url_stripped, "/".to_string()),
    };

    let (host, port) = match host_port.split_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().unwrap_or(443)),
        None => (host_port, 443),
    };

    let addr = format!("{}:{}", host, port);
    let mut stream = std::net::TcpStream::connect(&addr)
        .map_err(|e| anyhow!("Cannot connect to {}: {}", addr, e))?;
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .ok();

    let request = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: sandbox-cli/2.0\r\nConnection: close\r\n\r\n",
        path, host
    );
    use std::io::Write;
    stream.write_all(request.as_bytes())?;

    use std::io::Read;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf)?;
    let resp = String::from_utf8_lossy(&buf);

    let status = resp
        .lines()
        .next()
        .unwrap_or("HTTP/1.1 000")
        .split_whitespace()
        .nth(1)
        .unwrap_or("000");

    if status != "200" {
        return Err(anyhow!("HTTP {}: failed to download {} v{}", status, name, version));
    }

    // Split headers and body (binary)
    let header_end = buf.windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(0);
    let body = buf[header_end + 4..].to_vec();

    if body.is_empty() {
        return Err(anyhow!("Registry returned empty package body"));
    }

    Ok(body)
}

/// Resolve a version specifier (e.g. "^1.0", "~1.2", "1.0.0") against available versions.
/// Returns the best matching version string.
pub fn resolve_version(name: &str, specifier: &str) -> Result<String> {
    let info = fetch_package_info(name)?;
    let versions = info["versions"]
        .as_array()
        .ok_or_else(|| anyhow!("No versions available for {}", name))?;

    let available: Vec<String> = versions
        .iter()
        .filter_map(|v| {
            if v["yanked"].as_bool().unwrap_or(false) {
                None
            } else {
                v["version"].as_str().map(|s| s.to_string())
            }
        })
        .collect();

    if available.is_empty() {
        return Err(anyhow!("No non-yanked versions available for {}", name));
    }

    // Exact match
    if specifier == "*" || specifier.is_empty() {
        // Return latest (first in list, already sorted desc)
        return available
            .first()
            .cloned()
            .ok_or_else(|| anyhow!("No versions"));
    }

    // Try exact version match first
    if available.iter().any(|v| v == specifier) {
        return Ok(specifier.to_string());
    }

    // Parse prefix version specifiers: ^1.0, ~1.2, >=1.0
    let (op, ver_str) = if let Some(v) = specifier.strip_prefix('^') {
        ("^", v)
    } else if let Some(v) = specifier.strip_prefix('~') {
        ("~", v)
    } else if let Some(v) = specifier.strip_prefix(">=") {
        (">=", v)
    } else if let Some(v) = specifier.strip_prefix('>') {
        (">", v)
    } else if let Some(v) = specifier.strip_prefix('<') {
        ("<", v)
    } else if let Some(v) = specifier.strip_prefix("<=") {
        ("<=", v)
    } else {
        // Plain version like "1.0" — treat as "^1.0"
        ("^", specifier)
    };

    let ver_parts: Vec<u32> = ver_str
        .split('.')
        .filter_map(|p| p.parse().ok())
        .collect();

    let matches = |v: &str| -> bool {
        let v_parts: Vec<u32> = v.split('.').filter_map(|p| p.parse().ok()).collect();
        match op {
            "^" => {
                // Compatible: same major, >= minor.patch
                if v_parts.get(0) != ver_parts.get(0) {
                    return false;
                }
                v_parts >= ver_parts
            }
            "~" => {
                // Approximate: same major.minor, >= patch
                if v_parts.get(0) != ver_parts.get(0) {
                    return false;
                }
                if ver_parts.len() > 1 && v_parts.get(1) != ver_parts.get(1) {
                    return false;
                }
                v_parts >= ver_parts
            }
            ">=" => v_parts >= ver_parts,
            ">" => v_parts > ver_parts,
            "<" => v_parts < ver_parts,
            "<=" => v_parts <= ver_parts,
            _ => false,
        }
    };

    // Find best match (highest version that matches)
    let best = available.iter().find(|v| matches(v));
    best.cloned()
        .ok_or_else(|| anyhow!("No version matching '{}' for {}", specifier, name))
}

// ── CLI Commands ──

/// `sandbox pkg login`
pub fn pkg_login() -> Result<()> {
    println!("🔐 Sandbox Registry Login");
    println!();

    print!("Username: ");
    std::io::stdout().flush()?;
    let mut username = String::new();
    std::io::stdin().read_line(&mut username)?;
    let username = username.trim().to_string();

    print!("Password: ");
    std::io::stdout().flush()?;

    // Simple password input (no echo)
    let password = rpassword::prompt_password("")?;

    let login_body = serde_json::json!({
        "username": username,
        "password": password,
    });

    let registry = registry_url();
    let url = format!("{}/api/v1/auth/login", registry);

    let (status, body) = http_post_json(&url, &login_body.to_string(), None)?;

    if status == 200 {
        let resp: serde_json::Value = serde_json::from_str(&body)?;
        let api_key = resp["api_key"].as_str().ok_or_else(|| anyhow!("Invalid response"))?;
        save_api_key(api_key)?;
        println!("✅ Logged in as {}", resp["username"].as_str().unwrap_or(&username));
    } else {
        let resp: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        let msg = resp.get("message").and_then(|m| m.as_str()).unwrap_or("Login failed");
        println!("❌ Login failed: {}", msg);
    }

    Ok(())
}

/// `sandbox pkg register`
pub fn pkg_register() -> Result<()> {
    println!("📝 Create Account on Sandbox Registry");
    println!();

    print!("Username: ");
    std::io::stdout().flush()?;
    let mut username = String::new();
    std::io::stdin().read_line(&mut username)?;
    let username = username.trim().to_string();

    print!("Email: ");
    std::io::stdout().flush()?;
    let mut email = String::new();
    std::io::stdin().read_line(&mut email)?;
    let email = email.trim().to_string();

    print!("Password: ");
    std::io::stdout().flush()?;
    let password = rpassword::prompt_password("")?;

    let register_body = serde_json::json!({
        "username": username,
        "email": email,
        "password": password,
    });

    let registry = registry_url();
    let url = format!("{}/api/v1/users/register", registry);

    let (status, body) = http_post_json(&url, &register_body.to_string(), None)?;

    if status == 200 {
        let resp: serde_json::Value = serde_json::from_str(&body)?;
        let api_key = resp["api_key"].as_str().ok_or_else(|| anyhow!("Invalid response"))?;
        save_api_key(api_key)?;
        println!("✅ Account created! Logged in as {}", username);
    } else {
        let resp: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        let msg = resp.get("message").and_then(|m| m.as_str()).unwrap_or("Registration failed");
        println!("❌ Registration failed: {}", msg);
    }

    Ok(())
}

/// `sandbox pkg publish <file>`
pub fn pkg_publish(file_path: &str) -> Result<()> {
    let api_key = get_api_key()?;

    println!("📦 Publishing package...");

    let content = fs::read(file_path)
        .map_err(|e| anyhow!("Failed to read {}: {}", file_path, e))?;

    // Try to parse the file for metadata
    // For now, require sandbox.toml for metadata
    let toml_content = fs::read_to_string("sandbox.toml")
        .map_err(|_| anyhow!("No sandbox.toml found. Run `sandbox init` first."))?;
    let config: SandboxToml = toml::from_str(&toml_content)?;

    if config.package.name.is_empty() {
        return Err(anyhow!("Package name not set in sandbox.toml"));
    }

    let mut fields = std::collections::HashMap::new();
    fields.insert("name".to_string(), config.package.name.clone());
    fields.insert("version".to_string(), config.package.version.clone());
    if !config.package.description.is_empty() {
        fields.insert("description".to_string(), config.package.description.clone());
    }
    // Send dependencies as a JSON array
    if !config.dependencies.is_empty() {
        let deps_array: Vec<serde_json::Value> = config.dependencies.iter()
            .map(|(name, spec)| serde_json::json!({ "name": name, "spec": spec }))
            .collect();
        fields.insert("deps".to_string(), serde_json::to_string(&deps_array).unwrap_or_default());
    }

    // Compute checksum first (server verifies signature against this)
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(&content);
    let checksum = format!("sha256:{}", hex::encode(hasher.finalize()));

    // Sign the checksum string if a private key exists
    if keys_dir().join("private.key").exists() {
        match sign_bytes(checksum.as_bytes()) {
            Ok(sig) => {
                fields.insert("signature".to_string(), sig);
                println!("   🔏 Package signed with ed25519");
            }
            Err(e) => {
                println!("   ⚠ Could not sign package: {}", e);
            }
        }
    } else {
        println!("   ℹ No signing key found. Run `sandbox pkg keygen` to sign packages.");
    }

    let file_name = std::path::Path::new(file_path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "package.sbx".to_string());

    let registry = registry_url();
    let url = format!("{}/api/v1/packages", registry);

    let (status, body) = http_post_multipart(&url, &fields, "content", &content, &file_name, &api_key)?;

    if status == 200 || status == 201 {
        let resp: serde_json::Value = serde_json::from_str(&body)?;
        println!("✅ Published {} v{}", config.package.name, config.package.version);
        println!("   Checksum: {}", resp["checksum"].as_str().unwrap_or("unknown"));
    } else {
        let resp: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        let msg = resp.get("message").and_then(|m| m.as_str()).unwrap_or("Publish failed");
        println!("❌ Publish failed (HTTP {}): {}", status, msg);
    }

    Ok(())
}

/// `sandbox pkg search <query>`
pub fn pkg_search(query: &str) -> Result<()> {
    let registry = registry_url();
    let url = if query.is_empty() {
        format!("{}/api/v1/packages", registry)
    } else {
        format!("{}/api/v1/packages?q={}", registry, query)
    };

    let body = http_get(&url)?;
    let data: serde_json::Value = serde_json::from_str(&body)?;

    let packages = data["packages"]
        .as_array()
        .ok_or_else(|| anyhow!("Invalid response"))?;

    if packages.is_empty() {
        if query.is_empty() {
            println!("📦 No packages published yet");
        } else {
            println!("📦 No packages found for '{}'", query);
        }
        return Ok(());
    }

    println!("📦 {} package{} found:\n", packages.len(), if packages.len() == 1 { "" } else { "s" });
    for pkg in packages {
        let name = pkg["name"].as_str().unwrap_or("?");
        let version = pkg["latest_version"].as_str().unwrap_or("?");
        let desc = pkg["description"].as_str().unwrap_or("No description");
        println!("  {} v{}", name, version);
        println!("    {}", desc);
        println!();
    }

    Ok(())
}

/// `sandbox pkg info <name>`
pub fn pkg_info(name: &str) -> Result<()> {
    let registry = registry_url();
    let url = format!("{}/api/v1/packages/{}", registry, name);

    let body = http_get(&url)?;
    let pkg: serde_json::Value = serde_json::from_str(&body)?;

    let name = pkg["name"].as_str().unwrap_or("?");
    let desc = pkg["description"].as_str().unwrap_or("No description");
    let latest = pkg["latest_version"].as_str().unwrap_or("none");
    let default_versions = vec![];
    let versions = pkg["versions"].as_array().unwrap_or(&default_versions);

    println!("📦 {}", name);
    println!("   {}", desc);
    println!("   Latest: v{}", latest);
    println!();
    println!("   Versions ({}):", versions.len());
    for v in versions {
        let ver = v["version"].as_str().unwrap_or("?");
        let yanked = v["yanked"].as_bool().unwrap_or(false);
        let signed = v.get("signature").and_then(|s| s.as_str()).is_some();
        let signer = v.get("signed_by").and_then(|s| s.as_str()).unwrap_or("");
        let mut suffix = String::new();
        if yanked { suffix.push_str(" (yanked)"); }
        if signed { suffix.push_str(&format!(" 🔏 signed by {}", signer)); }
        println!("     v{}{}", ver, suffix);
    }

    Ok(())
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct SandboxToml {
    #[serde(default)]
    package: PkgInfo,
    #[serde(default)]
    dependencies: HashMap<String, String>,
}

/// `sandbox pkg init [name]` — scaffold a new publishable package.
pub fn pkg_init(name_arg: &str) -> Result<()> {
    // Determine package name from arg or current directory
    let name = if name_arg.is_empty() {
        std::env::current_dir()
            .ok()
            .and_then(|d| d.file_name().map(|f| f.to_string_lossy().to_string()))
            .unwrap_or_else(|| "my-package".to_string())
    } else {
        name_arg.to_string()
    };

    println!("📦 Scaffolding package '{}'", name);

    // Create directory structure
    fs::create_dir_all("src")?;

    // Write sandbox.toml
    let toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
description = "A Sandbox package"
entry_point = "src/main.sbx"

[dependencies]
# Add dependencies here, e.g.:
# my-lib = "^1.0"
"#,
        name
    );
    fs::write("sandbox.toml", &toml)?;

    // Write src/main.sbx
    let main_sbx = format!(
        r#"// {}/src/main.sbx
// Entry point for the package

fn main() {{
    print("Hello from {}!")
}}
"#,
        name, name
    );
    fs::write("src/main.sbx", &main_sbx)?;

    // Write .gitignore
    fs::write(".gitignore", ".sandbox/
*.sb
")?;

    println!("✅ Created sandbox.toml");
    println!("✅ Created src/main.sbx");
    println!("✅ Created .gitignore");
    println!();
    println!("Next steps:");
    println!("  1. Edit src/main.sbx with your code");
    println!("  2. Add dependencies to sandbox.toml");
    println!("  3. sandbox run src/main.sbx   # test locally");
    println!("  4. sandbox pkg publish src/main.sbx  # publish to registry");

    Ok(())
}

/// ── Key Management & Signing ──

/// Path to the keys directory (~/.sandbox/keys/)
fn keys_dir() -> PathBuf {
    dirs().join("keys")
}

/// `sandbox pkg keygen` — generate an ed25519 keypair.
pub fn pkg_keygen() -> Result<()> {
    use ed25519_dalek::{SigningKey};
    use rand::rngs::OsRng;

    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    let key_dir = keys_dir();
    fs::create_dir_all(&key_dir)?;

    let private_key_path = key_dir.join("private.key");
    let public_key_path = key_dir.join("public.key");

    // Save private key (hex-encoded 64 bytes = seed + public)
    let private_hex = hex::encode(signing_key.to_bytes());
    fs::write(&private_key_path, &private_hex)?;

    // Save public key (hex-encoded 32 bytes)
    let public_hex = hex::encode(verifying_key.as_bytes());
    fs::write(&public_key_path, &public_hex)?;

    println!("🔑 Generated ed25519 keypair");
    println!("   Private key: {}", private_key_path.display());
    println!("   Public key:  {}", public_key_path.display());
    println!();
    println!("⚠  Keep your private key secure! Never commit it to version control.");
    println!("   Register your public key with: sandbox pkg keys");

    Ok(())
}

/// Read the private key from disk.
fn read_private_key() -> Result<ed25519_dalek::SigningKey> {
    let path = keys_dir().join("private.key");
    let hex_key = fs::read_to_string(&path)
        .map_err(|_| anyhow!("No private key found at {}. Run `sandbox pkg keygen` first.", path.display()))?;
    let bytes = hex::decode(hex_key.trim())
        .map_err(|e| anyhow!("Invalid private key format: {}", e))?;
    let arr: [u8; 32] = bytes.try_into()
        .map_err(|_| anyhow!("Private key must be 32 bytes"))?;
    Ok(ed25519_dalek::SigningKey::from_bytes(&arr))
}

/// Sign a message with the private key, return hex-encoded signature.
pub fn sign_bytes(data: &[u8]) -> Result<String> {
    use ed25519_dalek::{Signer};
    let signing_key = read_private_key()?;
    let signature = signing_key.sign(data);
    Ok(hex::encode(signature.to_bytes()))
}

/// `sandbox pkg keys` — register public key with the registry.
pub fn pkg_keys_register() -> Result<()> {
    let api_key = get_api_key()?;

    let public_key_path = keys_dir().join("public.key");
    let public_hex = fs::read_to_string(&public_key_path)
        .map_err(|_| anyhow!("No public key found at {}. Run `sandbox pkg keygen` first.", public_key_path.display()))?;
    let public_hex = public_hex.trim().to_string();

    let registry = registry_url();
    let url = format!("{}/api/v1/users/keys", registry);
    let body = serde_json::json!({ "public_key": public_hex });

    let (status, resp_body) = http_post_json(&url, &body.to_string(), Some(&api_key))?;

    if status == 200 {
        println!("✅ Public key registered successfully");
        println!("   Key: {}...{}", &public_hex[..8], &public_hex[public_hex.len()-8..]);
    } else {
        let resp: serde_json::Value = serde_json::from_str(&resp_body).unwrap_or_default();
        let msg = resp.get("message").and_then(|m| m.as_str()).unwrap_or("Key registration failed");
        println!("❌ Key registration failed (HTTP {}): {}", status, msg);
    }

    Ok(())
}

/// Result of a signature verification check.
pub struct SignatureStatus {
    pub signed: bool,
    pub valid: bool,
    pub signed_by: String,
}

/// Verify a package's ed25519 signature via the registry (non-printing, for programmatic use).
pub fn verify_package_signature(name: &str, version: &str) -> Result<SignatureStatus> {
    let registry = registry_url();
    let url = format!("{}/api/v1/packages/{}/{}/verify", registry, name, version);

    let body = http_get(&url)?;
    let resp: serde_json::Value = serde_json::from_str(&body)?;

    Ok(SignatureStatus {
        signed: resp["signed"].as_bool().unwrap_or(false),
        valid: resp["valid"].as_bool().unwrap_or(false),
        signed_by: resp["signed_by"].as_str().unwrap_or("unknown").to_string(),
    })
}

/// `sandbox pkg verify <package> <version>` — verify a package's signature (user-facing).
pub fn pkg_verify(name: &str, version: &str) -> Result<()> {
    let status = verify_package_signature(name, version)?;

    if !status.signed {
        println!("⚠  {} v{} is NOT signed", name, version);
        return Ok(());
    }

    if status.valid {
        println!("✅ {} v{} signature is VALID", name, version);
        println!("   Signed by: {}", status.signed_by);
    } else {
        println!("❌ {} v{} signature is INVALID", name, version);
        println!("   This could indicate a tampered package!");
        println!("   Signed by: {}", status.signed_by);
    }

    Ok(())
}

/// Fetch the manifest (sandbox.toml) for a specific package version from the registry.
pub fn fetch_package_manifest(name: &str, version: &str) -> Result<Option<String>> {
    // The manifest is embedded in the package content.
    // For now, we fetch the package info which includes a description.
    // A proper implementation would fetch the manifest separately.
    let info = fetch_package_info(name)?;
    let default_versions = vec![];
    let versions = info["versions"].as_array().unwrap_or(&default_versions);
    let version_info = versions.iter().find(|v| v["version"].as_str() == Some(version));
    match version_info {
        Some(v) => Ok(v["description"].as_str().map(|s| s.to_string())),
        None => Ok(None),
    }
}

/// Fetch the declared dependencies of a specific package version from the registry.
/// Returns a list of (dep_name, dep_spec) pairs.
pub fn fetch_package_deps(name: &str, version: &str) -> Result<Vec<(String, String)>> {
    let registry = registry_url();
    let url = format!("{}/api/v1/packages/{}/{}/deps", registry, name, version);
    let body = http_get(&url)?;
    let val: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| anyhow!("Invalid deps response for {} v{}: {}", name, version, e))?;

    let deps_array = val["dependencies"].as_array()
        .ok_or_else(|| anyhow!("Missing 'dependencies' in response for {} v{}", name, version))?;

    let mut result = Vec::new();
    for dep in deps_array {
        if let (Some(n), Some(s)) = (dep["name"].as_str(), dep["spec"].as_str()) {
            result.push((n.to_string(), s.to_string()));
        }
    }
    Ok(result)
}

/// Resolve all dependencies for a package, including transitive ones.
/// Returns a list of (name, resolved_version, checksum) in topological order.
pub fn resolve_all_dependencies(
    deps: &HashMap<String, String>,
) -> Result<Vec<(String, String, String)>> {
    let mut resolved: Vec<(String, String, String)> = Vec::new();
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut resolving: std::collections::HashSet<String> = std::collections::HashSet::new();

    fn resolve_recursive(
        name: &str,
        spec: &str,
        resolved: &mut Vec<(String, String, String)>,
        visited: &mut std::collections::HashSet<String>,
        resolving: &mut std::collections::HashSet<String>,
    ) -> Result<()> {
        if visited.contains(name) {
            return Ok(());
        }

        if resolving.contains(name) {
            return Err(anyhow!("Circular dependency detected: {}", name));
        }
        resolving.insert(name.to_string());

        let version = resolve_version(name, spec)?;
        let info = fetch_package_info(name)?;
        let checksum = info["versions"]
            .as_array()
            .and_then(|versions| {
                versions.iter()
                    .find(|v| v["version"].as_str() == Some(&version))
                    .and_then(|v| v["checksum"].as_str().map(|s| s.to_string()))
            })
            .unwrap_or_else(|| "unknown".to_string());

        // Fetch and resolve transitive dependencies
        match fetch_package_deps(name, &version) {
            Ok(transitive_deps) => {
                for (dep_name, dep_spec) in &transitive_deps {
                    println!("  → {} depends on {} {}", name, dep_name, dep_spec);
                    resolve_recursive(dep_name, dep_spec, resolved, visited, resolving)?;
                }
            }
            Err(_) => {
                // Package has no deps or the endpoint isn't available — that's fine
            }
        }

        resolving.remove(name);
        visited.insert(name.to_string());
        resolved.push((name.to_string(), version, checksum));

        Ok(())
    }

    for (name, spec) in deps {
        resolve_recursive(name, spec, &mut resolved, &mut visited, &mut resolving)?;
    }

    Ok(resolved)
}

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct PkgInfo {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
}
