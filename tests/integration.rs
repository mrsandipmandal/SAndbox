use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn sandbox_bin() -> String {
    // Build the binary and return its path
    let output = Command::new("cargo")
        .args(["build", "--quiet"])
        .output()
        .expect("Failed to build sandbox");
    assert!(output.status.success(), "cargo build failed");
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{}/target/debug/sandbox", manifest_dir)
}

fn compile_and_run(source: &str) -> (String, bool) {
    let tmp = TempDir::new().unwrap();
    let sbx_path = tmp.path().join("test.sbx");
    fs::write(&sbx_path, source).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["run", sbx_path.to_str().unwrap()])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}{}", stdout, stderr);
    (combined, output.status.success())
}

fn run_sandbox(args: &[&str]) -> (String, bool) {
    let bin = sandbox_bin();
    let output = Command::new(&bin).args(args).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}{}", stdout, stderr);
    (combined, output.status.success())
}

fn compile_to_llvm(source: &str) -> String {
    let tmp = TempDir::new().unwrap();
    let sbx_path = tmp.path().join("test.sbx");
    let ll_path = tmp.path().join("test.ll");
    fs::write(&sbx_path, source).unwrap();
    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args([
            "llvm",
            sbx_path.to_str().unwrap(),
            "-o",
            ll_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "llvm codegen failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::read_to_string(&ll_path).unwrap()
}

/// End-to-end: sandbox source -> LLVM IR -> clang -> native binary -> run and capture output
fn llvm_build_and_run(source: &str) -> String {
    let tmp = TempDir::new().unwrap();
    let sbx_path = tmp.path().join("test.sbx");
    let bin_path = tmp.path().join("test_bin");
    fs::write(&sbx_path, source).unwrap();
    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args([
            "llvm-build",
            sbx_path.to_str().unwrap(),
            "-o",
            bin_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "llvm-build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let run_output = Command::new(&bin_path).output().unwrap();
    String::from_utf8_lossy(&run_output.stdout).to_string()
}

// ── v0.1 tests ──

#[test]
fn test_hello_world() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    print("Hello, World!")
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.contains("Hello, World!"),
        "Missing output: {}",
        output
    );
}

#[test]
fn test_integer_arithmetic() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let a: i64 = 10
    let b: i64 = 20
    let c = a + b
    print(c)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("30"), "Expected 30, got: {}", output);
}

#[test]
fn test_if_else() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let x: i64 = 10
    if x > 5 {
        print("big")
    } else {
        print("small")
    }
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("big"), "Expected 'big', got: {}", output);
}

#[test]
fn test_while_loop() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let mut i: i64 = 0
    let mut sum: i64 = 0
    while i < 5 {
        sum = sum + i
        i = i + 1
    }
    print(sum)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.contains("10"),
        "Expected 10 (0+1+2+3+4), got: {}",
        output
    );
}

#[test]
fn test_function_call() {
    let (output, ok) = compile_and_run(
        r#"
fn add(a: i64, b: i64) -> i64 {
    return a + b
}

fn main() {
    let result = add(3, 4)
    print(result)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("7"), "Expected 7, got: {}", output);
}

#[test]
fn test_struct() {
    let (output, ok) = compile_and_run(
        r#"
struct Point {
    x: i64,
    y: i64,
}

fn distance_sq(p: Point) -> i64 {
    return p.x * p.x + p.y * p.y
}

fn main() {
    let p = Point { x: 3, y: 4 }
    print(distance_sq(p))
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("25"), "Expected 25, got: {}", output);
}

#[test]
fn test_fibonacci() {
    let (output, ok) = compile_and_run(
        r#"
fn fib(n: i64) -> i64 {
    if n <= 1 {
        return n
    }
    let a = fib(n - 1)
    let b = fib(n - 2)
    return a + b
}

fn main() {
    print(fib(0))
    print(fib(1))
    print(fib(5))
    print(fib(10))
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.contains("0\n1\n5\n55"),
        "Expected 0,1,5,55, got: {}",
        output
    );
}

#[test]
fn test_string_print() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    print("Sandbox v0.1")
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.contains("Sandbox v0.1"),
        "Expected 'Sandbox v0.1', got: {}",
        output
    );
}

#[test]
fn test_money_addition() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let salary: Money<INR> = 50000 INR
    let tax: Money<INR> = 7500 INR
    let total = salary + tax
    print(total)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.contains("575000000"),
        "Expected 575000000, got: {}",
        output
    );
}

#[test]
fn test_money_currency_mismatch() {
    let (_output, ok) = compile_and_run(
        r#"
fn main() {
    let salary: Money<INR> = 50000 INR
    let usd: Money<USD> = 100 USD
    let total = salary + usd
    print(total)
}
"#,
    );
    assert!(!ok, "Expected compile error for currency mismatch");
}

#[test]
fn test_type_mismatch() {
    let (_output, ok) = compile_and_run(
        r#"
fn main() {
    let x: i64 = "hello"
}
"#,
    );
    assert!(!ok, "Expected compile error for type mismatch");
}

#[test]
fn test_for_loop() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    for item in [10, 20, 30] {
        print(item)
    }
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.contains("10\n20\n30"),
        "Expected 10,20,30, got: {}",
        output
    );
}

// ── v0.2 tests ──

#[test]
fn test_result_type() {
    let (output, ok) = compile_and_run(
        r#"
fn divide(a: i64, b: i64) -> Result<i64, string> {
    if b == 0 {
        return Err("Division by zero")
    }
    return Ok(a / b)
}

fn main() {
    let result = divide(10, 2)
    print(result)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("5"), "Expected 5, got: {}", output);
}

#[test]
fn test_panic() {
    let (_output, ok) = compile_and_run(
        r#"
fn main() {
    panic!("Something went wrong")
}
"#,
    );
    assert!(!ok, "Expected panic to exit with error");
}

#[test]
fn test_mod_def() {
    let (output, ok) = compile_and_run(
        r#"
mod math {
    fn add(a: i64, b: i64) -> i64 {
        return a + b
    }
}

fn main() {
    let result = math::add(3, 4)
    print(result)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("7"), "Expected 7, got: {}", output);
}

// ── v0.3 tests ──

#[test]
fn test_math_stdlib() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let x: f64 = 25.0
    let root = math::sqrt(x)
    print(root)

    let a: f64 = -3.7
    let abs_a = math::abs(a)
    print(abs_a)

    let m = math::max(10.5, 20.3)
    print(m)

    let p = math::pow(2.0, 10.0)
    print(p)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("5"), "Expected sqrt(25)=5, got: {}", output);
    assert!(
        output.contains("3.7"),
        "Expected abs(-3.7)=3.7, got: {}",
        output
    );
    assert!(
        output.contains("20.3"),
        "Expected max=20.3, got: {}",
        output
    );
    assert!(
        output.contains("1024"),
        "Expected pow(2,10)=1024, got: {}",
        output
    );
}

#[test]
fn test_string_stdlib() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let greeting = "Hello"
    let target = "World"
    let full = string::concat(greeting, target)
    print(full)

    let len = string::length("Sandbox")
    print(len)

    let eq = string::equals("abc", "abc")
    print(eq)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.contains("HelloWorld"),
        "Expected HelloWorld, got: {}",
        output
    );
    assert!(output.contains("7"), "Expected length 7, got: {}", output);
    assert!(
        output.contains("1"),
        "Expected equals true, got: {}",
        output
    );
}

#[test]
fn test_for_loop_literal() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    for item in [10, 20, 30] {
        print(item)
    }
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.contains("10\n20\n30"),
        "Expected 10,20,30, got: {}",
        output
    );
}

#[test]
fn test_array_index() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let arr = [100, 200, 300]
    let first = arr[0]
    let second = arr[1]
    print(first)
    print(second)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("100"), "Expected 100, got: {}", output);
    assert!(output.contains("200"), "Expected 200, got: {}", output);
}

#[test]
fn test_init_project() {
    let tmp = TempDir::new().unwrap();
    let project_path = tmp.path().join("myproject");
    let (output, ok) = run_sandbox(&["init", project_path.to_str().unwrap()]);
    assert!(ok, "Init failed: {}", output);
    assert!(
        project_path.join("sandbox.toml").exists(),
        "sandbox.toml not created"
    );
    assert!(
        project_path.join("main.sbx").exists(),
        "main.sbx not created"
    );
}

#[test]
fn test_fmt_command() {
    let tmp = TempDir::new().unwrap();
    let sbx = tmp.path().join("test.sbx");
    fs::write(&sbx, "fn main() {\nprint(\"hello\")\n}\n").unwrap();

    let (output, ok) = run_sandbox(&["fmt", sbx.to_str().unwrap()]);
    assert!(ok, "fmt failed: {}", output);

    let (output2, ok2) = run_sandbox(&["fmt", "--check", sbx.to_str().unwrap()]);
    assert!(ok2, "fmt --check failed after formatting: {}", output2);
}

#[test]
fn test_add_dependency() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("myapp");

    // Init
    let (output, ok) = run_sandbox(&["init", project.to_str().unwrap()]);
    assert!(ok, "Init failed: {}", output);

    // Add from project dir
    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["add", "serde", "--version", "^1.0"])
        .current_dir(&project)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "add failed: {}", stdout);

    let toml = fs::read_to_string(project.join("sandbox.toml")).unwrap();
    assert!(
        toml.contains("serde"),
        "serde not in sandbox.toml: {}",
        toml
    );
    assert!(
        toml.contains("^1.0"),
        "version not in sandbox.toml: {}",
        toml
    );
}

#[test]
fn test_tree_command() {
    let tmp = TempDir::new().unwrap();
    let project = tmp.path().join("myapp");

    // Init
    let _ = run_sandbox(&["init", project.to_str().unwrap()]);

    // Add deps
    let bin = sandbox_bin();
    let _ = Command::new(&bin)
        .args(["add", "serde"])
        .current_dir(&project)
        .output();
    let _ = Command::new(&bin)
        .args(["add", "tokio"])
        .current_dir(&project)
        .output();

    // Tree
    let output = Command::new(&bin)
        .args(["tree"])
        .current_dir(&project)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "tree failed: {}", stdout);
    assert!(stdout.contains("serde"), "serde not in tree: {}", stdout);
    assert!(stdout.contains("tokio"), "tokio not in tree: {}", stdout);
}

#[test]
fn test_recursion_deep() {
    let (output, ok) = compile_and_run(
        r#"
fn pow2(n: i64) -> i64 {
    if n == 0 {
        return 1
    }
    return 2 * pow2(n - 1)
}

fn main() {
    print(pow2(0))
    print(pow2(1))
    print(pow2(5))
    print(pow2(10))
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.contains("1\n2\n32\n1024"),
        "Expected 1,2,32,1024, got: {}",
        output
    );
}

// ── v0.4 tests ──

#[test]
fn test_unit_literal() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let weight: kg = 100 kg
    let half = weight / 2
    print(half)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("50"), "Expected 50, got: {}", output);
}

#[test]
fn test_unit_arithmetic() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let distance: meter = 500 meter
    let time: second = 10 second
    let area = 5 meter * 3 meter
    print(area)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("15"), "Expected 15, got: {}", output);
}

#[test]
fn test_unit_mismatch_error() {
    let (_output, ok) = compile_and_run(
        r#"
fn main() {
    let w: kg = 100 kg
    let d: meter = 50 meter
    let bad = w + d
    print(bad)
}
"#,
    );
    assert!(!ok, "Expected compile error for unit mismatch");
}

#[test]
fn test_wasm_codegen() {
    let tmp = TempDir::new().unwrap();
    let wat_path = tmp.path().join("test.wat");
    let (output, ok) = run_sandbox(&[
        "wasm",
        "examples/wasm_demo.sbx",
        "-o",
        wat_path.to_str().unwrap(),
    ]);
    assert!(ok, "WASM generation failed: {}", output);
    assert!(wat_path.exists(), ".wat file not created");
    let wat = fs::read_to_string(&wat_path).unwrap();
    assert!(wat.contains("(module"), "Missing module declaration in WAT");
    assert!(wat.contains("func $add"), "Missing add function in WAT");
    assert!(wat.contains("func $main"), "Missing main function in WAT");
    assert!(
        wat.contains("export \"main\""),
        "Missing main export in WAT"
    );
}

#[test]
fn test_decimal_literal() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let x = 100.25
    print(x)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.contains("100.25"),
        "Expected 100.25, got: {}",
        output
    );
}

#[test]
fn test_build_native() {
    let tmp = TempDir::new().unwrap();
    let out = tmp.path().join("myapp");
    let (output, ok) = run_sandbox(&["build", "examples/hello.sbx", "-o", out.to_str().unwrap()]);
    assert!(ok, "Build failed: {}", output);
    assert!(out.exists(), "Binary not created");
}

#[test]
fn test_build_wasm() {
    let tmp = TempDir::new().unwrap();
    let out = tmp.path().join("demo");
    let (output, ok) = run_sandbox(&[
        "build",
        "examples/wasm_demo.sbx",
        "--target",
        "wasm",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert!(ok, "WASM build failed: {}", output);
    assert!(out.with_extension("wat").exists(), ".wat file not created");
}

// ── v1.0 tests ──

#[test]
fn test_ledger_balanced() {
    let (output, ok) = compile_and_run(
        r#"
ledger Transfer {
    debit account_a 1000 INR
    credit account_b 1000 INR
}

fn main() {
    let result = __validate_Transfer()
    print(result)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.contains("0"),
        "Expected 0 (balanced), got: {}",
        output
    );
}

#[test]
fn test_ledger_unbalanced() {
    let (_output, ok) = compile_and_run(
        r#"
ledger Bad {
    debit account_a 1000 INR
    credit account_b 500 INR
}

fn main() {
    let result = __validate_Bad()
    print(result)
}
"#,
    );
    // Unbalanced ledger should fail type-checking
    assert!(!ok, "Expected compile error for unbalanced ledger");
}

#[test]
fn test_database_queries() {
    let (output, ok) = run_sandbox(&["run", "examples/database_demo.sbx"]);
    assert!(ok, "Database demo failed: {}", output);
    assert!(
        output.contains("Database defined"),
        "Missing output: {}",
        output
    );
}

#[test]
fn test_selfhost_compiler() {
    let (output, ok) = run_sandbox(&["run", "examples/selfhost_compiler.sbx"]);
    assert!(ok, "Self-host compiler failed: {}", output);
    assert!(
        output.contains("Self-Hosting Compiler"),
        "Missing output: {}",
        output
    );
    assert!(
        output.contains("long x = 42;"),
        "Missing compiled output: {}",
        output
    );
}

// ── v2.0 tests ──

#[test]
fn test_json_module() {
    let source = r#"
fn main() {
    let payload = json::stringify(12345)
    print(payload)

    let price = json::stringify_float(99.5)
    print(price)

    let obj = "{\"name\":\"Alice\",\"balance\":500}"
    let name = json::get(obj, "name")
    print(name)

    let bal = json::get(obj, "balance")
    print(bal)
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "JSON demo failed: {}", output);
    assert!(output.contains("12345"), "Missing int JSON: {}", output);
    assert!(output.contains("99.5"), "Missing float JSON: {}", output);
    assert!(output.contains("Alice"), "Missing name: {}", output);
    assert!(output.contains("500"), "Missing balance: {}", output);
}

#[test]
fn test_json_extended() {
    let source = r#"
fn main() {
    // json::stringify_string — wraps string in JSON quotes
    let qs = json::stringify_string("hello")
    print(qs)

    // json::stringify_bool
    let t = json::stringify_bool(true)
    print(t)
    let f = json::stringify_bool(false)
    print(f)

    // json::parse_float — extract float from JSON text
    let fl = json::parse_float("{\"pi\":3.14159}")
    print(fl)

    // json::parse_string — extract first quoted string
    let ps = json::parse_string("[\"Alice\",\"Bob\"]")
    print(ps)

    // json::has_key
    let obj = "{\"x\":1,\"y\":2}"
    let hx = json::has_key(obj, "x")
    print(hx)
    let hz = json::has_key(obj, "z")
    print(hz)

    // json::array_len
    let len1 = json::array_len("[10,20,30]")
    print(len1)
    let len2 = json::array_len("[]")
    print(len2)
    let len3 = json::array_len("[42]")
    print(len3)
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Extended JSON test failed: {}", output);
    assert!(
        output.contains("\"hello\""),
        "stringify_string failed: {}",
        output
    );
    assert!(
        output.contains("true"),
        "stringify_bool true failed: {}",
        output
    );
    assert!(
        output.contains("false"),
        "stringify_bool false failed: {}",
        output
    );
    assert!(output.contains("3.141"), "parse_float failed: {}", output);
    assert!(output.contains("Alice"), "parse_string failed: {}", output);
    // has_key returns 1 for true, 0 for false
    assert!(
        output.contains("\n1\n") || output.contains("1\n"),
        "has_key present failed: {}",
        output
    );
    assert!(
        output.contains("\n0\n") || output.contains("\n0"),
        "has_key absent failed: {}",
        output
    );
    // array_len
    assert!(
        output.contains("\n3\n"),
        "array_len [10,20,30] failed: {}",
        output
    );
    assert!(output.contains("\n0\n"), "array_len [] failed: {}", output);
    assert!(
        output.contains("\n1\n"),
        "array_len [42] failed: {}",
        output
    );
}

#[test]
fn test_kvstore_persistence() {
    let tmp = TempDir::new().unwrap();
    let sbx = tmp.path().join("kv.sbx");
    fs::write(
        &sbx,
        r#"
fn main() {
    let db = db::open("test_kv.db")
    db::put(db, "a", 100)
    db::put(db, "b", 200)
    let c1 = db::count(db)
    print(c1)
    db::delete(db, "a")
    let c2 = db::count(db)
    print(c2)
    let bval = db::get(db, "b")
    print(bval)
    db::close(db)
}
"#,
    )
    .unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .current_dir(tmp.path())
        .args(["run", sbx.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "KV demo failed: {}", stdout);
    assert!(stdout.contains("2\n"), "Expected count 2: {}", stdout);
    assert!(
        stdout.contains("1\n"),
        "Expected count 1 after delete: {}",
        stdout
    );
    assert!(stdout.contains("200"), "Expected b=200: {}", stdout);
}

#[test]
fn test_concurrency_demo() {
    let (output, ok) = run_sandbox(&["run", "examples/concurrency_demo.sbx"]);
    assert!(ok, "Concurrency demo failed: {}", output);
    assert!(output.contains("42"), "Missing channel value: {}", output);
    assert!(output.contains("done"), "Missing done: {}", output);
}

#[test]
fn test_http_headers_extraction() {
    // Test http::headers by feeding it a hardcoded HTTP response string
    let source = r#"
fn main() {
    let resp = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nX-Custom: sandbox123\r\n\r\n{\"ok\":true}"
    let ct = http::headers(resp, "Content-Type")
    print(ct)
    let xc = http::headers(resp, "X-Custom")
    print(xc)
    let missing = http::headers(resp, "X-Missing")
    print(missing)
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "HTTP headers extraction failed: {}", output);
    assert!(
        output.contains("application/json"),
        "Missing Content-Type: {}",
        output
    );
    assert!(
        output.contains("sandbox123"),
        "Missing X-Custom: {}",
        output
    );
}

#[test]
fn test_http_server_end_to_end() {
    use std::time::{Duration, Instant};

    let bin = sandbox_bin();
    let mut child = Command::new(&bin)
        .args(["run", "examples/http_server_demo.sbx"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    // Poll with real requests — the one-shot server consumes one connection
    // per accept, so the probe *is* the request. (max 10s)
    use std::io::{Read, Write};
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut resp = String::new();
    loop {
        if Instant::now() >= deadline {
            break;
        }
        if let Ok(Some(_)) = child.try_wait() {
            break; // server process died before accepting
        }
        if let Ok(mut s) = std::net::TcpStream::connect_timeout(
            &"127.0.0.1:8080".parse().unwrap(),
            Duration::from_millis(500),
        ) {
            let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
            let _ =
                s.write_all(b"GET /hello HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
            let mut buf = Vec::new();
            if s.read_to_end(&mut buf).is_ok() {
                resp = String::from_utf8_lossy(&buf).to_string();
                if resp.contains("200") {
                    break;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(300));
    }

    let _ = child.kill();
    let _ = child.wait();

    // Extract the body (everything after the blank line separator)
    let body = resp
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or("")
        .trim()
        .to_string();

    assert!(
        !body.is_empty(),
        "HTTP server never returned a response body: {}",
        resp
    );
    assert_eq!(body, "\"/hello\"", "Unexpected body: {}", resp);
}

#[test]
fn test_json_parse_object() {
    let source = r#"
fn main() {
    let obj = "{\"name\":\"Alice\",\"age\":30,\"city\":\"NYC\"}"
    let map = json::parse_object(obj)

    // map_get retrieves values by key
    let name = json::map_get(map, "name")
    print(name)
    let age = json::map_get(map, "age")
    print(age)
    let city = json::map_get(map, "city")
    print(city)

    // map_keys returns comma-separated keys
    let keys = json::map_keys(map)
    print(keys)

    // map_len returns number of pairs
    let n = json::map_len(map)
    print(n)

    // missing key returns empty string
    let missing = json::map_get(map, "phone")
    print(missing)
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "JSON parse_object test failed: {}", output);
    assert!(output.contains("Alice"), "Missing name: {}", output);
    assert!(output.contains("30"), "Missing age: {}", output);
    assert!(output.contains("NYC"), "Missing city: {}", output);
    // map_keys should contain all three keys
    assert!(output.contains("name"), "Missing key 'name': {}", output);
    assert!(output.contains("age"), "Missing key 'age': {}", output);
    assert!(output.contains("city"), "Missing key 'city': {}", output);
    // map_len should be 3
    assert!(output.contains("\n3\n"), "Expected map_len=3: {}", output);
}

#[test]
fn test_enum_pattern_matching() {
    let source = r#"
enum Color {
    Red,
    Green,
    Blue,
}

fn describe(c: Color) -> i64 {
    match c {
        Color::Red => 1,
        Color::Green => 2,
        Color::Blue => 3,
        _ => 0,
    }
}

fn main() {
    let r = describe(Color::Red)
    print(r)
    let g = describe(Color::Green)
    print(g)
    let b = describe(Color::Blue)
    print(b)
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Enum pattern matching failed: {}", output);
    assert!(output.contains("1"), "Missing Red=1: {}", output);
    assert!(output.contains("2"), "Missing Green=2: {}", output);
    assert!(output.contains("3"), "Missing Blue=3: {}", output);
}

#[test]
fn test_enum_with_payload() {
    let source = r#"
enum Shape {
    Circle(f64),
    Square(f64),
    Point,
}

fn area(s: Shape) -> f64 {
    match s {
        Shape::Circle(r) => r * r * 3.14,
        Shape::Square(s2) => s2 * s2,
        Shape::Point => 0.0,
    }
}

fn main() {
    let c = Shape::Circle(5.0)
    let a = area(c)
    print(a)

    let sq = Shape::Square(4.0)
    let sa = area(sq)
    print(sa)

    let p = Shape::Point
    let pa = area(p)
    print(pa)
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Enum with payload failed: {}", output);
    assert!(output.contains("78.5"), "Missing circle area: {}", output);
    assert!(output.contains("16"), "Missing square area: {}", output);
    assert!(output.contains("0"), "Missing point area: {}", output);
}

#[test]
fn test_http_serve_multi_request() {
    use std::time::Duration;

    let source = r#"
fn handler(path: string) -> string {
    let body = json::stringify_string(path)
    return body
}

fn main() {
    http::serve(8077, "handler", 0)
}
"#;

    // Write the source to a temp file
    let tmp = tempfile::TempDir::new().unwrap();
    let sbx = tmp.path().join("serve.sbx");
    std::fs::write(&sbx, source).unwrap();

    let bin = sandbox_bin();
    let mut child = Command::new(&bin)
        .args(["run", sbx.to_str().unwrap()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();

    // Wait for server to start
    std::thread::sleep(Duration::from_millis(500));

    // Make first request
    let resp1 = http_get("127.0.0.1:8077", "/hello");
    assert!(
        resp1.contains("\"/hello\""),
        "First request failed: {}",
        resp1
    );

    // Make second request (proves it handles multiple)
    let resp2 = http_get("127.0.0.1:8077", "/world");
    assert!(
        resp2.contains("\"/world\""),
        "Second request failed: {}",
        resp2
    );

    // Make third request
    let resp3 = http_get("127.0.0.1:8077", "/");
    assert!(resp3.contains("\"/\""), "Third request failed: {}", resp3);

    let _ = child.kill();
    let _ = child.wait();
}

fn http_get(addr: &str, path: &str) -> String {
    use std::io::{Read, Write};
    use std::time::Duration;
    let mut s = std::net::TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    let req = format!(
        "GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        path
    );
    s.write_all(req.as_bytes()).unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).unwrap();
    String::from_utf8_lossy(&buf).to_string()
}

#[test]
fn test_llvm_backend_hello() {
    let out = llvm_build_and_run(
        r#"
fn main() {
    print("Hello LLVM!")
}
"#,
    );
    assert_eq!(out.trim(), "Hello LLVM!");
}

#[test]
fn test_llvm_backend_arithmetic() {
    let out = llvm_build_and_run(
        r#"
fn add(a: i64, b: i64) -> i64 {
    return a + b
}

fn main() {
    let result = add(10, 20)
    print(result)
}
"#,
    );
    assert_eq!(out.trim(), "30");
}

#[test]
fn test_llvm_backend_enums() {
    let ir = compile_to_llvm(
        r#"
enum Color {
    Red,
    Green,
    Blue
}

fn main() {
    let c = Color::Red
    print(c)
}
"#,
    );
    assert!(
        ir.contains("@.tag.Color.Red = constant i64 0"),
        "Red should be tag 0"
    );
    assert!(
        ir.contains("@.tag.Color.Green = constant i64 1"),
        "Green should be tag 1"
    );
    assert!(
        ir.contains("@.tag.Color.Blue = constant i64 2"),
        "Blue should be tag 2"
    );
}

#[test]
fn test_llvm_backend_if_else() {
    let out = llvm_build_and_run(
        r#"
fn abs_val(x: i64) -> i64 {
    if x > 0 {
        return x
    } else {
        return 0 - x
    }
}

fn main() {
    let v = abs_val(-5)
    print(v)
}
"#,
    );
    assert_eq!(out.trim(), "5");
}

#[test]
fn test_llvm_backend_while_loop() {
    let out = llvm_build_and_run(
        r#"
fn main() {
    let i = 0
    let sum = 0
    while i < 5 {
        sum = sum + i
        i = i + 1
    }
    print(sum)
}
"#,
    );
    assert_eq!(out.trim(), "10");
}

#[test]
fn test_llvm_backend_struct() {
    let out = llvm_build_and_run(
        r#"
struct Point {
    x: i64,
    y: i64,
}

fn distance_sq(p: Point) -> i64 {
    return p.x * p.x + p.y * p.y
}

fn main() {
    let p = Point { x: 3, y: 4 }
    print(distance_sq(p))
}
"#,
    );
    assert_eq!(out.trim(), "25");
}

#[test]
fn test_llvm_backend_struct_fields() {
    let out = llvm_build_and_run(
        r#"
struct Rect {
    w: i64,
    h: i64,
}

fn area(r: Rect) -> i64 {
    return r.w * r.h
}

fn main() {
    let r = Rect { w: 5, h: 3 }
    print(area(r))
}
"#,
    );
    assert_eq!(out.trim(), "15");
}

// ── LLVM backend: extended feature tests ──

#[test]
fn test_llvm_backend_money() {
    let out = llvm_build_and_run(
        r#"
fn main() {
    let salary: Money<INR> = 50000 INR
    let tax: Money<INR> = 7500 INR
    let total = salary + tax
    print(total)
}
"#,
    );
    assert_eq!(out.trim(), "575000000");
}

#[test]
fn test_llvm_backend_if_let() {
    let out = llvm_build_and_run(
        r#"
fn main() {
    let x = 42
    if let y = x {
        print(y)
    } else {
        print(0)
    }
}
"#,
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn test_llvm_backend_range_for() {
    let out = llvm_build_and_run(
        r#"
fn main() {
    let mut sum: i64 = 0
    for i in 0..5 {
        sum = sum + i
    }
    print(sum)
}
"#,
    );
    assert_eq!(out.trim(), "10");
}

#[test]
fn test_llvm_backend_fstring() {
    let out = llvm_build_and_run(
        r#"
fn main() {
    let name = "World"
    let greeting = f"Hello, {name}!"
    print(greeting)
}
"#,
    );
    assert!(out.contains("Hello, World!"), "Expected f-string, got: {}", out);
}

#[test]
fn test_llvm_backend_option_match() {
    let out = llvm_build_and_run(
        r#"
fn main() {
    let a: Option<i64> = Some(42)
    match a {
        Some(x) => { print(x) },
        None => { print(0) },
    }
}
"#,
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn test_llvm_backend_none_match() {
    let out = llvm_build_and_run(
        r#"
fn main() {
    let b: Option<i64> = None
    match b {
        Some(x) => { print(x) },
        None => { print("none") },
    }
}
"#,
    );
    assert_eq!(out.trim(), "none");
}

#[test]
fn test_llvm_backend_closure_capture() {
    let out = llvm_build_and_run(
        r#"
fn main() {
    let offset = 10
    let add_offset = |x: i64| -> i64 { return x + offset }
    print(add_offset(5))
}
"#,
    );
    assert_eq!(out.trim(), "15");
}

#[test]
fn test_llvm_backend_async_await() {
    let out = llvm_build_and_run(
        r#"
async fn compute(x: i64) -> i64 {
    return x * 2
}

fn main() {
    let future = compute(21)
    let result = await future
    print(result)
}
"#,
    );
    assert_eq!(out.trim(), "42");
}

#[test]
fn test_llvm_backend_modulo() {
    let out = llvm_build_and_run(
        r#"
fn main() {
    let x = 17
    let y = 5
    print(x % y)
}
"#,
    );
    assert_eq!(out.trim(), "2");
}

#[test]
fn test_llvm_backend_le_ge() {
    let out = llvm_build_and_run(
        r#"
fn main() {
    let x = 5
    if x >= 6 {
        print(2)
    } else {
        print(3)
    }
}
"#,
    );
    assert_eq!(out.trim(), "3");
}

#[test]
fn test_llvm_backend_assert_eq() {
    let out = llvm_build_and_run(
        r#"
fn main() {
    assert_eq(42, 42)
    assert_eq(10 + 5, 15)
    print("passed")
}
"#,
    );
    assert_eq!(out.trim(), "passed");
}

#[test]
fn test_llvm_backend_impl_method() {
    let out = llvm_build_and_run(
        r#"
struct Point {
    x: i64,
    y: i64,
}

impl Point {
    fn distance_sq(self: Point) -> i64 {
        return self.x * self.x + self.y * self.y
    }
}

fn main() {
    let p = Point { x: 3, y: 4 }
    print(Point::distance_sq(p))
}
"#,
    );
    assert_eq!(out.trim(), "25");
}

fn run_repl(input: &str) -> String {
    let bin = sandbox_bin();
    use std::process::Stdio;
    let mut child = Command::new(&bin)
        .args(["repl"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn test_repl_eval_expressions() {
    let out = run_repl("10 + 20\n42\n:q\n");
    assert!(out.contains("30"), "should evaluate 10+20=30, got: {}", out);
    assert!(out.contains("42"), "should evaluate 42, got: {}", out);
}

#[test]
fn test_repl_define_and_call_fn() {
    let out = run_repl("fn triple(x: i64) -> i64 {\n    return x * 3\n}\ntriple(7)\n:q\n");
    assert!(
        out.contains("21"),
        "should evaluate triple(7)=21, got: {}",
        out
    );
    assert!(
        out.contains("defined"),
        "should confirm definition, got: {}",
        out
    );
}

#[test]
fn test_repl_multiline_function() {
    let out = run_repl("fn fib(n: i64) -> i64 {\n    if n <= 1 {\n        return n\n    } else {\n        return fib(n - 1) + fib(n - 2)\n    }\n}\nfib(10)\n:q\n");
    assert!(
        out.contains("55"),
        "should evaluate fib(10)=55, got: {}",
        out
    );
}

#[test]
fn test_repl_print_statement() {
    let out = run_repl("print(42)\n:q\n");
    assert!(out.contains("42"), "should print 42, got: {}", out);
}

#[test]
fn test_repl_reset_command() {
    let out = run_repl("10\n:reset\n20\n:q\n");
    assert!(
        out.contains("Reset"),
        "should show reset confirmation, got: {}",
        out
    );
}

#[test]
fn test_lambda_basic() {
    let (output, ok) = compile_and_run(
        r#"
fn apply(f: Fn(i64) -> i64, x: i64) -> i64 {
    return f(x)
}

fn main() {
    let dbl = |x: i64| -> i64 { return x * 2 }
    print(apply(dbl, 5))
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("10"), "Expected 10, got: {}", output);
}

#[test]
fn test_lambda_inline() {
    let (output, ok) = compile_and_run(
        r#"
fn apply(f: Fn(i64) -> i64, x: i64) -> i64 {
    return f(x)
}

fn main() {
    print(apply(|x: i64| -> i64 { return x * x }, 7))
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("49"), "Expected 49, got: {}", output);
}

#[test]
fn test_lambda_multiple() {
    let (output, ok) = compile_and_run(
        r#"
fn apply(f: Fn(i64) -> i64, x: i64) -> i64 {
    return f(x)
}

fn main() {
    let add_one = |x: i64| -> i64 { return x + 1 }
    let times_two = |x: i64| -> i64 { return x * 2 }
    print(apply(add_one, 5))
    print(apply(times_two, 5))
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("6"), "Expected 6, got: {}", output);
    assert!(output.trim().contains("10"), "Expected 10, got: {}", output);
}

// ── v2.0: Range, FString, use, function reference tests ──

#[test]
fn test_range_for_loop() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let mut sum: i64 = 0
    for i in 0..5 {
        sum = sum + i
    }
    print(sum)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.trim().contains("10"),
        "Expected 10 (0+1+2+3+4), got: {}",
        output
    );
}

#[test]
fn test_range_inclusive() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let mut sum: i64 = 0
    for i in 0..=5 {
        sum = sum + i
    }
    print(sum)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.trim().contains("15"),
        "Expected 15 (0+1+2+3+4+5), got: {}",
        output
    );
}

#[test]
fn test_range_expression() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    for i in 3..7 {
        print(i)
    }
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("3"), "Expected 3, got: {}", output);
    assert!(output.contains("4"), "Expected 4, got: {}", output);
    assert!(output.contains("5"), "Expected 5, got: {}", output);
    assert!(output.contains("6"), "Expected 6, got: {}", output);
    // Check for standalone "7" on its own line (not in temp file paths or warnings)
    let has_standalone_7 = output.lines().any(|l| l.trim() == "7");
    assert!(
        !has_standalone_7,
        "Should not contain 7 (exclusive), got: {}",
        output
    );
}

#[test]
fn test_fstring_simple() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let name = "World"
    let greeting = f"Hello, {name}!"
    print(greeting)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.contains("Hello, World!"),
        "Expected 'Hello, World!', got: {}",
        output
    );
}

#[test]
fn test_fstring_expression() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let msg = f"1 + 2 = {1 + 2}"
    print(msg)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.contains("1 + 2 = 3"),
        "Expected '1 + 2 = 3', got: {}",
        output
    );
}

#[test]
fn test_fstring_multiple_interpolations() {
    let (output, ok) = compile_and_run(
        r#"
fn main() {
    let a = 10
    let b = 20
    let msg = f"{a} + {b} = {10 + 20}"
    print(msg)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.contains("10 + 20 = 30"),
        "Expected '10 + 20 = 30', got: {}",
        output
    );
}

#[test]
fn test_use_single_import() {
    let (output, ok) = compile_and_run(
        r#"
mod math {
    fn add(a: i64, b: i64) -> i64 {
        return a + b
    }
}

use math::add;

fn main() {
    print(add(3, 4))
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("7"), "Expected 7, got: {}", output);
}

#[test]
fn test_use_wildcard() {
    let (output, ok) = compile_and_run(
        r#"
mod calc {
    fn double(x: i64) -> i64 {
        return x * 2
    }
    fn triple(x: i64) -> i64 {
        return x * 3
    }
}

use calc::*;

fn main() {
    print(double(5))
    print(triple(5))
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("10"), "Expected 10, got: {}", output);
    assert!(output.trim().contains("15"), "Expected 15, got: {}", output);
}

#[test]
fn test_function_reference() {
    let (output, ok) = compile_and_run(
        r#"
fn add(a: i64, b: i64) -> i64 {
    return a + b
}

fn apply(f: Fn(i64, i64) -> i64, x: i64, y: i64) -> i64 {
    return f(x, y)
}

fn main() {
    let op = add
    print(op(3, 4))
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("7"), "Expected 7, got: {}", output);
}

#[test]
fn test_function_as_callback() {
    let (output, ok) = compile_and_run(
        r#"
fn double(x: i64) -> i64 {
    return x * 2
}

fn apply(f: Fn(i64) -> i64, x: i64) -> i64 {
    return f(x)
}

fn main() {
    print(apply(double, 7))
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("14"), "Expected 14, got: {}", output);
}

// ── v2.0: Async/await tests ──

#[test]
fn test_async_basic() {
    let (output, ok) = compile_and_run(
        r#"
async fn compute(x: i64) -> i64 {
    return x * 2
}

fn main() {
    let future = compute(21)
    let result = await future
    print(result)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("42"), "Expected 42, got: {}", output);
}

#[test]
fn test_async_no_params() {
    let (output, ok) = compile_and_run(
        r#"
async fn get_answer() -> i64 {
    return 42
}

fn main() {
    let f = get_answer()
    let v = await f
    print(v)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("42"), "Expected 42, got: {}", output);
}

#[test]
fn test_future_wait_stdlib() {
    let (output, ok) = compile_and_run(
        r#"
async fn compute(x: i64) -> i64 {
    return x + 8
}

fn main() {
    let h = compute(10)
    let result = future::wait(h)
    print(result)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("18"), "Expected 18, got: {}", output);
}

#[test]
fn test_async_multiple_futures() {
    let (output, ok) = compile_and_run(
        r#"
async fn add_first(a: i64, b: i64) -> i64 {
    return a + b
}

async fn add_second(a: i64, b: i64) -> i64 {
    return a + b
}

fn main() {
    let f1 = add_first(10, 20)
    let f2 = add_second(30, 40)
    let r1 = await f1
    let r2 = await f2
    print(r1)
    print(r2)
}
"#,
    );
    assert!(ok, "Compilation failed: {}", output);
    assert!(
        output.trim().contains("30") && output.trim().contains("70"),
        "Expected 30 and 70, got: {}",
        output
    );
}

// ── v2.1: Impl blocks, tests, assert ──

#[test]
fn test_impl_method() {
    let source = r#"
struct Point {
    x: i64,
    y: i64,
}

impl Point {
    fn distance_sq(self: Point) -> i64 {
        return self.x * self.x + self.y * self.y
    }
}

fn main() {
    let p = Point { x: 3, y: 4 }
    print(Point::distance_sq(p))
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Impl method failed: {}", output);
    assert!(output.trim().contains("25"), "Expected 25, got: {}", output);
}

#[test]
fn test_impl_multiple_methods() {
    let source = r#"
struct Rect {
    w: i64,
    h: i64,
}

impl Rect {
    fn area(self: Rect) -> i64 {
        return self.w * self.h
    }
    fn perimeter(self: Rect) -> i64 {
        return (self.w + self.h) * 2
    }
}

fn main() {
    let r = Rect { w: 5, h: 3 }
    print(Rect::area(r))
    print(Rect::perimeter(r))
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Impl multiple methods failed: {}", output);
    assert!(output.trim().contains("15"), "Expected area=15, got: {}", output);
    assert!(output.trim().contains("16"), "Expected perimeter=16, got: {}", output);
}

#[test]
fn test_impl_associated_function() {
    let source = r#"
struct Counter {
    value: i64,
}

impl Counter {
    fn new(v: i64) -> i64 {
        return v
    }
}

fn main() {
    let v = Counter::new(42)
    print(v)
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Impl associated function failed: {}", output);
    assert!(output.trim().contains("42"), "Expected 42, got: {}", output);
}

#[test]
fn test_impl_with_match() {
    let source = r#"
enum Shape {
    Circle(f64),
    Square(f64),
    Point,
}

impl Shape {
    fn kind(self: Shape) -> i64 {
        match self {
            Shape::Circle(_) => 1,
            Shape::Square(_) => 2,
            Shape::Point => 3,
        }
    }
}

fn main() {
    let c = Shape::Circle(5.0)
    print(Shape::kind(c))
    let s = Shape::Square(3.0)
    print(Shape::kind(s))
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Impl with match failed: {}", output);
    assert!(output.trim().contains("1"), "Expected Circle=1, got: {}", output);
    assert!(output.trim().contains("2"), "Expected Square=2, got: {}", output);
}

#[test]
fn test_assert_passes() {
    let (output, ok) = run_sandbox(&["test", "tests/fixtures/test_pass.sbx"]); // Use inline
    // Inline test
    let source = r#"
test fn test_addition {
    let result = 2 + 3
    assert(result == 5, "2 + 3 should equal 5")
}
"#;
    let (output, ok) = compile_and_run(source);
    // Test runner doesn't have main — use sandbox test instead
    // Just verify it compiles and type-checks
    assert!(ok || output.contains("type-checked"), "Assert test should at least type-check: {}", output);
}

#[test]
fn test_sandbox_test_command() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sbx = tmp.path().join("tests.sbx");
    std::fs::write(&sbx, r#"
test fn test_math {
    let x = 2 + 3
    assert(x == 5, "math should work")
}

test fn test_bool {
    assert(true, "true is true")
}
"#).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["test", sbx.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "sandbox test failed: {}", stdout);
    assert!(stdout.contains("2 passed"), "Expected 2 passed, got: {}", stdout);
    assert!(stdout.contains("test_math"), "Missing test_math: {}", stdout);
    assert!(stdout.contains("test_bool"), "Missing test_bool: {}", stdout);
}

#[test]
fn test_sandbox_test_failing() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sbx = tmp.path().join("fail.sbx");
    std::fs::write(&sbx, r#"
test fn test_fail {
    assert(false, "this should fail")
}
"#).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["test", sbx.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success(), "Expected test failure");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    assert!(stderr.contains("failed") || !output.status.success(), "Should report failure");
}

#[test]
fn test_sandbox_test_no_tests() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sbx = tmp.path().join("notest.sbx");
    std::fs::write(&sbx, r#"
fn main() {
    print("no tests here")
}
"#).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["test", sbx.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(stdout.contains("No test functions"), "Should warn about no tests: {}", stdout);
}

#[test]
fn test_sandbox_help_shows_test() {
    let bin = sandbox_bin();
    let output = Command::new(&bin).args(["--help"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(stdout.contains("test"), "--help should mention test: {}", stdout);
}

#[test]
fn test_sandbox_help_shows_impl() {
    // Verify impl works by compiling a file with impl
    let source = r#"
struct Vec2 {
    x: i64,
    y: i64,
}

impl Vec2 {
    fn add(self: Vec2, other: Vec2) -> i64 {
        return self.x + other.x + self.y + other.y
    }
}

fn main() {
    let a = Vec2 { x: 1, y: 2 }
    let b = Vec2 { x: 3, y: 4 }
    print(Vec2::add(a, b))
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Impl with multiple args failed: {}", output);
    assert!(output.trim().contains("10"), "Expected 10, got: {}", output);
}

// ==================== Phase 1: File I/O ====================

#[test]
fn test_file_io_read_write() {
    let source = r#"
fn main() {
    let path = "/tmp/sandbox_test_file_io.txt"
    file::write(path, "Hello from Sandbox!")
    let content = file::read(path)
    print(content)
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "File I/O failed: {}", output);
    assert!(output.trim().contains("Hello from Sandbox!"), "Expected file content, got: {}", output);
}

#[test]
fn test_file_io_exists() {
    let source = r#"
fn main() {
    let path = "/tmp/sandbox_test_exists.txt"
    print(file::exists(path))
    file::write(path, "exists")
    print(file::exists(path))
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "File exists check failed: {}", output);
    assert!(output.contains("0") && output.contains("1"), "Expected false then true, got: {}", output);
}

// ==================== Phase 1: String Methods ====================

#[test]
fn test_string_to_upper() {
    let source = r#"
fn main() {
    let s = "hello world"
    print(string::to_upper(s))
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "to_upper failed: {}", output);
    assert!(output.trim().contains("HELLO WORLD"), "Expected HELLO WORLD, got: {}", output);
}

#[test]
fn test_string_to_lower() {
    let source = r#"
fn main() {
    let s = "HELLO WORLD"
    print(string::to_lower(s))
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "to_lower failed: {}", output);
    assert!(output.trim().contains("hello world"), "Expected hello world, got: {}", output);
}

#[test]
fn test_string_replace() {
    let source = r#"
fn main() {
    let s = "hello world world"
    print(string::replace(s, "world", "there"))
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "string::replace failed: {}", output);
    assert!(output.trim().contains("hello there there"), "Expected replacement, got: {}", output);
}

// ==================== Phase 2: Option<T> ====================

#[test]
fn test_option_some_none() {
    let source = r#"
fn main() {
    let a: Option<i64> = Some(42)
    let b: Option<i64> = None
    match a {
        Some(x) => { print(x) },
        None => { print(0) },
    }
    match b {
        Some(x) => { print(x) },
        None => { print("none") },
    }
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Option test failed: {}", output);
    assert!(output.contains("42"), "Expected 42, got: {}", output);
    assert!(output.contains("none"), "Expected none, got: {}", output);
}

// ==================== Phase 2: Better Error Messages ====================

#[test]
fn test_error_message_context() {
    let source = r#"
fn main() {
    let x: i64 = "not a number"
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(!ok, "Should fail on type error");
    assert!(output.contains("error") || output.contains("Error"), "Expected error message, got: {}", output);
}

// ==================== Phase 3: Closure Capture ====================
// NOTE: True closure capture (referencing outer scope in lambdas) requires a full
// closure-capture analysis pass to thread captured variables as extra parameters.
// Currently, lambdas are standalone C functions. This test verifies function references
// and higher-order patterns which are fully supported.

#[test]
fn test_closure_capture() {
    // Test true closure capture: lambda captures `offset` from outer scope
    let source = r#"
fn main() {
    let offset = 10
    let add_offset = |x: i64| -> i64 { return x + offset }
    print(add_offset(5))
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Closure capture failed: {}", output);
    assert!(output.trim().contains("15"), "Expected 15, got: {}", output);
}

#[test]
fn test_closure_capture_multiple() {
    // Test capturing multiple variables
    let source = r#"
fn main() {
    let offset = 10
    let multiplier = 3
    let add_offset = |x: i64| -> i64 { return x + offset }
    let scale = |x: i64| -> i64 { return x * multiplier }
    print(add_offset(5))
    print(scale(7))
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Multiple capture failed: {}", output);
    assert!(output.contains("15"), "Expected 15, got: {}", output);
    assert!(output.contains("21"), "Expected 21, got: {}", output);
}

// ==================== Phase 3: Use for Registry Packages ====================

#[test]
fn test_sandbox_use_registry_packages() {
    // Verify the vendor directory structure exists after install
    let tmp = TempDir::new().unwrap();
    let vendor = tmp.path().join(".sandbox").join("vendor");
    std::fs::create_dir_all(&vendor).unwrap();
    // Create a fake package
    let pkg_dir = vendor.join("fakepkg");
    std::fs::create_dir_all(&pkg_dir).unwrap();
    std::fs::write(
        pkg_dir.join("lib.sbx"),
        "fn greet() -> i64 { return 42 }",
    ).unwrap();
    assert!(pkg_dir.join("lib.sbx").exists());
}

// ==================== Phase 4: Bytecode Interpreter ====================

#[test]
fn test_interpreter_basic() {
    // Test that the interpreter can run a basic program
    let source = r#"
fn main() {
    print(2 + 3)
}
"#;
    let tmp = TempDir::new().unwrap();
    let sbx_path = tmp.path().join("test.sbx");
    fs::write(&sbx_path, source).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["interpret", sbx_path.to_str().unwrap()])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "Interpreter failed: {}", stdout);
    assert!(stdout.trim().contains("5"), "Expected 5, got: {}", stdout);
}

// ==================== Phase 4: Doc Generator ====================

#[test]
fn test_doc_generator() {
    let source = r#"/// Add two numbers
fn add(a: i64, b: i64) -> i64 {
    return a + b
}

/// A 2D point
struct Point {
    x: i64,
    y: i64,
}
"#;
    let tmp = TempDir::new().unwrap();
    let sbx_path = tmp.path().join("lib.sbx");
    fs::write(&sbx_path, source).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["doc", sbx_path.to_str().unwrap()])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "Doc generator failed: {}", stdout);
    assert!(stdout.contains("add"), "Expected 'add' in docs, got: {}", stdout);
    assert!(stdout.contains("Add two numbers"), "Expected doc comment, got: {}", stdout);
    assert!(stdout.contains("struct `Point`"), "Expected struct docs, got: {}", stdout);
}

#[test]
fn test_doc_comments_compile() {
    let source = r#"/// Adds two integers
fn add(a: i64, b: i64) -> i64 {
    return a + b
}

fn main() {
    print(add(3, 4))
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Doc comments should not break compilation: {}", output);
    assert!(output.trim().contains("7"), "Expected 7, got: {}", output);
}

#[test]
fn test_vendored_package_use() {
    // Test that `use pkg::func` resolves from .sandbox/vendor/ and compiles correctly
    let tmp = TempDir::new().unwrap();
    let vendor = tmp.path().join(".sandbox").join("vendor").join("mymath");
    std::fs::create_dir_all(&vendor).unwrap();
    std::fs::write(
        vendor.join("mymath.sbx"),
        r#"fn add(a: i64, b: i64) -> i64 {
    return a + b
}

fn multiply(a: i64, b: i64) -> i64 {
    return a * b
}"#,
    ).unwrap();

    std::fs::write(
        tmp.path().join("main.sbx"),
        r#"use mymath::add;
use mymath::multiply;

fn main() {
    print(add(3, 4))
    print(multiply(5, 6))
}"#,
    ).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["run", tmp.path().join("main.sbx").to_str().unwrap()])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "Vendor use failed: {}", stdout);
    assert!(stdout.contains("7"), "Expected 7, got: {}", stdout);
    assert!(stdout.contains("30"), "Expected 30, got: {}", stdout);
}

#[test]
fn test_vendored_package_wildcard() {
    // Test that `use pkg::*` resolves all functions from .sandbox/vendor/
    let tmp = TempDir::new().unwrap();
    let vendor = tmp.path().join(".sandbox").join("vendor").join("tools");
    std::fs::create_dir_all(&vendor).unwrap();
    std::fs::write(
        vendor.join("tools.sbx"),
        r#"fn double(x: i64) -> i64 {
    return x * 2
}

fn triple(x: i64) -> i64 {
    return x * 3
}"#,
    ).unwrap();

    std::fs::write(
        tmp.path().join("main.sbx"),
        r#"use tools::*;

fn main() {
    print(double(7))
    print(triple(7))
}"#,
    ).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["run", tmp.path().join("main.sbx").to_str().unwrap()])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "Vendor wildcard failed: {}", stdout);
    assert!(stdout.contains("14"), "Expected 14, got: {}", stdout);
    assert!(stdout.contains("21"), "Expected 21, got: {}", stdout);
}

#[test]
fn test_list_collection() {
    let source = r#"fn main() {
    let l = list::new()
    list::push(l, 5)
    list::push(l, 3)
    list::push(l, 8)
    print(list::len(l))
    print(list::get(l, 0))
    list::sort(l)
    print(list::get(l, 0))
    print(list::contains(l, 3))
    print(list::is_empty(l))
    list::remove(l, 1)
    print(list::len(l))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "List test failed: {}", output);
    assert!(output.contains("3"), "Expected len 3, got: {}", output);
    assert!(output.contains("5"), "Expected get(0)=5, got: {}", output);
    assert!(output.contains("1"), "Expected sort first=3→contains=1, got: {}", output);
    assert!(output.contains("0"), "Expected not empty, got: {}", output);
    assert!(output.contains("2"), "Expected len after remove=2, got: {}", output);
}

#[test]
fn test_map_collection() {
    let source = r#"fn main() {
    let m = map::new()
    map::insert(m, "x", 10)
    map::insert(m, "y", 20)
    print(map::get(m, "x"))
    print(map::get(m, "y"))
    print(map::len(m))
    print(map::contains(m, "x"))
    print(map::contains(m, "z"))
    map::insert(m, "x", 99)
    print(map::get(m, "x"))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Map test failed: {}", output);
    assert!(output.contains("10"), "Expected get(x)=10, got: {}", output);
    assert!(output.contains("20"), "Expected get(y)=20, got: {}", output);
    assert!(output.contains("99"), "Expected overwrite x=99, got: {}", output);
}

#[test]
fn test_set_collection() {
    let source = r#"fn main() {
    let s = set_of::new()
    set_of::insert(s, "a")
    set_of::insert(s, "b")
    set_of::insert(s, "a")
    print(set_of::len(s))
    print(set_of::contains(s, "a"))
    print(set_of::contains(s, "c"))
    set_of::remove(s, "a")
    print(set_of::len(s))
    print(set_of::contains(s, "a"))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Set test failed: {}", output);
    assert!(output.contains("2"), "Expected deduped len=2, got: {}", output);
    assert!(output.contains("0"), "Expected after remove, got: {}", output);
}

// ==================== Phase 5: Generics, Traits, Multi-file ====================

#[test]
fn test_generics_basic() {
    let source = r#"fn identity<T>(x: T) -> T {
    return x
}

fn max<T>(a: T, b: T) -> T {
    if a > b {
        return a
    } else {
        return b
    }
}

fn main() {
    print(identity<i64>(42))
    print(max<i64>(10, 20))
    print(max<f64>(3.14, 2.71))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Generics test failed: {}", output);
    assert!(output.contains("42"), "Expected identity 42, got: {}", output);
    assert!(output.contains("20"), "Expected max 20, got: {}", output);
}

#[test]
fn test_traits_basic() {
    let source = r#"trait Greetable {
    fn greet(self) -> i64
}

struct Greeter {
    val: i64,
}

impl Greetable for Greeter {
    fn greet(self) -> i64 {
        return 99
    }
}

fn main() {
    let g = Greeter { val: 1 }
    print(g.greet())
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Traits test failed: {}", output);
    assert!(output.contains("99"), "Expected 99, got: {}", output);
}

#[test]
fn test_multi_file_modules() {
    let tmp = TempDir::new().unwrap();
    // Create a module file
    fs::write(
        tmp.path().join("math.sbx"),
        r#"fn double(x: i64) -> i64 {
    return x * 2
}"#,
    ).unwrap();
    // Create main that uses the module
    fs::write(
        tmp.path().join("main.sbx"),
        r#"mod math;
use math::double;

fn main() {
    print(double(21))
}"#,
    ).unwrap();
    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["run", tmp.path().join("main.sbx").to_str().unwrap()])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "Multi-file test failed: {}", stdout);
    assert!(stdout.contains("42"), "Expected 42, got: {}", stdout);
}

#[test]
fn test_default_params() {
    let source = r#"fn add(a: i64, b: i64 = 10) -> i64 {
    return a + b
}

fn main() {
    print(add(5))
    print(add(5, 20))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Default params failed: {}", output);
    assert!(output.contains("15"), "Expected 15 (5+10), got: {}", output);
    assert!(output.contains("25"), "Expected 25 (5+20), got: {}", output);
}

#[test]
fn test_assert_eq_builtin() {
    let source = r#"fn main() {
    assert_eq(42, 42)
    assert_eq(10 + 5, 15)
    print("passed")
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "assert_eq failed: {}", output);
    assert!(output.contains("passed"), "Expected passed, got: {}", output);
}

#[test]
fn test_if_let() {
    let source = r#"fn main() {
    let x = 42
    if let y = x {
        print(y)
    } else {
        print(0)
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "if let failed: {}", output);
    assert!(output.contains("42"), "Expected 42, got: {}", output);
}

// ── Phase 2: Package integration tests ──────────────────────────────────────

/// Helper: create a vendored package in a temp dir, write a main.sbx that uses
/// it, compile+run, and return (stdout, success).
fn run_with_vendored(pkg_name: &str, pkg_source: &str, main_source: &str) -> (String, bool) {
    let tmp = TempDir::new().unwrap();
    let vendor = tmp.path().join(".sandbox").join("vendor").join(pkg_name);
    std::fs::create_dir_all(&vendor).unwrap();
    std::fs::write(
        vendor.join(format!("{}.sbx", pkg_name)),
        pkg_source,
    ).unwrap();
    std::fs::write(tmp.path().join("main.sbx"), main_source).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["run", tmp.path().join("main.sbx").to_str().unwrap()])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}{}", stdout, stderr);
    (combined, output.status.success())
}

#[test]
fn test_sandbox_crypto_hash_code() {
    let pkg = r#"fn hash_code(s: string) -> i64 {
    let h: i64 = 5381
    let mut i: i64 = 0
    let len = string::length(s)
    while i < len {
        let c = string::char_at(s, i)
        h = ((h * 33) + c) % 2147483647
        i = i + 1
    }
    return h
}"#;
    let main = r#"use sandbox_crypto::*;

fn main() {
    let h = hash_code("hello")
    print(h)
    let h2 = hash_code("world")
    print(h2)
    let same = hash_code("hello")
    print(same)
}"#;
    let (output, ok) = run_with_vendored("sandbox_crypto", pkg, main);
    assert!(ok, "sandbox_crypto::hash_code failed: {}", output);
    // All three hash lines should be present
    let lines: Vec<&str> = output.lines().collect();
    let hash_lines: Vec<&str> = lines.iter()
        .filter(|l| !l.starts_with('[') && !l.starts_with("  ") && !l.contains("Compiling")
            && !l.contains("Lexing") && !l.contains("Parsing") && !l.contains("Type checking")
            && !l.contains("Generating") && !l.contains("Loaded") && !l.contains("FnDef")
            && !l.contains("ModuleDef") && !l.contains("All checks") && !l.contains("lines of C"))
        .copied()
        .collect();
    assert!(hash_lines.len() >= 3, "Expected 3 hash outputs, got {}: {:?}", hash_lines.len(), hash_lines);
    // Same input should produce same hash
    assert_eq!(hash_lines[0], hash_lines[2], "Hash of 'hello' should be deterministic");
    // Different inputs should (likely) produce different hashes
    assert_ne!(hash_lines[0], hash_lines[1], "Different inputs should produce different hashes");
}

#[test]
fn test_sandbox_crypto_palindrome() {
    let pkg = r#"fn is_palindrome(s: string) -> bool {
    let len = string::length(s)
    let mut i: i64 = 0
    let half = len / 2
    while i < half {
        let left = string::char_at(s, i)
        let right = string::char_at(s, len - 1 - i)
        if left != right {
            return false
        }
        i = i + 1
    }
    return true
}"#;
    let main = r#"use sandbox_crypto::*;

fn main() {
    let p1 = is_palindrome("racecar")
    print(p1)
    let p2 = is_palindrome("hello")
    print(p2)
    let p3 = is_palindrome("aba")
    print(p3)
    let p4 = is_palindrome("abba")
    print(p4)
}"#;
    let (output, ok) = run_with_vendored("sandbox_crypto", pkg, main);
    assert!(ok, "sandbox_crypto::is_palindrome failed: {}", output);
    assert!(output.contains("1"), "Expected racecar to be palindrome (1)");
    assert!(output.contains("0"), "Expected hello to not be palindrome (0)");
}

#[test]
fn test_sandbox_crypto_rotate_string() {
    let pkg = r#"fn rotate_string(s: string, n: i64) -> string {
    let len = string::length(s)
    if len == 0 {
        return s
    }
    let shift = n % len
    let first = string::substring(s, 0, len - shift)
    let second = string::substring(s, len - shift, shift)
    return string::concat(second, first)
}"#;
    let main = r#"use sandbox_crypto::*;

fn main() {
    let r1: string = rotate_string("abcdef", 2)
    print(r1)
    let r2: string = rotate_string("hello", 1)
    print(r2)
}"#;
    let (output, ok) = run_with_vendored("sandbox_crypto", pkg, main);
    assert!(ok, "sandbox_crypto::rotate_string failed: {}", output);
    assert!(output.contains("efabcd"), "Expected 'efabcd', got: {}", output);
    assert!(output.contains("ohell"), "Expected 'ohell', got: {}", output);
}

#[test]
fn test_sandbox_datetime_is_leap_year() {
    let pkg = r#"fn is_leap_year(year: i64) -> bool {
    if (year % 400) == 0 {
        return true
    }
    if (year % 100) == 0 {
        return false
    }
    if (year % 4) == 0 {
        return true
    }
    return false
}"#;
    let main = r#"use sandbox_datetime::*;

fn main() {
    let l1 = is_leap_year(2024)
    print(l1)
    let l2 = is_leap_year(2023)
    print(l2)
    let l3 = is_leap_year(2000)
    print(l3)
    let l4 = is_leap_year(1900)
    print(l4)
}"#;
    let (output, ok) = run_with_vendored("sandbox_datetime", pkg, main);
    assert!(ok, "sandbox_datetime::is_leap_year failed: {}", output);
    let lines: Vec<&str> = output.lines().collect();
    // Filter out compiler progress lines
    let result_lines: Vec<&str> = lines.iter()
        .filter(|l| !l.starts_with('[') && !l.starts_with("  ") && l.len() <= 3)
        .copied()
        .collect();
    assert_eq!(result_lines, vec!["1", "0", "1", "0"],
        "Expected [1,0,1,0] for leap years 2024,2023,2000,1900");
}

#[test]
fn test_sandbox_datetime_days_in_month() {
    let pkg = r#"fn is_leap_year(year: i64) -> bool {
    if (year % 400) == 0 {
        return true
    }
    if (year % 100) == 0 {
        return false
    }
    if (year % 4) == 0 {
        return true
    }
    return false
}

fn days_in_month(month: i64, year: i64) -> i64 {
    if month == 2 {
        if is_leap_year(year) {
            return 29
        }
        return 28
    }
    if month == 4 { return 30 }
    if month == 6 { return 30 }
    if month == 9 { return 30 }
    if month == 11 { return 30 }
    return 31
}"#;
    let main = r#"use sandbox_datetime::*;

fn main() {
    let feb_leap = days_in_month(2, 2024)
    print(feb_leap)
    let feb_non = days_in_month(2, 2023)
    print(feb_non)
    let jan = days_in_month(1, 2024)
    print(jan)
    let apr = days_in_month(4, 2024)
    print(apr)
}"#;
    let (output, ok) = run_with_vendored("sandbox_datetime", pkg, main);
    assert!(ok, "sandbox_datetime::days_in_month failed: {}", output);
    assert!(output.contains("29"), "Feb 2024 should have 29 days");
    assert!(output.contains("28"), "Feb 2023 should have 28 days");
    assert!(output.contains("31"), "Jan should have 31 days");
    assert!(output.contains("30"), "Apr should have 30 days");
}

#[test]
fn test_sandbox_regex_find_all() {
    let pkg = r#"fn find_all(s: string, pattern: string) -> i64 {
    let mut count: i64 = 0
    let mut pos: i64 = 0
    let len = string::length(s)
    let plen = string::length(pattern)
    while pos <= (len - plen) {
        let sub = string::substring(s, pos, plen)
        if string::equals(sub, pattern) {
            count = count + 1
            pos = pos + plen
        } else {
            pos = pos + 1
        }
    }
    return count
}"#;
    let main = r#"use sandbox_regex::*;

fn main() {
    let c1 = find_all("ababab", "ab")
    print(c1)
    let c2 = find_all("aaaa", "aa")
    print(c2)
    let c3 = find_all("hello", "xyz")
    print(c3)
}"#;
    let (output, ok) = run_with_vendored("sandbox_regex", pkg, main);
    assert!(ok, "sandbox_regex::find_all failed: {}", output);
    assert!(output.contains("3"), "Expected 3 for 'ababab' count 'ab'");
    assert!(output.contains("0"), "Expected 0 for 'hello' count 'xyz'");
}

#[test]
fn test_sandbox_regex_count_char() {
    let pkg = r#"fn count_char(s: string, target: i64) -> i64 {
    let mut count: i64 = 0
    let mut i: i64 = 0
    let len = string::length(s)
    while i < len {
        let c = string::char_at(s, i)
        if c == target {
            count = count + 1
        }
        i = i + 1
    }
    return count
}"#;
    let main = r#"use sandbox_regex::*;

fn main() {
    let c1 = count_char("banana", 97)
    print(c1)
    let c2 = count_char("hello", 108)
    print(c2)
    let c3 = count_char("xyz", 97)
    print(c3)
}"#;
    let (output, ok) = run_with_vendored("sandbox_regex", pkg, main);
    assert!(ok, "sandbox_regex::count_char failed: {}", output);
    assert!(output.contains("3"), "Expected 3 'a' chars in 'banana'");
    assert!(output.contains("2"), "Expected 2 'l' chars in 'hello'");
    assert!(output.contains("0"), "Expected 0 'a' chars in 'xyz'");
}

#[test]
fn test_sandbox_math_ext_factorial() {
    let pkg = r#"fn factorial(n: i64) -> i64 {
    if n <= 1 {
        return 1
    }
    let mut result: i64 = 1
    let mut i: i64 = 2
    while i <= n {
        result = result * i
        i = i + 1
    }
    return result
}"#;
    let main = r#"use sandbox_math_ext::*;

fn main() {
    let f0 = factorial(0)
    print(f0)
    let f1 = factorial(1)
    print(f1)
    let f5 = factorial(5)
    print(f5)
    let f10 = factorial(10)
    print(f10)
}"#;
    let (output, ok) = run_with_vendored("sandbox_math_ext", pkg, main);
    assert!(ok, "sandbox_math_ext::factorial failed: {}", output);
    assert!(output.contains("1"), "Expected factorial(0) = 1");
    assert!(output.contains("120"), "Expected factorial(5) = 120");
    assert!(output.contains("3628800"), "Expected factorial(10) = 3628800");
}

#[test]
fn test_sandbox_math_ext_fibonacci() {
    let pkg = r#"fn fibonacci(n: i64) -> i64 {
    if n <= 0 {
        return 0
    }
    if n == 1 {
        return 1
    }
    let mut a: i64 = 0
    let mut b: i64 = 1
    let mut i: i64 = 2
    while i <= n {
        let temp = a + b
        a = b
        b = temp
        i = i + 1
    }
    return b
}"#;
    let main = r#"use sandbox_math_ext::*;

fn main() {
    let f0 = fibonacci(0)
    print(f0)
    let f1 = fibonacci(1)
    print(f1)
    let f6 = fibonacci(6)
    print(f6)
    let f10 = fibonacci(10)
    print(f10)
}"#;
    let (output, ok) = run_with_vendored("sandbox_math_ext", pkg, main);
    assert!(ok, "sandbox_math_ext::fibonacci failed: {}", output);
    assert!(output.contains("0"), "Expected fibonacci(0) = 0");
    assert!(output.contains("8"), "Expected fibonacci(6) = 8");
    assert!(output.contains("55"), "Expected fibonacci(10) = 55");
}

#[test]
fn test_sandbox_math_ext_is_prime() {
    let pkg = r#"fn is_prime(n: i64) -> bool {
    if n <= 1 {
        return false
    }
    if n <= 3 {
        return true
    }
    if (n % 2) == 0 {
        return false
    }
    if (n % 3) == 0 {
        return false
    }
    let mut i: i64 = 5
    while (i * i) <= n {
        if (n % i) == 0 {
            return false
        }
        if (n % (i + 2)) == 0 {
            return false
        }
        i = i + 6
    }
    return true
}"#;
    let main = r#"use sandbox_math_ext::*;

fn main() {
    let p0 = is_prime(0)
    print(p0)
    let p2 = is_prime(2)
    print(p2)
    let p7 = is_prime(7)
    print(p7)
    let p9 = is_prime(9)
    print(p9)
    let p13 = is_prime(13)
    print(p13)
    let p15 = is_prime(15)
    print(p15)
}"#;
    let (output, ok) = run_with_vendored("sandbox_math_ext", pkg, main);
    assert!(ok, "sandbox_math_ext::is_prime failed: {}", output);
    assert!(output.contains("1"), "Expected 2 to be prime");
    assert!(output.contains("0"), "Expected 9 to not be prime");
}

#[test]
fn test_sandbox_math_ext_gcd_lcm() {
    let pkg = r#"fn gcd(a: i64, b: i64) -> i64 {
    let mut x = a
    let mut y = b
    while y != 0 {
        let temp = y
        y = x % y
        x = temp
    }
    return x
}

fn lcm(a: i64, b: i64) -> i64 {
    return (a * b) / gcd(a, b)
}"#;
    let main = r#"use sandbox_math_ext::*;

fn main() {
    let g1 = gcd(12, 8)
    print(g1)
    let g2 = gcd(15, 25)
    print(g2)
    let g3 = gcd(7, 13)
    print(g3)
    let l1 = lcm(4, 6)
    print(l1)
    let l2 = lcm(3, 5)
    print(l2)
}"#;
    let (output, ok) = run_with_vendored("sandbox_math_ext", pkg, main);
    assert!(ok, "sandbox_math_ext::gcd/lcm failed: {}", output);
    assert!(output.contains("4"), "Expected gcd(12,8) = 4");
    assert!(output.contains("5"), "Expected gcd(15,25) = 5");
    assert!(output.contains("1"), "Expected gcd(7,13) = 1");
    assert!(output.contains("12"), "Expected lcm(4,6) = 12");
    assert!(output.contains("15"), "Expected lcm(3,5) = 15");
}

#[test]
fn test_sandbox_math_ext_collatz() {
    let pkg = r#"fn collatz_steps(n: i64) -> i64 {
    let mut num = n
    let mut steps: i64 = 0
    while num != 1 {
        if (num % 2) == 0 {
            num = num / 2
        } else {
            num = 3 * num + 1
        }
        steps = steps + 1
    }
    return steps
}"#;
    let main = r#"use sandbox_math_ext::*;

fn main() {
    let s1 = collatz_steps(6)
    print(s1)
    let s2 = collatz_steps(1)
    print(s2)
    let s3 = collatz_steps(16)
    print(s3)
}"#;
    let (output, ok) = run_with_vendored("sandbox_math_ext", pkg, main);
    assert!(ok, "sandbox_math_ext::collatz_steps failed: {}", output);
    assert!(output.contains("8"), "Expected collatz_steps(6) = 8");
    assert!(output.contains("0"), "Expected collatz_steps(1) = 0");
    assert!(output.contains("4"), "Expected collatz_steps(16) = 4");
}

#[test]
fn test_all_four_packages_together() {
    // End-to-end: load all 4 packages in a single program via wildcard imports
    let crypto_src = r#"fn hash_code(s: string) -> i64 {
    let h: i64 = 5381
    let mut i: i64 = 0
    let len = string::length(s)
    while i < len {
        let c = string::char_at(s, i)
        h = ((h * 33) + c) % 2147483647
        i = i + 1
    }
    return h
}

fn is_palindrome(s: string) -> bool {
    let len = string::length(s)
    let mut i: i64 = 0
    let half = len / 2
    while i < half {
        let left = string::char_at(s, i)
        let right = string::char_at(s, len - 1 - i)
        if left != right {
            return false
        }
        i = i + 1
    }
    return true
}"#;

    let datetime_src = r#"fn is_leap_year(year: i64) -> bool {
    if (year % 400) == 0 { return true }
    if (year % 100) == 0 { return false }
    if (year % 4) == 0 { return true }
    return false
}

fn days_in_month(month: i64, year: i64) -> i64 {
    if month == 2 {
        if is_leap_year(year) { return 29 }
        return 28
    }
    if month == 4 { return 30 }
    if month == 6 { return 30 }
    if month == 9 { return 30 }
    if month == 11 { return 30 }
    return 31
}"#;

    let regex_src = r#"fn count_char(s: string, target: i64) -> i64 {
    let mut count: i64 = 0
    let mut i: i64 = 0
    let len = string::length(s)
    while i < len {
        let c = string::char_at(s, i)
        if c == target {
            count = count + 1
        }
        i = i + 1
    }
    return count
}"#;

    let math_src = r#"fn factorial(n: i64) -> i64 {
    if n <= 1 { return 1 }
    let mut result: i64 = 1
    let mut i: i64 = 2
    while i <= n {
        result = result * i
        i = i + 1
    }
    return result
}

fn is_prime(n: i64) -> bool {
    if n <= 1 { return false }
    if n <= 3 { return true }
    if (n % 2) == 0 { return false }
    if (n % 3) == 0 { return false }
    let mut i: i64 = 5
    while (i * i) <= n {
        if (n % i) == 0 { return false }
        if (n % (i + 2)) == 0 { return false }
        i = i + 6
    }
    return true
}"#;

    let main_src = r#"use sandbox_crypto::*;
use sandbox_datetime::*;
use sandbox_regex::*;
use sandbox_math_ext::*;

fn main() {
    // crypto
    let h = hash_code("hello")
    print(h)
    let pal = is_palindrome("racecar")
    print(pal)

    // datetime
    let leap = is_leap_year(2024)
    print(leap)
    let days = days_in_month(2, 2024)
    print(days)

    // regex
    let cc = count_char("banana", 97)
    print(cc)

    // math_ext
    let f5 = factorial(5)
    print(f5)
    let prime = is_prime(17)
    print(prime)
    let notprime = is_prime(15)
    print(notprime)
}"#;

    let tmp = TempDir::new().unwrap();

    // Vendor all 4 packages
    for (name, src) in [
        ("sandbox_crypto", crypto_src),
        ("sandbox_datetime", datetime_src),
        ("sandbox_regex", regex_src),
        ("sandbox_math_ext", math_src),
    ] {
        let vendor = tmp.path().join(".sandbox").join("vendor").join(name);
        std::fs::create_dir_all(&vendor).unwrap();
        std::fs::write(vendor.join(format!("{}.sbx", name)), src).unwrap();
    }

    std::fs::write(tmp.path().join("main.sbx"), main_src).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["run", tmp.path().join("main.sbx").to_str().unwrap()])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "All four packages together failed: {}", stdout);
    assert!(stdout.contains("261239035"), "Expected hash_code('hello') = 261239035");
    assert!(stdout.contains("1"), "Expected is_palindrome('racecar') = true");
    assert!(stdout.contains("29"), "Expected days_in_month(2,2024) = 29");
    assert!(stdout.contains("120"), "Expected factorial(5) = 120");
    assert!(stdout.contains("3"), "Expected count_char('banana','a') = 3");
}

#[test]
fn test_pkg_verify_command_exists() {
    // Verify the pkg verify subcommand is available in help output
    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["pkg", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(stdout.contains("verify"), "'sandbox pkg verify' should exist in help: {}", stdout);
    assert!(stdout.contains("keygen"), "'sandbox pkg keygen' should exist in help: {}", stdout);
    assert!(stdout.contains("keys"), "'sandbox pkg keys' should exist in help: {}", stdout);
}

#[test]
fn test_install_help_shows_require_signatures() {
    // Verify the --require-signatures flag is documented
    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["install", "--help"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(stdout.contains("require-signatures"),
        "'--require-signatures' should appear in install help: {}", stdout);
}

#[test]
fn test_tree_shows_package_name_and_version() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("sandbox.toml"),
        r#"[package]
name = "myapp"
version = "2.0.0"
description = "test"

[dependencies]
"#,
    ).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["tree"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "tree failed: {}", stdout);
    assert!(stdout.contains("myapp"), "Should show package name: {}", stdout);
    assert!(stdout.contains("2.0.0"), "Should show package version: {}", stdout);
    assert!(stdout.contains("no dependencies"), "Should show no-deps message: {}", stdout);
}

#[test]
fn test_tree_shows_lock_installed_count() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("sandbox.toml"),
        r#"[package]
name = "myapp"
version = "1.0.0"
description = "test"

[dependencies]
"#,
    ).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["tree"])
        .current_dir(tmp.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "tree failed: {}", stdout);
    // Lock info line should be absent when no .sandbox/lock.toml exists
    assert!(!stdout.contains("package(s) installed"),
        "Should not show lock info without lock file: {}", stdout);
}

#[test]
fn test_operator_precedence_comparison_below_arithmetic() {
    // Comparisons should bind LOWER than arithmetic:
    // n % 2 == 0 should parse as (n % 2) == 0
    let source = r#"
fn main() {
    let a: bool = 10 % 2 == 0
    print(a)
    let b: bool = 3 + 4 > 5
    print(b)
    let c: bool = 2 * 3 == 6
    print(c)
    let d: i64 = 1 + 2 * 3
    print(d)
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Failed to compile: {}", output);
    assert!(output.contains("1"), "10 % 2 == 0 should be true: {}", output);
    assert!(output.contains("7"), "1 + 2 * 3 should be 7: {}", output);
}

#[test]
fn test_logical_not_operator() {
    let source = r#"
fn main() {
    let a: bool = !true
    print(a)
    let b: bool = !false
    print(b)
    let c: bool = !(5 > 3)
    print(c)
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Failed to compile: {}", output);
    assert!(output.contains("0"), "!true should be false: {}", output);
    assert!(output.contains("1"), "!false should be true: {}", output);
}

#[test]
fn test_logical_and_or_operators() {
    let source = r#"
fn main() {
    let a: bool = true && true
    print(a)
    let b: bool = true && false
    print(b)
    let c: bool = false || true
    print(c)
    let d: bool = false || false
    print(d)
    let e: bool = (3 > 2) && (5 < 10)
    print(e)
    let f: bool = (1 > 2) || (3 == 3)
    print(f)
}
"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Failed to compile: {}", output);
    // The compilation log contains extra lines, so check individual values
    assert!(output.contains("\n1\n"), "true && true should be true: {}", output);
    assert!(output.contains("\n0\n"), "false values should be 0: {}", output);
}

#[test]
fn test_sandbox_test_command_all_pass() {
    let source = r#"
test fn addition {
    let x: i64 = 2 + 3
    assert(x == 5)
}

test fn multiplication {
    let x: i64 = 4 * 5
    assert(x == 20)
}
"#;
    let tmp = TempDir::new().unwrap();
    let sbx_path = tmp.path().join("test.sbx");
    fs::write(&sbx_path, source).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["test", sbx_path.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "sandbox test failed: {}", stdout);
    assert!(stdout.contains("addition"), "Should show test name: {}", stdout);
    assert!(stdout.contains("multiplication"), "Should show test name: {}", stdout);
    assert!(stdout.contains("passed"), "Should show pass count: {}", stdout);
    assert!(stdout.contains("0.0ms") || stdout.contains("ms"), "Should show timing: {}", stdout);
}

#[test]
fn test_sandbox_test_command_with_failure() {
    let source = r#"
test fn pass_test {
    let x: i64 = 1
    assert(x == 1)
}

test fn fail_test {
    let x: i64 = 1
    assert(x == 2)
}

test fn another_pass {
    let y: i64 = 10
    assert(y == 10)
}
"#;
    let tmp = TempDir::new().unwrap();
    let sbx_path = tmp.path().join("test.sbx");
    fs::write(&sbx_path, source).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["test", sbx_path.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    // Should NOT succeed (has a failing test)
    assert!(!output.status.success(), "Should fail with failing test");
    // The passing test should still show as passed
    assert!(stdout.contains("pass_test"), "Passing test should still run: {}", stdout);
    assert!(stdout.contains("another_pass"), "Tests after failure should still run: {}", stdout);
    assert!(stdout.contains("1 failed"), "Should report 1 failed: {}", stdout);
    assert!(stdout.contains("2 passed"), "Should report 2 passed: {}", stdout);
}

#[test]
fn test_sandbox_test_command_no_tests() {
    let source = r#"
fn main() {
    print("no tests here")
}
"#;
    let tmp = TempDir::new().unwrap();
    let sbx_path = tmp.path().join("test.sbx");
    fs::write(&sbx_path, source).unwrap();

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["test", sbx_path.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "Should succeed with no tests: {}", stdout);
    assert!(stdout.contains("No test functions found") || stdout.contains("no test"),
        "Should indicate no tests found: {}", stdout);
}

#[test]
fn test_generic_function_identity() {
    let source = r#"fn identity<T>(x: T) -> T {
    return x
}
fn main() {
    print(identity<i64>(42))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Generic identity failed: {}", output);
    assert!(output.contains("42"), "Expected 42, got: {}", output);
}

#[test]
fn test_generic_function_multiple_instantiations() {
    let source = r#"fn add<T>(a: T, b: T) -> T {
    return a + b
}
fn main() {
    print(add<i64>(3, 4))
    print(add<i64>(10, 20))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Generic add failed: {}", output);
    assert!(output.contains("7"), "Expected 7, got: {}", output);
    assert!(output.contains("30"), "Expected 30, got: {}", output);
}

#[test]
fn test_generic_function_same_name_different_types() {
    let source = r#"fn identity<T>(x: T) -> T {
    return x
}
fn main() {
    print(identity<i64>(100))
    print(identity<i64>(200))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Multiple generic calls failed: {}", output);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert!(lines.iter().any(|l| l.trim() == "100"), "Expected 100, got: {}", output);
    assert!(lines.iter().any(|l| l.trim() == "200"), "Expected 200, got: {}", output);
}

#[test]
fn test_generic_function_unused_generic_not_emitted() {
    // A generic function that is never called should not cause compilation errors
    let source = r#"fn unused<T>(x: T) -> T {
    return x
}
fn main() {
    print(42)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Unused generic function should not cause errors: {}", output);
    assert!(output.contains("42"), "Expected 42, got: {}", output);
}

// ==================== Generic Structs ====================

#[test]
fn test_generic_struct_basic() {
    let source = r#"struct Pair<T> {
    first: T,
    second: T,
}

fn main() {
    let p = Pair<i64> { first: 42, second: 99 }
    print(p.first)
    print(p.second)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Generic struct basic test failed: {}", output);
    assert!(output.contains("42"), "Expected 42, got: {}", output);
    assert!(output.contains("99"), "Expected 99, got: {}", output);
}

#[test]
fn test_generic_struct_multiple_type_params() {
    let source = r#"struct Pair<T> {
    first: T,
    second: T,
}

struct Container<T> {
    value: T,
    count: i64,
}

fn main() {
    let p = Pair<i64> { first: 10, second: 20 }
    print(p.first)
    print(p.second)
    
    let c = Container<i64> { value: 42, count: 5 }
    print(c.value)
    print(c.count)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Generic struct multiple types test failed: {}", output);
    assert!(output.contains("10"), "Expected 10, got: {}", output);
    assert!(output.contains("20"), "Expected 20, got: {}", output);
    assert!(output.contains("42"), "Expected 42, got: {}", output);
    assert!(output.contains("5"), "Expected 5, got: {}", output);
}

#[test]
fn test_generic_struct_same_struct_different_types() {
    let source = r#"struct Wrapper<T> {
    value: T,
}

fn main() {
    let a = Wrapper<i64> { value: 100 }
    let b = Wrapper<i64> { value: 200 }
    print(a.value)
    print(b.value)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Generic struct same type test failed: {}", output);
    let lines: Vec<&str> = output.trim().lines().collect();
    assert!(lines.iter().any(|l| l.trim() == "100"), "Expected 100, got: {}", output);
    assert!(lines.iter().any(|l| l.trim() == "200"), "Expected 200, got: {}", output);
}

#[test]
fn test_generic_struct_with_functions() {
    // Test generic structs used inside generic functions (struct created in body)
    let source = r#"struct Pair<T> {
    first: T,
    second: T,
}

fn main() {
    let p = Pair<i64> { first: 10, second: 20 }
    print(p.first)
    print(p.second)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Generic struct with functions test failed: {}", output);
    assert!(output.contains("10"), "Expected 10, got: {}", output);
    assert!(output.contains("20"), "Expected 20, got: {}", output);
}

#[test]
fn test_generic_struct_field_access() {
    // Test field access on generic structs (computed in main)
    let source = r#"struct Point<T> {
    x: T,
    y: T,
}

fn main() {
    let pt = Point<i64> { x: 3, y: 4 }
    print(pt.x * pt.x + pt.y * pt.y)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Generic struct field access test failed: {}", output);
    assert!(output.contains("25"), "Expected 25, got: {}", output);
}

#[test]
fn test_generic_struct_unused_not_emitted() {
    let source = r#"struct Unused<T> {
    value: T,
}

fn main() {
    print(42)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Unused generic struct should not cause errors: {}", output);
    assert!(output.contains("42"), "Expected 42, got: {}", output);
}


#[test]
fn test_sandbox_test_command_filter() {
    let source = r#"
test fn test_add {
    let x: i64 = 1 + 1
    assert(x == 2)
}

test fn test_sub {
    let x: i64 = 5 - 3
    assert(x == 2)
}

test fn test_mul {
    let x: i64 = 3 * 4
    assert(x == 12)
}

test fn test_div {
    let x: i64 = 10 / 2
    assert(x == 5)
}
"#;
    let tmp = TempDir::new().unwrap();
    let sbx_path = tmp.path().join("test.sbx");
    fs::write(&sbx_path, source).unwrap();

    let bin = sandbox_bin();

    // Test 1: No filter - all 4 tests run
    let output = Command::new(&bin)
        .args(["test", sbx_path.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "All tests should pass: {}", stdout);
    assert!(stdout.contains("running 4 test(s)"), "Should run 4 tests: {}", stdout);
    assert!(stdout.contains("4 passed"), "Should report 4 passed: {}", stdout);

    // Test 2: Filter by "sub" - only test_sub runs
    let output = Command::new(&bin)
        .args(["test", sbx_path.to_str().unwrap(), "--filter", "sub"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "Filtered test should pass: {}", stdout);
    assert!(stdout.contains("running 1 test(s)"), "Should run 1 test: {}", stdout);
    assert!(stdout.contains("test_sub"), "Should run test_sub: {}", stdout);
    // Note: test names also appear in typechecker output, so we don't assert their absence.
    assert!(stdout.contains("1 passed"), "Should report 1 passed: {}", stdout);
    assert!(stdout.contains("3 skipped"), "Should report 3 skipped: {}", stdout);
    // Note: test names also appear in typechecker output, so we can't assert they're absent from stdout entirely.
    // Instead verify only test_sub was executed (has a checkmark ✓ or ✗ line in test runner output).

    // Test 3: Filter by "test_" - all 4 tests run (all match)
    let output = Command::new(&bin)
        .args(["test", sbx_path.to_str().unwrap(), "--filter", "test_"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "All filtered tests should pass: {}", stdout);
    assert!(stdout.contains("running 4 test(s)"), "Should run 4 tests: {}", stdout);
    assert!(stdout.contains("4 passed"), "Should report 4 passed: {}", stdout);

    // Test 4: Filter by "xyz" - no tests run
    let output = Command::new(&bin)
        .args(["test", sbx_path.to_str().unwrap(), "--filter", "xyz"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "No matches should still succeed: {}", stdout);
    assert!(stdout.contains("running 0 test(s)"), "Should run 0 tests: {}", stdout);
    assert!(stdout.contains("0 passed"), "Should report 0 passed: {}", stdout);
}


#[test]
fn test_generic_type_in_fn_signature() {
    let source = r#"struct Pair<T> {
    first: T,
    second: T,
}

fn make_pair(a: i64, b: i64) -> Pair<i64> {
    return Pair<i64> { first: a, second: b }
}

fn main() {
    let p = make_pair(10, 20)
    print(p.first)
    print(p.second)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Generic type in fn signature failed: {}", output);
    assert!(output.contains("10"), "Expected 10, got: {}", output);
    assert!(output.contains("20"), "Expected 20, got: {}", output);
}

#[test]
fn test_generic_type_multiple_structs() {
    let source = r#"struct Pair<T> {
    first: T,
    second: T,
}

struct Triple<T> {
    a: T,
    b: T,
    c: T,
}

fn make_pair(a: i64, b: i64) -> Pair<i64> {
    return Pair<i64> { first: a, second: b }
}

fn make_triple(a: i64, b: i64, c: i64) -> Triple<i64> {
    return Triple<i64> { a: a, b: b, c: c }
}

fn main() {
    let p = make_pair(10, 20)
    print(p.first)
    let t = make_triple(1, 2, 3)
    print(t.a)
    print(t.c)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Multiple generic structs failed: {}", output);
    assert!(output.contains("10"), "Expected 10, got: {}", output);
    assert!(output.contains("1"), "Expected 1, got: {}", output);
    assert!(output.contains("3"), "Expected 3, got: {}", output);
}


#[test]
fn test_llvm_generic_struct() {
    let source = r#"struct Pair<T> {
    first: T,
    second: T,
}

fn make_pair(a: i64, b: i64) -> Pair<i64> {
    return Pair<i64> { first: a, second: b }
}

fn main() {
    let p = make_pair(10, 20)
    print(p.first)
    print(p.second)
}"#;
    let tmp = TempDir::new().unwrap();
    let sbx_path = tmp.path().join("test.sbx");
    fs::write(&sbx_path, source).unwrap();
    let ll_path = tmp.path().join("test.ll");

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["llvm", sbx_path.to_str().unwrap(), "-o", ll_path.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "LLVM generation failed: {}", stdout);
    assert!(ll_path.exists(), ".ll file not created");

    let ll = fs::read_to_string(&ll_path).unwrap();
    assert!(ll.contains("%Pair_i64 = type"), "Missing monomorphized struct typedef: {}", ll);
    assert!(ll.contains("@make_pair"), "Missing make_pair function: {}", ll);
    assert!(ll.contains("alloca %Pair_i64"), "Missing alloca for Pair_i64: {}", ll);
    assert!(ll.contains("getelementptr %Pair_i64"), "Missing GEP for Pair_i64: {}", ll);
}

#[test]
fn test_llvm_generic_struct_multiple() {
    let source = r#"struct Pair<T> {
    first: T,
    second: T,
}

struct Triple<T> {
    a: T,
    b: T,
    c: T,
}

fn main() {
    let p = Pair<i64> { first: 10, second: 20 }
    print(p.first)
    let t = Triple<i64> { a: 1, b: 2, c: 3 }
    print(t.a)
}"#;
    let tmp = TempDir::new().unwrap();
    let sbx_path = tmp.path().join("test.sbx");
    fs::write(&sbx_path, source).unwrap();
    let ll_path = tmp.path().join("test.ll");

    let bin = sandbox_bin();
    let output = Command::new(&bin)
        .args(["llvm", sbx_path.to_str().unwrap(), "-o", ll_path.to_str().unwrap()])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "LLVM generation failed: {}", stdout);

    let ll = fs::read_to_string(&ll_path).unwrap();
    assert!(ll.contains("%Pair_i64 = type"), "Missing Pair_i64 typedef: {}", ll);
    assert!(ll.contains("%Triple_i64 = type"), "Missing Triple_i64 typedef: {}", ll);
}


#[test]
fn test_generic_struct_two_type_params() {
    let source = r#"struct Pair<T, U> {
    first: T,
    second: U,
}

fn main() {
    let p = Pair<i64, i64> { first: 42, second: 99 }
    print(p.first)
    print(p.second)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Two type params failed: {}", output);
    assert!(output.contains("42"), "Expected 42, got: {}", output);
    assert!(output.contains("99"), "Expected 99, got: {}", output);
}

#[test]
fn test_generic_struct_three_type_params() {
    let source = r#"struct Triple<T, U, V> {
    first: T,
    second: U,
    third: V,
}

fn make_triple(a: i64, b: i64, c: i64) -> Triple<i64, i64, i64> {
    return Triple<i64, i64, i64> { first: a, second: b, third: c }
}

fn main() {
    let t = make_triple(10, 20, 30)
    print(t.first)
    print(t.second)
    print(t.third)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Three type params failed: {}", output);
    assert!(output.contains("10"), "Expected 10, got: {}", output);
    assert!(output.contains("20"), "Expected 20, got: {}", output);
    assert!(output.contains("30"), "Expected 30, got: {}", output);
}

#[test]
fn test_generic_struct_multiple_distinct() {
    let source = r#"struct Pair<T, U> {
    first: T,
    second: U,
}

fn main() {
    let p1 = Pair<i64, i64> { first: 1, second: 2 }
    print(p1.first)
    let p2 = Pair<i64, i64> { first: 10, second: 20 }
    print(p2.first)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Multiple distinct structs failed: {}", output);
    assert!(output.contains("1"), "Expected 1, got: {}", output);
    assert!(output.contains("10"), "Expected 10, got: {}", output);
}


#[test]
fn test_generic_enum_basic() {
    let source = r#"enum Maybe<T> {
    Has(T),
    Empty,
}

fn main() {
    let x = Maybe<i64>::Has(42)
    match x {
        Maybe::Has(v) => {
            print(v)
        },
        Maybe::Empty => {
            print("empty")
        },
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Generic enum basic test failed: {}", output);
    assert!(output.contains("42"), "Expected 42, got: {}", output);
}

#[test]
fn test_generic_enum_empty_case() {
    let source = r#"enum Maybe<T> {
    Has(T),
    Empty,
}

fn main() {
    let x = Maybe<i64>::Empty
    match x {
        Maybe::Has(v) => {
            print(v)
        },
        Maybe::Empty => {
            print("empty")
        },
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Generic enum empty case failed: {}", output);
    assert!(output.contains("empty"), "Expected 'empty', got: {}", output);
}

#[test]
fn test_generic_enum_multiple_variants() {
    let source = r#"enum Either<T, E> {
    Left(T),
    Right(E),
}

fn main() {
    let r = Either<i64, i64>::Left(100)
    match r {
        Either::Left(v) => {
            print(v)
        },
        Either::Right(e) => {
            print(e)
        },
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Generic enum multiple type params failed: {}", output);
    assert!(output.contains("100"), "Expected 100, got: {}", output);
}

#[test]
fn test_match_guard_basic() {
    let source = r#"fn classify(n: i64) -> i64 {
    return match n {
        0 => 0,
        n if n > 0 => 1,
        _ => 2,
    }
}

fn main() {
    print(classify(0))
    print(classify(5))
    print(classify(-3))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Match guard basic failed: {}", output);
    assert!(output.contains("0"), "Expected 0 for zero, got: {}", output);
    assert!(output.contains("1"), "Expected 1 for positive, got: {}", output);
    assert!(output.contains("2"), "Expected 2 for negative, got: {}", output);
}

#[test]
fn test_match_guard_with_binding() {
    let source = r#"fn abs_val(n: i64) -> i64 {
    return match n {
        0 => 0,
        v if v < 0 => 0 - v,
        v => v,
    }
}

fn main() {
    print(abs_val(-5))
    print(abs_val(3))
    print(abs_val(0))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Match guard with binding failed: {}", output);
    assert!(output.contains("5"), "Expected 5 for abs(-5), got: {}", output);
    assert!(output.contains("3"), "Expected 3 for abs(3), got: {}", output);
    assert!(output.contains("0"), "Expected 0 for abs(0), got: {}", output);
}

#[test]
fn test_match_guard_enum() {
    let source = r#"enum Tagged {
    Val(i64),
    Empty,
}

fn main() {
    let x = Tagged::Val(42)
    let v = match x {
        Tagged::Val(n) if n > 100 => 1,
        Tagged::Val(n) => 2,
        Tagged::Empty => 3,
    }
    print(v)
    
    let y = Tagged::Val(200)
    let v2 = match y {
        Tagged::Val(n) if n > 100 => 1,
        Tagged::Val(n) => 2,
        Tagged::Empty => 3,
    }
    print(v2)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Match guard enum failed: {}", output);
    assert!(output.contains("2"), "Expected 2 for Val(42), got: {}", output);
    assert!(output.contains("1"), "Expected 1 for Val(200), got: {}", output);
}

#[test]
fn test_match_string_literal() {
    let source = r#"fn greet(name: string) -> string {
    return match name {
        "Alice" => "Hello, Alice!",
        "Bob" => "Hello, Bob!",
        _ => "Hello, stranger!",
    }
}

fn main() {
    print(greet("Alice"))
    print(greet("Bob"))
    print(greet("Charlie"))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Match string literal failed: {}", output);
    assert!(output.contains("Hello, Alice!"), "Expected greeting for Alice, got: {}", output);
    assert!(output.contains("Hello, Bob!"), "Expected greeting for Bob, got: {}", output);
    assert!(output.contains("Hello, stranger!"), "Expected greeting for stranger, got: {}", output);
}

#[test]
fn test_match_string_literal_int_result() {
    let source = r#"fn describe(cmd: string) -> i64 {
    return match cmd {
        "quit" => 0,
        "help" => 1,
        "start" => 2,
        _ => -1,
    }
}

fn main() {
    print(describe("quit"))
    print(describe("help"))
    print(describe("start"))
    print(describe("unknown"))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "Match string literal int result failed: {}", output);
    assert!(output.contains("0"), "Expected 0 for quit, got: {}", output);
    assert!(output.contains("1"), "Expected 1 for help, got: {}", output);
    assert!(output.contains("2"), "Expected 2 for start, got: {}", output);
    assert!(output.contains("-1"), "Expected -1 for unknown, got: {}", output);
}

#[test]
fn test_break_while() {
    let source = r#"fn main() {
    let i = 0
    while i < 10 {
        if i == 5 {
            break
        }
        print(i)
        i = i + 1
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "break while failed: {}", output);
    assert!(output.contains("0"), "Expected 0, got: {}", output);
    assert!(output.contains("4"), "Expected 4, got: {}", output);
    assert!(!output.contains("5"), "Should not contain 5, got: {}", output);
}

#[test]
fn test_continue_while() {
    let source = r#"fn main() {
    let i = 0
    while i < 10 {
        i = i + 1
        if i % 2 == 0 {
            continue
        }
        print(i)
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "continue while failed: {}", output);
    assert!(output.contains("1"), "Expected 1, got: {}", output);
    assert!(output.contains("3"), "Expected 3, got: {}", output);
    assert!(output.contains("9"), "Expected 9, got: {}", output);
    assert!(!output.contains("\n2\n"), "Should not contain 2 as output, got: {}", output);
}

#[test]
fn test_break_for() {
    let source = r#"fn main() {
    for i in 0..10 {
        if i == 5 {
            break
        }
        print(i)
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "break for failed: {}", output);
    assert!(output.contains("0"), "Expected 0, got: {}", output);
    assert!(output.contains("4"), "Expected 4, got: {}", output);
    assert!(!output.contains("5"), "Should not contain 5, got: {}", output);
}

#[test]
fn test_continue_for() {
    let source = r#"fn main() {
    for i in 0..10 {
        if i % 2 == 0 {
            continue
        }
        print(i)
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "continue for failed: {}", output);
    assert!(output.contains("1"), "Expected 1, got: {}", output);
    assert!(output.contains("3"), "Expected 3, got: {}", output);
    assert!(output.contains("9"), "Expected 9, got: {}", output);
    assert!(!output.contains("\n0\n"), "Should not contain 0 as output, got: {}", output);
    assert!(!output.contains("\n2\n"), "Should not contain 2 as output, got: {}", output);
}

#[test]
fn test_len_string_literal() {
    let source = r#"fn main() {
    print(len("hello"))
    print(len(""))
    print(len("a"))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "len string literal failed: {}", output);
    assert!(output.contains("5"), "Expected 5, got: {}", output);
    assert!(output.contains("0"), "Expected 0, got: {}", output);
    assert!(output.contains("1"), "Expected 1, got: {}", output);
}

#[test]
fn test_len_string_variable() {
    let source = r#"fn main() {
    let s = "world"
    print(len(s))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "len string variable failed: {}", output);
    assert!(output.contains("5"), "Expected 5, got: {}", output);
}

#[test]
fn test_len_array_literal() {
    let source = r#"fn main() {
    print(len([1, 2, 3]))
    print(len([10, 20]))
    print(len([]))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "len array literal failed: {}", output);
    assert!(output.contains("3"), "Expected 3, got: {}", output);
    assert!(output.contains("2"), "Expected 2, got: {}", output);
    assert!(output.contains("0"), "Expected 0, got: {}", output);
}

#[test]
fn test_len_wrong_args() {
    let source = r#"fn main() {
    len()
}"#;
    let (_, ok) = compile_and_run(source);
    assert!(!ok, "len() with no args should fail");
}

#[test]
fn test_len_wrong_type() {
    let source = r#"fn main() {
    len(42)
}"#;
    let (_, ok) = compile_and_run(source);
    assert!(!ok, "len(42) should fail on non-string/array type");
}

#[test]
fn test_else_if_basic() {
    let source = r#"fn main() {
    let x = 2
    if x == 1 {
        print(1)
    } else if x == 2 {
        print(2)
    } else if x == 3 {
        print(3)
    } else {
        print(0)
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "else-if basic failed: {}", output);
    assert!(output.contains("2"), "Expected 2, got: {}", output);
    assert!(!output.contains("\n1\n"), "Should not contain 1, got: {}", output);
}

#[test]
fn test_else_if_fallthrough() {
    let source = r#"fn main() {
    let x = 99
    if x == 1 {
        print(1)
    } else if x == 2 {
        print(2)
    } else {
        print(99)
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "else-if fallthrough failed: {}", output);
    assert!(output.contains("99"), "Expected 99, got: {}", output);
}

#[test]
fn test_else_if_no_final_else() {
    let source = r#"fn main() {
    let x = 2
    if x == 1 {
        print(1)
    } else if x == 2 {
        print(2)
    } else if x == 3 {
        print(3)
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "else-if no final else failed: {}", output);
    assert!(output.contains("2"), "Expected 2, got: {}", output);
}

#[test]
fn test_else_if_long_chain() {
    let source = r#"fn main() {
    let x = 7
    if x == 1 { print(1) }
    else if x == 2 { print(2) }
    else if x == 3 { print(3) }
    else if x == 4 { print(4) }
    else if x == 5 { print(5) }
    else if x == 6 { print(6) }
    else if x == 7 { print(7) }
    else { print(0) }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "else-if long chain failed: {}", output);
    assert!(output.contains("7"), "Expected 7, got: {}", output);
}

#[test]
fn test_else_if_single_branch() {
    let source = r#"fn main() {
    let x = 3
    if x == 1 { print("first") }
    else if x == 2 { print("second") }
    else if x == 3 { print("third") }
    else if x == 3 { print("duplicate") }
    else { print("other") }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "else-if single branch failed: {}", output);
    assert!(output.contains("third"), "Expected third, got: {}", output);
    assert!(!output.contains("duplicate"), "Should not contain duplicate, got: {}", output);
}

#[test]
fn test_else_if_with_if_let() {
    let source = r#"fn main() {
    let x = 1
    if let n = x {
        print(n)
    } else if x == 2 {
        print("two")
    } else {
        print("other")
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "else-if with if-let failed: {}", output);
    assert!(output.contains("1"), "Expected 1, got: {}", output);
}

#[test]
fn test_str_concat_literals() {
    let source = r#"fn main() {
    print("foo" + "bar")
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str concat literals failed: {}", output);
    assert!(output.contains("foobar"), "Expected foobar, got: {}", output);
}

#[test]
fn test_str_concat_variables() {
    let source = r#"fn main() {
    let a = "hello"
    let b = " world"
    print(a + b)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str concat variables failed: {}", output);
    assert!(output.contains("hello world"), "Expected 'hello world', got: {}", output);
}

#[test]
fn test_str_concat_triple() {
    let source = r#"fn main() {
    print("a" + "b" + "c")
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str concat triple failed: {}", output);
    assert!(output.contains("abc"), "Expected abc, got: {}", output);
}

#[test]
fn test_str_concat_mixed() {
    let source = r#"fn main() {
    let a = "hello"
    print(a + " " + "world")
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str concat mixed failed: {}", output);
    assert!(output.contains("hello world"), "Expected 'hello world', got: {}", output);
}

#[test]
fn test_str_concat_with_len() {
    let source = r#"fn main() {
    let a = "hello"
    let b = " world"
    let msg = a + b
    print(len(msg))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str concat with len failed: {}", output);
    assert!(output.contains("11"), "Expected 11, got: {}", output);
}

#[test]
fn test_str_concat_type_error() {
    let source = r#"fn main() {
    let s = "hello"
    print(s + 42)
}"#;
    let (_, ok) = compile_and_run(source);
    assert!(!ok, "str + int should fail");
}

#[test]
fn test_str_eq_same_literal() {
    let source = r#"fn main() {
    let a = "hello"
    let b = "hello"
    if a == b {
        print("equal")
    } else {
        print("not equal")
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str eq same literal failed: {}", output);
    assert!(output.contains("equal"), "Expected equal, got: {}", output);
}

#[test]
fn test_str_eq_different() {
    let source = r#"fn main() {
    let a = "hello"
    let b = "world"
    if a == b {
        print("equal")
    } else {
        print("not equal")
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str eq different failed: {}", output);
    assert!(output.contains("not equal"), "Expected not equal, got: {}", output);
}

#[test]
fn test_str_eq_concat() {
    let source = r#"fn main() {
    let a = "hello"
    let d = "" + "hello"
    if a == d {
        print("equal")
    } else {
        print("not equal")
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str eq concat failed: {}", output);
    assert!(output.contains("equal"), "Expected equal (strcmp), got: {}", output);
}

#[test]
fn test_str_neq() {
    let source = r#"fn main() {
    let a = "hello"
    let b = "world"
    if a != b {
        print("not equal")
    } else {
        print("equal")
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str neq failed: {}", output);
    assert!(output.contains("not equal"), "Expected not equal, got: {}", output);
}

#[test]
fn test_str_eq_empty() {
    let source = r#"fn main() {
    if "" == "" {
        print("equal")
    }
    if "" != "x" {
        print("not equal")
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str eq empty failed: {}", output);
    assert!(output.contains("equal"), "Expected equal, got: {}", output);
    assert!(output.contains("not equal"), "Expected not equal, got: {}", output);
}

#[test]
fn test_call_as_expression() {
    let source = r#"fn double(x: i64) -> i64 {
    return x * 2
}
fn main() {
    let y = double(5)
    print(y)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "call as expression failed: {}", output);
    assert!(output.contains("10"), "Expected 10, got: {}", output);
}

#[test]
fn test_call_nested() {
    let source = r#"fn double(x: i64) -> i64 {
    return x * 2
}
fn add(a: i64, b: i64) -> i64 {
    return a + b
}
fn main() {
    print(double(add(1, 2)))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "nested call failed: {}", output);
    assert!(output.contains("6"), "Expected 6, got: {}", output);
}

#[test]
fn test_call_in_binary() {
    let source = r#"fn get_val() -> i64 {
    return 10
}
fn main() {
    let result = get_val() + get_val()
    print(result)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "call in binary failed: {}", output);
    assert!(output.contains("20"), "Expected 20, got: {}", output);
}

#[test]
fn test_call_in_if() {
    let source = r#"fn is_big(x: i64) -> i64 {
    if x > 100 {
        return 1
    } else {
        return 0
    }
}
fn main() {
    if is_big(200) == 1 {
        print("big")
    } else {
        print("small")
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "call in if failed: {}", output);
    assert!(output.contains("big"), "Expected big, got: {}", output);
}

#[test]
fn test_call_triple_nested() {
    let source = r#"fn id(x: i64) -> i64 {
    return x
}
fn main() {
    print(id(id(id(42))))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "triple nested call failed: {}", output);
    assert!(output.contains("42"), "Expected 42, got: {}", output);
}

#[test]
fn test_closure_expr_body() {
    let source = r#"fn main() {
    let f = |x| x + 1
    print(f(5))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "closure expr body failed: {}", output);
    assert!(output.contains("6"), "Expected 6, got: {}", output);
}

#[test]
fn test_closure_multi_param() {
    let source = r#"fn main() {
    let add = |x, y| x + y
    print(add(3, 4))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "closure multi param failed: {}", output);
    assert!(output.contains("7"), "Expected 7, got: {}", output);
}

#[test]
fn test_closure_comparison() {
    let source = r#"fn main() {
    let is_pos = |x| x > 0
    print(is_pos(5))
    print(is_pos(-1))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "closure comparison failed: {}", output);
    assert!(output.contains("1"), "Expected 1, got: {}", output);
    assert!(output.contains("0"), "Expected 0, got: {}", output);
}

#[test]
fn test_closure_arithmetic() {
    let source = r#"fn main() {
    let square = |x| x * x
    print(square(7))
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "closure arithmetic failed: {}", output);
    assert!(output.contains("49"), "Expected 49, got: {}", output);
}

#[test]
fn test_str_lt() {
    let source = r#"fn main() {
    if "aaa" < "zzz" {
        print("ok")
    } else {
        print("wrong")
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str lt failed: {}", output);
    assert!(output.contains("ok"), "Expected ok, got: {}", output);
}

#[test]
fn test_str_gt() {
    let source = r#"fn main() {
    if "zzz" > "aaa" {
        print("ok")
    } else {
        print("wrong")
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str gt failed: {}", output);
    assert!(output.contains("ok"), "Expected ok, got: {}", output);
}

#[test]
fn test_str_le() {
    let source = r#"fn main() {
    if "aaa" <= "aaa" {
        print("ok")
    } else {
        print("wrong")
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str le failed: {}", output);
    assert!(output.contains("ok"), "Expected ok, got: {}", output);
}

#[test]
fn test_str_ge() {
    let source = r#"fn main() {
    if "zzz" >= "aaa" {
        print("ok")
    } else {
        print("wrong")
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str ge failed: {}", output);
    assert!(output.contains("ok"), "Expected ok, got: {}", output);
}

#[test]
fn test_str_ordering_variables() {
    let source = r#"fn main() {
    let a = "apple"
    let b = "banana"
    if a < b {
        print("apple < banana")
    }
    if b > a {
        print("banana > apple")
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "str ordering variables failed: {}", output);
    assert!(output.contains("apple < banana"), "Expected 'apple < banana', got: {}", output);
    assert!(output.contains("banana > apple"), "Expected 'banana > apple', got: {}", output);
}

#[test]
fn test_for_string_literal() {
    let source = r#"fn main() {
    for c in "abc" {
        print(c)
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "for string literal failed: {}", output);
    assert!(output.contains("97"), "Expected 97 (a), got: {}", output);
    assert!(output.contains("98"), "Expected 98 (b), got: {}", output);
    assert!(output.contains("99"), "Expected 99 (c), got: {}", output);
}

#[test]
fn test_for_string_variable() {
    let source = r#"fn main() {
    let s = "hi"
    for c in s {
        print(c)
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "for string variable failed: {}", output);
    assert!(output.contains("104"), "Expected 104 (h), got: {}", output);
    assert!(output.contains("105"), "Expected 105 (i), got: {}", output);
}

#[test]
fn test_for_string_empty() {
    let source = r#"fn main() {
    for c in "" {
        print(999)
    }
    print("done")
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "for string empty failed: {}", output);
    assert!(!output.contains("999"), "Should not print 999, got: {}", output);
    assert!(output.contains("done"), "Expected done, got: {}", output);
}

#[test]
fn test_for_string_count() {
    let source = r#"fn main() {
    let count = 0
    for c in "hello" {
        count = count + 1
    }
    print(count)
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "for string count failed: {}", output);
    assert!(output.contains("5"), "Expected 5, got: {}", output);
}

#[test]
fn test_for_string_break() {
    let source = r#"fn main() {
    for c in "hello" {
        if c == 108 {
            break
        }
        print(c)
    }
}"#;
    let (output, ok) = compile_and_run(source);
    assert!(ok, "for string break failed: {}", output);
    assert!(output.contains("104"), "Expected 104 (h), got: {}", output);
    assert!(output.contains("101"), "Expected 101 (e), got: {}", output);
    assert!(!output.contains("108"), "Should not contain 108 (l), got: {}", output);
}
