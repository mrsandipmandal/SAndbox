//! In-tree interpreter for .sbx source files.
//! Provides `sandbox interpret` for instant dev feedback without C compilation.

use crate::ast;
use crate::lexer;
use crate::parser;

/// Remove a fresh `__auto_` string value created by the last expression.
fn take_auto_str(state: &mut InterpreterState) -> Option<String> {
    let key = state
        .str_vars
        .keys()
        .filter(|k| {
            k.starts_with("__auto_")
                && !k.starts_with("__auto_arr_")
                && !k.starts_with("__auto_map_")
        })
        .max()
        .cloned()?;
    state.str_vars.remove(&key)
}

/// Resolve a string argument by AST shape first (literals and variables are
/// exact), falling back to eval + auto-string extraction only for computed
/// values. This keeps stale `__auto_` entries from earlier expressions from
/// leaking into a call's arguments.
fn resolve_arg_str(arg: &ast::Expr, state: &mut InterpreterState) -> String {
    match arg {
        ast::Expr::Str(v) => v.clone(),
        ast::Expr::Ident(n) => state.str_vars.get(n).cloned().unwrap_or_default(),
        _ => {
            eval_expr(arg, state).ok();
            take_auto_str(state)
                .or_else(|| match arg {
                    ast::Expr::Str(v) => Some(v.clone()),
                    ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
                    _ => None,
                })
                .unwrap_or_default()
        }
    }
}

// ── A3: HTTP (interpreter mirror of the C runtime semantics) ──

/// In-flight request/response context, filled by the interpreter's HTTP
/// server and read/mutated by the http::* accessors.
struct HttpCtx {
    method: String,
    query: String,
    body: String,
    headers: Vec<(String, String)>,
    status: i64,
    content_type: String,
    extra_headers: String,
}

impl HttpCtx {
    fn fresh() -> Self {
        Self {
            method: String::new(),
            query: String::new(),
            body: String::new(),
            headers: Vec::new(),
            status: 200,
            content_type: "application/json".to_string(),
            extra_headers: String::new(),
        }
    }
}

/// The active request context (None outside a handler).
static HTTP_CTX: std::sync::Mutex<Option<HttpCtx>> = std::sync::Mutex::new(None);

/// Directory for http::serve_static ("" = disabled).
static STATIC_DIR: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

fn http_reason(code: i64) -> &'static str {
    match code {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        418 => "I'm a teapot",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "Status",
    }
}

/// Mirrors __sbx_url_decode.
fn http_url_decode(s: &str) -> String {
    let hex = |c: u8| -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    };
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        if b[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(b[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Mirrors __sbx_url_pair_get: value of `name` in an "a=1&b=2" string.
fn http_url_pair_get(pairs: &str, name: &str) -> String {
    for seg in pairs.split('&') {
        if seg.is_empty() {
            continue;
        }
        match seg.split_once('=') {
            Some((k, v)) => {
                if k == name {
                    return http_url_decode(v);
                }
            }
            None => {
                if seg == name {
                    return String::new();
                }
            }
        }
    }
    String::new()
}

/// A4: value of `name` in a Cookie request header ("a=1; b=2").
/// Mirrors __sbx_http_cookie_get (first '=' splits; '; ' separates).
fn http_cookie_get(header: &str, name: &str) -> String {
    for seg in header.split(';') {
        let seg = seg.trim_start_matches([' ', '\t']);
        if let Some((k, v)) = seg.split_once('=') {
            if k == name {
                return v.to_string();
            }
        }
    }
    String::new()
}

/// A4: mirrors __sbx_html_escape (& first, then < > " ').
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// A4: mirrors __sbx_html_unescape — numeric (&#NN; / &#xHH;) and the five
/// named entities emitted by html::escape; unknown entities stay as-is.
fn html_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let b = s.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'&' {
            if let Some(end_rel) = s[i..].find(';') {
                let elen = end_rel + 1;
                if elen <= 10 {
                    let body = &s[i + 1..i + elen - 1];
                    let decoded: Option<char> = if let Some(rest) = body.strip_prefix('#') {
                        let (radix, digits) = if let Some(h) =
                            rest.strip_prefix('x').or_else(|| rest.strip_prefix('X'))
                        {
                            (16, h)
                        } else {
                            (10, rest)
                        };
                        if !digits.is_empty()
                            && digits.len() <= 6
                            && digits.chars().all(|c| {
                                c.is_ascii_digit() || (radix == 16 && c.is_ascii_hexdigit())
                            })
                        {
                            u32::from_str_radix(digits, radix)
                                .ok()
                                .and_then(char::from_u32)
                        } else {
                            None
                        }
                    } else {
                        match body {
                            "amp" => Some('&'),
                            "lt" => Some('<'),
                            "gt" => Some('>'),
                            "quot" => Some('"'),
                            "apos" => Some('\''),
                            _ => None,
                        }
                    };
                    if let Some(c) = decoded {
                        out.push(c);
                        i += elen;
                        continue;
                    }
                }
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// A4: mirrors __sbx_tmpl_render — replace %{key} with the paired value;
/// unknown keys stay as-is.
fn tmpl_render(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len() + 64);
    let b = template.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'%' && i + 1 < b.len() && b[i + 1] == b'{' {
            if let Some(end_rel) = template[i + 2..].find('}') {
                let key = &template[i + 2..i + 2 + end_rel];
                if let Some((_, v)) = pairs.iter().find(|(k, _)| *k == key) {
                    out.push_str(v);
                    i += 2 + end_rel + 1;
                    continue;
                }
            }
        }
        let ch = template[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Parse a raw HTTP request into (path, HttpCtx).
fn http_parse_request(req: &str) -> (String, HttpCtx) {
    let mut ctx = HttpCtx::fresh();
    let mut lines = req.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split(' ');
    ctx.method = parts.next().unwrap_or("").to_string();
    let target = parts.next().unwrap_or("/");
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.to_string(), String::new()),
    };
    ctx.query = query;
    let mut in_headers = true;
    for line in lines {
        if in_headers {
            if line.is_empty() {
                in_headers = false;
                continue;
            }
            if let Some((name, value)) = line.split_once(':') {
                ctx.headers
                    .push((name.trim().to_string(), value.trim().to_string()));
            }
        } else if !line.is_empty() {
            if !ctx.body.is_empty() {
                ctx.body.push_str("\r\n");
            }
            ctx.body.push_str(line);
        }
    }
    (path, ctx)
}

fn http_mime(path: &str) -> &'static str {
    let dot = path.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
    match dot {
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "txt" => "text/plain",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

/// Serve dir/path if it resolves to a regular file. Returns true when a
/// response was sent (file served or traversal 403); false = fall through.
/// Mirror of __sbx_http_try_static.
fn http_try_static(stream: &mut std::net::TcpStream, dir: &str, path: &str) -> bool {
    use std::io::{Read, Write};
    // Traversal guard applies even with no static dir (defense in depth).
    if path.contains("..") {
        let resp = "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nContent-Length: 9\r\nConnection: close\r\n\r\nForbidden";
        let _ = stream.write_all(resp.as_bytes());
        let _ = stream.flush();
        return true;
    }
    let rel = path.trim_start_matches('/');
    let full = if dir.ends_with('/') {
        format!("{}{}", dir, rel)
    } else {
        format!("{}/{}", dir, rel)
    };
    let meta = match std::fs::metadata(&full) {
        Ok(m) if m.is_file() => m,
        _ => return false,
    };
    let mut file = match std::fs::File::open(&full) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        http_mime(&full),
        meta.len()
    );
    if stream.write_all(head.as_bytes()).is_err() {
        return true;
    }
    let mut chunk = [0u8; 8192];
    loop {
        match file.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if stream.write_all(&chunk[..n]).is_err() {
                    break;
                }
            }
        }
    }
    let _ = stream.flush();
    true
}

/// The interpreter HTTP server (mirror of __sbx_serve / __sbx_serve_once).
/// `handler` receives the request path and returns the response body.
fn http_serve(
    port: i64,
    once: bool,
    handler: &mut dyn FnMut(&str) -> anyhow::Result<String>,
) -> anyhow::Result<()> {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind(("0.0.0.0", port as u16))
        .map_err(|e| anyhow::anyhow!("serve: bind failed on port {}: {}", port, e))?;
    eprintln!("sandbox: listening on port {}", port);
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(e) => {
                eprintln!("sandbox: accept err: {}", e);
                continue;
            }
        };
        let mut buf = [0u8; 8192];
        let n = match stream.read(&mut buf) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("sandbox: read err: {}", e);
                continue;
            }
        };
        let req = String::from_utf8_lossy(&buf[..n]).into_owned();
        let (path, req_ctx) = http_parse_request(&req);

        let static_dir = STATIC_DIR.lock().unwrap().clone();
        if http_try_static(&mut stream, &static_dir, &path) {
            if once {
                return Ok(());
            }
            continue;
        }

        // Install the request context for the handler's accessors.
        {
            let mut g = HTTP_CTX.lock().unwrap();
            *g = Some(HttpCtx {
                method: req_ctx.method,
                query: req_ctx.query,
                body: req_ctx.body,
                headers: req_ctx.headers,
                status: 200,
                content_type: "application/json".to_string(),
                extra_headers: String::new(),
            });
        }
        let body = match handler(&path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("sandbox: handler error on {}: {}", path, e);
                "{}".to_string()
            }
        };
        let (status, content_type, extra) = {
            let mut g = HTTP_CTX.lock().unwrap();
            match g.take() {
                Some(c) => (c.status, c.content_type, c.extra_headers),
                None => (200, "application/json".to_string(), String::new()),
            }
        };
        let resp = format!(
            "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n{}\r\n{}",
            status,
            http_reason(status),
            content_type,
            body.len(),
            extra,
            body
        );
        let _ = stream.write_all(resp.as_bytes());
        let _ = stream.flush();
        if once {
            return Ok(());
        }
    }
    Ok(())
}

/// Mirrors __sbx_http_headers: extract one header value from an HTTP response.
fn http_headers_extract(resp: &str, name: &str) -> String {
    let mut lines = resp.split("\r\n");
    lines.next(); // status line
    for line in lines {
        if line.is_empty() {
            break;
        }
        if let Some((h, v)) = line.split_once(':') {
            if h.trim().eq_ignore_ascii_case(name) {
                return v.trim().to_string();
            }
        }
    }
    String::new()
}

/// Simple HTTP client (mirror of __sbx_http_request): returns the raw response.
fn http_client_request(method: &str, url: &str, body: Option<&str>) -> String {
    use std::io::{Read, Write};
    let rest = url
        .strip_prefix("http://")
        .unwrap_or_else(|| url.strip_prefix("https://").unwrap_or(url));
    let (hostport, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h, p.parse().unwrap_or(80)),
        None => (hostport, 80),
    };
    let addr = format!("{}:{}", host, port);
    let mut stream = match std::net::TcpStream::connect(&addr) {
        Ok(s) => s,
        Err(_) => return String::new(),
    };
    let body = body.unwrap_or("");
    let req = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        method,
        path,
        host,
        body.len(),
        body
    );
    if stream.write_all(req.as_bytes()).is_err() {
        return String::new();
    }
    let mut resp = String::new();
    let _ = stream.read_to_string(&mut resp);
    resp
}

/// Mirrors __sbx_http_status: parse the status code out of a response string.
fn http_status_code(resp: &str) -> i64 {
    resp.split(' ')
        .nth(1)
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0)
}

/// Skip whitespace (mirrors __sbx_json_skip_ws).
fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t' || b[i] == b'\n' || b[i] == b'\r') {
        i += 1;
    }
    i
}

/// Read a quoted string starting at `i` (which must point at '"').
/// Returns (unquoted content, index after closing quote).
/// Mirrors __sbx_json_extract_string escape handling (n, t, r, backslash, quote).
fn read_json_string(b: &[u8], mut i: usize) -> Option<(String, usize)> {
    if i >= b.len() || b[i] != b'"' {
        return None;
    }
    i += 1;
    let mut out = String::new();
    while i < b.len() && b[i] != b'"' {
        if b[i] == b'\\' && i + 1 < b.len() {
            i += 1;
            let c = b[i];
            match c {
                b'n' => out.push('\n'),
                b't' => out.push('\t'),
                b'r' => out.push('\r'),
                b'\\' => out.push('\\'),
                b'"' => out.push('"'),
                _ => out.push(c as char),
            }
        } else {
            out.push(b[i] as char);
        }
        i += 1;
    }
    if i >= b.len() {
        return None;
    }
    Some((out, i + 1))
}

/// Skip one JSON value (string, number, bool, null, array, object) at `i`.
fn skip_json_value(b: &[u8], mut i: usize) -> usize {
    if i < b.len() && b[i] == b'"' {
        i += 1;
        while i < b.len() && b[i] != b'"' {
            if b[i] == b'\\' && i + 1 < b.len() {
                i += 1;
            }
            i += 1;
        }
        if i < b.len() {
            i += 1;
        }
        return i;
    }
    if i < b.len() && (b[i] == b'[' || b[i] == b'{') {
        let open = b[i];
        let close = if open == b'[' { b']' } else { b'}' };
        let mut depth = 0i64;
        let mut in_str = false;
        while i < b.len() {
            let c = b[i];
            if in_str {
                if c == b'\\' && i + 1 < b.len() {
                    i += 2;
                    continue;
                }
                if c == b'"' {
                    in_str = false;
                }
            } else if c == b'"' {
                in_str = true;
            } else if c == open {
                depth += 1;
            } else if c == close {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            i += 1;
        }
        return i;
    }
    while i < b.len() && b[i] != b',' && b[i] != b'}' && b[i] != b']' {
        i += 1;
    }
    i
}

/// Parse a JSON object into ordered (key, i64) pairs.
/// Mirrors __sbx_json_parse_map: numeric values exact; true/false/null →
/// 1/0/0; string, array and object values skipped.
fn parse_json_object_strict(s: &str) -> Vec<(String, i64)> {
    let mut out: Vec<(String, i64)> = Vec::new();
    let b = s.as_bytes();
    let mut i = 0usize;
    while i < b.len() && b[i] != b'{' {
        i += 1;
    }
    if i >= b.len() {
        return out;
    }
    i += 1;
    loop {
        i = skip_ws(b, i);
        if i >= b.len() || b[i] == b'}' {
            break;
        }
        if b[i] == b',' {
            i += 1;
            continue;
        }
        if b[i] != b'"' {
            break;
        }
        let (key, ni) = match read_json_string(b, i) {
            Some(v) => v,
            None => break,
        };
        i = ni;
        i = skip_ws(b, i);
        if i >= b.len() || b[i] != b':' {
            break;
        }
        i = skip_ws(b, i + 1);
        if i >= b.len() {
            break;
        }
        if b[i] == b'"' || b[i] == b'[' || b[i] == b'{' {
            i = skip_json_value(b, i);
        } else if b[i] == b't' && b[i..].starts_with(b"true") {
            out.push((key, 1));
            i += 4;
        } else if b[i] == b'f' && b[i..].starts_with(b"false") {
            out.push((key, 0));
            i += 5;
        } else if b[i] == b'n' && b[i..].starts_with(b"null") {
            out.push((key, 0));
            i += 4;
        } else {
            let start = i;
            if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
                i += 1;
            }
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            if i == start {
                break;
            }
            let v: i64 = s[start..i].parse().unwrap_or(0);
            out.push((key, v));
        }
    }
    out
}

/// Serialize ordered pairs to JSON text: {"k": v, ...} (mirrors
/// __sbx_json_stringify_map).
fn stringify_json_map(m: &[(String, i64)]) -> String {
    let mut out = String::from("{");
    for (i, (k, v)) in m.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('"');
        out.push_str(k);
        out.push_str("\":");
        out.push_str(&v.to_string());
    }
    out.push('}');
    out
}

/// Serialize a long slice to JSON: [1,2,3] (mirrors __sbx_json_stringify_array).
fn stringify_json_array(arr: &[i64]) -> String {
    let parts: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
    format!("[{}]", parts.join(","))
}

/// Read the raw text value following "key": (mirrors __sbx_json_get).
fn json_get_raw(s: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\"", key);
    let start = s.find(&needle)? + needle.len();
    let b = s.as_bytes();
    let mut i = start;
    while i < b.len() && (b[i] == b' ' || b[i] == b':' || b[i] == b'\t') {
        i += 1;
    }
    if i < b.len() && b[i] == b'"' {
        let (content, _) = read_json_string(b, i)?;
        return Some(content);
    }
    let begin = i;
    while i < b.len()
        && b[i] != b','
        && b[i] != b'}'
        && b[i] != b']'
        && b[i] != b'\n'
        && b[i] != b'\r'
        && b[i] != b' '
    {
        i += 1;
    }
    if i == begin {
        return None;
    }
    Some(s[begin..i].to_string())
}

/// String field of a JSON object ("" if missing; mirrors __sbx_json_get_str).
fn json_get_str(s: &str, key: &str) -> String {
    json_get_raw(s, key).unwrap_or_default()
}

/// Numeric field of a JSON object (0 if missing; mirrors __sbx_json_get_int).
fn json_get_int(s: &str, key: &str) -> i64 {
    match json_get_raw(s, key) {
        Some(v) => {
            let (neg, rest) = match v.strip_prefix('-') {
                Some(r) => (true, r),
                None => (false, v.as_str()),
            };
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            let v: i64 = digits.parse().unwrap_or(0);
            if neg {
                -v
            } else {
                v
            }
        }
        None => 0,
    }
}

/// Legacy v2.0 object encoding: "key\0value\0..." (mirrors
/// __sbx_json_parse_object; string values unquoted, numerics stringified).
fn json_parse_object_legacy(s: &str) -> String {
    let b = s.as_bytes();
    let mut i = 0usize;
    while i < b.len() && b[i] != b'{' {
        i += 1;
    }
    if i >= b.len() {
        return String::new();
    }
    i += 1;
    let mut out = String::new();
    loop {
        i = skip_ws(b, i);
        if i >= b.len() || b[i] == b'}' {
            break;
        }
        if b[i] == b',' {
            i += 1;
            continue;
        }
        if b[i] != b'"' {
            break;
        }
        let (key, ni) = match read_json_string(b, i) {
            Some(v) => v,
            None => break,
        };
        i = ni;
        i = skip_ws(b, i);
        if i >= b.len() || b[i] != b':' {
            break;
        }
        i = skip_ws(b, i + 1);
        let val = if i < b.len() && b[i] == b'"' {
            match read_json_string(b, i) {
                Some((content, ni)) => {
                    i = ni;
                    content
                }
                None => break,
            }
        } else {
            let start = i;
            while i < b.len()
                && b[i] != b','
                && b[i] != b'}'
                && b[i] != b' '
                && b[i] != b'\t'
                && b[i] != b'\n'
                && b[i] != b'\r'
            {
                i += 1;
            }
            s[start..i].to_string()
        };
        out.push_str(&key);
        out.push('\0');
        out.push_str(&val);
        out.push('\0');
    }
    out
}

/// Legacy helpers over the null-separated encoding (mirror the C
/// __sbx_json_map_get / _keys / _len helpers).
fn json_map_str_get(map_str: &str, key: &str) -> String {
    let mut parts = map_str.split('\0').peekable();
    while let (Some(k), Some(v)) = (parts.next(), parts.peek().copied()) {
        if k == key {
            return v.to_string();
        }
        parts.next();
    }
    String::new()
}
fn json_map_str_keys(map_str: &str) -> String {
    let fields: Vec<&str> = map_str.split('\0').collect();
    // Trailing key\0 (empty value) yields an empty last field; keep only
    // fields that start a real pair.
    let mut out: Vec<&str> = Vec::new();
    let mut idx = 0usize;
    while idx < fields.len() {
        if fields[idx].is_empty() {
            break;
        }
        out.push(fields[idx]);
        idx += 2;
    }
    out.join(",")
}

fn json_map_str_len(map_str: &str) -> i64 {
    let fields: Vec<&str> = map_str.split('\0').collect();
    let mut count = 0i64;
    let mut idx = 0usize;
    while idx < fields.len() {
        if fields[idx].is_empty() {
            break;
        }
        count += 1;
        idx += 2;
    }
    count
}

/// Element `idx` of the top-level JSON array (mirrors __sbx_json_array_get_int).
fn json_array_get_int(s: &str, idx: i64) -> i64 {
    let b = s.as_bytes();
    let mut i = 0usize;
    while i < b.len() && b[i] != b'[' {
        i += 1;
    }
    if i >= b.len() {
        return 0;
    }
    i += 1;
    let mut cur = 0i64;
    while i < b.len() {
        i = skip_ws(b, i);
        if i >= b.len() || b[i] == b']' {
            return 0;
        }
        if b[i] == b',' {
            i += 1;
            continue;
        }
        let start = i;
        if b[i] == b'"' || b[i] == b'[' || b[i] == b'{' {
            i = skip_json_value(b, i);
        } else {
            while i < b.len() && b[i] != b',' && b[i] != b']' {
                i += 1;
            }
        }
        if cur == idx {
            let raw = s[start..i.min(s.len())].trim();
            return raw.trim().parse().unwrap_or(0);
        }
        cur += 1;
        i = skip_ws(b, i);
        if i < b.len() && b[i] == b',' {
            i += 1;
        }
    }
    0
}

/// without compiling to C. Enables instant `sandbox interpret` for dev.
/// Scope captured by a lambda at creation time.
#[derive(Clone)]
struct CapturedScope {
    vars: std::collections::HashMap<String, i64>,
    str_vars: std::collections::HashMap<String, String>,
    arr_vars: std::collections::HashMap<String, Vec<i64>>,
    map_vars: std::collections::HashMap<String, Vec<(String, i64)>>,
    struct_instances: std::collections::HashMap<String, Vec<i64>>,
    struct_type_of: std::collections::HashMap<String, String>,
    enum_instances: std::collections::HashMap<String, (String, Option<String>, i64)>,
}

type LambdaMap =
    std::collections::HashMap<String, (Vec<ast::Param>, Vec<ast::Stmt>, CapturedScope)>;
type FnMap =
    std::collections::HashMap<String, (Vec<ast::Param>, Option<ast::Type>, Vec<ast::Stmt>)>;

struct InterpreterState {
    vars: std::collections::HashMap<String, i64>,
    str_vars: std::collections::HashMap<String, String>,
    arr_vars: std::collections::HashMap<String, Vec<i64>>,
    /// map<string,long> bindings as insertion-ordered (key, value) pairs.
    map_vars: std::collections::HashMap<String, Vec<(String, i64)>>,
    lambdas: LambdaMap,
    functions: FnMap,
    struct_fields: std::collections::HashMap<String, Vec<String>>,
    struct_instances: std::collections::HashMap<String, Vec<i64>>,
    struct_type_of: std::collections::HashMap<String, String>,
    impl_methods: FnMap,
    lambda_counter: usize,
    /// String value of the last `return <str-expr>` (http handler dispatch).
    last_returned_str: Option<String>,
    enum_defs: std::collections::HashMap<String, Vec<String>>,
    enum_instances: std::collections::HashMap<String, (String, Option<String>, i64)>,
}

impl InterpreterState {
    fn new() -> Self {
        Self {
            vars: std::collections::HashMap::new(),
            str_vars: std::collections::HashMap::new(),
            arr_vars: std::collections::HashMap::new(),
            map_vars: std::collections::HashMap::new(),
            lambdas: std::collections::HashMap::new(),
            functions: std::collections::HashMap::new(),
            struct_fields: std::collections::HashMap::new(),
            struct_instances: std::collections::HashMap::new(),
            struct_type_of: std::collections::HashMap::new(),
            impl_methods: std::collections::HashMap::new(),
            lambda_counter: 0,
            last_returned_str: None,
            enum_defs: std::collections::HashMap::new(),
            enum_instances: std::collections::HashMap::new(),
        }
    }

    /// Snapshot all auto-generated keys with the given prefix.
    fn snapshot_auto_keys(&self, prefix: &str) -> std::collections::HashSet<String> {
        self.str_vars
            .keys()
            .chain(self.arr_vars.keys())
            .chain(self.lambdas.keys())
            .chain(self.struct_instances.keys())
            .chain(self.enum_instances.keys())
            .chain(self.map_vars.keys())
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }

    /// After evaluating an expression, find the newest auto key (by prefix) that
    /// wasn't in `before` and transfer it to `name` in the appropriate map.
    /// Returns true if a key was transferred.
    fn transfer_new_key(
        &mut self,
        prefix: &str,
        name: &str,
        before: &std::collections::HashSet<String>,
    ) -> bool {
        // Determine which map to look in based on prefix
        let new_key = match prefix {
            p if p.starts_with("__struct_") => self
                .struct_instances
                .keys()
                .filter(|k| k.starts_with("__struct_") && !before.contains(*k))
                .max_by(|a, b| a.cmp(b))
                .cloned(),
            p if p.starts_with("__auto_arr_") => self
                .arr_vars
                .keys()
                .filter(|k| k.starts_with("__auto_arr_") && !before.contains(*k))
                .max_by(|a, b| a.cmp(b))
                .cloned(),
            p if p.starts_with("__lambda_") => self
                .lambdas
                .keys()
                .filter(|k| k.starts_with("__lambda_") && !before.contains(*k))
                .max_by(|a, b| a.cmp(b))
                .cloned(),
            p if p.starts_with("__enum_") => self
                .enum_instances
                .keys()
                .filter(|k| k.starts_with("__enum_") && !before.contains(*k))
                .max_by(|a, b| a.cmp(b))
                .cloned(),
            p if p.starts_with("__auto_map_") => self
                .map_vars
                .keys()
                .filter(|k| k.starts_with("__auto_map_") && !before.contains(*k))
                .max_by(|a, b| a.cmp(b))
                .cloned(),
            _ => {
                // "__auto_"
                self.str_vars
                    .keys()
                    .filter(|k| k.starts_with("__auto_") && !before.contains(*k))
                    .max_by(|a, b| a.cmp(b))
                    .cloned()
            }
        };
        if let Some(key) = new_key {
            match prefix {
                p if p.starts_with("__struct_") => {
                    if let Some(s) = self.struct_instances.remove(&key) {
                        self.struct_instances.insert(name.to_string(), s);
                        if let Some(t) = self.struct_type_of.remove(&key) {
                            self.struct_type_of.insert(name.to_string(), t);
                        }
                        return true;
                    }
                }
                p if p.starts_with("__auto_arr_") => {
                    if let Some(a) = self.arr_vars.remove(&key) {
                        self.arr_vars.insert(name.to_string(), a);
                        return true;
                    }
                }
                p if p.starts_with("__lambda_") => {
                    if let Some(l) = self.lambdas.remove(&key) {
                        self.lambdas.insert(name.to_string(), l);
                        return true;
                    }
                }
                p if p.starts_with("__enum_") => {
                    if let Some(e) = self.enum_instances.remove(&key) {
                        self.enum_instances.insert(name.to_string(), e);
                        return true;
                    }
                }
                p if p.starts_with("__auto_map_") => {
                    if let Some(m) = self.map_vars.remove(&key) {
                        self.map_vars.insert(name.to_string(), m);
                        return true;
                    }
                }
                _ => {
                    if let Some(s) = self.str_vars.remove(&key) {
                        self.str_vars.insert(name.to_string(), s);
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Check if name is bound in any type-specific map (not plain vars).
    fn is_bound_typed(&self, name: &str) -> bool {
        self.str_vars.contains_key(name)
            || self.arr_vars.contains_key(name)
            || self.lambdas.contains_key(name)
            || self.struct_instances.contains_key(name)
            || self.enum_instances.contains_key(name)
            || self.map_vars.contains_key(name)
    }

    /// Inject a captured scope into this state (for lambda invocation).
    fn inject_scope(&mut self, scope: &CapturedScope) {
        self.vars
            .extend(scope.vars.iter().map(|(k, v)| (k.clone(), *v)));
        self.str_vars
            .extend(scope.str_vars.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.arr_vars
            .extend(scope.arr_vars.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.map_vars
            .extend(scope.map_vars.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.struct_instances.extend(
            scope
                .struct_instances
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );
        self.struct_type_of.extend(
            scope
                .struct_type_of
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );
        self.enum_instances.extend(
            scope
                .enum_instances
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );
    }
}

pub fn interpret(source: &str, filename: &str) -> anyhow::Result<()> {
    let mut lexer = lexer::Lexer::new(source).with_source(filename);
    let tokens = lexer.tokenize()?;
    let mut parser = parser::Parser::new(tokens).with_source(source, filename);
    let program = parser.parse()?;

    let mut state = InterpreterState::new();
    for item in &program.items {
        match item {
            ast::TopLevel::FnDef {
                name,
                params,
                ret,
                body,
                ..
            } => {
                state
                    .functions
                    .insert(name.clone(), (params.clone(), ret.clone(), body.clone()));
            }
            ast::TopLevel::StructDef { name, fields, .. } => {
                state.struct_fields.insert(
                    name.clone(),
                    fields.iter().map(|f| f.name.clone()).collect(),
                );
            }
            ast::TopLevel::ImplDef {
                type_name, methods, ..
            } => {
                for method in methods {
                    if let ast::TopLevel::FnDef {
                        name,
                        params,
                        ret,
                        body,
                        ..
                    } = method
                    {
                        let key = format!("{}::{}", type_name, name);
                        state
                            .impl_methods
                            .insert(key, (params.clone(), ret.clone(), body.clone()));
                    }
                }
            }
            ast::TopLevel::EnumDef { name, variants, .. } => {
                state.enum_defs.insert(
                    name.clone(),
                    variants.iter().map(|v| v.name.clone()).collect(),
                );
            }
            _ => {}
        }
    }

    let main_fn = state
        .functions
        .get("main")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("No 'main' function found in {}", filename))?;

    exec_block(&main_fn.2, &mut state)?;
    Ok(())
}

fn exec_block(stmts: &[ast::Stmt], state: &mut InterpreterState) -> anyhow::Result<Option<i64>> {
    const BREAK_SENTINEL: i64 = -999999;
    const CONTINUE_SENTINEL: i64 = -999998;

    for stmt in stmts {
        match stmt {
            ast::Stmt::Let { name, value, .. } => {
                // Snapshot auto-keys before eval, clear old bindings, evaluate, transfer new keys
                let str_before = state.snapshot_auto_keys("__auto_");
                let arr_before = state.snapshot_auto_keys("__auto_arr_");
                let lam_before = state.snapshot_auto_keys("__lambda_");
                let struct_before = state.snapshot_auto_keys("__struct_");
                let enum_before = state.snapshot_auto_keys("__enum_");
                let map_before = state.snapshot_auto_keys("__auto_map_");
                let int_val = eval_expr(value, state)?;
                state.str_vars.remove(name);
                state.arr_vars.remove(name);
                state.lambdas.remove(name);
                state.map_vars.remove(name);
                state.transfer_new_key("__auto_", name, &str_before);
                state.transfer_new_key("__auto_arr_", name, &arr_before);
                state.transfer_new_key("__lambda_", name, &lam_before);
                state.transfer_new_key("__struct_", name, &struct_before);
                state.transfer_new_key("__enum_", name, &enum_before);
                state.transfer_new_key("__auto_map_", name, &map_before);
                if !state.is_bound_typed(name) {
                    state.vars.insert(name.clone(), int_val);
                }
                // Clean up leaked intermediate auto keys
                let leaked: Vec<String> = state
                    .str_vars
                    .keys()
                    .filter(|k| k.starts_with("__auto_") && **k != *name)
                    .cloned()
                    .collect();
                for k in leaked {
                    state.str_vars.remove(&k);
                }
            }
            ast::Stmt::Assign { name, value } => {
                // Snapshot, eval (old value still accessible), clear old bindings, transfer new
                let str_before = state.snapshot_auto_keys("__auto_");
                let arr_before = state.snapshot_auto_keys("__auto_arr_");
                let struct_before = state.snapshot_auto_keys("__struct_");
                let enum_before = state.snapshot_auto_keys("__enum_");
                let map_before = state.snapshot_auto_keys("__auto_map_");
                let int_val = eval_expr(value, state)?;
                state.str_vars.remove(name);
                state.arr_vars.remove(name);
                state.lambdas.remove(name);
                state.map_vars.remove(name);
                state.struct_instances.remove(name);
                state.struct_type_of.remove(name);
                state.enum_instances.remove(name);
                state.transfer_new_key("__auto_", name, &str_before);
                state.transfer_new_key("__auto_arr_", name, &arr_before);
                state.transfer_new_key("__struct_", name, &struct_before);
                state.transfer_new_key("__enum_", name, &enum_before);
                state.transfer_new_key("__auto_map_", name, &map_before);
                if !state.is_bound_typed(name) {
                    state.vars.insert(name.clone(), int_val);
                }
                let leaked: Vec<String> = state
                    .str_vars
                    .keys()
                    .filter(|k| k.starts_with("__auto_") && **k != *name)
                    .cloned()
                    .collect();
                for k in leaked {
                    state.str_vars.remove(&k);
                }
            }
            ast::Stmt::Print(expr) => {
                // Smart print: resolve strings/arrays from idents, eval everything else
                match expr {
                    ast::Expr::Ident(n) => {
                        if let Some(s) = state.str_vars.get(n) {
                            println!("{}", s);
                        } else if let Some(arr) = state.arr_vars.get(n) {
                            let elems: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                            println!("[{}]", elems.join(", "));
                        } else if let Some(m) = state.map_vars.get(n) {
                            let elems: Vec<String> =
                                m.iter().map(|(k, v)| format!("{}: {}", k, v)).collect();
                            println!("{{{}}}", elems.join(", "));
                        } else if let Some((params, _body, _cap)) = state.lambdas.get(n).cloned() {
                            // Print function pointer for lambdas
                            print!("<fn|");
                            for (i, p) in params.iter().enumerate() {
                                if i > 0 {
                                    print!(", ");
                                }
                                print!("{}: {}", p.name, p.ty);
                            }
                            println!("|>");
                        } else {
                            let val = state.vars.get(n.as_str()).unwrap_or(&0);
                            println!("{}", val);
                        }
                    }
                    ast::Expr::Str(s) => println!("{}", s),
                    // C backend prints bool literals as true/false
                    ast::Expr::Bool(b) => println!("{}", if *b { "true" } else { "false" }),
                    ast::Expr::ArrayLiteral(elems) => {
                        let vals: Vec<String> = elems
                            .iter()
                            .map(|e| match e {
                                ast::Expr::Str(s) => s.to_string(),
                                ast::Expr::Int(n) => n.to_string(),
                                ast::Expr::Bool(b) => {
                                    if *b {
                                        "true".to_string()
                                    } else {
                                        "false".to_string()
                                    }
                                }
                                _ => "?".to_string(),
                            })
                            .collect();
                        println!("[{}]", vals.join(", "));
                    }
                    ast::Expr::Call {
                        name,
                        type_args: _,
                        args,
                    } => {
                        if name == "print" {
                            if let Some(first) = args.first() {
                                // C backend prints bool literals as true/false
                                if let ast::Expr::Bool(b) = first {
                                    println!("{}", if *b { "true" } else { "false" });
                                    return Ok(Some(0));
                                }
                                // Snapshot str_vars before to detect new strings
                                let str_before: std::collections::HashSet<String> =
                                    state.str_vars.keys().cloned().collect();
                                let val = eval_expr(first, state)?;
                                // Check if a new string was created (from concat, etc.)
                                let new_str = state
                                    .str_vars
                                    .keys()
                                    .filter(|k| !str_before.contains(*k))
                                    .max_by(|a, b| a.cmp(b))
                                    .cloned();
                                if let Some(key) = new_str {
                                    if let Some(s) = state.str_vars.get(&key) {
                                        println!("{}", s);
                                    } else {
                                        println!("{}", val);
                                    }
                                } else if let ast::Expr::Ident(n) = first {
                                    if let Some(s) = state.str_vars.get(n) {
                                        println!("{}", s);
                                    } else if let Some(arr) = state.arr_vars.get(n) {
                                        let elems: Vec<String> =
                                            arr.iter().map(|v| v.to_string()).collect();
                                        println!("[{}]", elems.join(", "));
                                    } else {
                                        println!("{}", val);
                                    }
                                } else {
                                    println!("{}", val);
                                }
                            }
                        } else {
                            let str_before: std::collections::HashSet<String> =
                                state.str_vars.keys().cloned().collect();
                            let arr_before: std::collections::HashSet<String> =
                                state.arr_vars.keys().cloned().collect();
                            let val = eval_expr(expr, state)?;
                            if val != 0 {
                                println!("{}", val);
                            } else {
                                let new_str = state
                                    .str_vars
                                    .keys()
                                    .filter(|k| !str_before.contains(*k))
                                    .max_by(|a, b| a.cmp(b))
                                    .cloned();
                                if let Some(key) = new_str {
                                    if let Some(s) = state.str_vars.get(&key) {
                                        println!("{}", s);
                                    } else {
                                        println!("{}", val);
                                    }
                                } else {
                                    let new_arr = state
                                        .arr_vars
                                        .keys()
                                        .filter(|k| !arr_before.contains(*k))
                                        .max_by(|a, b| a.cmp(b))
                                        .cloned();
                                    if let Some(key) = new_arr {
                                        if let Some(arr) = state.arr_vars.get(&key) {
                                            let elems: Vec<String> =
                                                arr.iter().map(|v| v.to_string()).collect();
                                            println!("[{}]", elems.join(", "));
                                        } else {
                                            println!("{}", val);
                                        }
                                    } else {
                                        println!("{}", val);
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        // Snapshot to detect intermediate strings/arrays from expression evaluation
                        let str_before: std::collections::HashSet<String> =
                            state.str_vars.keys().cloned().collect();
                        let arr_before: std::collections::HashSet<String> =
                            state.arr_vars.keys().cloned().collect();
                        let val = eval_expr(expr, state)?;
                        let new_str = state
                            .str_vars
                            .keys()
                            .filter(|k| !str_before.contains(*k))
                            .max_by(|a, b| a.cmp(b))
                            .cloned();
                        if let Some(key) = new_str {
                            if let Some(s) = state.str_vars.get(&key) {
                                println!("{}", s);
                            } else {
                                println!("{}", val);
                            }
                        } else {
                            let new_arr = state
                                .arr_vars
                                .keys()
                                .filter(|k| !arr_before.contains(*k))
                                .max_by(|a, b| a.cmp(b))
                                .cloned();
                            if let Some(key) = new_arr {
                                if let Some(arr) = state.arr_vars.get(&key) {
                                    let elems: Vec<String> =
                                        arr.iter().map(|v| v.to_string()).collect();
                                    println!("[{}]", elems.join(", "));
                                } else {
                                    println!("{}", val);
                                }
                            } else {
                                println!("{}", val);
                            }
                        }
                    }
                }
            }
            ast::Stmt::Return(Some(expr)) => {
                let val = eval_expr(expr, state)?;
                // Record plain string returns (http handler dispatch reads this).
                match expr {
                    ast::Expr::Str(v) => state.last_returned_str = Some(v.clone()),
                    ast::Expr::Ident(n) => {
                        if let Some(s) = state.str_vars.get(n) {
                            state.last_returned_str = Some(s.clone());
                        }
                    }
                    _ => {}
                }
                return Ok(Some(val));
            }
            ast::Stmt::Return(None) => return Ok(Some(0)),
            ast::Stmt::Break => return Ok(Some(BREAK_SENTINEL)),
            ast::Stmt::Continue => return Ok(Some(CONTINUE_SENTINEL)),
            ast::Stmt::If {
                condition,
                then,
                else_,
            } => {
                let cond = eval_expr(condition, state)?;
                if cond != 0 {
                    if let Some(val) = exec_block(then, state)? {
                        return Ok(Some(val));
                    }
                } else if let Some(else_body) = else_ {
                    if let Some(val) = exec_block(else_body, state)? {
                        return Ok(Some(val));
                    }
                }
            }
            ast::Stmt::While { condition, body } => loop {
                let cond = eval_expr(condition, state)?;
                if cond == 0 {
                    break;
                }
                match exec_block(body, state)? {
                    Some(BREAK_SENTINEL) => break,
                    Some(CONTINUE_SENTINEL) => continue,
                    Some(val) => return Ok(Some(val)),
                    None => {}
                }
            },
            ast::Stmt::For {
                variable,
                iterable,
                body,
            } => {
                // Check if iterable is a string
                if let ast::Expr::Ident(n) = iterable {
                    if let Some(s) = state.str_vars.get(n).cloned() {
                        let chars: Vec<i64> = s.bytes().map(|b| b as i64).collect();
                        for c in chars {
                            state.vars.insert(variable.clone(), c);
                            state.str_vars.remove(variable); // shadow string with char
                            match exec_block(body, state)? {
                                Some(BREAK_SENTINEL) => break,
                                Some(CONTINUE_SENTINEL) => continue,
                                Some(val) => return Ok(Some(val)),
                                None => {}
                            }
                        }
                        continue;
                    }
                }
                if let ast::Expr::Str(s) = iterable {
                    let chars: Vec<i64> = s.bytes().map(|b| b as i64).collect();
                    for c in chars {
                        state.vars.insert(variable.clone(), c);
                        match exec_block(body, state)? {
                            Some(BREAK_SENTINEL) => break,
                            Some(CONTINUE_SENTINEL) => continue,
                            Some(val) => return Ok(Some(val)),
                            None => {}
                        }
                    }
                    continue;
                }
                // Array literal iteration (matches C/LLVM backends: int elements)
                if let ast::Expr::ArrayLiteral(elems) = iterable {
                    let mut vals = Vec::new();
                    for e in elems {
                        vals.push(eval_expr(e, state)?);
                    }
                    for v in vals {
                        state.vars.insert(variable.clone(), v);
                        state.arr_vars.remove(variable); // shadow array with element
                        match exec_block(body, state)? {
                            Some(BREAK_SENTINEL) => break,
                            Some(CONTINUE_SENTINEL) => continue,
                            Some(val) => return Ok(Some(val)),
                            None => {}
                        }
                    }
                    continue;
                }
                // Named array variable iteration
                if let ast::Expr::Ident(n) = iterable {
                    if let Some(arr) = state.arr_vars.get(n).cloned() {
                        for v in arr {
                            state.vars.insert(variable.clone(), v);
                            state.arr_vars.remove(variable); // shadow array with element
                            match exec_block(body, state)? {
                                Some(BREAK_SENTINEL) => break,
                                Some(CONTINUE_SENTINEL) => continue,
                                Some(val) => return Ok(Some(val)),
                                None => {}
                            }
                        }
                        continue;
                    }
                }
                // Regular numeric range
                let count = eval_expr(iterable, state)?;
                for i in 0..count {
                    state.vars.insert(variable.clone(), i);
                    match exec_block(body, state)? {
                        Some(BREAK_SENTINEL) => break,
                        Some(CONTINUE_SENTINEL) => continue,
                        Some(val) => return Ok(Some(val)),
                        None => {}
                    }
                }
            }
            ast::Stmt::ExprStmt(expr) => {
                eval_expr(expr, state)?;
            }
            ast::Stmt::IfLet {
                value, then, else_, ..
            } => {
                let val = eval_expr(value, state)?;
                if val != 0 {
                    if let Some(result) = exec_block(then, state)? {
                        return Ok(Some(result));
                    }
                } else if let Some(else_body) = else_ {
                    if let Some(result) = exec_block(else_body, state)? {
                        return Ok(Some(result));
                    }
                }
            }
        }
    }
    Ok(None)
}

/// Resolve the string value of an expression, if it has one.
fn resolve_str(expr: &ast::Expr, state: &InterpreterState) -> Option<String> {
    match expr {
        ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
        ast::Expr::Str(s) => Some(s.clone()),
        _ => None,
    }
}

/// Resolve the array value of an expression, if it has one.
fn resolve_arr(expr: &ast::Expr, state: &InterpreterState) -> Option<Vec<i64>> {
    match expr {
        ast::Expr::Ident(n) => state.arr_vars.get(n).cloned(),
        _ => None,
    }
}

fn eval_expr(expr: &ast::Expr, state: &mut InterpreterState) -> anyhow::Result<i64> {
    match expr {
        ast::Expr::Int(n) => Ok(*n),
        ast::Expr::Float(n) => Ok(*n as i64),
        ast::Expr::Bool(b) => Ok(if *b { 1 } else { 0 }),
        ast::Expr::Str(s) => {
            // Store with auto key — exec_block will rename it to the variable
            let key = format!("__auto_{}", state.str_vars.len());
            state.str_vars.insert(key, s.clone());
            Ok(0)
        }
        ast::Expr::Ident(name) => {
            // Check strings first, then arrays, then integers
            if state.str_vars.contains_key(name)
                || state.arr_vars.contains_key(name)
                || state.lambdas.contains_key(name)
            {
                Ok(0) // these types use 0 as the integer representation
            } else {
                Ok(*state.vars.get(name.as_str()).unwrap_or(&0))
            }
        }
        ast::Expr::ArrayLiteral(elems) => {
            let mut vals = Vec::new();
            for e in elems {
                let v = eval_expr(e, state)?;
                vals.push(v);
            }
            let key = format!("__auto_arr_{}", state.arr_vars.len());
            state.arr_vars.insert(key, vals);
            Ok(0)
        }
        ast::Expr::StructLiteral { name, fields, .. } => {
            // Evaluate struct literal: Point { x: 1, y: 2 }
            let field_defs = state.struct_fields.get(name).cloned().unwrap_or_default();
            let mut values = vec![0i64; field_defs.len()];
            for (fname, fval) in fields {
                if let Some(idx) = field_defs.iter().position(|f| f == fname) {
                    values[idx] = eval_expr(fval, state)?;
                }
            }
            let key = format!("__struct_{}", state.struct_instances.len());
            state.struct_type_of.insert(key.clone(), name.clone());
            state.struct_instances.insert(key, values);
            Ok(0)
        }
        ast::Expr::EnumVariant {
            enum_name,
            variant,
            payload,
            ..
        } => {
            // Evaluate an enum variant instance: Color::Green or Maybe::Has(42)
            let valid = state
                .enum_defs
                .get(enum_name)
                .map(|vs| vs.iter().any(|v| v == variant))
                .unwrap_or(false);
            if !valid {
                return Err(anyhow::anyhow!(
                    "Unknown enum variant {}::{}",
                    enum_name,
                    variant
                ));
            }
            let payload_val = match payload {
                Some(p) => eval_expr(p, state)?,
                None => 0,
            };
            let key = format!("__enum_{}", state.enum_instances.len());
            state
                .enum_instances
                .insert(key, (variant.clone(), Some(enum_name.clone()), payload_val));
            Ok(0)
        }
        ast::Expr::Match { scrutinee, arms } => {
            // Match-as-expression: return the matched arm's value.
            // Scrutinee may be an enum instance stored under a name or auto key.
            let scrut_key: Option<String> = match &**scrutinee {
                ast::Expr::Ident(n) => {
                    if state.enum_instances.contains_key(n.as_str()) {
                        Some(n.clone())
                    } else {
                        // Snapshot auto keys, eval, find new enum instance
                        let before: std::collections::HashSet<String> =
                            state.enum_instances.keys().cloned().collect();
                        eval_expr(scrutinee, state)?;
                        state
                            .enum_instances
                            .keys()
                            .filter(|k| !before.contains(*k))
                            .max_by(|a, b| a.cmp(b))
                            .cloned()
                    }
                }
                _ => {
                    let before: std::collections::HashSet<String> =
                        state.enum_instances.keys().cloned().collect();
                    eval_expr(scrutinee, state)?;
                    state
                        .enum_instances
                        .keys()
                        .filter(|k| !before.contains(*k))
                        .max_by(|a, b| a.cmp(b))
                        .cloned()
                }
            };
            let scrut_instance = scrut_key
                .as_ref()
                .and_then(|k| state.enum_instances.get(k).cloned());

            // Resolve scrutinee scalar/string/arr values for literal & variable patterns
            let scrut_val = eval_expr(scrutinee, state)?;
            let scrut_str = resolve_str(scrutinee, state);
            let scrut_arr = resolve_arr(scrutinee, state);

            for arm in arms {
                // Pattern check
                let matched: Option<Option<i64>> = match &arm.pattern {
                    ast::Pattern::EnumVariant {
                        variant, binding, ..
                    } => {
                        match &scrut_instance {
                            Some((sv, _, sp)) if sv == variant => {
                                // Bind payload if requested
                                if let Some(b) = binding {
                                    state.vars.insert(b.clone(), *sp);
                                }
                                Some(Some(*sp))
                            }
                            _ => None,
                        }
                    }
                    ast::Pattern::SomePattern { binding } => match &scrut_instance {
                        Some((sv, se, sp)) if se.is_none() && sv != "None" => {
                            if let Some(b) = binding {
                                state.vars.insert(b.clone(), *sp);
                            }
                            Some(Some(*sp))
                        }
                        _ => None,
                    },
                    ast::Pattern::NonePattern => match &scrut_instance {
                        Some((sv, _, _)) if sv == "None" => Some(None),
                        _ => None,
                    },
                    ast::Pattern::IntLiteral(n) => {
                        if scrut_val == *n {
                            Some(None)
                        } else {
                            None
                        }
                    }
                    ast::Pattern::BoolLiteral(b) => {
                        let bv = if *b { 1i64 } else { 0i64 };
                        if scrut_val == bv {
                            Some(None)
                        } else {
                            None
                        }
                    }
                    ast::Pattern::StrLiteral(s) => {
                        if scrut_str.as_deref() == Some(s.as_str()) {
                            Some(None)
                        } else {
                            None
                        }
                    }
                    ast::Pattern::Wildcard => Some(None),
                    ast::Pattern::Variable(v) => {
                        // Bind the scrutinee value to the pattern variable
                        if let Some(s) = &scrut_str {
                            state.str_vars.insert(v.clone(), s.clone());
                        } else if let Some(a) = &scrut_arr {
                            state.arr_vars.insert(v.clone(), a.clone());
                        } else {
                            state.vars.insert(v.clone(), scrut_val);
                        }
                        Some(None)
                        // variable patterns match unconditionally
                    }
                };

                if let Some(_payload_val) = matched {
                    // Guard check (binding already done above)
                    if let Some(guard_expr) = &arm.guard {
                        let g = eval_expr(guard_expr, state)?;
                        if g == 0 {
                            continue;
                        }
                    }
                    // Execute the arm body; last ExprStmt value is the match result
                    let mut result: i64 = 0;
                    let n_stmts = arm.body.len();
                    for (i, stmt) in arm.body.iter().enumerate() {
                        match stmt {
                            ast::Stmt::ExprStmt(e) if i == n_stmts - 1 => {
                                result = eval_expr(e, state)?;
                            }
                            ast::Stmt::Return(Some(e)) => {
                                return eval_expr(e, state);
                            }
                            ast::Stmt::Return(None) => return Ok(0),
                            _ => {
                                exec_block(std::slice::from_ref(stmt), state)?;
                            }
                        }
                    }
                    return Ok(result);
                }
            }
            // No arm matched
            Err(anyhow::anyhow!("match expression: no arm matched"))
        }
        ast::Expr::Lambda { params, body, .. } => {
            let key = format!("__lambda_{}", state.lambda_counter);
            state.lambda_counter += 1;
            state.lambdas.insert(
                key,
                (
                    params.clone(),
                    body.clone(),
                    CapturedScope {
                        vars: state.vars.clone(),
                        str_vars: state.str_vars.clone(),
                        arr_vars: state.arr_vars.clone(),
                        struct_instances: state.struct_instances.clone(),
                        struct_type_of: state.struct_type_of.clone(),
                        enum_instances: state.enum_instances.clone(),
                        map_vars: state.map_vars.clone(),
                    },
                ),
            );
            Ok(0)
        }
        ast::Expr::BinaryOp { op, left, right } => {
            // Snapshot auto keys before evaluating each side to detect intermediate strings
            let str_before_left: std::collections::HashSet<String> = state
                .str_vars
                .keys()
                .filter(|k| k.starts_with("__auto_"))
                .cloned()
                .collect();
            let l = eval_expr(left, state)?;
            let str_after_left: std::collections::HashSet<String> = state
                .str_vars
                .keys()
                .filter(|k| k.starts_with("__auto_"))
                .cloned()
                .collect();
            let left_auto_key: Option<String> = str_after_left
                .difference(&str_before_left)
                .cloned()
                .max_by(|a, b| a.cmp(b));

            let str_before_right: std::collections::HashSet<String> = state
                .str_vars
                .keys()
                .filter(|k| k.starts_with("__auto_"))
                .cloned()
                .collect();
            let r = eval_expr(right, state)?;
            let str_after_right: std::collections::HashSet<String> = state
                .str_vars
                .keys()
                .filter(|k| k.starts_with("__auto_"))
                .cloned()
                .collect();
            let right_auto_key: Option<String> = str_after_right
                .difference(&str_before_right)
                .cloned()
                .max_by(|a, b| a.cmp(b));

            // Resolve string values — prefer auto keys from intermediate eval, then AST-based resolution
            let left_str = left_auto_key
                .and_then(|k| state.str_vars.get(&k).cloned())
                .or_else(|| resolve_str(left, state));
            let right_str = right_auto_key
                .and_then(|k| state.str_vars.get(&k).cloned())
                .or_else(|| resolve_str(right, state));
            let left_arr = resolve_arr(left, state);
            let right_arr = resolve_arr(right, state);

            match op {
                ast::BinOp::Add => {
                    // String concatenation
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        let combined = format!("{}{}", ls, rs);
                        let key = format!("__auto_{}", state.str_vars.len());
                        state.str_vars.insert(key, combined);
                        return Ok(0);
                    }
                    // Array concat
                    if let (Some(la), Some(ra)) = (&left_arr, &right_arr) {
                        let mut combined = la.clone();
                        combined.extend(ra);
                        let key = format!("__auto_arr_{}", state.arr_vars.len());
                        state.arr_vars.insert(key, combined);
                        return Ok(0);
                    }
                    Ok(l + r)
                }
                ast::BinOp::Sub => Ok(l - r),
                ast::BinOp::Mul => Ok(l * r),
                ast::BinOp::Div => Ok(if r != 0 { l / r } else { 0 }),
                ast::BinOp::Mod => Ok(if r != 0 { l % r } else { 0 }),
                ast::BinOp::Eq => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls == rs { 1 } else { 0 })
                    } else {
                        Ok(if l == r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Neq => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls != rs { 1 } else { 0 })
                    } else {
                        Ok(if l != r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Lt => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls < rs { 1 } else { 0 })
                    } else {
                        Ok(if l < r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Gt => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls > rs { 1 } else { 0 })
                    } else {
                        Ok(if l > r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Le => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls <= rs { 1 } else { 0 })
                    } else {
                        Ok(if l <= r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Ge => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls >= rs { 1 } else { 0 })
                    } else {
                        Ok(if l >= r { 1 } else { 0 })
                    }
                }
                ast::BinOp::And => Ok(if l != 0 && r != 0 { 1 } else { 0 }),
                ast::BinOp::Or => Ok(if l != 0 || r != 0 { 1 } else { 0 }),
                // B1: bitwise — Rust i64 ops match two's-complement C/LLVM
                // semantics; shifts mask the shift amount like x86 (<<, >>
                // with r in 0..=63). C/LLVM would poison on r >= 64, so
                // mirror the masking for cross-backend agreement.
                ast::BinOp::BitAnd => Ok(l & r),
                ast::BinOp::BitOr => Ok(l | r),
                ast::BinOp::BitXor => Ok(l ^ r),
                ast::BinOp::Shl => Ok(if (r as u64) < 64 {
                    l << (r as u64 & 63)
                } else {
                    0
                }),
                ast::BinOp::Shr => {
                    if (r as u64) < 64 {
                        Ok(l >> (r as u64 & 63))
                    } else {
                        Ok(if l < 0 { -1 } else { 0 })
                    }
                }
            }
        }
        ast::Expr::UnaryOp { op, expr } => {
            let val = eval_expr(expr, state)?;
            match op {
                ast::UnOp::Neg => Ok(-val),
                ast::UnOp::Not => Ok(if val == 0 { 1 } else { 0 }),
                // B1: bitwise complement
                ast::UnOp::BitNot => Ok(!val),
            }
        }
        ast::Expr::MapLiteral(pairs) => {
            // Evaluate to an anonymous map stored under an auto key; the Let
            // handler transfers it to the variable name (same pattern as arrays).
            let mut entries: Vec<(String, i64)> = Vec::new();
            for (k, v) in pairs {
                let key = match k {
                    ast::Expr::Str(s) => s.clone(),
                    _ => String::new(),
                };
                let val = eval_expr(v, state)?;
                match entries.iter_mut().find(|(ek, _)| *ek == key) {
                    Some(entry) => entry.1 = val,
                    None => entries.push((key, val)),
                }
            }
            let key = format!("__auto_map_{}", state.map_vars.len());
            state.map_vars.insert(key, entries);
            Ok(0)
        }
        ast::Expr::Index { target, index } => {
            // Map indexing: m["key"] (string key) — distinct from array indexing.
            if let ast::Expr::Ident(n) = target.as_ref() {
                if state.map_vars.contains_key(n) {
                    let key = match index.as_ref() {
                        ast::Expr::Str(s) => s.clone(),
                        ast::Expr::Ident(k) => state.str_vars.get(k).cloned().unwrap_or_default(),
                        _ => String::new(),
                    };
                    let m = state.map_vars.get(n).cloned().unwrap_or_default();
                    return Ok(m
                        .iter()
                        .find(|(k, _)| *k == key)
                        .map(|(_, v)| *v)
                        .unwrap_or(0));
                }
            }
            let idx = eval_expr(index, state)? as usize;
            if let ast::Expr::Ident(n) = target.as_ref() {
                if let Some(arr) = state.arr_vars.get(n) {
                    return Ok(arr.get(idx).copied().unwrap_or(0));
                }
                if let Some(s) = state.str_vars.get(n) {
                    return Ok(s
                        .as_bytes()
                        .get(idx)
                        .copied()
                        .map(|b| b as i64)
                        .unwrap_or(0));
                }
            }
            Ok(0)
        }
        ast::Expr::Call {
            name,
            type_args: _,
            args,
        } => {
            if name == "print" {
                if !args.is_empty() {
                    // Inline print resolution for call context
                    match &args[0] {
                        ast::Expr::Ident(n) => {
                            if let Some(s) = state.str_vars.get(n) {
                                println!("{}", s);
                            } else if let Some(arr) = state.arr_vars.get(n) {
                                let elems: Vec<String> =
                                    arr.iter().map(|v| v.to_string()).collect();
                                println!("[{}]", elems.join(", "));
                            } else {
                                let val = state.vars.get(n.as_str()).unwrap_or(&0);
                                println!("{}", val);
                            }
                        }
                        ast::Expr::Str(s) => println!("{}", s),
                        // C backend prints bool literals as true/false
                        ast::Expr::Bool(b) => println!("{}", if *b { "true" } else { "false" }),
                        ast::Expr::ArrayLiteral(elems) => {
                            let vals: Vec<String> = elems
                                .iter()
                                .map(|e| match e {
                                    ast::Expr::Str(s) => s.to_string(),
                                    ast::Expr::Int(n) => n.to_string(),
                                    _ => "?".to_string(),
                                })
                                .collect();
                            println!("[{}]", vals.join(", "));
                        }
                        other => {
                            let str_before: std::collections::HashSet<String> =
                                state.str_vars.keys().cloned().collect();
                            let arr_before: std::collections::HashSet<String> =
                                state.arr_vars.keys().cloned().collect();
                            let val = eval_expr(other, state)?;
                            if val != 0 {
                                println!("{}", val);
                            } else {
                                let new_str = state
                                    .str_vars
                                    .keys()
                                    .filter(|k| !str_before.contains(*k))
                                    .max_by(|a, b| a.cmp(b))
                                    .cloned();
                                if let Some(key) = new_str {
                                    if let Some(s) = state.str_vars.get(&key) {
                                        println!("{}", s);
                                    } else {
                                        println!("{}", val);
                                    }
                                } else {
                                    let new_arr = state
                                        .arr_vars
                                        .keys()
                                        .filter(|k| !arr_before.contains(*k))
                                        .max_by(|a, b| a.cmp(b))
                                        .cloned();
                                    if let Some(key) = new_arr {
                                        if let Some(arr) = state.arr_vars.get(&key) {
                                            let elems: Vec<String> =
                                                arr.iter().map(|v| v.to_string()).collect();
                                            println!("[{}]", elems.join(", "));
                                        } else {
                                            println!("{}", val);
                                        }
                                    } else {
                                        println!("{}", val);
                                    }
                                }
                            }
                        }
                    }
                }
                return Ok(0);
            }
            if name == "len" && args.len() == 1 {
                // Snapshot auto-keys before evaluating arg (detect side-effectful calls)
                let str_keys_before: std::collections::HashSet<String> =
                    state.str_vars.keys().cloned().collect();
                let arr_keys_before: std::collections::HashSet<String> =
                    state.arr_vars.keys().cloned().collect();
                let _val = eval_expr(&args[0], state)?;
                // Check for new string auto-key
                let new_str_key = state
                    .str_vars
                    .keys()
                    .filter(|k| !str_keys_before.contains(*k))
                    .max_by(|a, b| a.cmp(b))
                    .cloned();
                if let Some(key) = new_str_key {
                    if let Some(s) = state.str_vars.get(&key) {
                        return Ok(s.len() as i64);
                    }
                }
                // Check for new array auto-key
                let new_arr_key = state
                    .arr_vars
                    .keys()
                    .filter(|k| !arr_keys_before.contains(*k))
                    .max_by(|a, b| a.cmp(b))
                    .cloned();
                if let Some(key) = new_arr_key {
                    if let Some(a) = state.arr_vars.get(&key) {
                        return Ok(a.len() as i64);
                    }
                }
                // Fallback: check named variables
                match &args[0] {
                    ast::Expr::Str(s) => return Ok(s.len() as i64),
                    ast::Expr::Ident(n) => {
                        if let Some(s) = state.str_vars.get(n) {
                            return Ok(s.len() as i64);
                        }
                        if let Some(a) = state.arr_vars.get(n) {
                            return Ok(a.len() as i64);
                        }
                        if let Some(m) = state.map_vars.get(n) {
                            return Ok(m.len() as i64);
                        }
                    }
                    ast::Expr::ArrayLiteral(elems) => return Ok(elems.len() as i64),
                    ast::Expr::MapLiteral(pairs) => return Ok(pairs.len() as i64),
                    _ => {}
                }
                return Ok(0);
            }
            // Array built-in: map(arr, lambda)
            if name == "map" && args.len() == 2 {
                // Get the array
                let arr = match &args[0] {
                    ast::Expr::Ident(n) => state.arr_vars.get(n).cloned().unwrap_or_default(),
                    _ => {
                        // Evaluate to get array - try to find auto key
                        eval_expr(&args[0], state)?;
                        let new_key = state
                            .arr_vars
                            .keys()
                            .find(|k| k.starts_with("__auto_arr_"))
                            .cloned();
                        if let Some(key) = new_key {
                            state.arr_vars.remove(&key).unwrap_or_default()
                        } else {
                            vec![]
                        }
                    }
                };
                // Get the lambda
                let lambda_info: Option<(Vec<ast::Param>, Vec<ast::Stmt>, CapturedScope)> =
                    match &args[1] {
                        ast::Expr::Lambda { params, body, .. } => Some((
                            params.clone(),
                            body.clone(),
                            CapturedScope {
                                vars: state.vars.clone(),
                                str_vars: state.str_vars.clone(),
                                arr_vars: state.arr_vars.clone(),
                                struct_instances: state.struct_instances.clone(),
                                struct_type_of: state.struct_type_of.clone(),
                                enum_instances: state.enum_instances.clone(),
                                map_vars: state.map_vars.clone(),
                            },
                        )),
                        ast::Expr::Ident(n) => state.lambdas.get(n).cloned(),
                        _ => None,
                    };
                if let Some((params, body, cap)) = lambda_info {
                    let mut result = Vec::new();
                    for elem in &arr {
                        let mut local_state = InterpreterState::new();
                        local_state.functions = state.functions.clone();
                        local_state.struct_fields = state.struct_fields.clone();
                        local_state.inject_scope(&cap);
                        if let Some(param) = params.first() {
                            local_state.vars.insert(param.name.clone(), *elem);
                        }
                        match exec_block(&body, &mut local_state)? {
                            Some(v) => result.push(v),
                            None => result.push(0),
                        }
                    }
                    let key = format!("__auto_arr_{}", state.arr_vars.len());
                    state.arr_vars.insert(key, result);
                    return Ok(0);
                }
                return Ok(0);
            }
            // Array built-in: filter(arr, lambda)
            if name == "filter" && args.len() == 2 {
                let arr = match &args[0] {
                    ast::Expr::Ident(n) => state.arr_vars.get(n).cloned().unwrap_or_default(),
                    _ => {
                        eval_expr(&args[0], state)?;
                        let new_key = state
                            .arr_vars
                            .keys()
                            .find(|k| k.starts_with("__auto_arr_"))
                            .cloned();
                        if let Some(key) = new_key {
                            state.arr_vars.remove(&key).unwrap_or_default()
                        } else {
                            vec![]
                        }
                    }
                };
                let lambda_info: Option<(Vec<ast::Param>, Vec<ast::Stmt>, CapturedScope)> =
                    match &args[1] {
                        ast::Expr::Lambda { params, body, .. } => Some((
                            params.clone(),
                            body.clone(),
                            CapturedScope {
                                vars: state.vars.clone(),
                                str_vars: state.str_vars.clone(),
                                arr_vars: state.arr_vars.clone(),
                                struct_instances: state.struct_instances.clone(),
                                struct_type_of: state.struct_type_of.clone(),
                                enum_instances: state.enum_instances.clone(),
                                map_vars: state.map_vars.clone(),
                            },
                        )),
                        ast::Expr::Ident(n) => state.lambdas.get(n).cloned(),
                        _ => None,
                    };
                if let Some((params, body, cap)) = lambda_info {
                    let mut result = Vec::new();
                    for elem in &arr {
                        let mut local_state = InterpreterState::new();
                        local_state.functions = state.functions.clone();
                        local_state.struct_fields = state.struct_fields.clone();
                        local_state.inject_scope(&cap);
                        if let Some(param) = params.first() {
                            local_state.vars.insert(param.name.clone(), *elem);
                        }
                        let cond = exec_block(&body, &mut local_state)?.unwrap_or_default();
                        if cond != 0 {
                            result.push(*elem);
                        }
                    }
                    let key = format!("__auto_arr_{}", state.arr_vars.len());
                    state.arr_vars.insert(key, result);
                    return Ok(0);
                }
                return Ok(0);
            }
            // Array built-in: reduce(arr, lambda, initial)
            if name == "reduce" && args.len() == 3 {
                let arr = match &args[0] {
                    ast::Expr::Ident(n) => state.arr_vars.get(n).cloned().unwrap_or_default(),
                    _ => {
                        eval_expr(&args[0], state)?;
                        let new_key = state
                            .arr_vars
                            .keys()
                            .find(|k| k.starts_with("__auto_arr_"))
                            .cloned();
                        if let Some(key) = new_key {
                            state.arr_vars.remove(&key).unwrap_or_default()
                        } else {
                            vec![]
                        }
                    }
                };
                let mut acc = eval_expr(&args[2], state)?;
                let lambda_info: Option<(Vec<ast::Param>, Vec<ast::Stmt>, CapturedScope)> =
                    match &args[1] {
                        ast::Expr::Lambda { params, body, .. } => Some((
                            params.clone(),
                            body.clone(),
                            CapturedScope {
                                vars: state.vars.clone(),
                                str_vars: state.str_vars.clone(),
                                arr_vars: state.arr_vars.clone(),
                                struct_instances: state.struct_instances.clone(),
                                struct_type_of: state.struct_type_of.clone(),
                                enum_instances: state.enum_instances.clone(),
                                map_vars: state.map_vars.clone(),
                            },
                        )),
                        ast::Expr::Ident(n) => state.lambdas.get(n).cloned(),
                        _ => None,
                    };
                if let Some((params, body, cap)) = lambda_info {
                    for elem in &arr {
                        let mut local_state = InterpreterState::new();
                        local_state.functions = state.functions.clone();
                        local_state.struct_fields = state.struct_fields.clone();
                        local_state.inject_scope(&cap);
                        if params.len() >= 2 {
                            local_state.vars.insert(params[0].name.clone(), acc);
                            local_state.vars.insert(params[1].name.clone(), *elem);
                        } else if let Some(param) = params.first() {
                            local_state.vars.insert(param.name.clone(), *elem);
                        }
                        acc = exec_block(&body, &mut local_state)?.unwrap_or_default();
                    }
                    return Ok(acc);
                }
                return Ok(0);
            }
            // ── A2: JSON parse/stringify on maps + arrays ──
            // Matches the C runtime: object values are numeric (true/false/null
            // → 1/0/0); string fields stay readable via json::get_str.
            if name == "json::parse_map" && args.len() == 1 {
                let text = resolve_arg_str(&args[0], state);
                let entries = parse_json_object_strict(&text);
                let key = format!("__auto_map_{}", state.map_vars.len());
                state.map_vars.insert(key, entries);
                return Ok(0);
            }
            if name == "json::stringify_map" && args.len() == 1 {
                // Map bindings already live in map_vars; read the source map directly
                let map_name = match &args[0] {
                    ast::Expr::Ident(n) if state.map_vars.contains_key(n) => Some(n.clone()),
                    _ => None,
                };
                let m = map_name
                    .as_deref()
                    .and_then(|n| state.map_vars.get(n).cloned())
                    .unwrap_or_default();
                let out = stringify_json_map(&m);
                let key = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(key, out);
                return Ok(0);
            }
            if name == "json::get_str" && args.len() == 2 {
                let text = resolve_arg_str(&args[0], state);
                let key = resolve_arg_str(&args[1], state);
                let out = json_get_str(&text, &key);
                let key = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(key, out);
                return Ok(0);
            }
            if name == "json::get_int" && args.len() == 2 {
                let text = resolve_arg_str(&args[0], state);
                let key = resolve_arg_str(&args[1], state);
                return Ok(json_get_int(&text, &key));
            }
            if name == "json::stringify_array" && args.len() == 1 {
                let arr = match &args[0] {
                    ast::Expr::Ident(n) => state.arr_vars.get(n).cloned().unwrap_or_default(),
                    ast::Expr::ArrayLiteral(elems) => {
                        let mut vals = Vec::new();
                        for e in elems {
                            vals.push(eval_expr(e, state)?);
                        }
                        vals
                    }
                    _ => {
                        eval_expr(&args[0], state)?;
                        let key = state
                            .arr_vars
                            .keys()
                            .filter(|k| k.starts_with("__auto_arr_"))
                            .max()
                            .cloned();
                        match key {
                            Some(k) => state.arr_vars.remove(&k).unwrap_or_default(),
                            None => Vec::new(),
                        }
                    }
                };
                let out = stringify_json_array(&arr);
                let key = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(key, out);
                return Ok(0);
            }
            if name == "json::array_get_int" && args.len() == 2 {
                let text = resolve_arg_str(&args[0], state);
                let idx = eval_expr(&args[1], state)?;
                return Ok(json_array_get_int(&text, idx));
            }
            // ── v2.0 scalar JSON builtins (mirrors the C runtime helpers;
            // float variants are omitted — the interpreter stores floats as
            // i64, so json::parse_float / json::stringify_float have no
            // interpreter parity) ──
            if name == "json::stringify" && args.len() == 1 {
                let v = eval_expr(&args[0], state)?;
                let key = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(key, v.to_string());
                return Ok(0);
            }
            if name == "json::stringify_string" && args.len() == 1 {
                let s = resolve_arg_str(&args[0], state);
                let key = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(key, format!("\"{s}\""));
                return Ok(0);
            }
            if name == "json::stringify_bool" && args.len() == 1 {
                let b = eval_expr(&args[0], state)?;
                let key = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(
                    key,
                    if b != 0 {
                        "true".into()
                    } else {
                        "false".into()
                    },
                );
                return Ok(0);
            }
            if name == "json::parse" && args.len() == 1 {
                let text = resolve_arg_str(&args[0], state);
                // C: first character that starts a number, then strtol
                let b = text.as_bytes();
                let mut i = 0usize;
                let mut result = 0i64;
                while i < b.len() {
                    if b[i].is_ascii_digit() || b[i] == b'-' || b[i] == b'+' {
                        let start = i;
                        if b[i] == b'-' || b[i] == b'+' {
                            i += 1;
                        }
                        while i < b.len() && b[i].is_ascii_digit() {
                            i += 1;
                        }
                        result = text[start..i].parse().unwrap_or(0);
                        break;
                    }
                    i += 1;
                }
                return Ok(result);
            }
            if name == "json::get" && args.len() == 2 {
                let text = resolve_arg_str(&args[0], state);
                let key = resolve_arg_str(&args[1], state);
                let out = json_get_str(&text, &key);
                let key = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(key, out);
                return Ok(0);
            }
            if name == "json::has_key" && args.len() == 2 {
                let text = resolve_arg_str(&args[0], state);
                let key = resolve_arg_str(&args[1], state);
                return Ok(text.contains(&format!("\"{key}\"")) as i64);
            }
            if name == "json::array_len" && args.len() == 1 {
                let text = resolve_arg_str(&args[0], state);
                // C counts commas outside strings after the first '[' (no depth
                // tracking — nested containers are counted naively; mirror it)
                let b = text.as_bytes();
                let mut i = 0usize;
                while i < b.len() && b[i] != b'[' {
                    i += 1;
                }
                if i >= b.len() {
                    return Ok(0);
                }
                i += 1;
                if i < b.len() && b[i] == b']' {
                    return Ok(0);
                }
                let mut count = 1i64;
                let mut in_string = false;
                while i < b.len() && b[i] != b']' {
                    if b[i] == b'"' && (i == 0 || b[i - 1] != b'\\') {
                        in_string = !in_string;
                    }
                    if !in_string && b[i] == b',' {
                        count += 1;
                    }
                    i += 1;
                }
                return Ok(count);
            }
            if name == "json::parse_string" && args.len() == 1 {
                // Mirrors __sbx_json_parse_string: first quoted string's content
                let text = resolve_arg_str(&args[0], state);
                let b = text.as_bytes();
                let mut i = 0usize;
                while i < b.len() && b[i] != b'"' {
                    i += 1;
                }
                if i >= b.len() {
                    let key = format!("__auto_{}", state.str_vars.len());
                    state.str_vars.insert(key, String::new());
                    return Ok(0);
                }
                i += 1;
                let mut out = String::new();
                while i < b.len() && b[i] != b'"' {
                    out.push(b[i] as char);
                    i += 1;
                }
                let key = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(key, out);
                return Ok(0);
            }
            if name == "json::parse_object" && args.len() == 1 {
                let text = resolve_arg_str(&args[0], state);
                let out = json_parse_object_legacy(&text);
                let key = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(key, out);
                return Ok(0);
            }
            if name == "json::map_get" && args.len() == 2 {
                let map_str = resolve_arg_str(&args[0], state);
                let key = resolve_arg_str(&args[1], state);
                let out = json_map_str_get(&map_str, &key);
                let key = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(key, out);
                return Ok(0);
            }
            if name == "json::map_keys" && args.len() == 1 {
                let map_str = resolve_arg_str(&args[0], state);
                let out = json_map_str_keys(&map_str);
                let key = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(key, out);
                return Ok(0);
            }
            if name == "json::map_len" && args.len() == 1 {
                let map_str = resolve_arg_str(&args[0], state);
                return Ok(json_map_str_len(&map_str));
            }
            // ── A3: HTTP builtins (mirror of the C runtime) ──
            if name == "http::method"
                || name == "http::query"
                || name == "http::body"
                || name == "http::form_param"
            {
                let g = HTTP_CTX.lock().unwrap();
                if let Some(c) = g.as_ref() {
                    let out = match name.as_str() {
                        "http::method" => c.method.clone(),
                        "http::query" => c.query.clone(),
                        "http::body" => c.body.clone(),
                        _ => {
                            let nm = take_auto_str(state)
                                .or_else(|| match &args[0] {
                                    ast::Expr::Str(v) => Some(v.clone()),
                                    ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
                                    _ => None,
                                })
                                .unwrap_or_default();
                            http_url_pair_get(&c.body, &nm)
                        }
                    };
                    let k = format!("__auto_{}", state.str_vars.len());
                    state.str_vars.insert(k, out);
                    return Ok(0);
                }
                return Ok(0);
            }
            if name == "http::req_header" && args.len() == 1 {
                let nm = resolve_arg_str(&args[0], state);
                let g = HTTP_CTX.lock().unwrap();
                let out = g
                    .as_ref()
                    .and_then(|c| {
                        c.headers
                            .iter()
                            .find(|(h, _)| h.eq_ignore_ascii_case(&nm))
                            .map(|(_, v)| v.clone())
                    })
                    .unwrap_or_default();
                drop(g);
                let k = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(k, out);
                return Ok(0);
            }
            if name == "http::set_status" && args.len() == 1 {
                let code = eval_expr(&args[0], state)?;
                let mut g = HTTP_CTX.lock().unwrap();
                if let Some(c) = g.as_mut() {
                    c.status = code;
                }
                return Ok(0);
            }
            if name == "http::set_header" && args.len() == 2 {
                let nm = take_auto_str(state)
                    .or_else(|| match &args[0] {
                        ast::Expr::Str(v) => Some(v.clone()),
                        ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
                        _ => None,
                    })
                    .unwrap_or_default();
                eval_expr(&args[1], state)?;
                let val = take_auto_str(state)
                    .or_else(|| match &args[1] {
                        ast::Expr::Str(v) => Some(v.clone()),
                        ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
                        _ => None,
                    })
                    .unwrap_or_default();
                let mut g = HTTP_CTX.lock().unwrap();
                if let Some(c) = g.as_mut() {
                    if nm.eq_ignore_ascii_case("content-type") {
                        c.content_type = val;
                    } else {
                        if !c.extra_headers.is_empty() {
                            c.extra_headers.push_str("\r\n");
                        }
                        c.extra_headers.push_str(&format!("{}: {}\r\n", nm, val));
                    }
                }
                return Ok(0);
            }
            if name == "http::status" && args.is_empty() {
                let g = HTTP_CTX.lock().unwrap();
                return Ok(g.as_ref().map_or(200, |c| c.status));
            }
            if name == "http::query_param" && args.len() == 2 {
                let q = resolve_arg_str(&args[0], state);
                let nm = resolve_arg_str(&args[1], state);
                let out = http_url_pair_get(&q, &nm);
                let k = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(k, out);
                return Ok(0);
            }
            if name == "http::form_get" && args.len() == 2 {
                let b = resolve_arg_str(&args[0], state);
                let nm = resolve_arg_str(&args[1], state);
                let out = http_url_pair_get(&b, &nm);
                let k = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(k, out);
                return Ok(0);
            }
            if name == "http::url_decode" && args.len() == 1 {
                let s = resolve_arg_str(&args[0], state);
                let out = http_url_decode(&s);
                let k = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(k, out);
                return Ok(0);
            }
            // ── A4: cookies ──
            if name == "http::set_cookie" && args.len() == 2 {
                let nm = take_auto_str(state)
                    .or_else(|| match &args[0] {
                        ast::Expr::Str(v) => Some(v.clone()),
                        ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
                        _ => None,
                    })
                    .unwrap_or_default();
                let val = take_auto_str(state)
                    .or_else(|| match &args[1] {
                        ast::Expr::Str(v) => Some(v.clone()),
                        ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
                        _ => None,
                    })
                    .unwrap_or_default();
                if !nm.is_empty() && !nm.contains(['\r', '\n', ';']) && !val.contains(['\r', '\n'])
                {
                    let mut g = HTTP_CTX.lock().unwrap();
                    if let Some(c) = g.as_mut() {
                        if !c.extra_headers.is_empty() {
                            c.extra_headers.push_str("\r\n");
                        }
                        c.extra_headers
                            .push_str(&format!("Set-Cookie: {}={}; Path=/\r\n", nm, val));
                    }
                }
                return Ok(0);
            }
            if name == "http::get_cookie" && args.len() == 1 {
                let nm = take_auto_str(state)
                    .or_else(|| match &args[0] {
                        ast::Expr::Str(v) => Some(v.clone()),
                        ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
                        _ => None,
                    })
                    .unwrap_or_default();
                let cookie_hdr = {
                    let g = HTTP_CTX.lock().unwrap();
                    g.as_ref()
                        .and_then(|c| {
                            c.headers
                                .iter()
                                .find(|(h, _)| h.eq_ignore_ascii_case("cookie"))
                                .map(|(_, v)| v.clone())
                        })
                        .unwrap_or_default()
                };
                let out = http_cookie_get(&cookie_hdr, &nm);
                let k = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(k, out);
                return Ok(0);
            }
            if name == "http::cookie_get" && args.len() == 2 {
                let hdr = resolve_arg_str(&args[0], state);
                let nm = resolve_arg_str(&args[1], state);
                let out = http_cookie_get(&hdr, &nm);
                let k = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(k, out);
                return Ok(0);
            }
            // ── A4: html ──
            if name == "html::escape" && args.len() == 1 {
                let s = resolve_arg_str(&args[0], state);
                let out = html_escape(&s);
                let k = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(k, out);
                return Ok(0);
            }
            if name == "html::unescape" && args.len() == 1 {
                let s = resolve_arg_str(&args[0], state);
                let out = html_unescape(&s);
                let k = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(k, out);
                return Ok(0);
            }
            // ── A4: %{key} templates ──
            if name == "tmpl::render" && args.len() >= 3 && args.len() % 2 == 1 {
                // Evaluate every argument left to right, collecting strings.
                let mut vals: Vec<String> = Vec::with_capacity(args.len());
                for a in args {
                    eval_expr(a, state)?;
                    vals.push(
                        take_auto_str(state)
                            .or_else(|| match a {
                                ast::Expr::Str(v) => Some(v.clone()),
                                ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
                                _ => None,
                            })
                            .unwrap_or_default(),
                    );
                }
                let template = vals.remove(0);
                let pairs: Vec<(&str, &str)> = vals
                    .chunks(2)
                    .filter_map(|c| match c {
                        [k, v] => Some((k.as_str(), v.as_str())),
                        _ => None,
                    })
                    .collect();
                let out = tmpl_render(&template, &pairs);
                let k = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(k, out);
                return Ok(0);
            }
            if name == "http::serve_static" && args.len() == 1 {
                let d = take_auto_str(state)
                    .or_else(|| match &args[0] {
                        ast::Expr::Str(v) => Some(v.clone()),
                        ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
                        _ => None,
                    })
                    .unwrap_or_default();
                *STATIC_DIR.lock().unwrap() = d;
                return Ok(0);
            }
            if (name == "http::serve" || name == "http::serve_once") && !args.is_empty() {
                let port = eval_expr(&args[0], state)?;
                let handler_name = match &args[1] {
                    ast::Expr::Str(s) => s.clone(),
                    _ => String::new(),
                };
                let once = name == "http::serve_once";
                // Each request re-runs the named function with the path.
                let mut call_handler = |path: &str| -> anyhow::Result<String> {
                    let mut local = InterpreterState::new();
                    local.functions = state.functions.clone();
                    local.impl_methods = state.impl_methods.clone();
                    local.struct_fields = state.struct_fields.clone();
                    local.struct_instances = state.struct_instances.clone();
                    local.struct_type_of = state.struct_type_of.clone();
                    // Bind the path parameter as a string variable.
                    if let Some((params, _ret, body)) = state.functions.get(&handler_name).cloned()
                    {
                        if let Some(p0) = params.first() {
                            local.str_vars.insert(p0.name.clone(), path.to_string());
                        }
                        exec_block(&body, &mut local)?;
                        // The handler's return string: a recorded plain return,
                        // else the newest __auto_ entry (string expressions).
                        if let Some(s) = local.last_returned_str.clone() {
                            return Ok(s);
                        }
                        let best = local
                            .str_vars
                            .keys()
                            .filter(|k| {
                                k.starts_with("__auto_")
                                    && !k.starts_with("__auto_arr_")
                                    && !k.starts_with("__auto_map_")
                            })
                            .max()
                            .cloned();
                        Ok(best
                            .and_then(|k| local.str_vars.get(&k).cloned())
                            .unwrap_or_else(|| "{}".to_string()))
                    } else {
                        Ok("{}".to_string())
                    }
                };
                http_serve(port, once, &mut call_handler)?;
                return Ok(0);
            }
            if name == "http::headers" && args.len() == 2 {
                let resp = take_auto_str(state)
                    .or_else(|| match &args[0] {
                        ast::Expr::Str(v) => Some(v.clone()),
                        ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
                        _ => None,
                    })
                    .unwrap_or_default();
                let nm = take_auto_str(state)
                    .or_else(|| match &args[1] {
                        ast::Expr::Str(v) => Some(v.clone()),
                        ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
                        _ => None,
                    })
                    .unwrap_or_default();
                let out = http_headers_extract(&resp, &nm);
                let k = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(k, out);
                return Ok(0);
            }
            if (name == "http::get"
                || name == "http::post"
                || name == "http::delete"
                || name == "http::put"
                || name == "http::patch")
                && !args.is_empty()
            {
                let url = take_auto_str(state)
                    .or_else(|| match &args[0] {
                        ast::Expr::Str(v) => Some(v.clone()),
                        ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
                        _ => None,
                    })
                    .unwrap_or_default();
                let body_arg = if args.len() > 1 {
                    take_auto_str(state).or_else(|| match &args[1] {
                        ast::Expr::Str(v) => Some(v.clone()),
                        ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
                        _ => None,
                    })
                } else {
                    None
                };
                let method = name.strip_prefix("http::").unwrap_or("GET").to_uppercase();
                let resp = match method.as_str() {
                    "GET" => http_client_request("GET", &url, None),
                    "DELETE" => http_client_request("DELETE", &url, None),
                    "POST" => http_client_request("POST", &url, body_arg.as_deref()),
                    "PUT" => http_client_request("PUT", &url, body_arg.as_deref()),
                    _ => http_client_request("PATCH", &url, body_arg.as_deref()),
                };
                let k = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(k, resp);
                return Ok(0);
            }
            if name == "http::status_code" && args.len() == 1 {
                let resp = take_auto_str(state)
                    .or_else(|| match &args[0] {
                        ast::Expr::Str(v) => Some(v.clone()),
                        ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
                        _ => None,
                    })
                    .unwrap_or_default();
                return Ok(http_status_code(&resp));
            }
            // Lambda call
            if let Some((params, body, cap)) = state.lambdas.get(name).cloned() {
                let mut local_state = InterpreterState::new();
                local_state.functions = state.functions.clone();
                local_state.struct_fields = state.struct_fields.clone();
                local_state.inject_scope(&cap);
                // Inject explicit arguments (override captured if same name)
                for (param, arg) in params.iter().zip(args.iter()) {
                    let val = eval_expr(arg, state)?;
                    local_state.vars.insert(param.name.clone(), val);
                    // If arg is an Ident referring to a struct, copy the struct instance data
                    if let ast::Expr::Ident(arg_name) = arg {
                        if let Some(inst) = state.struct_instances.get(arg_name.as_str()) {
                            local_state
                                .struct_instances
                                .insert(param.name.clone(), inst.clone());
                        }
                        if let Some(ty) = state.struct_type_of.get(arg_name.as_str()) {
                            local_state
                                .struct_type_of
                                .insert(param.name.clone(), ty.clone());
                        }
                    }
                }
                match exec_block(&body, &mut local_state)? {
                    Some(v) => Ok(v),
                    None => Ok(0),
                }
            } else if let Some((params, _ret, body)) = state
                .functions
                .get(name)
                .cloned()
                .or_else(|| state.impl_methods.get(name).cloned())
            {
                let mut local_state = InterpreterState::new();
                local_state.functions = state.functions.clone();
                local_state.impl_methods = state.impl_methods.clone();
                local_state.struct_fields = state.struct_fields.clone();
                local_state.struct_instances = state.struct_instances.clone();
                local_state.struct_type_of = state.struct_type_of.clone();
                for (param, arg) in params.iter().zip(args.iter()) {
                    let val = eval_expr(arg, state)?;
                    local_state.vars.insert(param.name.clone(), val);
                    // If arg is an Ident referring to a struct, copy the struct instance data
                    if let ast::Expr::Ident(arg_name) = arg {
                        if let Some(inst) = state.struct_instances.get(arg_name.as_str()) {
                            local_state
                                .struct_instances
                                .insert(param.name.clone(), inst.clone());
                        }
                        if let Some(ty) = state.struct_type_of.get(arg_name.as_str()) {
                            local_state
                                .struct_type_of
                                .insert(param.name.clone(), ty.clone());
                        }
                        // Maps pass by reference (C semantics): copy in, write back after
                        if let Some(m) = state.map_vars.get(arg_name.as_str()) {
                            local_state.map_vars.insert(param.name.clone(), m.clone());
                        }
                    }
                }
                let result = exec_block(&body, &mut local_state)?.unwrap_or_default();
                // Write back mutated map args (by-reference semantics)
                for (param, arg) in params.iter().zip(args.iter()) {
                    if let ast::Expr::Ident(arg_name) = arg {
                        if let Some(m) = local_state.map_vars.get(param.name.as_str()) {
                            state.map_vars.insert(arg_name.clone(), m.clone());
                        }
                    }
                }
                // A string returned from the callee (a plain `return <str>`)
                // surfaces here; the caller sees it as the newest __auto_ entry.
                if let Some(sv) = local_state.last_returned_str.clone() {
                    let k = format!("__auto_{}{}", state.str_vars.len(), "_fnret");
                    state.str_vars.insert(k, sv);
                }
                // A map returned from the callee surfaces as a new __auto_map_ key;
                // copy callee-created maps so the caller's Let can transfer them.
                let returned: Vec<(String, Vec<(String, i64)>)> = local_state
                    .map_vars
                    .iter()
                    .filter(|(k, _)| k.starts_with("__auto_map_"))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                for (k, v) in returned {
                    state.map_vars.insert(k, v);
                }
                Ok(result)
            } else {
                Ok(0)
            }
        }
        ast::Expr::Range {
            start,
            end,
            inclusive,
        } => {
            let s = eval_expr(start, state)?;
            let e = eval_expr(end, state)?;
            let count = if *inclusive { e - s + 1 } else { e - s };
            Ok(if count > 0 { count } else { 0 })
        }
        ast::Expr::FieldAccess { target, field } => {
            if let ast::Expr::Ident(name) = target.as_ref() {
                // Struct field access: my_struct.field
                if let Some(instance) = state.struct_instances.get(name) {
                    // Look up field defs using the type name, not the variable name
                    let type_name = state.struct_type_of.get(name).cloned();
                    if let Some(ref tn) = type_name {
                        if let Some(fields) = state.struct_fields.get(tn) {
                            if let Some(idx) = fields.iter().position(|f| f == field) {
                                return Ok(instance.get(idx).copied().unwrap_or(0));
                            }
                        }
                    }
                }
                // Fallback: string/array .len
                if let Some(s) = state.str_vars.get(name) {
                    if field == "len" {
                        return Ok(s.len() as i64);
                    }
                }
                if let Some(a) = state.arr_vars.get(name) {
                    if field == "len" {
                        return Ok(a.len() as i64);
                    }
                }
            }
            Ok(0)
        }
        ast::Expr::MethodCall {
            target,
            method,
            args,
        } => {
            // Built-in method sugar. Struct/trait methods resolve elsewhere;
            // here we handle string and array builtins natively.
            // Chained targets (s.trim().to_lower()) carry their value in a fresh
            // __auto_ string key: resolve the target first so chains work.
            let mut chain_target: Option<ast::Expr> = None;
            if !matches!(target.as_ref(), ast::Expr::Ident(_)) {
                let before: std::collections::HashSet<String> =
                    state.str_vars.keys().cloned().collect();
                eval_expr(target, state)?;
                if let Some(k) = state
                    .str_vars
                    .keys()
                    .filter(|k| k.starts_with("__auto_") && !before.contains(*k))
                    .max_by(|x, y| x.cmp(y))
                    .cloned()
                {
                    chain_target = Some(ast::Expr::Ident(k));
                }
            }
            let target = chain_target.map(Box::new).unwrap_or_else(|| target.clone());
            let is_map_target =
                matches!(target.as_ref(), ast::Expr::Ident(n) if state.map_vars.contains_key(n));
            if is_map_target {
                let map_name = match target.as_ref() {
                    ast::Expr::Ident(n) => n.clone(),
                    _ => String::new(),
                };
                // Evaluate args: string keys via auto-keys/Str/str-var, int values via eval
                let mut arg_vals: Vec<i64> = Vec::new();
                let mut arg_strs: Vec<String> = Vec::new();
                for a in args {
                    match a {
                        ast::Expr::Str(s) => arg_strs.push(s.clone()),
                        ast::Expr::Int(v) => arg_vals.push(*v),
                        // A string variable is a key (e.g. m.insert(k, 5))
                        ast::Expr::Ident(n) if state.str_vars.contains_key(n) => {
                            arg_strs.push(state.str_vars.get(n).cloned().unwrap_or_default());
                        }
                        _ => {
                            let before: std::collections::HashSet<String> =
                                state.str_vars.keys().cloned().collect();
                            let v = eval_expr(a, state)?;
                            if let Some(k) = state
                                .str_vars
                                .keys()
                                .filter(|k| k.starts_with("__auto_") && !before.contains(*k))
                                .max_by(|x, y| x.cmp(y))
                                .cloned()
                            {
                                arg_strs.push(state.str_vars.get(&k).cloned().unwrap_or_default());
                                state.str_vars.remove(&k);
                            } else {
                                arg_vals.push(v);
                            }
                        }
                    }
                }
                let key = arg_strs.first().cloned().unwrap_or_default();
                let m = state.map_vars.get(&map_name).cloned().unwrap_or_default();
                match method.as_str() {
                    "insert" if arg_strs.len() == 1 && arg_vals.len() == 1 => {
                        let val = arg_vals[0];
                        let m_mut = state.map_vars.get_mut(&map_name).unwrap();
                        match m_mut.iter_mut().find(|(k, _)| *k == key) {
                            Some(entry) => entry.1 = val,
                            None => m_mut.push((key, val)),
                        }
                        return Ok(val);
                    }
                    "get" if arg_strs.len() == 1 && arg_vals.len() <= 1 => {
                        let def = arg_vals.first().copied().unwrap_or(0);
                        return Ok(m
                            .iter()
                            .find(|(k, _)| *k == key)
                            .map(|(_, v)| *v)
                            .unwrap_or(def));
                    }
                    "has" if arg_strs.len() == 1 => {
                        return Ok(m.iter().any(|(k, _)| *k == key) as i64);
                    }
                    "remove" if arg_strs.len() == 1 => {
                        let m_mut = state.map_vars.get_mut(&map_name).unwrap();
                        let old_len = m_mut.len();
                        m_mut.retain(|(k, _)| *k != key);
                        return Ok((m_mut.len() != old_len) as i64);
                    }
                    "keys" if args.is_empty() => {
                        let joined = m
                            .iter()
                            .map(|(k, _)| k.clone())
                            .collect::<Vec<_>>()
                            .join(", ");
                        let out_key = format!("__auto_{}", state.str_vars.len());
                        state.str_vars.insert(out_key, joined);
                        return Ok(0);
                    }
                    "len" if args.is_empty() => return Ok(m.len() as i64),
                    _ => {
                        return Err(anyhow::anyhow!(
                            "Unknown map method '{}' (with {} argument(s))",
                            method,
                            args.len()
                        ));
                    }
                }
            }
            let is_array_target =
                matches!(target.as_ref(), ast::Expr::Ident(n) if state.arr_vars.contains_key(n));
            let is_string_target =
                matches!(target.as_ref(), ast::Expr::Ident(n) if state.str_vars.contains_key(n));
            if is_array_target {
                match method.as_str() {
                    "map" | "filter" if args.len() == 1 => {
                        let call = ast::Expr::Call {
                            name: method.clone(),
                            type_args: vec![],
                            args: vec![target.as_ref().clone(), args[0].clone()],
                        };
                        return eval_expr(&call, state);
                    }
                    "reduce" if args.len() == 2 => {
                        let call = ast::Expr::Call {
                            name: "reduce".to_string(),
                            type_args: vec![],
                            args: vec![target.as_ref().clone(), args[0].clone(), args[1].clone()],
                        };
                        return eval_expr(&call, state);
                    }
                    "push" if args.len() == 1 => {
                        let v = eval_expr(&args[0], state)?;
                        if let ast::Expr::Ident(n) = target.as_ref() {
                            if let Some(a) = state.arr_vars.get_mut(n) {
                                a.push(v);
                            }
                        }
                        return Ok(0);
                    }
                    "pop" if args.is_empty() => {
                        if let ast::Expr::Ident(n) = target.as_ref() {
                            if let Some(a) = state.arr_vars.get_mut(n) {
                                return Ok(a.pop().unwrap_or(0));
                            }
                        }
                        return Ok(0);
                    }
                    "sort" if args.is_empty() => {
                        if let ast::Expr::Ident(n) = target.as_ref() {
                            if let Some(a) = state.arr_vars.get_mut(n) {
                                a.sort();
                            }
                        }
                        return Ok(0);
                    }
                    "reverse" if args.is_empty() => {
                        if let ast::Expr::Ident(n) = target.as_ref() {
                            if let Some(a) = state.arr_vars.get_mut(n) {
                                a.reverse();
                            }
                        }
                        return Ok(0);
                    }
                    _ => return Ok(0),
                }
            }
            if is_string_target {
                let s = match target.as_ref() {
                    ast::Expr::Ident(n) => state.str_vars.get(n).cloned().unwrap_or_default(),
                    _ => String::new(),
                };
                // Evaluate string-method args (all are strings or ints)
                let mut arg_vals: Vec<i64> = Vec::new();
                let mut arg_strs: Vec<String> = Vec::new();
                for a in args {
                    // Strings come back via __auto_ keys; ints via value
                    let before: std::collections::HashSet<String> =
                        state.str_vars.keys().cloned().collect();
                    let v = eval_expr(a, state)?;
                    let new_key = state
                        .str_vars
                        .keys()
                        .filter(|k| k.starts_with("__auto_") && !before.contains(*k))
                        .max_by(|x, y| x.cmp(y))
                        .cloned();
                    if let Some(k) = new_key {
                        arg_strs.push(state.str_vars.get(&k).cloned().unwrap_or_default());
                        state.str_vars.remove(&k);
                    } else if let ast::Expr::Str(sv) = a {
                        arg_strs.push(sv.clone());
                    } else {
                        arg_strs.push(String::new());
                        arg_vals.push(v);
                    }
                }
                let result: String = match method.as_str() {
                    "to_upper" => s.to_uppercase(),
                    "to_lower" => s.to_lowercase(),
                    "trim" => s.trim().to_string(),
                    "replace" if arg_strs.len() == 2 => s.replace(&arg_strs[0], &arg_strs[1]),
                    "substring" if arg_vals.len() == 2 => {
                        let start = arg_vals[0].max(0) as usize;
                        let end = (start + arg_vals[1].max(0) as usize).min(s.len());
                        s.chars()
                            .skip(start)
                            .take(end.saturating_sub(start))
                            .collect()
                    }
                    "char_at" if arg_vals.len() == 1 => {
                        // C runtime returns the char code; keep backends in parity
                        s.chars()
                            .nth(arg_vals[0].max(0) as usize)
                            .map(|c| (c as i64).to_string())
                            .unwrap_or_else(|| "0".to_string())
                    }
                    "repeat" if arg_vals.len() == 1 => s.repeat(arg_vals[0].max(0) as usize),
                    _ => {
                        // Boolean/int-valued methods: compute and return as int
                        let b: i64 = match method.as_str() {
                            "contains" if arg_strs.len() == 1 => s.contains(&arg_strs[0]) as i64,
                            "starts_with" if arg_strs.len() == 1 => {
                                s.starts_with(&arg_strs[0]) as i64
                            }
                            "ends_with" if arg_strs.len() == 1 => s.ends_with(&arg_strs[0]) as i64,
                            "equals" if arg_strs.len() == 1 => (s == arg_strs[0]) as i64,
                            "is_empty" if args.is_empty() => s.is_empty() as i64,
                            "len" => s.chars().count() as i64,
                            "find" if arg_strs.len() == 1 => {
                                s.find(&arg_strs[0]).map(|i| i as i64).unwrap_or(-1)
                            }
                            _ => return Err(anyhow::anyhow!("Unknown string method '{}'", method)),
                        };
                        return Ok(b);
                    }
                };
                let key = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(key, result);
                return Ok(0);
            }
            // Not a builtin target: fall back to struct/trait method call
            let method_name = if let ast::Expr::Ident(n) = target.as_ref() {
                // Try Type_method for struct instances
                if let Some(ty) = state.struct_type_of.get(n) {
                    format!("{}_{}", ty, method)
                } else {
                    method.clone()
                }
            } else {
                method.clone()
            };
            let mut call_args = vec![target.as_ref().clone()];
            call_args.extend(args.iter().cloned());
            let call = ast::Expr::Call {
                name: method_name,
                type_args: vec![],
                args: call_args,
            };
            eval_expr(&call, state)
        }
        ast::Expr::FString(parts) => {
            // Build the f-string by evaluating each part
            let mut result = String::new();
            for part in parts {
                match part {
                    crate::ast::FStringPart::Literal(s) => {
                        result.push_str(s);
                    }
                    crate::ast::FStringPart::Expr(expr) => {
                        // Evaluate the expression
                        let val = eval_expr(expr, state)?;
                        // Check if a new string was produced (from string ops)
                        let str_before: std::collections::HashSet<String> =
                            state.str_vars.keys().cloned().collect();
                        // The val is 0 for string expressions, so we need to
                        // check if a new auto-key was created
                        let new_str_key = state
                            .str_vars
                            .keys()
                            .filter(|k| k.starts_with("__auto_") && !str_before.contains(*k))
                            .max_by(|a, b| a.cmp(b))
                            .cloned();
                        if let Some(key) = new_str_key {
                            if let Some(s) = state.str_vars.get(&key) {
                                result.push_str(s);
                                // Clean up the consumed key
                                state.str_vars.remove(&key);
                            } else {
                                result.push_str(&val.to_string());
                            }
                        } else {
                            // Check if the expression is a string variable
                            if let ast::Expr::Ident(n) = expr.as_ref() {
                                if let Some(s) = state.str_vars.get(n) {
                                    result.push_str(s);
                                } else {
                                    result.push_str(&val.to_string());
                                }
                            } else {
                                result.push_str(&val.to_string());
                            }
                        }
                    }
                }
            }
            // Store the result in str_vars with an auto key
            let key = format!("__auto_{}", state.str_vars.len());
            state.str_vars.insert(key, result);
            Ok(0)
        }
        _ => Ok(0),
    }
}
