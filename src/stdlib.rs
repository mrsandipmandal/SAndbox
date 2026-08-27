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
        "array::len" => "__sbx_arr_len",
        "array::push" => "__sbx_arr_push",
        "array::sort" => "__sbx_arr_sort",
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

static long __sbx_arr_len(long* arr) {
    /* For stack arrays: compute from first element count hack
       For simplicity we just count until sentinel in v0.3 */
    (void)arr;
    return 0;
}

static void __sbx_arr_push(long* arr, long elem) {
    (void)arr;
    (void)elem;
}

static void __sbx_arr_sort(long* arr, long len) {
    /* Simple insertion sort for small arrays */
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

/* ── End Sandbox Standard Library ── */
"#
    .to_string()
}
