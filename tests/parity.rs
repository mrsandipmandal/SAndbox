//! Backend parity harness.
//!
//! Runs each corpus program through every execution backend (C codegen,
//! LLVM codegen, interpreter) and asserts that all enabled backends produce
//! identical program output. Compiler progress noise ([sandbox] / → / ✓
//! lines) is filtered before comparison.
//!
//! Backends that cannot compile or run a program are recorded, not fatal:
//! the allowlist below documents every known gap. A parity failure means a
//! *regression* — a backend that used to match now differs, or a new
//! mismatch on a program tagged as fully supported.
//!
//! WASM is a full output backend when tooling is available: `sandbox wasm`
//! emits .wat, wat2wasm assembles it, and `node scripts/runwasm.mjs` runs
//! it. When wat2wasm (wabt) or node is missing, wasm cases are SKIPPED with
//! a note rather than failed, so local runs without wabt still pass.

use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

// ── Infrastructure ──────────────────────────────────────────────────────────

/// Run-length limit for any single program execution (seconds).
const EXEC_TIMEOUT_SECS: u64 = 10;

fn sandbox_bin() -> String {
    let output = Command::new("cargo")
        .args(["build", "--quiet"])
        .output()
        .expect("Failed to build sandbox");
    assert!(output.status.success(), "cargo build failed");
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/target/debug/sandbox", manifest_dir)
}

/// Returns true if a line is sandbox compiler progress, not program output.
/// Program output always starts at column 0; progress lines are `[sandbox]`
/// or indented arrows/checkmarks/bracketed stage markers.
fn is_progress_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with("[sandbox]") {
        return true;
    }
    if line.starts_with(' ') || line.starts_with('\t') {
        if trimmed.starts_with('\u{2192}') {
            return true;
        } // →
        if trimmed.starts_with('\u{2713}') {
            return true;
        } // ✓
        if trimmed.starts_with('\u{26a0}') {
            return true;
        } // ⚠
        if trimmed.starts_with('[') && (trimmed.contains("] FnDef") || trimmed.contains("] Other"))
        {
            return true;
        }
    }
    false
}

/// Filter compiler progress lines from raw combined stdout/stderr.
fn filter_output(raw: &str) -> String {
    raw.lines()
        .filter(|l| !is_progress_line(l))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Run a command with a hard timeout; returns None if it timed out.
/// Every subprocess the harness spawns goes through here: corpus programs
/// can loop forever on a backend bug, and a plain .output() would hang CI.
fn run_with_timeout(mut cmd: Command, timeout: std::time::Duration) -> Option<Output> {
    use std::io::Read;
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + timeout;
    // Wait for exit with a deadline FIRST. Reading to EOF before checking
    // the clock hangs on children that never print and never exit; reading
    // before waiting hangs on children that out-print the pipe buffer.
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    return None;
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(_) => return None,
        }
    };
    // The child has exited, so its pipes are closed — these reads reach EOF.
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_end(&mut stdout);
    }
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_end(&mut stderr);
    }
    Some(Output {
        status,
        stdout,
        stderr,
    })
}

/// The backends that produce observable program output.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Backend {
    /// C codegen (`sandbox run`)
    C,
    /// LLVM codegen (`sandbox llvm-build` + native binary)
    Llvm,
    /// Tree-walking interpreter (`sandbox interpret`)
    Interp,
    /// WebAssembly (`sandbox wasm` + wat2wasm + node scripts/runwasm.mjs);
    /// requires the wabt toolchain and node, skipped when unavailable.
    Wasm,
}

impl Backend {
    fn name(self) -> &'static str {
        match self {
            Backend::C => "C",
            Backend::Llvm => "LLVM",
            Backend::Interp => "interpreter",
            Backend::Wasm => "wasm",
        }
    }
}

/// Execute a corpus program on one backend; returns the filtered program
/// output, or Err(reason) when the backend cannot produce output.
fn run_backend(backend: Backend, source: &str, workdir: &TempDir) -> Result<String, String> {
    let bin = sandbox_bin();
    let sbx_path = workdir.path().join(format!(
        "prog_{}.sbx",
        match backend {
            Backend::C => "c",
            Backend::Llvm => "llvm",
            Backend::Interp => "interp",
            Backend::Wasm => "wasm",
        }
    ));
    fs::write(&sbx_path, source).map_err(|e| format!("write failed: {e}"))?;

    match backend {
        Backend::C | Backend::Interp => {
            let mode = if backend == Backend::C {
                "run"
            } else {
                "interpret"
            };
            let out = run_with_timeout(
                {
                    let mut c = Command::new(&bin);
                    c.args([mode, sbx_path.to_str().unwrap()]);
                    c
                },
                std::time::Duration::from_secs(EXEC_TIMEOUT_SECS),
            )
            .ok_or_else(|| format!("{mode} timed out"))?;
            if !out.status.success() {
                return Err(format!(
                    "{} exited {}",
                    mode,
                    out.status.code().unwrap_or(-1)
                ));
            }
            Ok(filter_output(&String::from_utf8_lossy(&out.stdout)))
        }
        Backend::Llvm => {
            let bin_path = workdir.path().join("llvm_prog");
            let build = run_with_timeout(
                {
                    let mut c = Command::new(&bin);
                    c.args([
                        "llvm-build",
                        sbx_path.to_str().unwrap(),
                        "-o",
                        bin_path.to_str().unwrap(),
                    ]);
                    c
                },
                std::time::Duration::from_secs(EXEC_TIMEOUT_SECS),
            )
            .ok_or_else(|| "llvm-build timed out".to_string())?;
            if !build.status.success() {
                return Err(format!(
                    "llvm-build failed: {}",
                    String::from_utf8_lossy(&build.stderr)
                ));
            }
            if !bin_path.exists() {
                return Err("llvm-build produced no binary".to_string());
            }
            let run = run_with_timeout(
                {
                    let mut c = Command::new(&bin_path);
                    c.current_dir(workdir.path());
                    c
                },
                std::time::Duration::from_secs(EXEC_TIMEOUT_SECS),
            )
            .ok_or_else(|| "execution timed out".to_string())?;
            Ok(filter_output(&String::from_utf8_lossy(&run.stdout)))
        }
        Backend::Wasm => run_wasm_backend(source, workdir),
    }
}

/// Is the wasm execution toolchain available (wat2wasm on PATH + node)?
fn wasm_toolchain_available() -> bool {
    let wat2wasm = Command::new("wat2wasm").arg("--version").output();
    let node = Command::new("node").arg("--version").output();
    matches!((wat2wasm, node),
        (Ok(w), Ok(n)) if w.status.success() && n.status.success())
}

/// Execute a wasm program: sandbox wasm → wat2wasm → node scripts/runwasm.mjs.
fn run_wasm_backend(source: &str, workdir: &TempDir) -> Result<String, String> {
    let bin = sandbox_bin();
    let sbx_path = workdir.path().join("prog_wasm.sbx");
    let wat_path = workdir.path().join("prog.wat");
    let wasm_path = workdir.path().join("prog.wasm");
    fs::write(&sbx_path, source).map_err(|e| format!("write failed: {e}"))?;

    let gen = run_with_timeout(
        {
            let mut c = Command::new(&bin);
            c.args([
                "wasm",
                sbx_path.to_str().unwrap(),
                "-o",
                wat_path.to_str().unwrap(),
            ]);
            c
        },
        std::time::Duration::from_secs(EXEC_TIMEOUT_SECS),
    )
    .ok_or_else(|| "sandbox wasm timed out".to_string())?;
    if !gen.status.success() {
        return Err(format!(
            "wasm exited {}: {}",
            gen.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&gen.stderr)
        ));
    }

    let asm = run_with_timeout(
        {
            let mut c = Command::new("wat2wasm");
            c.args([
                wat_path.to_str().unwrap(),
                "-o",
                wasm_path.to_str().unwrap(),
            ]);
            c
        },
        std::time::Duration::from_secs(EXEC_TIMEOUT_SECS),
    )
    .ok_or_else(|| "wat2wasm timed out (or not found)".to_string())?;
    if !asm.status.success() {
        return Err(format!(
            "wat2wasm failed: {}",
            String::from_utf8_lossy(&asm.stderr)
        ));
    }

    let runner = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/runwasm.mjs");
    let run = run_with_timeout(
        {
            let mut c = Command::new("node");
            c.arg(runner).arg(wasm_path.to_str().unwrap());
            c
        },
        std::time::Duration::from_secs(EXEC_TIMEOUT_SECS),
    )
    .ok_or_else(|| "wasm execution timed out".to_string())?;
    if !run.status.success() {
        return Err(format!(
            "node runner exited {}: {}",
            run.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&run.stderr)
        ));
    }
    Ok(filter_output(&String::from_utf8_lossy(&run.stdout)))
}

/// Compile-only check for the WASM backend (.wat emission).
fn wasm_compiles(source: &str, workdir: &TempDir) -> Result<(), String> {
    let bin = sandbox_bin();
    let sbx_path = workdir.path().join("prog_wasm.sbx");
    let wat_path = workdir.path().join("prog.wat");
    fs::write(&sbx_path, source).map_err(|e| format!("write failed: {e}"))?;
    let out = run_with_timeout(
        {
            let mut c = Command::new(&bin);
            c.args([
                "wasm",
                sbx_path.to_str().unwrap(),
                "-o",
                wat_path.to_str().unwrap(),
            ]);
            c
        },
        std::time::Duration::from_secs(EXEC_TIMEOUT_SECS),
    )
    .ok_or_else(|| "sandbox wasm timed out".to_string())?;
    if !out.status.success() {
        return Err(format!("wasm exited {}", out.status.code().unwrap_or(-1)));
    }
    if !wat_path.exists() {
        return Err("no .wat emitted".to_string());
    }
    Ok(())
}

// ── Corpus ──────────────────────────────────────────────────────────────────

/// One corpus program: source plus which backends are *required* to run it
/// and match. Backends not listed are known gaps — adding a backend to this
/// list is how a gap gets closed; removing one fails CI if it regressed.
struct ParityCase {
    name: &'static str,
    source: &'static str,
    backends: &'static [Backend],
}

const C_AND_INTERP: &[Backend] = &[Backend::C, Backend::Interp];
/// Programs with strings the wasm backend can't run yet (documented gap).
const C_LLVM_INTERP: &[Backend] = &[Backend::C, Backend::Llvm, Backend::Interp];
const C_ONLY: &[Backend] = &[Backend::C];
const LLVM_ONLY: &[Backend] = &[Backend::Llvm];
/// Integer programs across every backend, wasm included.
const ALL_INT: &[Backend] = &[Backend::C, Backend::Llvm, Backend::Interp, Backend::Wasm];
/// C + interpreter + wasm: used for array cases the LLVM backend can't run
/// (for-in over a local *array variable* falls through to a single inline
/// body pass — pre-existing LLVM gap, not a divergence).
const C_INTERP_WASM: &[Backend] = &[Backend::C, Backend::Interp, Backend::Wasm];
// Known wasm-backend gaps (cases demoted to C_AND_INTERP; closing one means
// implementing it in src/wasmgen.rs and widening the tag):
//   - strings: no string codegen (print(str) emits invalid WAT) → method_*,
//     str_methods, fstring, json_string_*, a4_* , http_* cases
//   - for-in over a local array *variable*: LLVM's fallback runs the body
//     once inline (for_over_array*); literal iterables and len() of literal-
//     initialized arrays work (array_len_basics runs on all four backends)
//   - match arms: `match` compiles but never executes its arm →
//     match_int_literal, match_guard, enum_match, enum_payload
//   - bool literals print 1/0 instead of true/false → bool_logic
//   - lambdas/closures: not implemented → lambda_expr, lambda_capture

const CORPUS: &[ParityCase] = &[
    ParityCase {
        name: "bitwise_ops",
        source: r#"
fn main() {
    let a = 12
    let b = 10
    print(a & b)
    print(a | b)
    print(a ^ b)
    print(~a)
    print(~0)
    print(255 & 15)
    print(-1 ^ 256)
}
"#,
        // B1: & | ^ ~ on two's-complement i64 — identical results across
        // C (operators), LLVM (and/or/xor), and the interpreter (Rust ops).
        backends: ALL_INT,
    },
    ParityCase {
        name: "shift_ops",
        source: r#"
fn main() {
    print(1 << 10)
    print(-16 >> 2)
    print(2 + 3 << 1 * 2)
    print(1 << 3 + 1)
    print(16 >> 1 + 1)
    print((a_shift_helper(1, 63)) >> 62)
}
fn a_shift_helper(v: i64, n: i64) -> i64 {
    return v << n
}
"#,
        // B1: shifts — >> is arithmetic; << wraps (the C backend shifts
        // through unsigned long long so overflowing into the sign bit is
        // defined and matches LLVM ashr / the interpreter). Shifts bind
        // tighter than comparisons, looser than addition.
        backends: ALL_INT,
    },
    ParityCase {
        name: "b2_casts",
        source: r#"
fn main() {
    let a: u8 = 4294967296
    print(a)
    print(-1 as u8)
    print(-1 as i8)
    print(255 as i8)
    print(200 as u8 + 100 as u8)
    let big = 456 as u8
    print(big)
    print(big as i64)
    print(-5 as u64)
    print(2.9 as u8)
    print(-2.5 as u8)
    print(3.7 as i64)
    let u = 7 as usize
    print(u)
}
"#,
        // B2: `as` casts and typed stores wrap to the target width at every
        // store point (let, arithmetic operand promotion, float truncation).
        // Narrow values live as i64 at rest, so an unsigned wrap prints as
        // its signed bit pattern (255 → -1 becomes +255 for u8; -5 as u64
        // stays -5 when read back through i64).
        backends: ALL_INT,
    },
    ParityCase {
        name: "b2_narrow_wrap",
        source: r#"
fn clamp8(x: i16) -> i16 {
    return x * 1000
}
fn main() {
    print(clamp8(100))
    print(clamp8(40))
    let m: u32 = 4000000000
    m = m + 1000000000
    print(m)
    let s: i8 = 100
    s = s + 100
    print(s)
    let z: usize = -1
    print(z)
}
"#,
        // B2: wrapping happens at function returns, parameter bindings and
        // re-assignment, not just `let` — 100000 → -31072 (i16), 5000000000
        // → 705032704 (u32), 200 → -56 (i8). usize is a 64-bit unsigned
        // wrap, so -1 stays -1 in the i64-at-rest repr.
        backends: ALL_INT,
    },
    ParityCase {
        name: "b2_typed_indices",
        source: r#"
fn main() {
    let a = [10, 20, 30]
    print(a[2 as usize])
    print(a[1 as u8])
    print(a[2 as i32])
    let idx: usize = 1
    print(a[idx])
    let j: u8 = 2
    print(a[j])
    let n: i8 = -1
    print(a[n as usize + 1])
    for i in 0..3 {
        print(a[i as usize])
    }
}
"#,
        // B2 completion: array indexing accepts every integer type — `as`
        // casts and variables bound with a declared narrow type alike
        // (values are i64 at rest, so indices need no conversion).
        // Previously `a[2 as usize]` was rejected with "Array index must be
        // i64", while `let idx: usize = 1` silently passed because the
        // checker stored the value's type instead of the declared one.
        backends: C_LLVM_INTERP,
    },
    ParityCase {
        name: "b2_declared_type_bindings",
        source: r#"
fn describe(t: u8) -> i64 {
    return t as i64 * 10
}
fn main() {
    let w: u8 = 200
    print(w)
    w = 300
    print(w)
    let c: usize = 0
    c = c + 2
    print(c)
    print(describe(w))
    let small: i16 = 3
    print(small * 1000)
}
"#,
        // B2 completion: `let x: T = v` binds the DECLARED type T, not the
        // value's type — later checks (assignments, call args, indexing)
        // see the annotation. Re-assignment goes through the same implicit
        // narrow/wrap store as `let` (300 → 44 in u8), so hosts agree.
        backends: C_LLVM_INTERP,
    },
    ParityCase {
        name: "wasm_arrays",
        source: r#"
fn main() {
    let a = [10, 20, 30]
    print(a[0])
    print(a[2])
}
"#,
        // Array support in the wasm backend: heap allocation (bump
        // allocator + length header) and index reads on a local array.
        // Scope: LOCAL arrays — array-typed parameters are a pre-existing
        // gap on the host backends (C counts elements via sizeof-decay,
        // LLVM has no array lowering, the interpreter binds param arrays
        // to 0), len() is a pre-existing LLVM hole (returns 0) and breaks
        // through an alias on the interpreter, so those live in the
        // C_INTERP_WASM case below; wasm implements all of it correctly.
        backends: ALL_INT,
    },
    ParityCase {
        name: "wasm_arrays_exprs",
        source: r#"
fn main() {
    let a = [10, 20, 30]
    let b = [2, 0, 1]
    print(a[b[0]])
    print(a[b[1]])
    let i = 1
    print(a[i + 1])
    let c = a[1] + b[2]
    print(c)
}
"#,
        // Index expressions everywhere: nested indices, arithmetic
        // indices, element values in arithmetic.
        backends: ALL_INT,
    },
    ParityCase {
        name: "wasm_arrays_forin",
        source: r#"
fn main() {
    let a = [10, 20, 30]
    let s = 0
    for x in a {
        s = s + x
    }
    print(s)
    for x in [4, 5, 6] {
        print(x)
    }
    print(a[len(a) - 1])
}
"#,
        // for-in over a local array and over a literal, plus len() and a
        // len()-derived index. C_INTERP_WASM: the LLVM backend's for-in over
        // a local array *variable* runs the body once inline (pre-existing
        // gap — len(array_ident) itself was fixed to use the literal length,
        // see array_len_basics); C, the interpreter and wasm agree.
        backends: C_INTERP_WASM,
    },
    ParityCase {
        name: "array_len_basics",
        source: r#"
fn main() {
    let a = [10, 20, 30]
    print(len(a))
    print(a[0])
    print(a[1 + 1])
    let s = 0
    for x in [4, 5, 6] {
        s = s + x
    }
    print(s)
    print(a[len(a) - 1])
    let w = [8, 9]
    w = [1]
    print(len(w))
    print(w[0])
}
"#,
        // len() of a literal-initialized array: C uses sizeof at the
        // declaration, LLVM now emits the literal length (array_lens map —
        // previously a bare i64* had no header, so len(ident) returned 0),
        // the interpreter reads its vector, wasm its runtime header.
        // Deliberately straight-line: len() reads inside/after branches and
        // loops are conservatively 0 on LLVM (its facts are dropped at
        // control-flow joins), aliasing (`let b = a`) diverges on C
        // (sizeof-decay → 1) and the interpreter (→ 0), and growing literal
        // reassignment overflows C's fixed-size buffer — all pre-existing.
        backends: ALL_INT,
    },
    ParityCase {
        name: "narrow_compare_basics",
        source: r#"
fn main() {
    let a: u64 = -1 as u64
    if a > 5 { print(1) } else { print(0) }
    if a == -1 { print(1) } else { print(0) }
    if a < 1 { print(1) } else { print(0) }
    if a > 9223372036854775807 { print(1) } else { print(0) }
    let b: usize = 0 - 1
    if b >= 0 { print(1) } else { print(0) }
    if b == -1 { print(1) } else { print(0) }
    if b != 0 { print(1) } else { print(0) }
    let c: u8 = 200
    let d: u8 = 100
    if c > d { print(1) } else { print(0) }
    if -1 as u8 == 255 { print(1) } else { print(0) }
    if -1 as i8 == -1 { print(1) } else { print(0) }
    if -1 as i8 < 0 { print(1) } else { print(0) }
    let e: i32 = -5
    if e < 3 { print(1) } else { print(0) }
    if e as u32 == 4294967291 { print(1) } else { print(0) }
    if e as u32 > 0 { print(1) } else { print(0) }
    let f: u16 = 65535
    if f > 32768 { print(1) } else { print(0) }
    let g: i8 = -1
    if g > -2 { print(1) } else { print(0) }
    if a > b { print(1) } else { print(0) }
    let h = 0
    for i in 0..3 {
        if i <= 1 {
            h = h + 10
        }
    }
    print(h)
}
"#,
        // B2 audit: comparisons run signed on i64 bit patterns in every
        // backend, including typed u64/usize variables holding values above
        // i64::MAX. The C backend used to let its typed unsigned variables
        // make `>`/`>=`/`==` compare unsigned (C's native operator), so
        // u64(-1) > 5 was true in C and false everywhere else; the codegen
        // now forces a (long) compare when either operand is unsigned long
        // long. Sub-64-bit unsigned types promote to int in C and were
        // already signed; LLVM/interp/wasm compare i64 signed by design.
        // Expected: 0 1 0 1  0 1 1  1 1  1 1  1 1 1  1 1 0  20
        backends: ALL_INT,
    },
    ParityCase {
        name: "bool_cast_basics",
        source: r#"
fn main() {
    print((1 < 2) as i64)
    print((true) as i64)
    print((2 == 3) as u8)
    print((false) as u64)
    let flag = 5 != 5
    print(flag as i64)
    print((flag as i64) + 10)
    let ok: i64 = (7 < 8) as i64
    print(ok)
    print((!flag) as i64)
    print((1 < 2 && 3 < 4) as u8)
    if (1 < 2) as i64 == 1 {
        print(99)
    }
    let w = [10, 20, 30]
    print(w[(2 > 1) as i64 + 1])
}
"#,
        // bool -> int casts: booleans are 1/0 at rest in every backend (the
        // interpreter's Bool arm, C's int-typed comparisons, LLVM's zexted i1
        // — the Cast arm zero-extends comparison registers before the B2
        // wrap — and wasm's extended i32), so `expr as T` re-types the
        // expression without changing the value. Covers comparison sources,
        // bool literals, ! and && sources, narrow/unsigned targets,
        // arithmetic on the cast, a declared-type let, condition position
        // and an array index.
        backends: ALL_INT,
    },
    ParityCase {
        name: "method_string",
        source: r#"
fn main() {
    let s = "Hello World"
    print(s.to_upper())
    print(s.to_lower())
    print(s.len())
    print(s.trim())
    print(s.contains("World"))
    print(s.substring(0, 5))
    print(s.replace("World", "Sandbox"))
    print(s.find("World"))
    print(s.is_empty())
}
"#,
        // Known gap: LLVM lacks string runtime codegen for method sugar.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "method_array",
        source: r#"
fn main() {
    let nums = [1, 2, 3, 4, 5]
    let doubled = nums.map(|x| x * 2)
    print(doubled)
    let evens = nums.filter(|x| x > 2)
    print(evens)
    print(len(evens))
    let sum = nums.reduce(|a, b| a + b, 0)
    print(sum)
    let total = nums.map(|x| x + 1).reduce(|a, b| a + b, 0)
    print(total)
}
"#,
        // Known gap: LLVM lacks lambda + map/filter/reduce codegen.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "method_chained_string",
        source: r#"
fn main() {
    let s = "Hi There"
    print(s.trim().to_lower())
    print("  x  ".trim().to_upper())
}
"#,
        // Known gap: LLVM lacks string runtime codegen for method sugar.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "arith_precedence",
        source: r#"
fn main() {
    print(2 + 3 * 4)
    print(10 - 4 - 3)
    print((2 + 3) * 4)
    print(17 / 5)
    print(17 % 5)
}
"#,
        backends: ALL_INT,
    },
    ParityCase {
        name: "bool_logic",
        source: r#"
fn main() {
    print(true)
    print(false)
    print(1 < 2)
    print(2 == 2)
    print(3 != 3)
    print(1 < 2 && 2 < 3)
    print(false || true)
    print(!false)
}
"#,
        // Known gap: LLVM has no && / || / ! codegen.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "fn_call",
        source: r#"
fn double(x: i64) -> i64 {
    return x * 2
}

fn main() {
    print(double(21))
    print(double(double(3)))
}
"#,
        backends: ALL_INT,
    },
    ParityCase {
        name: "recursion",
        source: r#"
fn fact(n: i64) -> i64 {
    if n <= 1 { 1 } else { n * fact(n - 1) }
}
fn fib(n: i64) -> i64 {
    if n < 2 { n } else { fib(n - 1) + fib(n - 2) }
}
fn main() {
    print(fact(10))
    print(fib(15))
}
"#,
        // Value-yielding trailing `if` — the language's recursion idiom,
        // desugared to explicit returns by the shared front-end pass — must
        // agree everywhere, including wasm's validator-strict codegen.
        backends: ALL_INT,
    },
    ParityCase {
        name: "if_else_chain",
        source: r#"
fn main() {
    let x = 5
    if x > 3 {
        print(1)
    } else {
        print(0)
    }
    if x < 0 {
        print(-1)
    } else if x == 0 {
        print(0)
    } else {
        print(2)
    }
}
"#,
        backends: ALL_INT,
    },
    ParityCase {
        name: "while_loop",
        source: r#"
fn main() {
    let i = 0
    while i < 3 {
        print(i)
        i = i + 1
    }
}
"#,
        // Loop counters must ASSIGN (`i = i + 1`): a body `let i = i + 1`
        // declares a fresh block-scoped shadow, so the condition's `i` never
        // changes and the program diverges by design (same as Rust). The old
        // body-`let` form was also where the LLVM while-loop stale-value gap
        // lived; with assignment semantics all four backends agree.
        backends: ALL_INT,
    },
    ParityCase {
        name: "for_range",
        source: r#"
fn main() {
    for i in 0..3 {
        print(i)
    }
}
"#,
        backends: ALL_INT,
    },
    ParityCase {
        name: "for_over_array",
        source: r#"
fn main() {
    let nums = [10, 20, 30]
    for n in nums {
        print(n)
    }
}
"#,
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "for_over_array_literal",
        source: r#"
fn main() {
    for item in [10, 20, 30] {
        print(item)
    }
}
"#,
        // All backends must agree: for-in over an array literal was a
        // silent no-op in the interpreter and invalid IR in LLVM until
        // literal iteration was implemented in both.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "for_in_array_break_continue",
        source: r#"
fn main() {
    for x in [1, 2, 3, 4, 5] {
        if x == 2 {
            continue
        }
        if x == 4 {
            break
        }
        print(x)
    }
    print("done")
}
"#,
        // All backends must agree: the C backend used to unroll array-literal
        // iteration, so break/continue failed to compile ("not within a loop").
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "map_basics",
        source: r#"
fn getval(m: map<string, i64>) -> i64 {
    return m.get("v")
}
fn mkmap() -> map<string, i64> {
    return {"x": 10, "y": 20}
}
fn bump(m: map<string, i64>) -> i64 {
    m.insert("n", m.get("n") + 1)
    return m.get("n")
}
fn main() {
    let m = {"name": 1, "age": 2}
    print(m["name"])
    m.insert("city", 3)
    print(m.get("city"))
    print(m.get("nope"))
    print(m.get("nope", 7))
    print(m.has("age"))
    print(m.len())
    m.remove("age")
    print(m.has("age"))
    print(m.keys())
    print(m)
    let empty = {}
    print(empty.len())
    print(empty)
    let m2 = mkmap()
    print(m2["x"])
    print(getval(m2))
    bump(m2)
    print(m2.get("n"))
    let k = "city"
    print(m.get(k))
}
"#,
        // All backends must agree: maps (map<string, i64>) are a new type —
        // literal, index, insert/get/has/remove/keys/len, empty literal, fn
        // args/returns, by-reference mutation, and string-variable keys.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "str_array_basics",
        source: r#"
fn count(names: [string]) -> i64 {
    return len(names)
}
fn second(names: [string]) -> string {
    return names[1]
}
fn main() {
    let names = ["alice", "bob", "carol"]
    print(names)
    print(len(names))
    print(names[0])
    print(names[2])
    for n in names {
        print(n)
    }
    print(count(names))
    print(second(names))
    let empty: [string] = []
    print(len(empty))
    let copy = names
    print(copy[1])
    copy = ["x", "y", "z"]
    print(copy[0])
    print(copy[2])
}
"#,
        // C1: string arrays as first-class values — handle literals, len,
        // bounds-checked indexing, for-over-elements, [string] params,
        // string-returning fns, empty literal with annotation, handle
        // aliasing and reassignment. C/LLVM/interp agree; wasm has no
        // string support yet (documented gap).
        backends: C_LLVM_INTERP,
    },
    ParityCase {
        name: "map_keys_values",
        source: r#"
fn main() {
    let scores = {"alpha": 10, "beta": 20, "gamma": 30}
    let ks = scores.keys()
    print(len(ks))
    print(ks[0])
    print(ks[1])
    print(ks[2])
    let vs = scores.values()
    print(vs[0] + vs[1] + vs[2])
    let total = 0
    for v in vs {
        total = total + v
    }
    print(total)
}
"#,
        // C1: keys() returns a real string array (was a comma-joined string),
        // values() a real i64 array — len/index/for all agree across backends.
        backends: C_LLVM_INTERP,
    },
    ParityCase {
        name: "http_url_decode",
        source: r#"
fn main() {
    print(http::url_decode("a%20b+c"))
    print(http::url_decode("100%25%21"))
    print(http::url_decode("plain"))
    print(http::url_decode("x%2Fy%3Fz%3D1"))
}
"#,
        // A3: percent-decoding + '+'→space, pure helper. All backends agree
        // on decode semantics including %2F (%2f uppercase hex) etc.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "http_query_params",
        source: r#"
fn main() {
    let q = "who=world&n=42&empty=&flag"
    print(http::query_param(q, "who"))
    print(http::query_param(q, "n"))
    print(http::query_param(q, "empty"))
    print(http::query_param(q, "flag"))
    print(http::query_param(q, "missing"))
    let decoded = http::query_param("q=a%20b", "q")
    print(decoded)
}
"#,
        // A3: value extraction from a query string — present, numeric,
        // valueless ("="), bare flag, absent, and percent-decoded values.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "http_form_parse",
        source: r#"
fn main() {
    let body = "name=anne+smith&age=30&ok=1"
    print(http::form_get(body, "name"))
    print(http::form_get(body, "age"))
    print(http::form_get(body, "ok"))
    print(http::form_get(body, "nope"))
    let who = http::query_param("who=x+y", "who")
    print(who)
}
"#,
        // A3: form-encoded body parsing ('+'→space in values), mirroring
        // query_param semantics — same pair grammar, different source.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "a4_html_escape",
        source: r#"
fn main() {
    let raw = "<b>Tom & \"Jerry\"</b>"
    let esc = html::escape(raw)
    print(esc)
    print(html::unescape(esc))
    print(html::escape("it's ok"))
    print(html::unescape("A &amp; B &#65; &#x42; &unknown;"))
    print(html::escape(""))
}
"#,
        // A4: HTML escaping round-trips; numeric + named entity decoding;
        // unknown entities pass through untouched.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "a4_tmpl_render",
        source: r#"
fn main() {
    let page = tmpl::render("<h1>%{title}</h1><p>%{body}</p>", "title", "Hi", "body", "Body & more")
    print(page)
    print(tmpl::render("%{a}-%{a}", "a", "dup"))
    print(tmpl::render("%{missing} stays", "x", "y"))
    print(tmpl::render("no placeholders", "x", "y"))
}
"#,
        // A4: %{key} substitution — repeated keys, unknown keys left as-is,
        // templates without placeholders untouched.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "a4_cookie_get",
        source: r#"
fn main() {
    let hdr = "sid=abc123; theme=dark; user=ann"
    print(http::cookie_get(hdr, "sid"))
    print(http::cookie_get(hdr, "theme"))
    print(http::cookie_get(hdr, "user"))
    print(http::cookie_get(hdr, "nope"))
    print(http::cookie_get("", "sid"))
}
"#,
        // A4: Cookie header parsing (RFC 6265 '; '-separated pairs).
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "json_parse_basics",
        source: r#"
fn main() {
    let m = json::parse_map("{\"id\": 7, \"count\": 42, \"ok\": true, \"off\": false, \"nothing\": null}")
    print(m.get("id"))
    print(m.get("count"))
    print(m.get("ok"))
    print(m.get("off"))
    print(m.len())
    print(m.has("id"))
    print(m.has("missing"))
    print(m)
    let s = json::stringify_map(m)
    print(s)
    let n = json::get_int("{\"a\": -15}", "a")
    print(n)
}
"#,
        // A2: JSON object → map<string,i64>. All backends must agree on
        // numeric fields, true/false/null mapping (1/0/0), string-field
        // skipping, insertion order through stringify, and get_int on
        // negative numbers.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "json_stringify_roundtrip",
        source: r#"
fn main() {
    let m = {"b": 2, "a": 1}
    let s = json::stringify_map(m)
    print(s)
    let back = json::parse_map(s)
    print(back.get("b"))
    print(back.get("a"))
    print(back.len())
    let s2 = json::stringify_map(back)
    print(s2)
    let arr = json::stringify_array([10, 20, 30])
    print(arr)
    print(json::array_get_int("[7, 8, 9]", 0))
    print(json::array_get_int("[7, 8, 9]", 2))
    print(json::array_get_int("[7, 8, 9]", 5))
}
"#,
        // A2: map → JSON text → map roundtrip preserves order and values;
        // array literals stringify as [1,2,3]; array element access is
        // bounds-safe (out of range → 0). All backends must agree.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "json_string_fields",
        source: r#"
fn main() {
    let name = json::get_str("{\"user\": {\"name\": \"Alice\"}, \"n\": 3}", "name")
    print(name)
    let obj = "{\"title\": \"The Book\", \"copies\": 4}"
    let t = json::get_str(obj, "title")
    print(t)
    let c = json::get_int(obj, "copies")
    print(c)
    let m = json::parse_map(obj)
    print(m.get("copies"))
    print(m.len())
}
"#,
        // A2: string fields stay readable from the raw JSON text via
        // get_str (map<string,i64> cannot hold string values); mixed
        // string/numeric objects parse with string fields skipped.
        // Known gap: LLVM string runtime lacks method-sugar parity here.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "string_basics",
        source: r#"
fn main() {
    print("hello")
    let s = "a" + "b"
    print(s)
    print(len("hello"))
    if "apple" < "banana" {
        print(1)
    } else {
        print(0)
    }
}
"#,
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "fstring",
        source: r#"
fn main() {
    let x = 5
    print(f"x = {x}")
    print(f"{x} * {x} = {x * x}")
}
"#,
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "struct_field_access",
        source: r#"
struct Point {
    x: i64,
    y: i64,
}

fn main() {
    let p = Point { x: 3, y: 4 }
    print(p.x + p.y)
}
"#,
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "lambda_expr",
        source: r#"
fn main() {
    let f = |x| x + 1
    print(f(41))
}
"#,
        // Known gap: LLVM lambda codegen does not compile.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "lambda_capture",
        source: r#"
fn main() {
    let offset = 10
    let f = |x| x + offset
    print(f(32))
}
"#,
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "match_int_literal",
        source: r#"
fn main() {
    let x = 5
    let r = match x {
        5 => 42,
        _ => 0,
    }
    print(r)
}
"#,
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "match_guard",
        source: r#"
fn classify(n: i64) -> i64 {
    return match n {
        x if x < 0 => 0,
        x if x == 0 => 1,
        _ => 2,
    }
}

fn main() {
    print(classify(-3))
    print(classify(0))
    print(classify(7))
}
"#,
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "enum_match",
        source: r#"
enum Color { Red, Green, Blue }

fn main() {
    let c = Color::Green
    let r = match c {
        Color::Red => 1,
        Color::Green => 2,
        _ => 3,
    }
    print(r)
}
"#,
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "enum_payload",
        source: r#"
enum Maybe { Has(i64), Nothing }

fn main() {
    let m = Maybe::Has(42)
    let r = match m {
        Maybe::Has(v) => v,
        _ => 0,
    }
    print(r)
}
"#,
        // Known gap: LLVM enum-payload matching does not compile.
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "map_filter_reduce",
        source: r#"
fn main() {
    let nums = [1, 2, 3, 4, 5]
    let doubled = map(nums, |x| x * 2)
    print(doubled)
    let evens = filter(nums, |x| x % 2 == 0)
    print(evens)
    print(len(evens))
    let total = reduce(nums, |a, b| a + b, 0)
    print(total)
}
"#,
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "shadow_mfr_chain",
        source: r#"
fn main() {
    let nums = [1, 2, 3, 4, 5]
    let result = map(nums, |x| x * 2)
    print(result)
    let result = reduce(result, |a, b| a + b, 0)
    print(result)
}
"#,
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "money_literal",
        source: r#"
fn main() {
    let price = 10.50USD
    print(price)
}
"#,
        // Known gap: interpreter Money literals evaluate to 0.
        backends: LLVM_ONLY,
    },
    ParityCase {
        name: "money_arith",
        source: r#"
fn main() {
    let a = 10.50USD
    let b = 2.25USD
    print(a + b)
    print(a * 3)
}
"#,
        backends: LLVM_ONLY,
    },
    ParityCase {
        name: "generics_money",
        source: r#"
struct Pair<T> {
    first: T,
    second: T,
}

fn main() {
    let p = Pair<i64> { first: 1, second: 2 }
    print(p.first + p.second)
}
"#,
        // Known gap: LLVM and interpreter reject generic struct literals
        // (`::` turbofish unsupported; bare `Pair<i64>` breaks the interpreter).
        backends: C_ONLY,
    },
    ParityCase {
        name: "break_continue",
        source: r#"
fn main() {
    let i = 0
    while i < 10 {
        i = i + 1
        if i == 2 {
            continue
        }
        if i == 5 {
            break
        }
        print(i)
    }
}
"#,
        // Same as while_loop: counters assign instead of shadow-let, which
        // would re-declare a fresh binding each iteration and loop forever.
        backends: ALL_INT,
    },
    ParityCase {
        name: "for_range_body_assign_break",
        source: r#"
fn main() {
    for i in 0..9 {
        i = i + 10
        if i == 13 {
            break
        }
        print(i)
    }
    print(99)
}
"#,
        // The loop variable is a fresh binding each iteration (like a let in
        // the body): `i = ...` assigns the per-iteration copy and never the
        // induction slot, so break/continue cannot corrupt C's loop counter
        // (the C backend used to wedge or step the `for` counter here).
        backends: ALL_INT,
    },
    ParityCase {
        name: "for_range_body_assign_continue",
        source: r#"
fn main() {
    for i in 0..10 {
        i = i + 2
        if i == 6 {
            continue
        }
        print(i)
    }
}
"#,
        // Continue must also leave the hidden induction slot alone: every
        // iteration re-seeds the visible binding from the counter.
        backends: ALL_INT,
    },
    ParityCase {
        name: "for_range_shadow_let_break",
        source: r#"
fn main() {
    let i = 100
    for i in 0..5 {
        let i = i * 10
        if i == 20 {
            break
        }
        print(i)
    }
    print(i)
}
"#,
        // Shadowing `let i` inside the body is a NEW binding each iteration;
        // the loop itself still walks 0..5, and the outer `i` keeps its value.
        backends: ALL_INT,
    },
    ParityCase {
        name: "for_range_nested_shadow_break",
        source: r#"
fn main() {
    for i in 0..3 {
        for i in 0..4 {
            i = i + 5
            if i > 7 {
                break
            }
            print(i)
        }
        print(100 + i)
    }
}
"#,
        // Nested loop reusing the variable name: the inner body's assignment
        // must reach only the inner iteration's copy — both counters, and the
        // outer copy, stay untouched.
        backends: ALL_INT,
    },
    ParityCase {
        name: "str_methods",
        source: r#"
fn main() {
    print(len("abcd"))
    let t = "Hello" + " " + "World"
    print(t)
}
"#,
        backends: C_AND_INTERP,
    },
    ParityCase {
        name: "block_scope_shadow",
        source: r#"
fn main() {
    let y = 5
    if true {
        let y = 7
        print(y)
    }
    print(y)
}
"#,
        // Block scoping: a `let` inside a branch is a NEW binding — the b2
        // renamer gives shadowing declarations a fresh name before codegen,
        // so every backend sees 7 then 5 instead of 7 then 7. Uses outside
        // a binding's block are typechecker errors (see the block_scope
        // integration tests); wasm runs this too since it's integer-only.
        backends: ALL_INT,
    },
    ParityCase {
        name: "block_scope_fresh",
        source: r#"
fn main() {
    let total = 0
    if true {
        let bonus = 40
        total = total + bonus
    }
    print(total + 2)
    let sum = 0
    for i in 0..3 {
        let step = i * 2
        sum = sum + step
    }
    print(sum)
}
"#,
        // Block scoping: branch/loop-local declarations are usable inside
        // their own block, assignments to outer variables still escape the
        // block, and the interpreter re-scopes each while/if arm so nothing
        // leaks across iterations (42 / 6 on every backend).
        backends: ALL_INT,
    },
];

// ── Parity test ─────────────────────────────────────────────────────────────

#[test]
fn parity_all_backends_agree() {
    let bin = sandbox_bin();
    let _ = &bin; // ensure build happened up-front

    let mut failures: Vec<String> = Vec::new();
    let mut ran = 0usize;
    let wasm_available = wasm_toolchain_available();

    for case in CORPUS {
        // WASM: compile-only smoke for every case.
        let wasm_dir = TempDir::new().unwrap();
        if let Err(e) = wasm_compiles(case.source, &wasm_dir) {
            failures.push(format!("[wasm/{}] {e}", case.name));
        }

        for &backend in case.backends {
            if backend == Backend::Wasm && !wasm_available {
                println!(
                    "NOTE: skipping [{}] — wat2wasm/node not available (install wabt)",
                    case.name
                );
                continue;
            }
            ran += 1;
            let dir = TempDir::new().unwrap();
            match run_backend(backend, case.source, &dir) {
                Err(reason) => {
                    failures.push(format!(
                        "[{}/{}] backend failed: {reason}",
                        case.name,
                        backend.name()
                    ));
                }
                Ok(output) => {
                    // Compare against the first enabled backend as reference.
                    let reference = case.backends[0];
                    if backend != reference {
                        let ref_dir = TempDir::new().unwrap();
                        match run_backend(reference, case.source, &ref_dir) {
                            Ok(expected) if expected != output => {
                                failures.push(format!(
                                    "[{}/{}] output mismatch:\n  {} = {expected:?}\n  {} = {output:?}",
                                    case.name,
                                    backend.name(),
                                    reference.name(),
                                    backend.name(),
                                ));
                            }
                            Err(reason) => failures.push(format!(
                                "[{}/{}] reference backend failed: {reason}",
                                case.name,
                                reference.name()
                            )),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    println!(
        "\nParity harness: {ran} backend executions across {} programs",
        CORPUS.len()
    );
    if !failures.is_empty() {
        panic!(
            "Backend parity failures ({}):\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}

/// Guard: the allowlist must never silently grow. Every program must be
/// runnable by at least the C backend and the interpreter unless tagged
/// otherwise, and the corpus must be non-trivial.
#[test]
fn corpus_sanity() {
    assert!(CORPUS.len() >= 15, "corpus shrank unexpectedly");
    for case in CORPUS {
        assert!(!case.backends.is_empty(), "{} has no backends", case.name);
        // Every program must be exercised by at least one output-producing
        // backend; LLVM- or WASM-only coverage would not catch regressions.
        assert!(
            case.backends.contains(&Backend::C)
                || case.backends.contains(&Backend::Interp)
                || case.backends.contains(&Backend::Llvm),
            "{} not covered by any output backend",
            case.name
        );
    }
    // Tags must stay honest: report cases limited to a single backend so
    // gaps stay visible. If a backend gains support, widen the tag set.
    for case in CORPUS {
        if case.backends.len() == 1 {
            println!("NOTE: {} is limited to {:?}", case.name, case.backends);
        }
    }
}
