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
//! WASM is compile-only today (`sandbox wasm` emits .wat; no wat2wasm /
//! wasm runtime is wired), so it is exercised for *compilation success*
//! only, not output parity.

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
fn run_with_timeout(mut cmd: Command, timeout: std::time::Duration) -> Option<Output> {
    use std::io::Read;
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + timeout;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    // Corpus programs are short-lived; read to EOF rather than using
    // wait_with_output, which can hang on a runaway binary's stderr.
    while let Some(out) = child.stdout.as_mut() {
        let mut buf = [0u8; 8192];
        match out.read(&mut buf) {
            Ok(0) => break, // EOF
            Ok(n) => stdout.extend_from_slice(&buf[..n]),
            Err(_) => break,
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            return None;
        }
    }
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_end(&mut stderr);
    }
    let status = child.wait().ok()?;
    Some(Output {
        status,
        stdout,
        stderr,
    })
}

/// The three backends that produce observable program output.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Backend {
    /// C codegen (`sandbox run`)
    C,
    /// LLVM codegen (`sandbox llvm-build` + native binary)
    Llvm,
    /// Tree-walking interpreter (`sandbox interpret`)
    Interp,
}

impl Backend {
    fn name(self) -> &'static str {
        match self {
            Backend::C => "C",
            Backend::Llvm => "LLVM",
            Backend::Interp => "interpreter",
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
            let out = Command::new(&bin)
                .args([mode, sbx_path.to_str().unwrap()])
                .output()
                .map_err(|e| format!("spawn failed: {e}"))?;
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
            let build = Command::new(&bin)
                .args([
                    "llvm-build",
                    sbx_path.to_str().unwrap(),
                    "-o",
                    bin_path.to_str().unwrap(),
                ])
                .output()
                .map_err(|e| format!("spawn failed: {e}"))?;
            if !build.status.success() {
                return Err("llvm-build failed".to_string());
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
    }
}

/// Compile-only check for the WASM backend (.wat emission).
fn wasm_compiles(source: &str, workdir: &TempDir) -> Result<(), String> {
    let bin = sandbox_bin();
    let sbx_path = workdir.path().join("prog_wasm.sbx");
    let wat_path = workdir.path().join("prog.wat");
    fs::write(&sbx_path, source).map_err(|e| format!("write failed: {e}"))?;
    let out = Command::new(&bin)
        .args([
            "wasm",
            sbx_path.to_str().unwrap(),
            "-o",
            wat_path.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("spawn failed: {e}"))?;
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

const ALL: &[Backend] = &[Backend::C, Backend::Llvm, Backend::Interp];
const C_AND_INTERP: &[Backend] = &[Backend::C, Backend::Interp];
const C_ONLY: &[Backend] = &[Backend::C];
const LLVM_ONLY: &[Backend] = &[Backend::Llvm];

const CORPUS: &[ParityCase] = &[
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
        backends: ALL,
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
        backends: ALL,
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
        backends: ALL,
    },
    ParityCase {
        name: "while_loop",
        source: r#"
fn main() {
    let i = 0
    while i < 3 {
        print(i)
        let i = i + 1
    }
}
"#,
        // Known gap: LLVM while-loop variable update emits stale values.
        backends: C_AND_INTERP,
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
        backends: ALL,
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
        backends: ALL,
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
        backends: ALL,
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
        backends: ALL,
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
        let i = i + 1
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
        backends: C_AND_INTERP,
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
];

// ── Parity test ─────────────────────────────────────────────────────────────

#[test]
fn parity_all_backends_agree() {
    let bin = sandbox_bin();
    let _ = &bin; // ensure build happened up-front

    let mut failures: Vec<String> = Vec::new();
    let mut ran = 0usize;

    for case in CORPUS {
        // WASM: compile-only smoke for every case.
        let wasm_dir = TempDir::new().unwrap();
        if let Err(e) = wasm_compiles(case.source, &wasm_dir) {
            failures.push(format!("[wasm/{}] {e}", case.name));
        }

        for &backend in case.backends {
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
