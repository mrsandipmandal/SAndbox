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
    register(
        &mut m,
        "math::abs",
        vec![("x".into(), Type::F64)],
        Type::F64,
    );
    register(
        &mut m,
        "math::max",
        vec![("a".into(), Type::F64), ("b".into(), Type::F64)],
        Type::F64,
    );
    register(
        &mut m,
        "math::min",
        vec![("a".into(), Type::F64), ("b".into(), Type::F64)],
        Type::F64,
    );
    register(
        &mut m,
        "math::sqrt",
        vec![("x".into(), Type::F64)],
        Type::F64,
    );
    register(
        &mut m,
        "math::pow",
        vec![("base".into(), Type::F64), ("exp".into(), Type::F64)],
        Type::F64,
    );
    register(
        &mut m,
        "math::floor",
        vec![("x".into(), Type::F64)],
        Type::F64,
    );
    register(
        &mut m,
        "math::ceil",
        vec![("x".into(), Type::F64)],
        Type::F64,
    );
    register(
        &mut m,
        "math::log",
        vec![("x".into(), Type::F64)],
        Type::F64,
    );
    register(
        &mut m,
        "math::log2",
        vec![("x".into(), Type::F64)],
        Type::F64,
    );
    register(
        &mut m,
        "math::log10",
        vec![("x".into(), Type::F64)],
        Type::F64,
    );

    // ── string module ──
    register(
        &mut m,
        "string::length",
        vec![("s".into(), Type::String)],
        Type::I64,
    );
    register(
        &mut m,
        "string::concat",
        vec![("a".into(), Type::String), ("b".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "string::substring",
        vec![
            ("s".into(), Type::String),
            ("start".into(), Type::I64),
            ("len".into(), Type::I64),
        ],
        Type::String,
    );
    register(
        &mut m,
        "string::equals",
        vec![("a".into(), Type::String), ("b".into(), Type::String)],
        Type::Bool,
    );
    register(
        &mut m,
        "string::trim",
        vec![("s".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "string::starts_with",
        vec![("s".into(), Type::String), ("prefix".into(), Type::String)],
        Type::Bool,
    );
    register(
        &mut m,
        "string::contains",
        vec![("s".into(), Type::String), ("sub".into(), Type::String)],
        Type::Bool,
    );
    register(
        &mut m,
        "string::find",
        vec![("s".into(), Type::String), ("sub".into(), Type::String)],
        Type::I64,
    );

    // ── array module ──
    register(
        &mut m,
        "array::len",
        vec![("arr".into(), Type::Array(Box::new(Type::I64)))],
        Type::I64,
    );
    register(
        &mut m,
        "array::push",
        vec![
            ("arr".into(), Type::Array(Box::new(Type::I64))),
            ("elem".into(), Type::I64),
        ],
        Type::Void,
    );
    register(
        &mut m,
        "array::sort",
        vec![("arr".into(), Type::Array(Box::new(Type::I64)))],
        Type::Void,
    );

    // ── v2.0: json module ──
    register(
        &mut m,
        "json::stringify",
        vec![("v".into(), Type::I64)],
        Type::String,
    );
    register(
        &mut m,
        "json::stringify_float",
        vec![("v".into(), Type::F64)],
        Type::String,
    );
    register(
        &mut m,
        "json::parse",
        vec![("s".into(), Type::String)],
        Type::I64,
    );
    register(
        &mut m,
        "json::get",
        vec![("s".into(), Type::String), ("key".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "json::stringify_string",
        vec![("s".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "json::stringify_bool",
        vec![("b".into(), Type::Bool)],
        Type::String,
    );
    register(
        &mut m,
        "json::parse_float",
        vec![("s".into(), Type::String)],
        Type::F64,
    );
    register(
        &mut m,
        "json::parse_string",
        vec![("s".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "json::has_key",
        vec![("s".into(), Type::String), ("key".into(), Type::String)],
        Type::Bool,
    );
    register(
        &mut m,
        "json::array_len",
        vec![("s".into(), Type::String)],
        Type::I64,
    );
    register(
        &mut m,
        "json::parse_object",
        vec![("s".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "json::map_get",
        vec![("s".into(), Type::String), ("key".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "json::map_keys",
        vec![("s".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "json::map_len",
        vec![("s".into(), Type::String)],
        Type::I64,
    );

    // ── v2.0: http module ──
    register(
        &mut m,
        "http::get",
        vec![("url".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "http::post",
        vec![("url".into(), Type::String), ("body".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "http::serve_once",
        vec![
            ("port".into(), Type::I64),
            ("handler".into(), Type::String),
            ("arg".into(), Type::I64),
        ],
        Type::Void,
    );
    register(
        &mut m,
        "http::serve",
        vec![
            ("port".into(), Type::I64),
            ("handler".into(), Type::String),
            ("arg".into(), Type::I64),
        ],
        Type::Void,
    );
    register(
        &mut m,
        "http::status_code",
        vec![("s".into(), Type::String)],
        Type::I64,
    );
    register(
        &mut m,
        "http::delete",
        vec![("url".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "http::put",
        vec![("url".into(), Type::String), ("body".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "http::patch",
        vec![("url".into(), Type::String), ("body".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "http::headers",
        vec![("s".into(), Type::String), ("name".into(), Type::String)],
        Type::String,
    );

    // ── v2.0: concurrency ──
    register(
        &mut m,
        "spawn",
        vec![("fn_name".into(), Type::String), ("arg".into(), Type::I64)],
        Type::Void,
    );
    register(&mut m, "chan::create", vec![], Type::I64);
    register(
        &mut m,
        "chan::send",
        vec![("ch".into(), Type::I64), ("val".into(), Type::I64)],
        Type::Void,
    );
    register(
        &mut m,
        "chan::recv",
        vec![("ch".into(), Type::I64)],
        Type::I64,
    );
    register(&mut m, "sleep", vec![("ms".into(), Type::I64)], Type::Void);
    register(&mut m, "time::ms", vec![], Type::I64);

    // ── v2.0: Future ──
    register(
        &mut m,
        "future::wait",
        vec![("handle".into(), Type::I64)],
        Type::I64,
    );
    register(
        &mut m,
        "future::is_ready",
        vec![("handle".into(), Type::I64)],
        Type::I64,
    );

    // ── v2.1: file I/O module ──
    register(
        &mut m,
        "file::read",
        vec![("path".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "file::write",
        vec![("path".into(), Type::String), ("data".into(), Type::String)],
        Type::Void,
    );
    register(
        &mut m,
        "file::exists",
        vec![("path".into(), Type::String)],
        Type::Bool,
    );
    register(
        &mut m,
        "file::mkdir",
        vec![("path".into(), Type::String)],
        Type::Bool,
    );
    register(
        &mut m,
        "file::remove",
        vec![("path".into(), Type::String)],
        Type::Bool,
    );
    register(
        &mut m,
        "file::read_dir",
        vec![("path".into(), Type::String)],
        Type::String,
    );

    // ── v2.1: additional string methods ──
    register(
        &mut m,
        "string::replace",
        vec![
            ("s".into(), Type::String),
            ("from".into(), Type::String),
            ("to".into(), Type::String),
        ],
        Type::String,
    );
    register(
        &mut m,
        "string::to_upper",
        vec![("s".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "string::to_lower",
        vec![("s".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "string::char_at",
        vec![("s".into(), Type::String), ("i".into(), Type::I64)],
        Type::I64,
    );
    register(
        &mut m,
        "string::repeat",
        vec![("s".into(), Type::String), ("n".into(), Type::I64)],
        Type::String,
    );
    register(
        &mut m,
        "string::split",
        vec![("s".into(), Type::String), ("delim".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "string::join",
        vec![("arr".into(), Type::String), ("sep".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "string::parse_int",
        vec![("s".into(), Type::String)],
        Type::I64,
    );
    register(
        &mut m,
        "string::parse_float",
        vec![("s".into(), Type::String)],
        Type::F64,
    );
    register(
        &mut m,
        "string::ends_with",
        vec![("s".into(), Type::String), ("suffix".into(), Type::String)],
        Type::Bool,
    );
    register(
        &mut m,
        "string::is_empty",
        vec![("s".into(), Type::String)],
        Type::Bool,
    );

    // ── v2.0: database (file-backed persistence) ──
    register(
        &mut m,
        "db::open",
        vec![("path".into(), Type::String)],
        Type::I64,
    );
    register(
        &mut m,
        "db::close",
        vec![("handle".into(), Type::I64)],
        Type::Void,
    );
    register(
        &mut m,
        "db::put",
        vec![
            ("handle".into(), Type::I64),
            ("key".into(), Type::String),
            ("val".into(), Type::I64),
        ],
        Type::Void,
    );
    register(
        &mut m,
        "db::get",
        vec![("handle".into(), Type::I64), ("key".into(), Type::String)],
        Type::I64,
    );
    register(
        &mut m,
        "db::delete",
        vec![("handle".into(), Type::I64), ("key".into(), Type::String)],
        Type::Void,
    );
    register(
        &mut m,
        "db::count",
        vec![("handle".into(), Type::I64)],
        Type::I64,
    );

    // ── assert_eq! (builtin) ──
    register(&mut m, "assert_eq", vec![("a".into(), Type::I64), ("b".into(), Type::I64)], Type::Void);

    // ── collections module ──
    // List — opaque pointer handle (i64)
    register(&mut m, "list::new", vec![], Type::I64);
    register(&mut m, "list::len", vec![("l".into(), Type::I64)], Type::I64);
    register(&mut m, "list::push", vec![("l".into(), Type::I64), ("v".into(), Type::I64)], Type::Void);
    register(&mut m, "list::get", vec![("l".into(), Type::I64), ("i".into(), Type::I64)], Type::I64);
    register(&mut m, "list::set", vec![("l".into(), Type::I64), ("i".into(), Type::I64), ("v".into(), Type::I64)], Type::Void);
    register(&mut m, "list::contains", vec![("l".into(), Type::I64), ("v".into(), Type::I64)], Type::Bool);
    register(&mut m, "list::sort", vec![("l".into(), Type::I64)], Type::Void);
    register(&mut m, "list::remove", vec![("l".into(), Type::I64), ("i".into(), Type::I64)], Type::Void);
    register(&mut m, "list::is_empty", vec![("l".into(), Type::I64)], Type::Bool);
    // Map — opaque pointer handle (i64)
    register(&mut m, "map::new", vec![], Type::I64);
    register(&mut m, "map::len", vec![("m".into(), Type::I64)], Type::I64);
    register(&mut m, "map::insert", vec![("m".into(), Type::I64), ("k".into(), Type::String), ("v".into(), Type::I64)], Type::Void);
    register(&mut m, "map::get", vec![("m".into(), Type::I64), ("k".into(), Type::String)], Type::I64);
    register(&mut m, "map::contains", vec![("m".into(), Type::I64), ("k".into(), Type::String)], Type::Bool);
    register(&mut m, "map::remove", vec![("m".into(), Type::I64), ("k".into(), Type::String)], Type::Void);
    register(&mut m, "map::keys", vec![("m".into(), Type::I64)], Type::String);
    // Set — opaque pointer handle (i64)
    register(&mut m, "set_of::new", vec![], Type::I64);
    register(&mut m, "set_of::len", vec![("s".into(), Type::I64)], Type::I64);
    register(&mut m, "set_of::insert", vec![("s".into(), Type::I64), ("v".into(), Type::String)], Type::Void);
    register(&mut m, "set_of::contains", vec![("s".into(), Type::I64), ("v".into(), Type::String)], Type::Bool);
    register(&mut m, "set_of::remove", vec![("s".into(), Type::I64), ("v".into(), Type::String)], Type::Void);

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
        "string::replace" => "__sbx_str_replace",
        "string::to_upper" => "__sbx_str_to_upper",
        "string::to_lower" => "__sbx_str_to_lower",
        "string::char_at" => "__sbx_str_char_at",
        "string::repeat" => "__sbx_str_repeat",
        "string::split" => "__sbx_str_split",
        "string::join" => "__sbx_str_join",
        "string::parse_int" => "__sbx_str_parse_int",
        "string::parse_float" => "__sbx_str_parse_float",
        "string::ends_with" => "__sbx_str_ends_with",
        "string::is_empty" => "__sbx_str_is_empty",
        "file::read" => "__sbx_file_read",
        "file::write" => "__sbx_file_write",
        "file::exists" => "__sbx_file_exists",
        "file::mkdir" => "__sbx_file_mkdir",
        "file::remove" => "__sbx_file_remove",
        "file::read_dir" => "__sbx_file_read_dir",
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
        // collections
        "list::new" => "__sbx_list_new",
        "list::len" => "__sbx_list_len",
        "list::push" => "__sbx_list_push",
        "list::get" => "__sbx_list_get",
        "list::set" => "__sbx_list_set",
        "list::contains" => "__sbx_list_contains",
        "list::sort" => "__sbx_list_sort",
        "list::remove" => "__sbx_list_remove",
        "list::is_empty" => "__sbx_list_is_empty",
        "map::new" => "__sbx_map_new",
        "map::len" => "__sbx_map_len",
        "map::insert" => "__sbx_map_insert",
        "map::get" => "__sbx_map_get",
        "map::contains" => "__sbx_map_contains",
        "map::remove" => "__sbx_map_remove",
        "map::keys" => "__sbx_map_keys",
        "set_of::new" => "__sbx_set_new",
        "set_of::len" => "__sbx_set_len",
        "set_of::insert" => "__sbx_set_insert",
        "set_of::contains" => "__sbx_set_contains",
        "set_of::remove" => "__sbx_set_remove",
        "assert_eq" => "__sbx_assert_eq",
        "__sbx_rc_retain" => "sbx_rc_retain",
        "__sbx_rc_release" => "sbx_rc_release",
        "future::wait" => "__sbx_future_wait",
        "future::is_ready" => "__sbx_future_is_ready",
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
#include <setjmp.h>
#include <stdarg.h>
#include <sys/stat.h>
#include <dirent.h>

/* ── Reference Counting Runtime ── */

typedef struct {
    long refcount;
    size_t size;   /* size of payload (excluding header) */
} sbx_rc_header;

/* Allocate a new RC-managed block. refcount starts at 1. */
static void* sbx_rc_alloc(size_t size) {
    sbx_rc_header* h = (sbx_rc_header*)malloc(sizeof(sbx_rc_header) + size);
    if (!h) { fprintf(stderr, "sandbox: out of memory\n"); exit(1); }
    h->refcount = 1;
    h->size = size;
    return (void*)(h + 1);  /* return pointer past header */
}

/* Increment refcount */
static void sbx_rc_retain(void* ptr) {
    if (!ptr) return;
    sbx_rc_header* h = ((sbx_rc_header*)ptr) - 1;
    h->refcount++;
}

/* Decrement refcount; free if zero */
static void sbx_rc_release(void* ptr) {
    if (!ptr) return;
    sbx_rc_header* h = ((sbx_rc_header*)ptr) - 1;
    h->refcount--;
    if (h->refcount <= 0) {
        free(h);
    }
}

/* Get current refcount (for debugging) */
static long sbx_rc_refcount(void* ptr) {
    if (!ptr) return 0;
    sbx_rc_header* h = ((sbx_rc_header*)ptr) - 1;
    return h->refcount;
}

/* ── string helpers ── */

static long __sbx_str_len(const char* s) {
    return (long)strlen(s);
}

static const char* __sbx_str_concat(const char* a, const char* b) {
    size_t la = strlen(a);
    size_t lb = strlen(b);
    char* out = (char*)sbx_rc_alloc(la + lb + 1);
    memcpy(out, a, la);
    memcpy(out + la, b, lb);
    out[la + lb] = '\0';
    return out;
}

static const char* __sbx_str_sub(const char* s, long start, long len) {
    size_t slen = strlen(s);
    if (start < 0) start = 0;
    if ((size_t)start >= slen) {
        char* empty = (char*)sbx_rc_alloc(1);
        empty[0] = '\0';
        return empty;
    }
    if ((size_t)(start + len) > slen) len = (long)(slen - start);
    char* out = (char*)sbx_rc_alloc((size_t)len + 1);
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
    char* out = (char*)sbx_rc_alloc(len + 1);
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
    char* out = (char*)sbx_rc_alloc(32);
    snprintf(out, 32, "%ld", v);
    return out;
}

static const char* __sbx_json_stringify(long v) {
    static char buf[32];
    snprintf(buf, sizeof(buf), "%ld", v);
    return buf;
}

static const char* __sbx_json_stringify_float(double v) {
    char* out = (char*)sbx_rc_alloc(64);
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
        char* out = (char*)sbx_rc_alloc(len + 1);
        memcpy(out, p, len);
        out[len] = '\0';
        return out;
    }
    /* number or bare value: read until , } ] or space */
    const char* start = p;
    while (*p && *p != ',' && *p != '}' && *p != ']' && *p != '\n' && *p != '\r' && *p != ' ') p++;
    size_t len = (size_t)(p - start);
    char* out = (char*)sbx_rc_alloc(len + 1);
    memcpy(out, start, len);
    out[len] = '\0';
    return out;
}

static const char* __sbx_json_stringify_string(const char* s) {
    /* Quote a string for JSON: hello -> "hello" */
    size_t len = strlen(s);
    char* out = (char*)sbx_rc_alloc(len + 3);
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
    char* out = (char*)sbx_rc_alloc(len + 1);
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

/* ── v2.0: Spawn + time + Future ── */

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

/* ── Future type ──
   A Future stores a thread handle + result slot.
   future::wait() blocks until the thread completes.
   future::is_ready() checks non-blocking.
*/
#define SBX_MAX_FUTURES 128

typedef struct {
    pthread_t tid;
    long result;       /* result value (for long-returning futures) */
    void* result_ptr;  /* result pointer (for string-returning futures) */
    int done;          /* 1 = thread finished */
    int used;          /* 1 = slot in use */
    pthread_mutex_t mutex;
    pthread_cond_t done_cond;
} sbx_future;

static sbx_future sbx_futures[SBX_MAX_FUTURES];

/* Wrapper: runs user fn, stores result, marks done */
typedef struct {
    long (*fn)(void);
    int future_id;
} sbx_future_task;

static void* __sbx_future_run(void* p) {
    sbx_future_task* ft = (sbx_future_task*)p;
    long result = ft->fn();
    sbx_future* f = &sbx_futures[ft->future_id];
    pthread_mutex_lock(&f->mutex);
    f->result = result;
    f->done = 1;
    pthread_cond_signal(&f->done_cond);
    pthread_mutex_unlock(&f->mutex);
    free(ft);
    return NULL;
}

/* Create a future: spawns fn on a thread, returns future handle (1-based) */
static long __sbx_future_spawn(long (*fn)(void)) {
    for (long i = 0; i < SBX_MAX_FUTURES; i++) {
        if (!sbx_futures[i].used) {
            sbx_future* f = &sbx_futures[i];
            f->used = 1;
            f->done = 0;
            f->result = 0;
            f->result_ptr = NULL;
            pthread_mutex_init(&f->mutex, NULL);
            pthread_cond_init(&f->done_cond, NULL);
            sbx_future_task* ft = (sbx_future_task*)malloc(sizeof(sbx_future_task));
            ft->fn = fn;
            ft->future_id = (int)i;
            pthread_create(&f->tid, NULL, __sbx_future_run, ft);
            return i + 1; /* 1-based handle */
        }
    }
    return -1; /* no free slot */
}

/* Await: block until future completes, return result */
static long __sbx_future_wait(long handle) {
    if (handle < 1 || handle > SBX_MAX_FUTURES) return -1;
    sbx_future* f = &sbx_futures[handle - 1];
    if (!f->used) return -1;
    pthread_mutex_lock(&f->mutex);
    while (!f->done) {
        pthread_cond_wait(&f->done_cond, &f->mutex);
    }
    long result = f->result;
    pthread_mutex_unlock(&f->mutex);
    /* Cleanup */
    pthread_mutex_destroy(&f->mutex);
    pthread_cond_destroy(&f->done_cond);
    f->used = 0;
    return result;
}

/* Check if future is done (non-blocking) */
static long __sbx_future_is_ready(long handle) {
    if (handle < 1 || handle > SBX_MAX_FUTURES) return 0;
    sbx_future* f = &sbx_futures[handle - 1];
    if (!f->used) return 0;
    pthread_mutex_lock(&f->mutex);
    long ready = f->done;
    pthread_mutex_unlock(&f->mutex);
    return ready;
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
    char* buf = (char*)sbx_rc_alloc(64);
    snprintf(buf, 64, "%ld", v);
    return buf;
}

static const char* __sbx_to_string_f(double v) {
    char* buf = (char*)sbx_rc_alloc(64);
    snprintf(buf, 64, "%g", v);
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
    char* out = (char*)sbx_rc_alloc(total + 1);
    out[0] = '\0';
    va_start(args, count);
    for (int i = 0; i < count; i++) {
        const char* s = va_arg(args, const char*);
        if (s) strcat(out, s);
    }
    va_end(args);
    return out;
}

/* ── v2.1: File I/O helpers ── */

static const char* __sbx_file_read(const char* path) {
    FILE* fp = fopen(path, "r");
    if (!fp) return "";
    fseek(fp, 0, SEEK_END);
    long size = ftell(fp);
    fseek(fp, 0, SEEK_SET);
    char* buf = (char*)sbx_rc_alloc((size_t)size + 1);
    size_t n = fread(buf, 1, (size_t)size, fp);
    buf[n] = '\0';
    fclose(fp);
    return buf;
}

static long __sbx_file_write(const char* path, const char* data) {
    FILE* fp = fopen(path, "w");
    if (!fp) return 0;
    size_t len = strlen(data);
    fwrite(data, 1, len, fp);
    fclose(fp);
    return (long)len;
}

static long __sbx_file_exists(const char* path) {
    struct stat st;
    return stat(path, &st) == 0 ? 1 : 0;
}

static long __sbx_file_mkdir(const char* path) {
    return mkdir(path, 0755) == 0 ? 1 : 0;
}

static long __sbx_file_remove(const char* path) {
    return remove(path) == 0 ? 1 : 0;
}

static const char* __sbx_file_read_dir(const char* path) {
    DIR* d = opendir(path);
    if (!d) return "";
    /* Build comma-separated list of entries */
    static char out[65536];
    size_t pos = 0;
    struct dirent* ent;
    int first = 1;
    while ((ent = readdir(d)) != NULL && pos < sizeof(out) - 256) {
        if (strcmp(ent->d_name, ".") == 0 || strcmp(ent->d_name, "..") == 0) continue;
        if (!first && pos < sizeof(out) - 1) out[pos++] = ',';
        first = 0;
        size_t nlen = strlen(ent->d_name);
        if (pos + nlen < sizeof(out)) {
            memcpy(out + pos, ent->d_name, nlen);
            pos += nlen;
        }
    }
    out[pos] = '\0';
    closedir(d);
    return out;
}

/* ── v2.1: Additional string helpers ── */

static const char* __sbx_str_replace(const char* s, const char* from, const char* to) {
    size_t flen = strlen(from);
    if (flen == 0) return s;
    /* Count occurrences */
    long count = 0;
    const char* p = s;
    while ((p = strstr(p, from)) != NULL) { count++; p += flen; }
    size_t tlen = strlen(to);
    size_t slen = strlen(s);
    size_t outlen = slen + count * (tlen > flen ? tlen - flen : flen - tlen);
    char* out = (char*)sbx_rc_alloc(outlen + 1);
    char* dst = out;
    const char* src = s;
    const char* match;
    while ((match = strstr(src, from)) != NULL) {
        size_t prefix = (size_t)(match - src);
        memcpy(dst, src, prefix);
        dst += prefix;
        memcpy(dst, to, tlen);
        dst += tlen;
        src = match + flen;
    }
    strcpy(dst, src);
    return out;
}

static const char* __sbx_str_to_upper(const char* s) {
    size_t len = strlen(s);
    char* out = (char*)sbx_rc_alloc(len + 1);
    for (size_t i = 0; i < len; i++) {
        out[i] = (s[i] >= 'a' && s[i] <= 'z') ? s[i] - 32 : s[i];
    }
    out[len] = '\0';
    return out;
}

static const char* __sbx_str_to_lower(const char* s) {
    size_t len = strlen(s);
    char* out = (char*)sbx_rc_alloc(len + 1);
    for (size_t i = 0; i < len; i++) {
        out[i] = (s[i] >= 'A' && s[i] <= 'Z') ? s[i] + 32 : s[i];
    }
    out[len] = '\0';
    return out;
}

static long __sbx_str_char_at(const char* s, long i) {
    if (i < 0 || (size_t)i >= strlen(s)) return 0;
    return (long)(unsigned char)s[i];
}

static const char* __sbx_str_repeat(const char* s, long n) {
    if (n <= 0) {
        char* empty = (char*)sbx_rc_alloc(1);
        empty[0] = '\0';
        return empty;
    }
    size_t slen = strlen(s);
    size_t total = slen * (size_t)n;
    char* out = (char*)sbx_rc_alloc(total + 1);
    for (long i = 0; i < n; i++) {
        memcpy(out + i * slen, s, slen);
    }
    out[total] = '\0';
    return out;
}

static const char* __sbx_str_split(const char* s, const char* delim) {
    /* Returns comma-separated parts (simplified — real implementation would return array) */
    static char out[65536];
    size_t pos = 0;
    size_t dlen = strlen(delim);
    const char* p = s;
    int first = 1;
    while (*p) {
        const char* match = strstr(p, delim);
        size_t part_len = match ? (size_t)(match - p) : strlen(p);
        if (!first && pos < sizeof(out) - 1) out[pos++] = ',';
        first = 0;
        if (pos + part_len < sizeof(out)) {
            memcpy(out + pos, p, part_len);
            pos += part_len;
        }
        p += part_len;
        if (match) p += dlen;
        else break;
    }
    out[pos] = '\0';
    return out;
}

static const char* __sbx_str_join(const char* arr, const char* sep) {
    /* arr is comma-separated, join with sep */
    return arr; /* simplified — full impl would split and rejoin */
}

static long __sbx_str_parse_int(const char* s) {
    return strtol(s, NULL, 10);
}

static double __sbx_str_parse_float(const char* s) {
    return strtod(s, NULL);
}

static long __sbx_str_ends_with(const char* s, const char* suffix) {
    size_t slen = strlen(s);
    size_t suffix_len = strlen(suffix);
    if (suffix_len > slen) return 0;
    return strcmp(s + slen - suffix_len, suffix) == 0 ? 1 : 0;
}

static long __sbx_str_is_empty(const char* s) {
    return s[0] == '\0' ? 1 : 0;
}

/* ══════════════════════════════════════════════════════════════
   Collections: List<T>, Map<K,V>, Set<V>
   ══════════════════════════════════════════════════════════════ */

/* ── List ── */
typedef struct {
    long* data;
    long  len;
    long  cap;
} sbx_list;

static long __sbx_list_new(void) {
    sbx_list* l = (sbx_list*)sbx_rc_alloc(sizeof(sbx_list));
    l->data = NULL;
    l->len  = 0;
    l->cap  = 0;
    return (long)l;
}

static long __sbx_list_len(long ptr) {
    sbx_list* l = (sbx_list*)ptr;
    if (!l) return 0;
    return l->len;
}

static void __sbx_list_push(long ptr, long val) {
    sbx_list* l = (sbx_list*)ptr;
    if (!l) return;
    if (l->len >= l->cap) {
        l->cap = l->cap == 0 ? 8 : l->cap * 2;
        l->data = (long*)realloc(l->data, (size_t)l->cap * sizeof(long));
    }
    l->data[l->len++] = val;
}

static long __sbx_list_get(long ptr, long idx) {
    sbx_list* l = (sbx_list*)ptr;
    if (!l) return 0;
    if (idx < 0 || idx >= l->len) return 0;
    return l->data[idx];
}

static void __sbx_list_set(long ptr, long idx, long val) {
    sbx_list* l = (sbx_list*)ptr;
    if (!l) return;
    if (idx < 0 || idx >= l->len) return;
    l->data[idx] = val;
}

static long __sbx_list_contains(long ptr, long val) {
    sbx_list* l = (sbx_list*)ptr;
    if (!l) return 0;
    for (long i = 0; i < l->len; i++) {
        if (l->data[i] == val) return 1;
    }
    return 0;
}

static void __sbx_list_sort(long ptr) {
    sbx_list* l = (sbx_list*)ptr;
    if (!l) return;
    /* insertion sort */
    for (long i = 1; i < l->len; i++) {
        long key = l->data[i];
        long j = i - 1;
        while (j >= 0 && l->data[j] > key) {
            l->data[j + 1] = l->data[j];
            j--;
        }
        l->data[j + 1] = key;
    }
}

static void __sbx_list_remove(long ptr, long idx) {
    sbx_list* l = (sbx_list*)ptr;
    if (!l) return;
    if (idx < 0 || idx >= l->len) return;
    for (long i = idx; i < l->len - 1; i++) {
        l->data[i] = l->data[i + 1];
    }
    l->len--;
}

static long __sbx_list_is_empty(long ptr) {
    sbx_list* l = (sbx_list*)ptr;
    if (!l) return 1;
    return l->len == 0 ? 1 : 0;
}

/* ── Map ── */
typedef struct {
    const char** keys;
    long*        vals;
    long         len;
    long         cap;
} sbx_map;

static long __sbx_map_new(void) {
    sbx_map* m = (sbx_map*)sbx_rc_alloc(sizeof(sbx_map));
    m->keys = NULL;
    m->vals = NULL;
    m->len  = 0;
    m->cap  = 0;
    return (long)m;
}

static long __sbx_map_len(long ptr) {
    sbx_map* m = (sbx_map*)ptr;
    if (!m) return 0;
    return m->len;
}

static void __sbx_map_insert(long ptr, const char* key, long val) {
    sbx_map* m = (sbx_map*)ptr;
    if (!m) return;
    /* update existing key */
    for (long i = 0; i < m->len; i++) {
        if (strcmp(m->keys[i], key) == 0) {
            m->vals[i] = val;
            return;
        }
    }
    /* new key */
    if (m->len >= m->cap) {
        m->cap = m->cap == 0 ? 8 : m->cap * 2;
        m->keys = (const char**)realloc(m->keys, (size_t)m->cap * sizeof(const char*));
        m->vals = (long*)realloc(m->vals, (size_t)m->cap * sizeof(long));
    }
    /* copy the key string */
    size_t klen = strlen(key);
    char* kcopy = (char*)sbx_rc_alloc(klen + 1);
    memcpy(kcopy, key, klen + 1);
    m->keys[m->len] = kcopy;
    m->vals[m->len] = val;
    m->len++;
}

static long __sbx_map_get(long ptr, const char* key) {
    sbx_map* m = (sbx_map*)ptr;
    if (!m) return 0;
    for (long i = 0; i < m->len; i++) {
        if (strcmp(m->keys[i], key) == 0) return m->vals[i];
    }
    return 0;
}

static long __sbx_map_contains(long ptr, const char* key) {
    sbx_map* m = (sbx_map*)ptr;
    if (!m) return 0;
    for (long i = 0; i < m->len; i++) {
        if (strcmp(m->keys[i], key) == 0) return 1;
    }
    return 0;
}

static void __sbx_map_remove(long ptr, const char* key) {
    sbx_map* m = (sbx_map*)ptr;
    if (!m) return;
    for (long i = 0; i < m->len; i++) {
        if (strcmp(m->keys[i], key) == 0) {
            for (long j = i; j < m->len - 1; j++) {
                m->keys[j] = m->keys[j + 1];
                m->vals[j] = m->vals[j + 1];
            }
            m->len--;
            return;
        }
    }
}

/* Returns keys as a comma-separated string */
static const char* __sbx_map_keys(long ptr) {
    sbx_map* m = (sbx_map*)ptr;
    if (!m) return "";
    if (m->len == 0) return "";
    /* Estimate size */
    size_t total = 0;
    for (long i = 0; i < m->len; i++) {
        total += strlen(m->keys[i]) + 1; /* +1 for comma */
    }
    char* out = (char*)sbx_rc_alloc(total + 1);
    out[0] = '\0';
    for (long i = 0; i < m->len; i++) {
        if (i > 0) strcat(out, ",");
        strcat(out, m->keys[i]);
    }
    return out;
}

/* ── Set ── */
typedef struct {
    const char** items;
    long         len;
    long         cap;
} sbx_set;

static long __sbx_set_new(void) {
    sbx_set* s = (sbx_set*)sbx_rc_alloc(sizeof(sbx_set));
    s->items = NULL;
    s->len   = 0;
    s->cap   = 0;
    return (long)s;
}

static long __sbx_set_len(long ptr) {
    sbx_set* s = (sbx_set*)ptr;
    if (!s) return 0;
    return s->len;
}

static void __sbx_set_insert(long ptr, const char* val) {
    sbx_set* s = (sbx_set*)ptr;
    if (!s) return;
    /* check duplicate */
    for (long i = 0; i < s->len; i++) {
        if (strcmp(s->items[i], val) == 0) return;
    }
    if (s->len >= s->cap) {
        s->cap = s->cap == 0 ? 8 : s->cap * 2;
        s->items = (const char**)realloc(s->items, (size_t)s->cap * sizeof(const char*));
    }
    size_t vlen = strlen(val);
    char* vcopy = (char*)sbx_rc_alloc(vlen + 1);
    memcpy(vcopy, val, vlen + 1);
    s->items[s->len++] = vcopy;
}

static long __sbx_set_contains(long ptr, const char* val) {
    sbx_set* s = (sbx_set*)ptr;
    if (!s) return 0;
    for (long i = 0; i < s->len; i++) {
        if (strcmp(s->items[i], val) == 0) return 1;
    }
    return 0;
}

static void __sbx_set_remove(long ptr, const char* val) {
    sbx_set* s = (sbx_set*)ptr;
    if (!s) return;
    for (long i = 0; i < s->len; i++) {
        if (strcmp(s->items[i], val) == 0) {
            for (long j = i; j < s->len - 1; j++) {
                s->items[j] = s->items[j + 1];
            }
            s->len--;
            return;
        }
    }
}

/* ── assert_eq (builtin) ── */
static void __sbx_assert_eq(long a, long b) {
    if (a != b) {
        fprintf(stderr, "assert_eq failed: %ld != %ld\n", a, b);
        exit(1);
    }
}

/* ── Result unwrap (for ? operator) ── */
static long __sbx_result_unwrap(long val) {
    return val;
}

/* ── Bounds checking ── */
static long __sbx_bounds_check(long idx, long len) {
    if (idx < 0 || idx >= len) {
        fprintf(stderr, "sandbox: index %ld out of bounds (len=%ld)\n", idx, len);
        exit(1);
    }
    return idx;
}

/* ── End Sandbox Standard Library ── */
"#
    .to_string()
}
