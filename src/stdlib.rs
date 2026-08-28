use crate::ast::Type;
use std::collections::HashMap;

/// Represents a built-in standard library function
pub struct StdlibFn {
    pub params: Vec<(String, Type)>,
    pub ret: Type,
}

/// Returns all built-in stdlib function signatures
pub fn builtins() -> HashMap<String, StdlibFn> {
    let mut m = HashMap::new();

    // ── math module ──
    register(&mut m, "math::abs", vec![("x".into(), Type::F64)], Type::F64);
    register(&mut m, "math::max", vec![("a".into(), Type::F64), ("b".into(), Type::F64)], Type::F64);
    register(&mut m, "math::min", vec![("a".into(), Type::F64), ("b".into(), Type::F64)], Type::F64);
    register(&mut m, "math::sqrt", vec![("x".into(), Type::F64)], Type::F64);
    register(&mut m, "math::pow", vec![("base".into(), Type::F64), ("exp".into(), Type::F64)], Type::F64);
    register(&mut m, "math::floor", vec![("x".into(), Type::F64)], Type::F64);
    register(&mut m, "math::ceil", vec![("x".into(), Type::F64)], Type::F64);
    register(&mut m, "math::log", vec![("x".into(), Type::F64)], Type::F64);
    register(&mut m, "math::log2", vec![("x".into(), Type::F64)], Type::F64);
    register(&mut m, "math::log10", vec![("x".into(), Type::F64)], Type::F64);

    // ── string module ──
    register(&mut m, "string::length", vec![("s".into(), Type::String)], Type::I64);
    register(&mut m, "string::concat", vec![("a".into(), Type::String), ("b".into(), Type::String)], Type::String);
    register(&mut m, "string::substring", vec![("s".into(), Type::String), ("start".into(), Type::I64), ("len".into(), Type::I64)], Type::String);
    register(&mut m, "string::equals", vec![("a".into(), Type::String), ("b".into(), Type::String)], Type::Bool);
    register(&mut m, "string::trim", vec![("s".into(), Type::String)], Type::String);
    register(&mut m, "string::starts_with", vec![("s".into(), Type::String), ("prefix".into(), Type::String)], Type::Bool);
    register(&mut m, "string::contains", vec![("s".into(), Type::String), ("sub".into(), Type::String)], Type::Bool);
    register(&mut m, "string::find", vec![("s".into(), Type::String), ("sub".into(), Type::String)], Type::I64);

    // ── array module ──
    register(&mut m, "array::len", vec![("arr".into(), Type::Array(Box::new(Type::I64)))], Type::I64);
    register(&mut m, "array::push", vec![("arr".into(), Type::Array(Box::new(Type::I64))), ("elem".into(), Type::I64)], Type::Void);
    register(&mut m, "array::sort", vec![("arr".into(), Type::Array(Box::new(Type::I64)))], Type::Void);

    // ── v2.0: json module ──
    register(&mut m, "json::stringify", vec![("v".into(), Type::I64)], Type::String);
    register(&mut m, "json::stringify_float", vec![("v".into(), Type::F64)], Type::String);
    register(&mut m, "json::parse", vec![("s".into(), Type::String)], Type::I64);
    register(&mut m, "json::get", vec![("s".into(), Type::String), ("key".into(), Type::String)], Type::String);
    register(&mut m, "json::stringify_string", vec![("s".into(), Type::String)], Type::String);
    register(&mut m, "json::stringify_bool", vec![("b".into(), Type::Bool)], Type::String);
    register(&mut m, "json::parse_float", vec![("s".into(), Type::String)], Type::F64);
    register(&mut m, "json::parse_string", vec![("s".into(), Type::String)], Type::String);
    register(&mut m, "json::has_key", vec![("s".into(), Type::String), ("key".into(), Type::String)], Type::Bool);
    register(&mut m, "json::array_len", vec![("s".into(), Type::String)], Type::I64);
    register(&mut m, "json::parse_object", vec![("s".into(), Type::String)], Type::String);
    register(&mut m, "json::map_get", vec![("s".into(), Type::String), ("key".into(), Type::String)], Type::String);
    register(&mut m, "json::map_keys", vec![("s".into(), Type::String)], Type::String);
    register(&mut m, "json::map_len", vec![("s".into(), Type::String)], Type::I64);

    // ── v2.0: http module ──
    register(&mut m, "http::get", vec![("url".into(), Type::String)], Type::String);
    register(&mut m, "http::post", vec![("url".into(), Type::String), ("body".into(), Type::String)], Type::String);
    register(&mut m, "http::serve_once", vec![("port".into(), Type::I64), ("handler".into(), Type::String), ("arg".into(), Type::I64)], Type::Void);
    register(&mut m, "http::serve", vec![("port".into(), Type::I64), ("handler".into(), Type::String), ("arg".into(), Type::I64)], Type::Void);
    register(&mut m, "http::status_code", vec![("s".into(), Type::String)], Type::I64);
    register(&mut m, "http::delete", vec![("url".into(), Type::String)], Type::String);
    register(&mut m, "http::put", vec![("url".into(), Type::String), ("body".into(), Type::String)], Type::String);
    register(&mut m, "http::patch", vec![("url".into(), Type::String), ("body".into(), Type::String)], Type::String);
    register(&mut m, "http::headers", vec![("s".into(), Type::String), ("name".into(), Type::String)], Type::String);

    // ── v2.0: concurrency ──
    register(&mut m, "spawn", vec![("fn_name".into(), Type::String), ("arg".into(), Type::I64)], Type::Void);
    register(&mut m, "chan::create", vec![], Type::I64);
    register(&mut m, "chan::send", vec![("ch".into(), Type::I64), ("val".into(), Type::I64)], Type::Void);
    register(&mut m, "chan::recv", vec![("ch".into(), Type::I64)], Type::I64);
    register(&mut m, "sleep", vec![("ms".into(), Type::I64)], Type::Void);
    register(&mut m, "time::ms", vec![], Type::I64);

    // ── v2.0: database (file-backed persistence) ──
    register(&mut m, "db::open", vec![("path".into(), Type::String)], Type::I64);
    register(&mut m, "db::close", vec![("handle".into(), Type::I64)], Type::Void);
    register(&mut m, "db::put", vec![("handle".into(), Type::I64), ("key".into(), Type::String), ("val".into(), Type::I64)], Type::Void);
    register(&mut m, "db::get", vec![("handle".into(), Type::I64), ("key".into(), Type::String)], Type::I64);
    register(&mut m, "db::delete", vec![("handle".into(), Type::I64), ("key".into(), Type::String)], Type::Void);
    register(&mut m, "db::count", vec![("handle".into(), Type::I64)], Type::I64);

    m
}

fn register(m: &mut HashMap<String, StdlibFn>, name: &str, params: Vec<(String, Type)>, ret: Type) {
    m.insert(name.to_string(), StdlibFn { params, ret });
}

/// Returns true if the function name is a known stdlib builtin
pub fn is_builtin(name: &str) -> bool {
    builtins().contains_key(name)
}

/// Maps a stdlib function name to its C equivalent
pub fn c_name(name: &str) -> &str {
    match name {
        "math::abs" => "fabs",
        "math::max" => "fmax",
        "math::min" => "fmin",
        "math::sqrt" => "sqrt",
        "math::pow" => "pow",
        "math::floor" => "floor",
        "math::ceil" => "ceil",
        "math::log" => "log",
        "math::log2" => "log2",
        "math::log10" => "log10",
        "string::length" => "__sbx_str_len",
        "string::concat" => "__sbx_str_concat",
        "string::substring" => "__sbx_str_sub",
        "string::equals" => "__sbx_str_eq",
        "string::trim" => "__sbx_str_trim",
        "string::starts_with" => "__sbx_str_starts_with",
        "string::contains" => "__sbx_str_contains",
        "string::find" => "__sbx_str_find",
        "array::len" => "__sbx_arr_len",
        "array::push" => "__sbx_arr_push",
        "array::sort" => "__sbx_arr_sort",
        "json::stringify" => "__sbx_json_stringify",
        "json::stringify_float" => "__sbx_json_stringify_float",
        "json::parse" => "__sbx_json_parse",
        "json::get" => "__sbx_json_get",
        "json::stringify_string" => "__sbx_json_stringify_string",
        "json::stringify_bool" => "__sbx_json_stringify_bool",
        "json::parse_float" => "__sbx_json_parse_float",
        "json::parse_string" => "__sbx_json_parse_string",
        "json::has_key" => "__sbx_json_has_key",
        "json::array_len" => "__sbx_json_array_len",
        "json::parse_object" => "__sbx_json_parse_object",
        "json::map_get" => "__sbx_json_map_get",
        "json::map_keys" => "__sbx_json_map_keys",
        "json::map_len" => "__sbx_json_map_len",
        "http::get" => "__sbx_http_get",
        "http::post" => "__sbx_http_post",
        "http::serve_once" => "__sbx_serve_once",
        "http::serve" => "__sbx_serve",
        "http::status_code" => "__sbx_http_status",
        "http::delete" => "__sbx_http_delete",
        "http::put" => "__sbx_http_put",
        "http::patch" => "__sbx_http_patch",
        "http::headers" => "__sbx_http_headers",
        "spawn" => "__sbx_spawn",
        "chan::create" => "__sbx_chan_create",
        "chan::send" => "__sbx_chan_send",
        "chan::recv" => "__sbx_chan_recv",
        "sleep" => "__sbx_sleep",
        "time::ms" => "__sbx_time_ms",
        "db::open" => "__sbx_db_open",
        "db::close" => "__sbx_db_close",
        "db::put" => "__sbx_db_put",
        "db::get" => "__sbx_db_get",
        "db::delete" => "__sbx_db_delete",
        "db::count" => "__sbx_db_count",
        _ => name,
    }
}

/// Returns C preamble for stdlib helper functions
pub fn c_preamble() -> String {
    r#"
/* ── Sandbox Standard Library (C runtime) ── */

#include <math.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <unistd.h>
#include <pthread.h>
#include <sys/socket.h>
#include <netdb.h>
#include <arpa/inet.h>
#include <time.h>
#include <stdarg.h>

/* ── string helpers ── */

static long __sbx_str_len(const char* s) {
    return (long)strlen(s);
}

static const char* __sbx_str_concat(const char* a, const char* b) {
    size_t la = strlen(a);
    size_t lb = strlen(b);
    char* out = (char*)malloc(la + lb + 1);
    memcpy(out, a, la);
    memcpy(out + la, b, lb);
    out[la + lb] = '\0';
    return out;
}

static const char* __sbx_str_sub(const char* s, long start, long len) {
    size_t slen = strlen(s);
    if (start < 0) start = 0;
    if ((size_t)start >= slen) {
        char* empty = (char*)malloc(1);
        empty[0] = '\0';
        return empty;
    }
    if ((size_t)(start + len) > slen) len = (long)(slen - start);
    char* out = (char*)malloc((size_t)len + 1);
    memcpy(out, s + start, (size_t)len);
    out[len] = '\0';
    return out;
}

static int __sbx_str_eq(const char* a, const char* b) {
    return strcmp(a, b) == 0;
}

static const char* __sbx_str_trim(const char* s) {
    while (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\r') s++;
    size_t len = strlen(s);
    while (len > 0 && (s[len-1] == ' ' || s[len-1] == '\t' || s[len-1] == '\n' || s[len-1] == '\r')) len--;
    char* out = (char*)malloc(len + 1);
    memcpy(out, s, len);
    out[len] = '\0';
    return out;
}

static int __sbx_str_starts_with(const char* s, const char* prefix) {
    return strncmp(s, prefix, strlen(prefix)) == 0;
}

static int __sbx_str_contains(const char* s, const char* sub) {
    return strstr(s, sub) != NULL;
}

static long __sbx_str_find(const char* s, const char* sub) {
    const char* pos = strstr(s, sub);
    return pos ? (long)(pos - s) : -1;
}

static long __sbx_arr_len(long* arr) {
    (void)arr;
    return 0;
}

static void __sbx_arr_push(long* arr, long elem) {
    (void)arr;
    (void)elem;
}

static void __sbx_arr_sort(long* arr, long len) {
    for (long i = 1; i < len; i++) {
        long key = arr[i];
        long j = i - 1;
        while (j >= 0 && arr[j] > key) {
            arr[j + 1] = arr[j];
            j--;
        }
        arr[j + 1] = key;
    }
}

/* ── v2.0: JSON helpers ── */

static const char* __sBx_json_stringify(long v) {
    char* out = (char*)malloc(32);
    snprintf(out, 32, "%ld", v);
    return out;
}

static const char* __sbx_json_stringify(long v) {
    static char buf[32];
    snprintf(buf, sizeof(buf), "%ld", v);
    return buf;
}

static const char* __sbx_json_stringify_float(double v) {
    char* out = (char*)malloc(64);
    snprintf(out, 64, "%f", v);
    return out;
}

static long __sbx_json_parse(const char* s) {
    /* Find the first number in the JSON text */
    const char* p = s;
    while (*p) {
        if ((*p >= '0' && *p <= '9') || *p == '-' || *p == '+') {
            return strtol(p, NULL, 10);
        }
        p++;
    }
    return 0;
}

static const char* __sbx_json_get(const char* s, const char* key) {
    /* Search for "key": and extract the value */
    char needle[256];
    snprintf(needle, sizeof(needle), "\"%s\"", key);
    const char* p = strstr(s, needle);
    if (!p) return "";
    p += strlen(needle);
    while (*p && (*p == ' ' || *p == ':' || *p == '\t')) p++;
    if (*p == '"') {
        p++;
        const char* end = strchr(p, '"');
        if (!end) return "";
        size_t len = (size_t)(end - p);
        char* out = (char*)malloc(len + 1);
        memcpy(out, p, len);
        out[len] = '\0';
        return out;
    }
    /* number or bare value: read until , } ] or space */
    const char* start = p;
    while (*p && *p != ',' && *p != '}' && *p != ']' && *p != '\n' && *p != '\r' && *p != ' ') p++;
    size_t len = (size_t)(p - start);
    char* out = (char*)malloc(len + 1);
    memcpy(out, start, len);
    out[len] = '\0';
    return out;
}

static const char* __sbx_json_stringify_string(const char* s) {
    /* Quote a string for JSON: hello -> "hello" */
    size_t len = strlen(s);
    char* out = (char*)malloc(len + 3);
    out[0] = '"';
    memcpy(out + 1, s, len);
    out[len + 1] = '"';
    out[len + 2] = '\0';
    return out;
}

static const char* __sbx_json_stringify_bool(long b) {
    return b ? "true" : "false";
}

static double __sbx_json_parse_float(const char* s) {
    /* Find the first float number in the JSON text */
    const char* p = s;
    while (*p) {
        if ((*p >= '0' && *p <= '9') || *p == '-' || *p == '+') {
            char* end;
            return strtod(p, &end);
        }
        p++;
    }
    return 0.0;
}

static const char* __sbx_json_parse_string(const char* s) {
    /* Find the first quoted string in JSON and return its content */
    const char* p = s;
    while (*p && *p != '"') p++;
    if (*p == '"') p++;
    else return "";
    const char* end = strchr(p, '"');
    if (!end) return "";
    size_t len = (size_t)(end - p);
    char* out = (char*)malloc(len + 1);
    memcpy(out, p, len);
    out[len] = '\0';
    return out;
}

static long __sbx_json_has_key(const char* s, const char* key) {
    /* Check if a key exists in a JSON object */
    char needle[256];
    snprintf(needle, sizeof(needle), "\"%s\"", key);
    return strstr(s, needle) != NULL ? 1 : 0;
}

static long __sbx_json_array_len(const char* s) {
    /* Count comma-separated elements in a JSON array "[1,2,3]" */
    const char* p = s;
    while (*p && *p != '[') p++;
    if (*p != '[') return 0;
    p++;
    if (*p == ']') return 0;
    long count = 1;
    int in_string = 0;
    while (*p && *p != ']') {
        if (*p == '"' && (p == s || *(p-1) != '\\')) in_string = !in_string;
        if (!in_string && *p == ',') count++;
        p++;
    }
    return count;
}

/* ── JSON object parsing (key-value pairs) ── */

/* Extract a JSON string value (without quotes) from position p.
   p should point to the opening '"'. Returns pointer after closing '"'.
   Writes the unquoted content into out (up to out_size-1 chars).
   Returns 1 on success, 0 on failure. */
static int __sbx_json_extract_string(const char** pp, char* out, size_t out_size) {
    const char* p = *pp;
    if (*p != '"') return 0;
    p++;
    size_t i = 0;
    while (*p && *p != '"' && i < out_size - 1) {
        if (*p == '\\' && *(p+1)) {
            p++;
            switch (*p) {
                case 'n': out[i++] = '\n'; break;
                case 't': out[i++] = '\t'; break;
                case 'r': out[i++] = '\r'; break;
                case '\\': out[i++] = '\\'; break;
                case '"': out[i++] = '"'; break;
                default: out[i++] = *p; break;
            }
        } else {
            out[i++] = *p;
        }
        p++;
    }
    if (*p != '"') return 0;
    p++;
    out[i] = '\0';
    *pp = p;
    return 1;
}

/* Skip a JSON value (string, number, bool, null, array, object) at *pp.
   Advances *pp past the value. */
static void __sbx_json_skip_value(const char** pp) {
    const char* p = *pp;
    if (*p == '"') {
        /* skip string */
        p++;
        while (*p && *p != '"') { if (*p == '\\' && *(p+1)) p++; p++; }
        if (*p == '"') p++;
    } else if (*p == '{' || *p == '[') {
        /* skip nested object or array */
        char open = *p;
        char close = (open == '{') ? '}' : ']';
        p++;
        int depth = 1;
        int in_str = 0;
        while (*p && depth > 0) {
            if (*p == '"' && (p == *(pp) || *(p-1) != '\\')) in_str = !in_str;
            if (!in_str) {
                if (*p == open) depth++;
                if (*p == close) depth--;
            }
            p++;
        }
    } else {
        /* skip number, bool, null */
        while (*p && *p != ',' && *p != '}' && *p != ']') p++;
    }
    *pp = p;
}

static const char* __sbx_json_parse_object(const char* s) {
    /* Parse {"key1":val1, "key2":val2} into null-separated pairs:
       "key1\0val1_str\0key2\0val2_str\0"
       Numeric values are stringified; string values are unquoted.
       Max output size: 64KB. */
    static char out[65536];
    size_t pos = 0;
    const char* p = s;
    /* find opening brace */
    while (*p && *p != '{') p++;
    if (*p != '{') { out[0] = '\0'; return out; }
    p++;
    char key[256], val[256];
    while (*p && *p != '}' && pos < sizeof(out) - 512) {
        /* skip whitespace and commas */
        while (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r' || *p == ',') p++;
        if (*p == '}' || *p == '\0') break;
        /* extract key */
        if (!__sbx_json_extract_string(&p, key, sizeof(key))) break;
        /* skip colon */
        while (*p == ' ' || *p == '\t') p++;
        if (*p != ':') break;
        p++;
        while (*p == ' ' || *p == '\t') p++;
        /* extract value */
        if (*p == '"') {
            /* string value */
            if (!__sbx_json_extract_string(&p, val, sizeof(val))) break;
        } else {
            /* number, bool, null — read until delimiter */
            const char* start = p;
            while (*p && *p != ',' && *p != '}' && *p != ' ' && *p != '\t' && *p != '\n' && *p != '\r') p++;
            size_t vlen = (size_t)(p - start);
            if (vlen >= sizeof(val)) vlen = sizeof(val) - 1;
            memcpy(val, start, vlen);
            val[vlen] = '\0';
        }
        /* write key\0value\0 to output */
        size_t klen = strlen(key);
        size_t vlen2 = strlen(val);
        if (pos + klen + vlen2 + 2 < sizeof(out)) {
            memcpy(out + pos, key, klen); pos += klen;
            out[pos++] = '\0';
            memcpy(out + pos, val, vlen2); pos += vlen2;
            out[pos++] = '\0';
        }
    }
    out[pos] = '\0';
    return out;
}

static const char* __sbx_json_map_get(const char* map_str, const char* key) {
    /* Search for key\0 in the null-separated map string.
       Returns the value after it, or "" if not found. */
    size_t klen = strlen(key);
    const char* p = map_str;
    while (*p) {
        if (strncmp(p, key, klen) == 0 && p[klen] == '\0') {
            return p + klen + 1; /* point to value */
        }
        /* skip key */
        while (*p && *p != '\0') p++;
        if (*p == '\0') p++;
        /* skip value */
        while (*p && *p != '\0') p++;
        if (*p == '\0') p++;
    }
    return "";
}

static const char* __sbx_json_map_keys(const char* map_str) {
    /* Extract all keys, comma-separated: "key1,key2,key3" */
    static char out[4096];
    size_t pos = 0;
    const char* p = map_str;
    int first = 1;
    while (*p) {
        /* read key */
        const char* kstart = p;
        while (*p && *p != '\0') p++;
        size_t klen = (size_t)(p - kstart);
        if (*p == '\0') p++; /* skip key null */
        /* skip value */
        while (*p && *p != '\0') p++;
        if (*p == '\0') p++; /* skip value null */
        if (!first && pos < sizeof(out) - 1) out[pos++] = ',';
        first = 0;
        if (pos + klen < sizeof(out)) {
            memcpy(out + pos, kstart, klen);
            pos += klen;
        }
    }
    out[pos] = '\0';
    return out;
}

static long __sbx_json_map_len(const char* map_str) {
    /* Count key-value pairs (each pair ends with two null bytes) */
    long count = 0;
    const char* p = map_str;
    while (*p) {
        /* skip key */
        while (*p && *p != '\0') p++;
        if (*p == '\0') p++;
        /* skip value */
        while (*p && *p != '\0') p++;
        if (*p == '\0') p++;
        count++;
    }
    return count;
}

/* ── v2.0: Channels ── */

#define SBX_MAX_CHANS 64
#define SBX_CHAN_CAP 64

typedef struct {
    long buf[SBX_CHAN_CAP];
    long head, tail, count;
    pthread_mutex_t mutex;
    pthread_cond_t not_empty;
    pthread_cond_t not_full;
    int used;
} sbx_chan;

static sbx_chan sbx_chans[SBX_MAX_CHANS];

static long __sbx_chan_create(void) {
    for (long i = 0; i < SBX_MAX_CHANS; i++) {
        if (!sbx_chans[i].used) {
            sbx_chans[i].head = 0;
            sbx_chans[i].tail = 0;
            sbx_chans[i].count = 0;
            sbx_chans[i].used = 1;
            pthread_mutex_init(&sbx_chans[i].mutex, NULL);
            pthread_cond_init(&sbx_chans[i].not_empty, NULL);
            pthread_cond_init(&sbx_chans[i].not_full, NULL);
            return i + 1; /* 1-based handle; 0 = invalid */
        }
    }
    return -1;
}

static void __sbx_chan_send(long ch, long val) {
    if (ch < 1 || ch > SBX_MAX_CHANS || !sbx_chans[ch - 1].used) return;
    sbx_chan* c = &sbx_chans[ch - 1];
    pthread_mutex_lock(&c->mutex);
    while (c->count >= SBX_CHAN_CAP) pthread_cond_wait(&c->not_full, &c->mutex);
    c->buf[c->tail] = val;
    c->tail = (c->tail + 1) % SBX_CHAN_CAP;
    c->count++;
    pthread_cond_signal(&c->not_empty);
    pthread_mutex_unlock(&c->mutex);
}

static long __sbx_chan_recv(long ch) {
    if (ch < 1 || ch > SBX_MAX_CHANS || !sbx_chans[ch - 1].used) return -1;
    sbx_chan* c = &sbx_chans[ch - 1];
    pthread_mutex_lock(&c->mutex);
    while (c->count <= 0) pthread_cond_wait(&c->not_empty, &c->mutex);
    long val = c->buf[c->head];
    c->head = (c->head + 1) % SBX_CHAN_CAP;
    c->count--;
    pthread_cond_signal(&c->not_full);
    pthread_mutex_unlock(&c->mutex);
    return val;
}

/* ── v2.0: Spawn + time ── */

typedef struct {
    void (*fn)(long);
    long arg;
} sbx_task;

static void* __sbx_task_run(void* p) {
    sbx_task* t = (sbx_task*)p;
    t->fn(t->arg);
    free(t);
    return NULL;
}

static void __sbx_spawn(void (*fn)(long), long arg) {
    sbx_task* t = (sbx_task*)malloc(sizeof(sbx_task));
    t->fn = fn;
    t->arg = arg;
    pthread_t tid;
    pthread_create(&tid, NULL, __sbx_task_run, t);
    pthread_detach(tid);
}

static void __sbx_sleep(long ms) {
    struct timespec ts;
    ts.tv_sec = ms / 1000;
    ts.tv_nsec = (ms % 1000) * 1000000L;
    nanosleep(&ts, NULL);
}

static long __sbx_time_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return ts.tv_sec * 1000L + ts.tv_nsec / 1000000L;
}

/* ── v2.0: File-backed key-value database ── */

#define SBX_MAX_DB 16

typedef struct {
    FILE* fp;
    char* path;
    int used;
} sbx_db;

static sbx_db sbx_dbs[SBX_MAX_DB];

static long __sbx_db_open(const char* path) {
    for (long i = 0; i < SBX_MAX_DB; i++) {
        if (!sbx_dbs[i].used) {
            FILE* fp = fopen(path, "a+");
            if (!fp) return -1;
            sbx_dbs[i].fp = fp;
            sbx_dbs[i].path = strdup(path);
            sbx_dbs[i].used = 1;
            return i + 1;
        }
    }
    return -1;
}

static void __sbx_db_close(long h) {
    if (h < 1 || h > SBX_MAX_DB || !sbx_dbs[h - 1].used) return;
    fclose(sbx_dbs[h - 1].fp);
    free(sbx_dbs[h - 1].path);
    sbx_dbs[h - 1].used = 0;
}

static void __sbx_db_put(long h, const char* key, long val) {
    if (h < 1 || h > SBX_MAX_DB || !sbx_dbs[h - 1].used) return;
    /* append key=val\n */
    fprintf(sbx_dbs[h - 1].fp, "%s=%ld\n", key, val);
    fflush(sbx_dbs[h - 1].fp);
}

static long __sbx_db_get(long h, const char* key) {
    if (h < 1 || h > SBX_MAX_DB || !sbx_dbs[h - 1].used) return -1;
    sbx_db* db = &sbx_dbs[h - 1];
    rewind(db->fp);
    char line[512];
    long result = -1;
    size_t klen = strlen(key);
    while (fgets(line, sizeof(line), db->fp)) {
        if (strncmp(line, key, klen) == 0 && line[klen] == '=') {
            result = strtol(line + klen + 1, NULL, 10);
        }
    }
    return result;
}

static void __sbx_db_delete(long h, const char* key) {
    if (h < 1 || h > SBX_MAX_DB || !sbx_dbs[h - 1].used) return;
    /* rewrite file without matching lines */
    sbx_db* db = &sbx_dbs[h - 1];
    char tmp[1024];
    snprintf(tmp, sizeof(tmp), "%s.tmp", db->path);
    FILE* out = fopen(tmp, "w");
    if (!out) return;
    fflush(db->fp);
    rewind(db->fp);
    char line[512];
    size_t klen = strlen(key);
    while (fgets(line, sizeof(line), db->fp)) {
        if (strncmp(line, key, klen) == 0 && line[klen] == '=') continue;
        fputs(line, out);
    }
    fclose(out);
    fclose(db->fp);
    if (rename(tmp, db->path) != 0) {
        db->fp = fopen(db->path, "a+");
        return;
    }
    db->fp = fopen(db->path, "a+");
}

static long __sbx_db_count(long h) {
    if (h < 1 || h > SBX_MAX_DB || !sbx_dbs[h - 1].used) return 0;
    sbx_db* db = &sbx_dbs[h - 1];
    rewind(db->fp);
    char line[512];
    long n = 0;
    while (fgets(line, sizeof(line), db->fp)) {
        if (line[0] != '\0' && line[0] != '\n') n++;
    }
    return n;
}

/* ── v2.0: HTTP client + server ── */

static long __sbx_http_status(const char* s) {
    /* response looks like: HTTP/1.1 200 OK ... */
    if (strncmp(s, "HTTP/", 5) != 0) return -1;
    const char* p = s + 9;
    return strtol(p, NULL, 10);
}

typedef struct {
    const char* host;
    int port;
    const char* path;
} sbx_url;

static int __sbx_parse_url(const char* url, sbx_url* u) {
    u->host = NULL;
    u->port = 80;
    u->path = "/";
    const char* p = url;
    if (strncmp(p, "http://", 7) == 0) {
        p += 7;
    } else if (strncmp(p, "https://", 8) == 0) {
        return 0; /* https not supported in v2.0 runtime */
    }
    u->host = p;
    while (*p && *p != ':' && *p != '/') p++;
    if (*p == ':') {
        u->port = (int)strtol(p + 1, NULL, 10);
        while (*p && *p != '/') p++;
    }
    if (*p == '/') u->path = p;
    return 1;
}

static const char* __sbx_http_request(const char* url, const char* method, const char* body) {
    sbx_url u;
    if (!__sbx_parse_url(url, &u) || !u.host) return "";
    char hostname[256];
    size_t hl = 0;
    const char* h = u.host;
    while (*h && *h != ':' && *h != '/' && hl < 255) { hostname[hl++] = *h; h++; }
    hostname[hl] = '\0';

    struct addrinfo hints, *res = NULL;
    memset(&hints, 0, sizeof(hints));
    hints.ai_family = AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    char portstr[16];
    snprintf(portstr, sizeof(portstr), "%d", u.port);
    if (getaddrinfo(hostname, portstr, &hints, &res) != 0) return "";
    int fd = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
    if (fd < 0) { freeaddrinfo(res); return ""; }
    if (connect(fd, res->ai_addr, res->ai_addrlen) != 0) {
        close(fd);
        freeaddrinfo(res);
        return "";
    }
    freeaddrinfo(res);

    char req[4096];
    if (body) {
        snprintf(req, sizeof(req),
            "%s %s HTTP/1.1\r\nHost: %s\r\nContent-Type: application/json\r\nContent-Length: %zu\r\nConnection: close\r\n\r\n%s",
            method, u.path, hostname, strlen(body), body);
    } else {
        snprintf(req, sizeof(req),
            "%s %s HTTP/1.1\r\nHost: %s\r\nConnection: close\r\n\r\n",
            method, u.path, hostname);
    }
    send(fd, req, strlen(req), 0);

    /* read full response (up to 64KB) */
    char* resp = (char*)malloc(65536);
    size_t total = 0;
    ssize_t n;
    while (total < 65535 && (n = recv(fd, resp + total, 65536 - total - 1, 0)) > 0) {
        total += (size_t)n;
    }
    resp[total] = '\0';
    close(fd);

    /* find header/body separator */
    char* sep = strstr(resp, "\r\n\r\n");
    if (sep) {
        char* body_out = sep + 4;
        char* result = strdup(body_out);
        free(resp);
        return result;
    }
    return resp;
}

static const char* __sbx_http_get(const char* url) {
    return __sbx_http_request(url, "GET", NULL);
}

static const char* __sbx_http_post(const char* url, const char* body) {
    return __sbx_http_request(url, "POST", body);
}

static const char* __sbx_http_delete(const char* url) {
    return __sbx_http_request(url, "DELETE", NULL);
}

static const char* __sbx_http_put(const char* url, const char* body) {
    return __sbx_http_request(url, "PUT", body);
}

static const char* __sbx_http_patch(const char* url, const char* body) {
    return __sbx_http_request(url, "PATCH", body);
}

static const char* __sbx_http_headers(const char* s, const char* name) {
    /* Extract header value from HTTP response.
       Response format: "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\nbody"
       We search for "Name: value\r\n" and return "value". */
    const char* p = s;
    /* skip first line (status line) */
    const char* eol = strstr(p, "\r\n");
    if (!eol) return "";
    p = eol + 2;

    /* build case-insensitive search for "Name: " */
    size_t nlen = strlen(name);
    while (*p && !(p[0] == '\r' && p[1] == '\n')) {
        /* check if this line starts with our header name */
        size_t i = 0;
        while (i < nlen && p[i] && (p[i] == name[i] || (p[i] >= 'A' && p[i] <= 'Z' && p[i] + 32 == name[i]) || (name[i] >= 'A' && name[i] <= 'Z' && name[i] + 32 == p[i]))) i++;
        if (i == nlen && p[i] == ':') {
            p += i + 1;
            while (*p == ' ' || *p == '\t') p++;
            const char* end = strstr(p, "\r\n");
            if (!end) return p;
            size_t len = (size_t)(end - p);
            char* out = (char*)malloc(len + 1);
            memcpy(out, p, len);
            out[len] = '\0';
            return out;
        }
        /* skip to next line */
        const char* next = strstr(p, "\r\n");
        if (!next) break;
        p = next + 2;
    }
    return "";
}

static void __sbx_serve_once(long port, const char* (*handler)(const char*)) {
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) { fprintf(stderr, "serve: socket failed\n"); return; }
    int opt = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_ANY);
    addr.sin_port = htons((uint16_t)port);
    if (bind(fd, (struct sockaddr*)&addr, sizeof(addr)) != 0) {
        fprintf(stderr, "serve: bind failed on port %ld\n", port);
        close(fd);
        return;
    }
    if (listen(fd, 4) != 0) { close(fd); return; }

    struct sockaddr_in client;
    socklen_t clen = sizeof(client);
    int cfd = accept(fd, (struct sockaddr*)&client, &clen);
    if (cfd < 0) { close(fd); return; }

    char req[8192];
    ssize_t n = recv(cfd, req, sizeof(req) - 1, 0);
    req[n > 0 ? n : 0] = '\0';

    /* parse request line: METHOD /path HTTP/1.1 */
    char path[1024] = "/";
    if (n > 0) {
        char* sp1 = strchr(req, ' ');
        if (sp1) {
            char* sp2 = strchr(sp1 + 1, ' ');
            size_t plen = sp2 ? (size_t)(sp2 - sp1 - 1) : strlen(sp1 + 1);
            if (plen < sizeof(path)) {
                memcpy(path, sp1 + 1, plen);
                path[plen] = '\0';
            }
        }
    }

    const char* body = handler ? handler(path) : "{}";
    size_t blen = strlen(body);
    char resp[8192 + 256];
    int rn = snprintf(resp, sizeof(resp),
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: %zu\r\nConnection: close\r\n\r\n%s",
        blen, body);
    send(cfd, resp, (size_t)rn, 0);
    close(cfd);
    close(fd);
}

static void __sbx_serve(long port, const char* (*handler)(const char*)) {
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) { fprintf(stderr, "serve: socket failed\n"); return; }
    int opt = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));
    struct sockaddr_in addr;
    memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_addr.s_addr = htonl(INADDR_ANY);
    addr.sin_port = htons((uint16_t)port);
    if (bind(fd, (struct sockaddr*)&addr, sizeof(addr)) != 0) {
        fprintf(stderr, "serve: bind failed on port %ld\n", port);
        close(fd);
        return;
    }
    if (listen(fd, 128) != 0) { close(fd); return; }
    fprintf(stderr, "sandbox: listening on port %ld\n", port);

    while (1) {
        struct sockaddr_in client;
        socklen_t clen = sizeof(client);
        int cfd = accept(fd, (struct sockaddr*)&client, &clen);
        if (cfd < 0) continue;

        char req[8192];
        ssize_t n = recv(cfd, req, sizeof(req) - 1, 0);
        req[n > 0 ? n : 0] = '\0';

        /* parse request line: METHOD /path HTTP/1.1 */
        char path[1024] = "/";
        if (n > 0) {
            char* sp1 = strchr(req, ' ');
            if (sp1) {
                char* sp2 = strchr(sp1 + 1, ' ');
                size_t plen = sp2 ? (size_t)(sp2 - sp1 - 1) : strlen(sp1 + 1);
                if (plen < sizeof(path)) {
                    memcpy(path, sp1 + 1, plen);
                    path[plen] = '\0';
                }
            }
        }

        const char* body = handler ? handler(path) : "{}";
        size_t blen = strlen(body);
        char resp[8192 + 256];
        int rn = snprintf(resp, sizeof(resp),
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: %zu\r\nConnection: close\r\n\r\n%s",
            blen, body);
        send(cfd, resp, (size_t)rn, 0);
        close(cfd);
    }
    close(fd);
}

/* ── Enum tagged union (for enums with payloads) ── */
typedef struct {
    long tag;
    union {
        double d;
        long i64_val;
    } payload;
} sbx_enum;

/* ── Range helpers ── */
/* Ranges are used by for-in loops; the codegen emits C for-loops directly.
   These helpers exist for cases where a range value is stored in a variable. */
typedef struct { long start; long end; int inclusive; } sbx_range_t;

static sbx_range_t sbx_range(long start, long end) {
    sbx_range_t r = { start, end, 0 };
    return r;
}

static sbx_range_t sbx_range_inclusive(long start, long end) {
    sbx_range_t r = { start, end, 1 };
    return r;
}

/* ── f-string helpers ── */
static const char* __sbx_to_string(long v) {
    char* buf = (char*)malloc(64);
    snprintf(buf, sizeof(buf), "%ld", v);
    return buf;
}

static const char* __sbx_to_string_f(double v) {
    char* buf = (char*)malloc(64);
    snprintf(buf, sizeof(buf), "%g", v);
    return buf;
}

static const char* __sbx_str_concat_multi(int count, ...) {
    /* Concatenate variadic string arguments */
    va_list args;
    va_start(args, count);
    size_t total = 0;
    /* First pass: measure lengths */
    for (int i = 0; i < count; i++) {
        const char* s = va_arg(args, const char*);
        if (s) total += strlen(s);
    }
    va_end(args);
    /* Second pass: concatenate */
    char* out = (char*)malloc(total + 1);
    out[0] = '\0';
    va_start(args, count);
    for (int i = 0; i < count; i++) {
        const char* s = va_arg(args, const char*);
        if (s) strcat(out, s);
    }
    va_end(args);
    return out;
}

/* ── End Sandbox Standard Library ── */
"#
    .to_string()
}
