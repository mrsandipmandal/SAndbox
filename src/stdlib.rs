use crate::ast::Type;
use std::collections::HashMap;

/// Represents a built-in standard library function
#[derive(Clone)]
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
    // ── A2: real JSON parse/stringify on maps + arrays ──
    register(
        &mut m,
        "json::parse_map",
        vec![("s".into(), Type::String)],
        Type::Map(Box::new(Type::String), Box::new(Type::I64)),
    );
    register(
        &mut m,
        "json::stringify_map",
        vec![(
            "m".into(),
            Type::Map(Box::new(Type::String), Box::new(Type::I64)),
        )],
        Type::String,
    );
    register(
        &mut m,
        "json::get_str",
        vec![("s".into(), Type::String), ("key".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "json::get_int",
        vec![("s".into(), Type::String), ("key".into(), Type::String)],
        Type::I64,
    );
    register(
        &mut m,
        "json::stringify_array",
        vec![("arr".into(), Type::Array(Box::new(Type::I64)))],
        Type::String,
    );
    register(
        &mut m,
        "json::array_get_int",
        vec![("s".into(), Type::String), ("i".into(), Type::I64)],
        Type::I64,
    );
    // Renamed from the repurposed json::stringify: explicit int stringify
    register(
        &mut m,
        "json::stringify_int",
        vec![("v".into(), Type::I64)],
        Type::String,
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

    // ── v2.1: A3 real HTTP server ──
    register(&mut m, "http::method", vec![], Type::String);
    register(&mut m, "http::query", vec![], Type::String);
    register(&mut m, "http::body", vec![], Type::String);
    register(
        &mut m,
        "http::req_header",
        vec![("name".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "http::set_status",
        vec![("code".into(), Type::I64)],
        Type::Void,
    );
    register(
        &mut m,
        "http::set_header",
        vec![
            ("name".into(), Type::String),
            ("value".into(), Type::String),
        ],
        Type::Void,
    );
    register(&mut m, "http::status", vec![], Type::I64);
    register(
        &mut m,
        "http::query_param",
        vec![
            ("query".into(), Type::String),
            ("name".into(), Type::String),
        ],
        Type::String,
    );
    register(
        &mut m,
        "http::form_get",
        vec![("body".into(), Type::String), ("name".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "http::form_param",
        vec![("name".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "http::url_decode",
        vec![("s".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "http::serve_static",
        vec![("dir".into(), Type::String)],
        Type::Void,
    );
    // ── A4: cookies ──
    register(
        &mut m,
        "http::set_cookie",
        vec![
            ("name".into(), Type::String),
            ("value".into(), Type::String),
        ],
        Type::Void,
    );
    register(
        &mut m,
        "http::get_cookie",
        vec![("name".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "http::cookie_get",
        vec![
            ("cookie_header".into(), Type::String),
            ("name".into(), Type::String),
        ],
        Type::String,
    );

    // ── A4: html ──
    register(
        &mut m,
        "html::escape",
        vec![("s".into(), Type::String)],
        Type::String,
    );
    register(
        &mut m,
        "html::unescape",
        vec![("s".into(), Type::String)],
        Type::String,
    );

    // ── A4: %{key} templates — tmpl::render(t, k1, v1, k2, v2, ...)
    register(
        &mut m,
        "tmpl::render",
        vec![
            ("template".into(), Type::String),
            ("key".into(), Type::String),
            ("value".into(), Type::String),
        ],
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
    register(
        &mut m,
        "assert_eq",
        vec![("a".into(), Type::I64), ("b".into(), Type::I64)],
        Type::Void,
    );

    // ── collections module ──
    // List — opaque pointer handle (i64)
    register(&mut m, "list::new", vec![], Type::I64);
    register(
        &mut m,
        "list::len",
        vec![("l".into(), Type::I64)],
        Type::I64,
    );
    register(
        &mut m,
        "list::push",
        vec![("l".into(), Type::I64), ("v".into(), Type::I64)],
        Type::Void,
    );
    register(
        &mut m,
        "list::get",
        vec![("l".into(), Type::I64), ("i".into(), Type::I64)],
        Type::I64,
    );
    register(
        &mut m,
        "list::set",
        vec![
            ("l".into(), Type::I64),
            ("i".into(), Type::I64),
            ("v".into(), Type::I64),
        ],
        Type::Void,
    );
    register(
        &mut m,
        "list::contains",
        vec![("l".into(), Type::I64), ("v".into(), Type::I64)],
        Type::Bool,
    );
    register(
        &mut m,
        "list::sort",
        vec![("l".into(), Type::I64)],
        Type::Void,
    );
    register(
        &mut m,
        "list::remove",
        vec![("l".into(), Type::I64), ("i".into(), Type::I64)],
        Type::Void,
    );
    register(
        &mut m,
        "list::is_empty",
        vec![("l".into(), Type::I64)],
        Type::Bool,
    );
    // Map — module-call API on real map<string, i64> values. Legacy
    // handles-typed-as-i64 rows were retired when maps became first-class
    // (A1); these signatures match the sbx_map* runtime exactly.
    register(
        &mut m,
        "map::new",
        vec![],
        Type::Map(Box::new(Type::String), Box::new(Type::I64)),
    );
    register(
        &mut m,
        "map::len",
        vec![(
            "m".into(),
            Type::Map(Box::new(Type::String), Box::new(Type::I64)),
        )],
        Type::I64,
    );
    register(
        &mut m,
        "map::insert",
        vec![
            (
                "m".into(),
                Type::Map(Box::new(Type::String), Box::new(Type::I64)),
            ),
            ("k".into(), Type::String),
            ("v".into(), Type::I64),
        ],
        Type::I64,
    );
    register(
        &mut m,
        "map::get",
        vec![
            (
                "m".into(),
                Type::Map(Box::new(Type::String), Box::new(Type::I64)),
            ),
            ("k".into(), Type::String),
        ],
        Type::I64,
    );
    register(
        &mut m,
        "map::contains",
        vec![
            (
                "m".into(),
                Type::Map(Box::new(Type::String), Box::new(Type::I64)),
            ),
            ("k".into(), Type::String),
        ],
        Type::Bool,
    );
    register(
        &mut m,
        "map::remove",
        vec![
            (
                "m".into(),
                Type::Map(Box::new(Type::String), Box::new(Type::I64)),
            ),
            ("k".into(), Type::String),
        ],
        Type::Bool,
    );
    register(
        &mut m,
        "map::keys",
        vec![(
            "m".into(),
            Type::Map(Box::new(Type::String), Box::new(Type::I64)),
        )],
        Type::Array(Box::new(Type::String)),
    );
    register(
        &mut m,
        "map::values",
        vec![(
            "m".into(),
            Type::Map(Box::new(Type::String), Box::new(Type::I64)),
        )],
        Type::Array(Box::new(Type::I64)),
    );
    // Set — opaque pointer handle (i64)
    register(&mut m, "set_of::new", vec![], Type::I64);
    register(
        &mut m,
        "set_of::len",
        vec![("s".into(), Type::I64)],
        Type::I64,
    );
    register(
        &mut m,
        "set_of::insert",
        vec![("s".into(), Type::I64), ("v".into(), Type::String)],
        Type::Void,
    );
    register(
        &mut m,
        "set_of::contains",
        vec![("s".into(), Type::I64), ("v".into(), Type::String)],
        Type::Bool,
    );
    register(
        &mut m,
        "set_of::remove",
        vec![("s".into(), Type::I64), ("v".into(), Type::String)],
        Type::Void,
    );

    m
}

fn register(m: &mut HashMap<String, StdlibFn>, name: &str, params: Vec<(String, Type)>, ret: Type) {
    m.insert(name.to_string(), StdlibFn { params, ret });
}

/// Returns true if the function name is a known stdlib builtin
pub fn is_builtin(name: &str) -> bool {
    builtins().contains_key(name)
}

// ── Method dispatch table (single source of truth) ──
//
// Every layer consults this table instead of keeping its own method
// list: the typechecker derives signatures from `builtin`, the C/LLVM
// backends derive the call target from `c_fn`, and the parser uses
// `is_array_method` to decide desugaring. Adding a method = one row
// here (+ one execution arm in the interpreter).

/// One builtin string method: `s.name(args)` ≡ `builtin(s, args…)`.
/// `builtin` must exist in `builtins()`; its first param is the receiver.
/// `c_fn` is the C runtime symbol the backends emit.
pub struct StringMethod {
    pub name: &'static str,
    pub builtin: &'static str,
    pub c_fn: &'static str,
}

const STRING_METHODS: &[StringMethod] = &[
    StringMethod {
        name: "to_upper",
        builtin: "string::to_upper",
        c_fn: "__sbx_str_to_upper",
    },
    StringMethod {
        name: "to_lower",
        builtin: "string::to_lower",
        c_fn: "__sbx_str_to_lower",
    },
    StringMethod {
        name: "trim",
        builtin: "string::trim",
        c_fn: "__sbx_str_trim",
    },
    StringMethod {
        name: "replace",
        builtin: "string::replace",
        c_fn: "__sbx_str_replace",
    },
    StringMethod {
        name: "contains",
        builtin: "string::contains",
        c_fn: "__sbx_str_contains",
    },
    StringMethod {
        name: "starts_with",
        builtin: "string::starts_with",
        c_fn: "__sbx_str_starts_with",
    },
    StringMethod {
        name: "ends_with",
        builtin: "string::ends_with",
        c_fn: "__sbx_str_ends_with",
    },
    StringMethod {
        name: "find",
        builtin: "string::find",
        c_fn: "__sbx_str_find",
    },
    StringMethod {
        name: "substring",
        builtin: "string::substring",
        c_fn: "__sbx_str_sub",
    },
    StringMethod {
        name: "char_at",
        builtin: "string::char_at",
        c_fn: "__sbx_str_char_at",
    },
    StringMethod {
        name: "repeat",
        builtin: "string::repeat",
        c_fn: "__sbx_str_repeat",
    },
    StringMethod {
        name: "equals",
        builtin: "string::equals",
        c_fn: "__sbx_str_eq",
    },
    // 'len' aliases the stdlib's 'length' spelling
    StringMethod {
        name: "len",
        builtin: "string::length",
        c_fn: "__sbx_str_len",
    },
    StringMethod {
        name: "is_empty",
        builtin: "string::is_empty",
        c_fn: "__sbx_str_is_empty",
    },
];

/// Look up a string method by name.
pub fn string_method(name: &str) -> Option<&'static StringMethod> {
    STRING_METHODS.iter().find(|m| m.name == name)
}

/// Array methods that take a lambda first (after receiver):
/// `nums.map(f)` ≡ `map(nums, f)` free-function form. The parser
/// desugars these at parse time so every backend reuses the
/// free-function codegen path.
// ── NOTE: this const currently guards parser desugaring only; the C
// runtime equivalents (`__sbx_arr_map`, `__sbx_arr_filter`, `__sbx_arr_reduce`)
// must also exist for `sandbox run` to succeed. Don't extend this list
// without adding/verifying the matching C runtime function. ──
const ARRAY_LAMBDA_METHODS: &[&str] = &["map", "filter", "reduce"];

pub fn is_array_method(name: &str) -> bool {
    ARRAY_LAMBDA_METHODS.contains(&name)
}

/// The C return kind of a string method: `"i"` for long/bool results,
/// `"s"` for string results. Derived from the stdlib signature so there
/// is no second hand-maintained return-type table.
pub fn string_method_ret_kind(name: &str) -> Option<&'static str> {
    let m = string_method(name)?;
    let ret = builtins().get(m.builtin)?.ret.clone();
    Some(match ret {
        Type::String => "s",
        _ => "i",
    })
}

/// LLVM return types for builtins whose C runtime functions do not return
/// a plain long (the generic LLVM builtin path assumes i64).
pub fn builtin_llvm_ret(name: &str) -> Option<&'static str> {
    match name {
        "map::new" => Some("i8*"),
        "json::parse_map"
        | "json::stringify_map"
        | "json::get_str"
        | "json::stringify_array"
        | "json::stringify_int"
        | "json::stringify"
        | "json::stringify_string"
        | "json::stringify_bool"
        | "json::get"
        | "json::parse_string"
        | "json::parse_object"
        | "json::map_get"
        | "json::map_keys"
        | "json::stringify_float" => Some("i8*"),
        "json::parse_float" => Some("double"),
        // A3 HTTP: string accessors/parsers return RC-allocated strings
        "http::method" | "http::query" | "http::body" | "http::req_header"
        | "http::query_param" | "http::form_get" | "http::form_param" | "http::url_decode"
        // A4: cookies, html and templates return strings
        | "http::get_cookie" | "http::cookie_get" | "html::escape" | "html::unescape"
        | "tmpl::render" => Some("i8*"),
        _ => None,
    }
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
        "map" => "__sbx_arr_map",
        "filter" => "__sbx_arr_filter",
        "reduce" => "__sbx_arr_reduce",
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
        "json::parse_map" => "__sbx_json_parse_map",
        "json::stringify_map" => "__sbx_json_stringify_map",
        "json::get_str" => "__sbx_json_get_str",
        "json::get_int" => "__sbx_json_get_int",
        "json::stringify_array" => "__sbx_json_stringify_array",
        "json::array_get_int" => "__sbx_json_array_get_int",
        "json::stringify_int" => "__sbx_json_stringify",
        "http::get" => "__sbx_http_get",
        "http::post" => "__sbx_http_post",
        "http::serve_once" => "__sbx_serve_once",
        "http::serve" => "__sbx_serve",
        "http::set_cookie" => "__sbx_http_set_cookie",
        "http::get_cookie" => "__sbx_http_get_cookie",
        "http::cookie_get" => "__sbx_http_cookie_get",
        "html::escape" => "__sbx_html_escape",
        "html::unescape" => "__sbx_html_unescape",
        "tmpl::render" => "__sbx_tmpl_render",
        "http::status_code" => "__sbx_http_status",
        "http::delete" => "__sbx_http_delete",
        "http::put" => "__sbx_http_put",
        "http::patch" => "__sbx_http_patch",
        "http::headers" => "__sbx_http_headers",
        "http::method" => "__sbx_http_method",
        "http::query" => "__sbx_http_query",
        "http::body" => "__sbx_http_body",
        "http::req_header" => "__sbx_http_req_header",
        "http::set_status" => "__sbx_http_set_status",
        "http::set_header" => "__sbx_http_set_header",
        "http::status" => "__sbx_http_status_code",
        "http::query_param" => "__sbx_http_query_param",
        "http::form_get" => "__sbx_http_form_get",
        "http::form_param" => "__sbx_http_form_param",
        "http::url_decode" => "__sbx_url_decode",
        "http::serve_static" => "__sbx_http_serve_static",
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
        "map::new" => "sbx_map_new",
        "map::len" => "__sbx_map_len",
        "map::insert" => "__sbx_map_insert",
        "map::get" => "__sbx_map_get",
        "map::contains" => "__sbx_map_has",
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
#include <signal.h>

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

// ── array helpers (runtime) ──
//
// The sandbox ecosystem maps `nums.map(f)` → free-function `map(nums, f)`
// in the parser; the C runtime below is what `sandbox run` links against.
// Keep these in sync with `ARRAY_LAMBDA_METHODS` in stdlib.rs.

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

// Array lambda helpers — called as map(src, len, lambda, captures...).
// `src`/`target` are long*; `len` is element count. The lambda receives
// (element, captures...) and returns the new element. Captures are passed
// through so closure-captured variables reach the lambda.
static void __sbx_arr_map(long* target, long* src, long len, long (*lambda)(long, long*)) {
    for (long i = 0; i < len; i++) {
        target[i] = lambda(src[i], NULL);
    }
}

static long __sbx_arr_filter(long* target, long* src, long len, long (*lambda)(long, long*)) {
    long j = 0;
    for (long i = 0; i < len; i++) {
        if (lambda(src[i], NULL)) {
            target[j++] = src[i];
        }
    }
    return j;
}

static long __sbx_arr_reduce(long* src, long len, long (*lambda)(long, long), long init) {
    long acc = init;
    for (long i = 0; i < len; i++) {
        acc = lambda(acc, src[i]);
    }
    return acc;
}

/* ── map<string,long> runtime (insertion-ordered) ──
   ABI note: every long-returning helper matches the LLVM declares
   (i64), so results stay valid across the C/LLVM link boundary. */
/* Open addressing with tombstones; keys are RC-managed strings. */

typedef struct { const char* key; long val; int state; } sbx_map_slot; /* state: 0 empty, 1 used, 2 tombstone */

typedef struct {
    sbx_map_slot* slots;
    long cap;       /* power of two */
    long used;      /* live entries */
    long tombs;     /* tombstones present */
    /* insertion order log: indices into a parallel key/val list */
    const char** ord_keys;
    long* ord_vals;
    long ord_len, ord_cap;
} sbx_map;

static const char* __sbx_map_strdup(const char* s) {
    size_t n = strlen(s);
    char* out = (char*)sbx_rc_alloc(n + 1);
    memcpy(out, s, n + 1);
    return out;
}

static unsigned long __sbx_map_hash(const char* s) {
    unsigned long h = 1469598103934665603UL; /* FNV-1a */
    while (*s) { h ^= (unsigned char)*s++; h *= 1099511628211UL; }
    return h;
}

static sbx_map* sbx_map_new(void) {
    sbx_map* m = (sbx_map*)sbx_rc_alloc(sizeof(sbx_map));
    m->cap = 8;
    m->slots = (sbx_map_slot*)sbx_rc_alloc(sizeof(sbx_map_slot) * (size_t)m->cap);
    for (long i = 0; i < m->cap; i++) m->slots[i].state = 0;
    m->used = 0; m->tombs = 0;
    m->ord_cap = 8; m->ord_len = 0;
    m->ord_keys = (const char**)sbx_rc_alloc(sizeof(char*) * (size_t)m->ord_cap);
    m->ord_vals = (long*)sbx_rc_alloc(sizeof(long) * (size_t)m->ord_cap);
    return m;
}

static void sbx_map_ord_push(sbx_map* m, const char* key, long val) {
    if (m->ord_len == m->ord_cap) {
        long nc = m->ord_cap * 2;
        const char** nk = (const char**)sbx_rc_alloc(sizeof(char*) * (size_t)nc);
        long* nv = (long*)sbx_rc_alloc(sizeof(long) * (size_t)nc);
        memcpy(nk, m->ord_keys, sizeof(char*) * (size_t)m->ord_len);
        memcpy(nv, m->ord_vals, sizeof(long) * (size_t)m->ord_len);
        m->ord_keys = nk; m->ord_vals = nv; m->ord_cap = nc;
    }
    m->ord_keys[m->ord_len] = key;
    m->ord_vals[m->ord_len] = val;
    m->ord_len++;
}

/* Find slot index for key; sets *out_found. Probes from hash. */
static long sbx_map_probe(sbx_map* m, const char* key, int* out_found) {
    unsigned long h = __sbx_map_hash(key);
    long mask = m->cap - 1;
    long i = (long)(h & (unsigned long)mask);
    long first_tomb = -1;
    *out_found = 0;
    for (long p = 0; p < m->cap; p++) {
        sbx_map_slot* s = &m->slots[i];
        if (s->state == 0) {
            return first_tomb >= 0 ? first_tomb : i;
        }
        if (s->state == 2) {
            if (first_tomb < 0) first_tomb = i;
        } else if (strcmp(s->key, key) == 0) {
            *out_found = 1;
            return i;
        }
        i = (i + 1) & mask;
    }
    return first_tomb >= 0 ? first_tomb : -1;
}

static void sbx_map_grow(sbx_map* m) {
    long ncap = m->cap * 2;
    sbx_map_slot* nslots = (sbx_map_slot*)sbx_rc_alloc(sizeof(sbx_map_slot) * (size_t)ncap);
    for (long i = 0; i < ncap; i++) nslots[i].state = 0;
    for (long i = 0; i < m->cap; i++) {
        if (m->slots[i].state != 1) continue;
        unsigned long h = __sbx_map_hash(m->slots[i].key);
        long j = (long)(h & (unsigned long)(ncap - 1));
        while (nslots[j].state == 1) j = (j + 1) & (ncap - 1);
        nslots[j] = m->slots[i];
    }
    m->slots = nslots;
    m->cap = ncap;
    m->tombs = 0;
}

/* Insert or update. Returns the inserted value (matches the interpreter). */
static long __sbx_map_insert(sbx_map* m, const char* key, long val) {
    int found;
    long i = sbx_map_probe(m, key, &found);
    if (found) {
        m->slots[i].val = val;
        for (long k = 0; k < m->ord_len; k++) {
            if (m->ord_keys[k] == m->slots[i].key) { m->ord_vals[k] = val; break; }
        }
        return val;
    }
    if (i < 0) { sbx_map_grow(m); return __sbx_map_insert(m, key, val); }
    if (m->slots[i].state == 2) m->tombs--;
    m->slots[i].key = __sbx_map_strdup(key);
    m->slots[i].val = val;
    m->slots[i].state = 1;
    m->used++;
    sbx_map_ord_push(m, m->slots[i].key, val);
    if ((m->used + m->tombs) * 10 >= m->cap * 7) sbx_map_grow(m);
    return val;
}

static long __sbx_map_get(sbx_map* m, const char* key, long def) {
    int found;
    long i = sbx_map_probe(m, key, &found);
    if (found) return m->slots[i].val;
    return def;
}

/* m.get(key, default) — same as __sbx_map_get; separate name for clarity. */
static long __sbx_map_get_default(sbx_map* m, const char* key, long def) {
    return __sbx_map_get(m, key, def);
}

static long __sbx_map_has(sbx_map* m, const char* key) {
    int found;
    sbx_map_probe(m, key, &found);
    return found;
}

static long __sbx_map_remove(sbx_map* m, const char* key) {
    int found;
    long i = sbx_map_probe(m, key, &found);
    if (!found) return 0;
    m->slots[i].state = 2;
    m->slots[i].key = NULL;
    m->used--; m->tombs++;
    for (long k = 0; k < m->ord_len; k++) {
        if (m->ord_keys[k] && strcmp(m->ord_keys[k], key) == 0) {
            for (long j = k; j + 1 < m->ord_len; j++) {
                m->ord_keys[j] = m->ord_keys[j + 1];
                m->ord_vals[j] = m->ord_vals[j + 1];
            }
            m->ord_len--;
            break;
        }
    }
    if ((m->used + m->tombs) * 10 >= m->cap * 7) sbx_map_grow(m);
    return 1;
}

static long __sbx_map_len(sbx_map* m) {
    return m->used;
}

/* Human-readable form "{k: v, ...}" in insertion order (print parity with the interpreter). */
static const char* __sbx_map_format(sbx_map* m) {
    long total = 3;
    for (long k = 0; k < m->ord_len; k++) total += (long)strlen(m->ord_keys[k]) + 24;
    char* out = (char*)sbx_rc_alloc((size_t)total);
    out[0] = '{';
    size_t off = 1;
    for (long k = 0; k < m->ord_len; k++) {
        if (k > 0) { out[off++] = ','; out[off++] = ' '; }
        size_t kl = strlen(m->ord_keys[k]);
        memcpy(out + off, m->ord_keys[k], kl);
        off += kl;
        off += (size_t)snprintf(out + off, (size_t)(total - (long)off), ": %ld", m->ord_vals[k]);
    }
    out[off++] = '}';
    out[off] = '\0';
    return out;
}

/* ── String arrays: sbx_strarr — growable const char* array (heap-backed
   handle, mirrors the sbx_map* pattern: literals/keys()/params all pass the
   same pointer, so indexing/len/for work uniformly without length tracking). ── */
typedef struct { const char** items; long len; long cap; } sbx_strarr;

static sbx_strarr* sbx_strarr_new(long cap_hint) {
    sbx_strarr* a = (sbx_strarr*)malloc(sizeof(sbx_strarr));
    if (cap_hint < 1) cap_hint = 1;
    a->items = (const char**)malloc(sizeof(const char*) * (size_t)cap_hint);
    a->len = 0;
    a->cap = cap_hint;
    return a;
}

static void sbx_strarr_push(sbx_strarr* a, const char* s) {
    if (a->len >= a->cap) {
        long ncap = a->cap * 2;
        const char** ni = (const char**)malloc(sizeof(const char*) * (size_t)ncap);
        for (long i = 0; i < a->len; i++) ni[i] = a->items[i];
        free(a->items);
        a->items = ni;
        a->cap = ncap;
    }
    a->items[a->len++] = s;
}

static const char* sbx_strarr_get(sbx_strarr* a, long i) {
    if (i < 0 || i >= a->len) return "";
    return a->items[i];
}

static void sbx_strarr_set(sbx_strarr* a, long i, const char* s) {
    if (i < 0 || i >= a->len) return;
    a->items[i] = s;
}

static long sbx_strarr_len(sbx_strarr* a) {
    return a->len;
}

/* Human-readable "[a, b]" form (print parity with the interpreter). */
static const char* __sbx_strarr_format(sbx_strarr* a) {
    long total = 3;
    for (long i = 0; i < a->len; i++) total += (long)strlen(a->items[i]) + 3;
    char* out = (char*)malloc((size_t)total);
    size_t off = 0;
    out[off++] = '[';
    for (long i = 0; i < a->len; i++) {
        if (i > 0) { out[off++] = ','; out[off++] = ' '; }
        size_t l = strlen(a->items[i]);
        memcpy(out + off, a->items[i], l);
        off += l;
    }
    out[off++] = ']';
    out[off] = '\0';
    return out;
}

/* Keys in insertion order as a real string array (sbx_strarr handle). */
static sbx_strarr* __sbx_map_keys(sbx_map* m) {
    sbx_strarr* out = sbx_strarr_new(m->ord_len > 0 ? m->ord_len : 1);
    for (long k = 0; k < m->ord_len; k++) sbx_strarr_push(out, m->ord_keys[k]);
    return out;
}

/* ── C1: i64-array heap handle (sbx_i64arr) — mirrors sbx_strarr so that
   map.values() and friends carry a runtime length everywhere (len, for, print). ── */
typedef struct { long* items; long len; long cap; } sbx_i64arr;

static sbx_i64arr* sbx_i64arr_new(long cap) {
    if (cap < 1) cap = 1;
    sbx_i64arr* a = (sbx_i64arr*)malloc(sizeof(sbx_i64arr));
    a->items = (long*)malloc(sizeof(long) * (size_t)cap);
    a->len = 0; a->cap = cap;
    return a;
}

static void sbx_i64arr_push(sbx_i64arr* a, long v) {
    if (a->len >= a->cap) {
        a->cap *= 2;
        a->items = (long*)realloc(a->items, sizeof(long) * (size_t)a->cap);
    }
    a->items[a->len++] = v;
}

static long sbx_i64arr_get(sbx_i64arr* a, long i) { return a->items[i]; }

static long sbx_i64arr_len(sbx_i64arr* a) { return a->len; }

static const char* __sbx_i64arr_format(sbx_i64arr* a) {
    long total = 3;
    for (long i = 0; i < a->len; i++) total += 21;
    char* out = (char*)malloc((size_t)total);
    size_t off = 0;
    out[off++] = '[';
    for (long i = 0; i < a->len; i++) {
        if (i > 0) { out[off++] = ','; out[off++] = ' '; }
        off += (size_t)snprintf(out + off, 24, "%ld", a->items[i]);
    }
    out[off++] = ']';
    out[off] = '\0';
    return out;
}

/* Values in insertion order as a plain long array (caller tracks the length
   via __sbx_map_len(m) at the binding site). */
/* Values in insertion order as a real i64 array (sbx_i64arr handle, carries
   its length — unlike a bare long* — so len/for/print agree across backends). */
static sbx_i64arr* __sbx_map_values(sbx_map* m) {
    sbx_i64arr* out = sbx_i64arr_new(m->ord_len > 0 ? m->ord_len : 1);
    for (long k = 0; k < m->ord_len; k++) sbx_i64arr_push(out, m->ord_vals[k]);
    return out;
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
    /* %g-style: up to 15 significant digits, trim trailing zeros —
       round-trips values instead of a lossy fixed 6-decimal form. */
    snprintf(out, 64, "%.15g", v);
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

/* ── A2: JSON parse/stringify on maps + arrays ──
   map<string,long> cannot hold string values (A1 decision), so
   object parsing stores only numeric values (true/false/null → 1/0/0);
   string fields stay readable via json::get_str on the raw text. */

static void __sbx_json_skip_ws(const char** pp) {
    const char* p = *pp;
    while (*p == ' ' || *p == '\t' || *p == '\n' || *p == '\r') p++;
    *pp = p;
}

/* Parse a JSON object into a map<string,long>. Keys keep source order.
   Numeric values are stored exactly; true/false/null become 1/0/0;
   string, array and object values are skipped. */
static sbx_map* __sbx_json_parse_map(const char* s) {
    sbx_map* m = sbx_map_new();
    const char* p = s;
    while (*p && *p != '{') p++;
    if (*p != '{') return m;
    p++;
    char key[256];
    for (;;) {
        __sbx_json_skip_ws(&p);
        if (*p == '}' || *p == '\0') break;
        if (*p == ',') { p++; continue; }
        if (*p != '"') break;
        if (!__sbx_json_extract_string(&p, key, sizeof(key))) break;
        __sbx_json_skip_ws(&p);
        if (*p != ':') break;
        p++;
        __sbx_json_skip_ws(&p);
        if (*p == '"') {
            /* string value: skip (read string fields via json::get_str) */
            p++;
            while (*p && *p != '"') { if (*p == '\\' && *(p+1)) p++; p++; }
            if (*p) p++;
        } else if (*p == '[' || *p == '{') {
            __sbx_json_skip_value(&p);
        } else if (*p == 't' && strncmp(p, "true", 4) == 0) {
            __sbx_map_insert(m, key, 1);
            p += 4;
        } else if (*p == 'f' && strncmp(p, "false", 5) == 0) {
            __sbx_map_insert(m, key, 0);
            p += 5;
        } else if (*p == 'n' && strncmp(p, "null", 4) == 0) {
            __sbx_map_insert(m, key, 0);
            p += 4;
        } else {
            char* end;
            long v = strtol(p, &end, 10);
            if (end == p) break; /* malformed */
            __sbx_map_insert(m, key, v);
            p = end;
        }
    }
    return m;
}

/* Serialize map<string,long> to JSON: {"k": v, ...} in insertion order. */
static const char* __sbx_json_stringify_map(sbx_map* m) {
    long total = 4;
    for (long k = 0; k < m->ord_len; k++) total += (long)strlen(m->ord_keys[k]) + 32;
    char* out = (char*)sbx_rc_alloc((size_t)total);
    size_t off = 0;
    out[off++] = '{';
    for (long k = 0; k < m->ord_len; k++) {
        if (k > 0) out[off++] = ',';
        out[off++] = '"';
        size_t kl = strlen(m->ord_keys[k]);
        memcpy(out + off, m->ord_keys[k], kl);
        off += kl;
        out[off++] = '"';
        out[off++] = ':';
        off += (size_t)snprintf(out + off, (size_t)(total - (long)off), "%ld", m->ord_vals[k]);
    }
    out[off++] = '}';
    out[off] = '\0';
    return out;
}

/* String field of a JSON object, read from the raw text. "" if missing. */
static const char* __sbx_json_get_str(const char* s, const char* key) {
    return __sbx_json_get(s, key);
}

/* Numeric field of a JSON object, read from the raw text. 0 if missing. */
static long __sbx_json_get_int(const char* s, const char* key) {
    char needle[256];
    snprintf(needle, sizeof(needle), "\"%s\"", key);
    const char* p = strstr(s, needle);
    if (!p) return 0;
    p += strlen(needle);
    while (*p && (*p == ' ' || *p == ':' || *p == '\t')) p++;
    int neg = 0;
    if (*p == '-') { neg = 1; p++; }
    if (*p < '0' || *p > '9') return 0;
    long v = strtol(p, NULL, 10);
    return neg ? -v : v;
}

/* Serialize a long array as [1,2,3]. Two forms so inline array literals
   work on every backend: variadic __sbx_json_stringify_array(len, e0, e1, ...)
   for literal arguments, and __sbx_json_stringify_array_p(long* arr, long len)
   when the elements live in a real array. */
static const char* __sbx_json_stringify_array(long len, ...) {
    if (len < 0) len = 0;
    char* out = (char*)sbx_rc_alloc((size_t)(len * 24 + 4));
    size_t off = 0;
    va_list ap;
    va_start(ap, len);
    out[off++] = '[';
    for (long i = 0; i < len; i++) {
        if (i > 0) out[off++] = ',';
        off += (size_t)snprintf(out + off, (size_t)(len * 24 + 4 - (long)off), "%ld",
                                va_arg(ap, long));
    }
    va_end(ap);
    out[off++] = ']';
    out[off] = '\0';
    return out;
}

static const char* __sbx_json_stringify_array_p(long* arr, long len) {
    if (len < 0) len = 0;
    char* out = (char*)sbx_rc_alloc((size_t)(len * 24 + 4));
    size_t off = 0;
    out[off++] = '[';
    for (long i = 0; i < len; i++) {
        if (i > 0) out[off++] = ',';
        off += (size_t)snprintf(out + off, (size_t)(len * 24 + 4 - (long)off), "%ld", arr[i]);
    }
    out[off++] = ']';
    out[off] = '\0';
    return out;
}

/* Element `idx` of the top-level JSON array in `s`. 0 if out of range. */
static long __sbx_json_array_get_int(const char* s, long idx) {
    const char* p = s;
    while (*p && *p != '[') p++;
    if (*p != '[') return 0;
    p++;
    long i = 0;
    while (*p) {
        __sbx_json_skip_ws(&p);
        if (*p == ']' || *p == '\0') return 0;
        if (*p == ',') { p++; continue; }
        const char* start = p;
        if (*p == '"') {
            p++;
            while (*p && *p != '"') { if (*p == '\\' && *(p+1)) p++; p++; }
            if (*p) p++;
        } else if (*p == '[' || *p == '{') {
            __sbx_json_skip_value(&p);
        } else {
            while (*p && *p != ',' && *p != ']') p++;
        }
        if (i == idx) {
            char buf[32];
            size_t n = (size_t)(p - start);
            if (n >= sizeof(buf)) n = sizeof(buf) - 1;
            memcpy(buf, start, n);
            buf[n] = '\0';
            return strtol(buf, NULL, 10);
        }
        i++;
        __sbx_json_skip_ws(&p);
        if (*p == ',') p++;
    }
    return 0;
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

/* ── v2.1: request context (implicit accessors) ──
   The active server fills this per request; handler code reads it through
   http::method / http::query / http::body / http::req_header and mutates
   the response through http::set_status / http::set_header. */
#define SBX_HTTP_MAX_HEADERS 32
typedef struct {
    char method[16];
    char target[2048];            /* raw request target incl. query */
    char query[1024];             /* after '?' (no leading '?'), "" if none */
    char body[8192 - 3072];
    char header_names[SBX_HTTP_MAX_HEADERS][128];
    char header_values[SBX_HTTP_MAX_HEADERS][1024];
    int header_count;
    int status;                   /* response status (default 200) */
    char content_type[128];       /* response content type (default json) */
    char extra_headers[4096];     /* user-set response headers, CRLF-joined */
} sbx_http_req;
static sbx_http_req __sbx_http_ctx;

static const char* __sbx_http_method(void) {
    char* out = (char*)sbx_rc_alloc(16);
    snprintf(out, 16, "%s", __sbx_http_ctx.method);
    return out;
}

static const char* __sbx_http_query(void) {
    char* out = (char*)sbx_rc_alloc(sizeof(__sbx_http_ctx.query));
    snprintf(out, sizeof(__sbx_http_ctx.query), "%s", __sbx_http_ctx.query);
    return out;
}

static const char* __sbx_http_body(void) {
    char* out = (char*)sbx_rc_alloc(sizeof(__sbx_http_ctx.body));
    snprintf(out, sizeof(__sbx_http_ctx.body), "%s", __sbx_http_ctx.body);
    return out;
}

static const char* __sbx_http_req_header(const char* name) {
    for (int i = 0; i < __sbx_http_ctx.header_count; i++) {
        const char* h = __sbx_http_ctx.header_names[i];
        const char* n = name;
        while (*h && *n && (*h == *n || (*h >= 'A' && *h <= 'Z' && *h + 32 == *n) || (*n >= 'A' && *n <= 'Z' && *n + 32 == *h))) { h++; n++; }
        if (*h == '\0' && *n == '\0') {
            char* out = (char*)sbx_rc_alloc(sizeof(__sbx_http_ctx.header_values[i]));
            snprintf(out, sizeof(__sbx_http_ctx.header_values[i]), "%s", __sbx_http_ctx.header_values[i]);
            return out;
        }
    }
    return "";
}

static void __sbx_http_set_status(long code) {
    __sbx_http_ctx.status = (int)code;
}

static void __sbx_http_set_header(const char* name, const char* value) {
    if (!name || !*name) return;
    /* special-case content-type: replaces the default instead of appending */
    const char* n = name;
    const char* c = "content-type";
    while (*n && *c && (*n == *c || (*n >= 'A' && *n <= 'Z' && *n + 32 == *c))) { n++; c++; }
    if (*n == '\0' && *c == '\0') {
        snprintf(__sbx_http_ctx.content_type, sizeof(__sbx_http_ctx.content_type), "%s", value);
        return;
    }
    size_t len = strlen(__sbx_http_ctx.extra_headers);
    snprintf(__sbx_http_ctx.extra_headers + len, sizeof(__sbx_http_ctx.extra_headers) - len,
        "%s%s: %s\r\n", len ? "" : "", name, value);
}

static long __sbx_http_status_code(void) {
    return __sbx_http_ctx.status;
}

/* ── v2.1: url decode / query & form params (pure, parity-tested) ── */
static int __sbx_hex_val(char c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

static const char* __sbx_url_decode(const char* s) {
    if (!s) return "";
    size_t len = strlen(s);
    char* out = (char*)sbx_rc_alloc(len + 1);
    size_t o = 0;
    for (size_t i = 0; i < len; i++) {
        if (s[i] == '%' && i + 2 < len && __sbx_hex_val(s[i+1]) >= 0 && __sbx_hex_val(s[i+2]) >= 0) {
            out[o++] = (char)(__sbx_hex_val(s[i+1]) * 16 + __sbx_hex_val(s[i+2]));
            i += 2;
        } else if (s[i] == '+') {
            out[o++] = ' ';
        } else {
            out[o++] = s[i];
        }
    }
    out[o] = '\0';
    return out;
}

/* Extract one "name=value" pair's decoded value from a query/form string.
   Mirrored in the interpreter (http_query_param/http_form_get helpers). */
static const char* __sbx_url_pair_get(const char* pairs, const char* name) {
    if (!pairs || !name) return "";
    size_t nlen = strlen(name);
    const char* p = pairs;
    while (*p) {
        const char* amp = strchr(p, '&');
        size_t seglen = amp ? (size_t)(amp - p) : strlen(p);
        const char* eq = memchr(p, '=', seglen);
        size_t keylen = eq ? (size_t)(eq - p) : seglen;
        if (keylen == nlen && strncmp(p, name, nlen) == 0) {
            if (!eq) return "";
            char tmp[2048];
            size_t vlen = seglen - keylen - 1;
            if (eq + 1 + vlen > p + seglen) vlen = 0;
            if (vlen >= sizeof(tmp)) vlen = sizeof(tmp) - 1;
            memcpy(tmp, eq + 1, vlen);
            tmp[vlen] = '\0';
            return __sbx_url_decode(tmp);
        }
        if (!amp) break;
        p = amp + 1;
    }
    return "";
}

static const char* __sbx_http_query_param(const char* query, const char* name) {
    return __sbx_url_pair_get(query, name);
}

static const char* __sbx_http_form_get(const char* body, const char* name) {
    return __sbx_url_pair_get(body, name);
}

static const char* __sbx_http_form_param(const char* name) {
    return __sbx_url_pair_get(__sbx_http_ctx.body, name);
}

/* Fill the request context from a raw request. Returns the path (no query). */
static void __sbx_http_parse_request(const char* req, char* path, size_t pathcap) {
    memset(&__sbx_http_ctx, 0, sizeof(__sbx_http_ctx));
    __sbx_http_ctx.status = 200;
    snprintf(__sbx_http_ctx.content_type, sizeof(__sbx_http_ctx.content_type), "application/json");
    snprintf(path, pathcap, "/");
    if (!req || !*req) return;

    /* request line: METHOD SP TARGET SP HTTP/x */
    const char* sp1 = strchr(req, ' ');
    if (!sp1) return;
    size_t mlen = (size_t)(sp1 - req);
    if (mlen >= sizeof(__sbx_http_ctx.method)) mlen = sizeof(__sbx_http_ctx.method) - 1;
    memcpy(__sbx_http_ctx.method, req, mlen);
    __sbx_http_ctx.method[mlen] = '\0';

    const char* sp2 = strchr(sp1 + 1, ' ');
    size_t tlen = sp2 ? (size_t)(sp2 - sp1 - 1) : strlen(sp1 + 1);
    if (tlen >= sizeof(__sbx_http_ctx.target)) tlen = sizeof(__sbx_http_ctx.target) - 1;
    memcpy(__sbx_http_ctx.target, sp1 + 1, tlen);
    __sbx_http_ctx.target[tlen] = '\0';

    /* split target into path + query */
    const char* qm = strchr(__sbx_http_ctx.target, '?');
    if (qm) {
        snprintf(__sbx_http_ctx.query, sizeof(__sbx_http_ctx.query), "%s", qm + 1);
        size_t plen = (size_t)(qm - __sbx_http_ctx.target);
        if (plen >= pathcap) plen = pathcap - 1;
        memcpy(path, __sbx_http_ctx.target, plen);
        path[plen] = '\0';
    } else {
        snprintf(path, pathcap, "%s", __sbx_http_ctx.target);
    }

    /* headers until blank line, then body */
    const char* line = strstr(req, "\r\n");
    const char* end = strstr(req, "\r\n\r\n");
    const char* bodyp = end ? end + 4 : (line ? line + 2 : req + strlen(req));
    snprintf(__sbx_http_ctx.body, sizeof(__sbx_http_ctx.body), "%s", bodyp);
    if (!end) end = req + strlen(req);
    const char* p = line ? line + 2 : req;
    while (p < end && __sbx_http_ctx.header_count < SBX_HTTP_MAX_HEADERS) {
        const char* eol = strstr(p, "\r\n");
        if (!eol || eol > end) eol = end;
        const char* colon = memchr(p, ':', (size_t)(eol - p));
        if (colon) {
            size_t nlen = (size_t)(colon - p);
            if (nlen >= sizeof(__sbx_http_ctx.header_names[0])) nlen = sizeof(__sbx_http_ctx.header_names[0]) - 1;
            memcpy(__sbx_http_ctx.header_names[__sbx_http_ctx.header_count], p, nlen);
            __sbx_http_ctx.header_names[__sbx_http_ctx.header_count][nlen] = '\0';
            const char* vp = colon + 1;
            while (vp < eol && (*vp == ' ' || *vp == '\t')) vp++;
            size_t vlen = (size_t)(eol - vp);
            if (vlen >= sizeof(__sbx_http_ctx.header_values[0])) vlen = sizeof(__sbx_http_ctx.header_values[0]) - 1;
            memcpy(__sbx_http_ctx.header_values[__sbx_http_ctx.header_count], vp, vlen);
            __sbx_http_ctx.header_values[__sbx_http_ctx.header_count][vlen] = '\0';
            __sbx_http_ctx.header_count++;
        }
        if (eol >= end) break;
        p = eol + 2;
    }
}

/* Reason phrase for the common status codes (others get a generic one). */
static const char* __sbx_http_reason(int code) {
    switch (code) {
        case 200: return "OK";
        case 201: return "Created";
        case 204: return "No Content";
        case 301: return "Moved Permanently";
        case 302: return "Found";
        case 304: return "Not Modified";
        case 400: return "Bad Request";
        case 401: return "Unauthorized";
        case 403: return "Forbidden";
        case 404: return "Not Found";
        case 405: return "Method Not Allowed";
        case 418: return "I'm a teapot";
        case 500: return "Internal Server Error";
        case 502: return "Bad Gateway";
        case 503: return "Service Unavailable";
        default: return "Status";
    }
}

/* Send one response using the current context. */
static void __sbx_http_respond_raw(int cfd, const char* body) {
    size_t blen = body ? strlen(body) : 2;
    char head[1024];
    int hn = snprintf(head, sizeof(head),
        "HTTP/1.1 %d %s\r\nContent-Type: %s\r\nContent-Length: %zu\r\nConnection: close\r\n%s\r\n",
        __sbx_http_ctx.status, __sbx_http_reason(__sbx_http_ctx.status),
        __sbx_http_ctx.content_type, blen, __sbx_http_ctx.extra_headers);
    send(cfd, head, (size_t)hn, 0);
    if (body && blen) send(cfd, body, blen, 0);
}

static void __sbx_serve_once(long port, const char* (*handler)(const char*)) {
    /* A client that disconnects mid-response must not kill the server. */
    signal(SIGPIPE, SIG_IGN);
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

    char path[1024];
    __sbx_http_parse_request(req, path, sizeof(path));

    const char* body = handler ? handler(path) : "{}";
    __sbx_http_respond_raw(cfd, body);
    close(cfd);
    close(fd);
}

/* ── v2.1: static files (served before the handler; traversal-protected) ── */
/* Directory for http::serve_static ("" = disabled). Declared before the
   serve functions that read it. */
static char __sbx_static_dir[2048] = "";

static void __sbx_http_serve_static(const char* dir) {
    snprintf(__sbx_static_dir, sizeof(__sbx_static_dir), "%s", dir ? dir : "");
}

static const char* __sbx_http_mime(const char* path) {
    const char* dot = strrchr(path, '.');
    if (!dot) return "application/octet-stream";
    if (!strcmp(dot, ".html") || !strcmp(dot, ".htm")) return "text/html";
    if (!strcmp(dot, ".css")) return "text/css";
    if (!strcmp(dot, ".js")) return "application/javascript";
    if (!strcmp(dot, ".json")) return "application/json";
    if (!strcmp(dot, ".png")) return "image/png";
    if (!strcmp(dot, ".jpg") || !strcmp(dot, ".jpeg")) return "image/jpeg";
    if (!strcmp(dot, ".gif")) return "image/gif";
    if (!strcmp(dot, ".svg")) return "image/svg+xml";
    if (!strcmp(dot, ".txt")) return "text/plain";
    if (!strcmp(dot, ".ico")) return "image/x-icon";
    return "application/octet-stream";
}

/* Serve dir/path if it resolves to a regular file under dir.
   Returns 1 if the response was sent, 0 if the caller should fall through. */
static int __sbx_http_try_static(int cfd, const char* dir, const char* path) {
    /* traversal protection: any ".." in the URL path is rejected outright,
       even with no static dir configured (defense in depth) */
    if (strstr(path, "..")) {
        const char* msg = "Forbidden";
        char head[256];
        int hn = snprintf(head, sizeof(head),
            "HTTP/1.1 403 Forbidden\r\nContent-Type: text/plain\r\nContent-Length: %zu\r\nConnection: close\r\n\r\n%s",
            strlen(msg), msg);
        send(cfd, head, (size_t)hn, 0);
        return 1;
    }
    if (!dir || !*dir) return 0;
    while (*path == '/') path++;
    char full[4096];
    if (snprintf(full, sizeof(full), "%s%s%s", dir, (*dir && dir[strlen(dir)-1] == '/') ? "" : "/", path) >= (int)sizeof(full)) return 0;
    struct stat st;
    if (stat(full, &st) != 0 || !S_ISREG(st.st_mode)) return 0;
    FILE* fp = fopen(full, "rb");
    if (!fp) return 0;
    char head[512];
    int hn = snprintf(head, sizeof(head),
        "HTTP/1.1 200 OK\r\nContent-Type: %s\r\nContent-Length: %ld\r\nConnection: close\r\n\r\n",
        __sbx_http_mime(full), (long)st.st_size);
    send(cfd, head, (size_t)hn, 0);
    char chunk[8192];
    size_t r;
    while ((r = fread(chunk, 1, sizeof(chunk), fp)) > 0) {
        if (send(cfd, chunk, r, 0) < 0) break;
    }
    fclose(fp);
    return 1;
}

/* ── A4: cookies ──
   set_cookie appends a Set-Cookie response header; get_cookie extracts a
   value from the request's Cookie header ("a=1; b=2", RFC 6265 style). */
static void __sbx_http_set_cookie(const char* name, const char* value) {
    if (!name || !*name || !value) return;
    /* reject CR/LF so a cookie can't inject headers */
    for (const char* p = name; *p; p++)
        if (*p == '\r' || *p == '\n' || *p == ';') return;
    for (const char* p = value; *p; p++)
        if (*p == '\r' || *p == '\n') return;
    char* ctx = __sbx_http_ctx.extra_headers;
    size_t used = strlen(ctx);
    char one[512];
    int n = snprintf(one, sizeof(one), "Set-Cookie: %s=%s; Path=/\r\n", name, value);
    if (n > 0 && used + (size_t)n < sizeof(__sbx_http_ctx.extra_headers)) {
        memcpy(ctx + used, one, (size_t)n);
        ctx[used + (size_t)n] = '\0';
    }
}

/* Extract `name`'s value from a Cookie request header (or "" if absent).
   Cookie pairs are '; '-separated; the first '=' splits name and value. */
static const char* __sbx_http_cookie_get(const char* cookie_header, const char* name) {
    char* out = (char*)sbx_rc_alloc(1024);
    out[0] = '\0';
    if (!cookie_header || !name) return out;
    size_t nlen = strlen(name);
    const char* p = cookie_header;
    while (*p) {
        while (*p == ' ' || *p == '\t') p++;
        const char* semi = strchr(p, ';');
        size_t seg = semi ? (size_t)(semi - p) : strlen(p);
        if (seg > nlen && strncmp(p, name, nlen) == 0 && p[nlen] == '=') {
            size_t vlen = seg - nlen - 1;
            if (vlen >= 1024) vlen = 1023;
            memcpy(out, p + nlen + 1, vlen);
            out[vlen] = '\0';
            return out;
        }
        if (!semi) break;
        p = semi + 1;
    }
    return out;
}

static const char* __sbx_http_get_cookie(const char* name) {
    const char* hdr = __sbx_http_req_header("Cookie");
    return __sbx_http_cookie_get(hdr, name);
}

/* ── A4: html escape/unescape ──
   escape: & < > " ' → entities (must do & first). unescape: numeric
   (&#NN; / &#xHH;) plus the five named entities above. */
static const char* __sbx_html_escape(const char* s) {
    if (!s) s = "";
    size_t cap = strlen(s) * 6 + 16;
    char* out = (char*)sbx_rc_alloc(cap);
    size_t o = 0;
    for (const char* p = s; *p; p++) {
        const char* rep = NULL;
        switch (*p) {
            case '&': rep = "&amp;"; break;
            case '<': rep = "&lt;"; break;
            case '>': rep = "&gt;"; break;
            case '"': rep = "&quot;"; break;
            case '\'': rep = "&#39;"; break;
            default: break;
        }
        if (rep) {
            size_t rl = strlen(rep);
            if (o + rl < cap) { memcpy(out + o, rep, rl); o += rl; }
        } else if (o + 1 < cap) {
            out[o++] = *p;
        }
    }
    out[o] = '\0';
    return out;
}

static unsigned long __sbx_html_entity_val(const char* p, size_t len, size_t* consumed) {
    /* p points at '&' ... len bytes available; returns value, sets consumed
       to bytes eaten (0 = not an entity) */
    if (len < 3 || p[0] != '&' || p[len - 1] != ';') { *consumed = 0; return 0; }
    const char* body = p + 1;
    size_t blen = len - 2;
    if (body[0] == '#') {
        int hex = (blen > 2 && (body[1] == 'x' || body[1] == 'X'));
        const char* digits = body + (hex ? 2 : 1);
        size_t dlen = blen - (hex ? 2 : 1);
        if (dlen == 0 || dlen > 6) { *consumed = 0; return 0; }
        unsigned long v = 0;
        for (size_t i = 0; i < dlen; i++) {
            char c = digits[i];
            int d = (c >= '0' && c <= '9') ? c - '0'
                  : (hex && c >= 'a' && c <= 'f') ? c - 'a' + 10
                  : (hex && c >= 'A' && c <= 'F') ? c - 'A' + 10 : -1;
            if (d < 0) { *consumed = 0; return 0; }
            v = v * (hex ? 16UL : 10UL) + (unsigned long)d;
        }
        *consumed = blen + 2;
        return v;
    }
    if (blen == 3 && !strncmp(body, "amp", 3)) { *consumed = blen + 2; return '&'; }
    if (blen == 2 && !strncmp(body, "lt", 2)) { *consumed = blen + 2; return '<'; }
    if (blen == 2 && !strncmp(body, "gt", 2)) { *consumed = blen + 2; return '>'; }
    if (blen == 4 && !strncmp(body, "quot", 4)) { *consumed = blen + 2; return '"'; }
    if (blen == 4 && !strncmp(body, "apos", 4)) { *consumed = blen + 2; return '\''; }
    *consumed = 0;
    return 0;
}

static const char* __sbx_html_unescape(const char* s) {
    if (!s) s = "";
    char* out = (char*)sbx_rc_alloc(strlen(s) + 16);
    size_t o = 0;
    size_t n = strlen(s);
    for (size_t i = 0; i < n;) {
        if (s[i] == '&') {
            const char* semi = memchr(s + i, ';', n - i);
            if (semi && (size_t)(semi - (s + i)) <= 10) {
                size_t elen = (size_t)(semi - (s + i)) + 1;
                size_t consumed = 0;
                unsigned long v = __sbx_html_entity_val(s + i, elen, &consumed);
                if (consumed && v <= 0x10FFFF) {
                    /* encode as UTF-8 */
                    if (v < 0x80) {
                        out[o++] = (char)v;
                    } else if (v < 0x800) {
                        out[o++] = (char)(0xC0 | (v >> 6));
                        out[o++] = (char)(0x80 | (v & 0x3F));
                    } else if (v < 0x10000) {
                        out[o++] = (char)(0xE0 | (v >> 12));
                        out[o++] = (char)(0x80 | ((v >> 6) & 0x3F));
                        out[o++] = (char)(0x80 | (v & 0x3F));
                    } else {
                        out[o++] = (char)(0xF0 | (v >> 18));
                        out[o++] = (char)(0x80 | ((v >> 12) & 0x3F));
                        out[o++] = (char)(0x80 | ((v >> 6) & 0x3F));
                        out[o++] = (char)(0x80 | (v & 0x3F));
                    }
                    i += consumed;
                    continue;
                }
            }
        }
        out[o++] = s[i++];
    }
    out[o] = '\0';
    return out;
}

/* ── A4: %{key} template rendering ──
   __sbx_tmpl_render(count, s1, s2, ...): s1 is the template, the rest are
   key/value pairs. Replaces %{key} occurrences; unknown keys stay as-is. */
static const char* __sbx_tmpl_render(long count, ...) {
    va_list args;
    va_start(args, count);
    const char* tmpl = count > 0 ? va_arg(args, const char*) : "";
    long npairs = (count - 1) / 2;
    /* measure output */
    size_t total = strlen(tmpl) + 16;
    {
        const char* p = tmpl;
        while ((p = strstr(p, "%{")) != NULL) {
            const char* end = strchr(p + 2, '}');
            if (!end) break;
            size_t klen = (size_t)(end - p - 2);
            total += 512; /* worst-case replacement slack */
            p = end + 1;
            (void)klen;
        }
    }
    char* out = (char*)sbx_rc_alloc(total + 1);
    size_t o = 0;
    const char* p = tmpl;
    while (*p) {
        if (p[0] == '%' && p[1] == '{') {
            const char* end = strchr(p + 2, '}');
            if (end) {
                size_t klen = (size_t)(end - p - 2);
                va_start(args, count);
                va_arg(args, const char*); /* skip template */
                const char* val = NULL;
                for (long i = 0; i < npairs; i++) {
                    const char* k = va_arg(args, const char*);
                    const char* v = va_arg(args, const char*);
                    if (k && strlen(k) == klen && strncmp(k, p + 2, klen) == 0) {
                        val = v ? v : "";
                        break;
                    }
                }
                va_end(args);
                if (val) {
                    size_t vl = strlen(val);
                    memcpy(out + o, val, vl);
                    o += vl;
                    p = end + 1;
                    continue;
                }
            }
        }
        out[o++] = *p++;
    }
    out[o] = '\0';
    return out;
}

static void __sbx_serve(long port, const char* (*handler)(const char*)) {
    /* A client that disconnects mid-response must not kill the server. */
    signal(SIGPIPE, SIG_IGN);
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

        char path[1024];
        __sbx_http_parse_request(req, path, sizeof(path));

        int st = __sbx_http_try_static(cfd, __sbx_static_dir, path);
        if (st == 0) {
            const char* body = handler ? handler(path) : "{}";
            __sbx_http_respond_raw(cfd, body);
        }
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
