//! The Sandbox compiler compiled to WebAssembly, powering the in-browser
//! playground served by the package registry.
//!
//! This crate includes the compiler's leaf modules (lexer, parser,
//! typechecker, b2 desugar pass, wasmgen) by path — the same modules the
//! binary crate's `mod` tree uses — because the main crate is a binary, not
//! a library, and its `compiler.rs` needs `std::process::Command`, which has
//! no meaning on `wasm32-unknown-unknown`. The pipeline here is the
//! `parse_for_codegen_with_packages` flow minus the filesystem: sources for
//! `use pkg::...` packages are handed in across the ABI boundary (fetched
//! from the registry API by the page's JS) and injected as ModuleDefs.
//!
//! ABI (no wasm-bindgen; the page's JS writes guest memory directly via the
//! module's exported linear memory):
//!
//! - `sbx_version() -> *const c_char`        static version string
//! - `sbx_alloc(len) -> *mut u8`             guest-allocated buffer for JS
//! - `sbx_free(ptr, len)`                    release that buffer
//! - `sbx_compile(in_ptr, in_len, pkgs_ptr, pkgs_len, err_ptr) -> *mut u8`
//!   Compiles the program at `in_ptr..in_len`; `pkgs_ptr..pkgs_len` holds a
//!   NUL-separated list of `name=source\0name=source\0` entries. Returns a
//!   guest pointer to `len:u32 LE || wat_utf8` (frees itself never; JS reads
//!   then calls `sbx_free`). On failure returns 0 and sets `err_ptr` to the
//!   same layout holding the error message.
//!
//! Panics abort (panic=abort in Cargo.toml) and are treated as compile
//! failures by the caller.

// The C ABI entry points below take raw pointers from the host by design
// (that is the whole point of a wasm export); clippy's rule against pub fns
// dereferencing raw pointer args does not apply to this boundary.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

mod ast;
mod b2;
mod diagnostic;
mod lexer;
mod parser;
// Lint policy lives here, not in the copied files: they are kept byte-identical
// to the src/ originals so drift is visible with a plain diff. The C/LLVM
// helper surface in stdlib (c_name, c_preamble, ...) has no callers on wasm32.
#[allow(dead_code)]
mod stdlib;
mod token;
mod typechecker;
mod wasmgen;

use std::collections::HashMap;

use parser::Parser;
use wasmgen::WasmGen;

const VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");

/// Parse a package source quietly (no diagnostics printing; there is no
/// stdout here).
fn parse_package_source(source: &str) -> anyhow::Result<ast::Program> {
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse()
}

/// The compile pipeline, shared shape with `Compiler::parse_for_codegen`.
fn compile_to_wat(source: &str, packages: &HashMap<String, String>) -> anyhow::Result<String> {
    let mut lexer = lexer::Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let mut program = parser.parse()?;

    // Inject package sources as ModuleDefs, matching how the CLI wraps
    // vendored packages so `use pkg::item` registers pkg::item functions.
    for (name, pkg_source) in packages {
        let vendor_program = parse_package_source(pkg_source)
            .map_err(|e| anyhow::anyhow!("package `{}` failed to parse: {}", name, e))?;
        let module = ast::TopLevel::ModuleDef {
            name: name.clone(),
            items: vendor_program.items,
            doc: None,
        };
        program.items.insert(0, module);
    }

    let mut checker = typechecker::TypeChecker::new().quiet();
    checker.check(&program)?;
    b2::desugar_typed_stores(&mut program);
    b2::implicit_returns(&mut program);
    b2::resolve_block_scoping(&mut program);

    let mut wasmgen = WasmGen::new();
    Ok(wasmgen.generate(&program))
}

/// Split the packages blob (`name=source\0name=source\0`) into a map.
fn parse_packages_blob(bytes: &[u8]) -> anyhow::Result<HashMap<String, String>> {
    let mut packages = HashMap::new();
    let text = std::str::from_utf8(bytes)?;
    for entry in text.split('\0') {
        if entry.is_empty() {
            continue;
        }
        let (name, source) = entry
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("malformed package entry (expected name=source)"))?;
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return Err(anyhow::anyhow!("invalid package name `{}`", name));
        }
        packages.insert(name.to_string(), source.to_string());
    }
    Ok(packages)
}

// ── C ABI surface ────────────────────────────────────────────────────────────

fn write_out_blob(payload: &[u8]) -> *mut u8 {
    // Layout: 4-byte LE length prefix + payload.
    let total = 4 + payload.len();
    let mut buf = Vec::with_capacity(total);
    buf.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    buf.extend_from_slice(payload);
    let leaked = buf.leak();
    leaked.as_mut_ptr()
}

/// Static version string (NUL-terminated), for the playground About line.
#[no_mangle]
pub extern "C" fn sbx_version() -> *const u8 {
    VERSION.as_ptr()
}

/// Allocate `len` bytes for the host to write into. Returns a guest pointer
/// the host passes back to the compile/free entry points.
#[no_mangle]
pub extern "C" fn sbx_alloc(len: u32) -> *mut u8 {
    if len == 0 {
        return std::ptr::NonNull::<u8>::dangling().as_ptr();
    }
    let layout = std::alloc::Layout::from_size_align(len as usize, 1).expect("bad alloc size");
    unsafe { std::alloc::alloc(layout) }
}

/// Free a pointer previously returned by `sbx_alloc` or a compile entry
/// point. `len` must be the length the host wrote (input buffers) — output
/// blobs free the whole envelope, so any len > 0 works for them.
#[no_mangle]
pub extern "C" fn sbx_free(ptr: *mut u8, len: u32) {
    if ptr.is_null() || len == 0 {
        return;
    }
    let layout = std::alloc::Layout::from_size_align(len as usize, 1).expect("bad free size");
    unsafe { std::alloc::dealloc(ptr, layout) }
}

/// Compile a program with registry packages.
///
/// - `in_ptr/in_len`: the main .sbx program source (UTF-8)
/// - `pkgs_ptr/pkgs_len`: `name=source\0...` blob (may be null/0 for none)
/// - `err_out`: written with a guest pointer to an error envelope on
///   failure; untouched on success.
///
/// Returns a guest pointer to a `[len: u32 LE][wat bytes]` envelope, or 0
/// on failure (check `err_out`).
#[no_mangle]
pub extern "C" fn sbx_compile(
    in_ptr: *const u8,
    in_len: u32,
    pkgs_ptr: *const u8,
    pkgs_len: u32,
    err_out: *mut *mut u8,
) -> *mut u8 {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        compile_inner(in_ptr, in_len, pkgs_ptr, pkgs_len)
    }));
    match result {
        Ok(Ok(wat)) => write_out_blob(wat.as_bytes()),
        Ok(Err(e)) => {
            if !err_out.is_null() {
                unsafe {
                    *err_out = write_out_blob(e.to_string().as_bytes());
                }
            }
            std::ptr::null_mut()
        }
        Err(panic_msg) => {
            let err_msg = if let Some(s) = panic_msg.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_msg.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "compiler panicked".to_string()
            };
            if !err_out.is_null() {
                unsafe {
                    *err_out = write_out_blob(err_msg.as_bytes());
                }
            }
            std::ptr::null_mut()
        }
    }
}

fn compile_inner(
    in_ptr: *const u8,
    in_len: u32,
    pkgs_ptr: *const u8,
    pkgs_len: u32,
) -> anyhow::Result<String> {
    let source = unsafe { std::slice::from_raw_parts(in_ptr, in_len as usize) };
    let source = std::str::from_utf8(source)?;

    let packages = if pkgs_ptr.is_null() || pkgs_len == 0 {
        HashMap::new()
    } else {
        let blob = unsafe { std::slice::from_raw_parts(pkgs_ptr, pkgs_len as usize) };
        parse_packages_blob(blob)?
    };

    compile_to_wat(source, &packages)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Folded-WAT s-expression reader: enough structure (parens, quoted
    // strings, `;` comments, bare atoms) to catch mis-nested forms that are
    // still paren-balanced — the exact bug class that put a stray
    // `(else ...)` inside $sbx_gdiv's outer `(then ...)` while every paren
    // count stayed at zero.
    #[derive(Debug)]
    struct SExpr {
        head: String,
        atoms: Vec<String>,
        children: Vec<SExpr>,
    }

    fn parse_sexprs(text: &str) -> Result<Vec<SExpr>, String> {
        let chars: Vec<char> = text.chars().collect();
        let mut pos = 0usize;
        let mut stack: Vec<SExpr> = Vec::new();
        let mut roots: Vec<SExpr> = Vec::new();

        while pos < chars.len() {
            match chars[pos] {
                ';' => {
                    while pos < chars.len() && chars[pos] != '\n' {
                        pos += 1;
                    }
                }
                '"' => {
                    pos += 1;
                    while pos < chars.len() && chars[pos] != '"' {
                        if chars[pos] == '\\' {
                            pos += 1;
                        }
                        pos += 1;
                    }
                    pos += 1;
                }
                '(' => {
                    let mut end = pos + 1;
                    while end < chars.len()
                        && !chars[end].is_whitespace()
                        && chars[end] != '('
                        && chars[end] != ')'
                        && chars[end] != '"'
                        && chars[end] != ';'
                    {
                        end += 1;
                    }
                    let head: String = chars[pos + 1..end].iter().collect();
                    stack.push(SExpr {
                        atoms: vec![head.clone()],
                        head,
                        children: Vec::new(),
                    });
                    // Skip past the head atom: the next loop iteration must
                    // not re-read it as a bare atom.
                    pos = end;
                }
                ')' => {
                    let node = stack.pop().ok_or("unbalanced ')' in emitted WAT")?;
                    match stack.last_mut() {
                        Some(parent) => parent.children.push(node),
                        None => roots.push(node),
                    }
                    pos += 1;
                }
                c if c.is_whitespace() => pos += 1,
                _ => {
                    // Bare atom (symbol/number): record it on the nearest
                    // enclosing form so `$sbx_gdiv` and friends are visible.
                    let mut end = pos;
                    while end < chars.len()
                        && !chars[end].is_whitespace()
                        && chars[end] != '('
                        && chars[end] != ')'
                        && chars[end] != '"'
                        && chars[end] != ';'
                    {
                        end += 1;
                    }
                    let atom: String = chars[pos..end].iter().collect();
                    if let Some(parent) = stack.last_mut() {
                        parent.atoms.push(atom);
                    }
                    pos = end;
                }
            }
        }
        if !stack.is_empty() {
            return Err(format!("{} unclosed '(' in emitted WAT", stack.len()));
        }
        Ok(roots)
    }

    fn find_func<'a>(node: &'a SExpr, func_name: &str) -> Option<&'a SExpr> {
        if node.head == "func" && node.atoms.get(1).map(String::as_str) == Some(func_name) {
            return Some(node);
        }
        node.children.iter().find_map(|c| find_func(c, func_name))
    }

    fn find_child<'a>(node: &'a SExpr, head: &str) -> Option<&'a SExpr> {
        node.children.iter().find(|c| c.head == head)
    }

    fn count_descendants(node: &SExpr, head: &str) -> usize {
        node.children
            .iter()
            .map(|c| count_descendants(c, head) + usize::from(c.head == head))
            .sum()
    }

    fn count_local_ops(node: &SExpr, op: &str, local: &str) -> usize {
        node.children
            .iter()
            .map(|c| {
                count_local_ops(c, op, local)
                    + usize::from(c.head == op && c.atoms.iter().any(|a| a == local))
            })
            .sum()
    }

    fn assert_total_division_helper(module_wat: &str, func_name: &str, div_op: &str) {
        let roots = parse_sexprs(module_wat).expect("emitted WAT failed to parse");
        let func = roots
            .iter()
            .find_map(|n| find_func(n, func_name))
            .unwrap_or_else(|| panic!("{func_name} not found in emitted module"));

        // Exactly two ifs: outer (b != 0) and inner (b != -1), with the
        // inner nested INSIDE the outer's (then ...) — not after it.
        assert_eq!(
            count_descendants(func, "if"),
            2,
            "{func_name}: expected outer b!=0 + inner b!=-1 ifs"
        );

        let outer_if =
            find_child(func, "if").unwrap_or_else(|| panic!("{func_name}: no top-level if"));
        let outer_then = find_child(outer_if, "then")
            .unwrap_or_else(|| panic!("{func_name}: outer if has no (then)"));
        find_child(outer_if, "else").unwrap_or_else(|| {
            panic!("{func_name}: outer if has no (else) — the else closed inside the (then)")
        });

        let inner_if = find_child(outer_then, "if").unwrap_or_else(|| {
            panic!("{func_name}: inner b!=-1 if is not inside the outer (then)")
        });
        find_child(inner_if, "then")
            .unwrap_or_else(|| panic!("{func_name}: inner if has no (then)"));
        find_child(inner_if, "else")
            .unwrap_or_else(|| panic!("{func_name}: inner if has no (else)"));

        // THE regression: a stray (else ...) inside the outer (then ...)
        // keeps every paren count balanced but mis-nests the tree — wabt
        // and the page encoder reject it. Exactly one else may live under
        // the outer then (the inner if's own), and two in the whole helper.
        assert_eq!(
            count_descendants(outer_then, "else"),
            1,
            "{func_name}: stray else inside the outer (then)"
        );
        assert_eq!(
            count_descendants(func, "else"),
            2,
            "{func_name}: expected exactly inner + outer else"
        );

        // The result flows through the $r temp: three local.sets (div/rem
        // arm, MIN/-1 arm, zero arm) and a single tail load.
        assert_eq!(
            count_local_ops(func, "local.set", "$r"),
            3,
            "{func_name}: expected 3 local.sets on $r"
        );
        assert_eq!(
            count_local_ops(func, "local.get", "$r"),
            1,
            "{func_name}: expected a single tail (local.get $r)"
        );
        // The real division happens exactly once, guarded by the inner if.
        assert_eq!(
            count_descendants(func, div_op),
            1,
            "{func_name}: expected exactly one {div_op}"
        );
    }

    #[test]
    fn total_division_helpers_emit_valid_wat() {
        // The runtime preamble (including both total-division helpers) is
        // emitted for every program, so a trivial main exercises them too.
        let wat = compile_to_wat("fn main() { print(1) }", &HashMap::new()).unwrap();
        assert_total_division_helper(&wat, "$sbx_gdiv", "i64.div_s");
        assert_total_division_helper(&wat, "$sbx_grem", "i64.rem_s");
    }

    #[test]
    fn division_program_compiles_with_valid_helpers() {
        let wat = compile_to_wat(
            "fn main() {\n    let zero = 1 - 1\n    let mn = 0 - 9223372036854775807 - 1\n    print(mn / zero)\n    print(mn % zero)\n}",
            &HashMap::new(),
        )
        .unwrap();
        assert_total_division_helper(&wat, "$sbx_gdiv", "i64.div_s");
        assert_total_division_helper(&wat, "$sbx_grem", "i64.rem_s");
    }
}
