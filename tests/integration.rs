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
        .args(["llvm", sbx_path.to_str().unwrap(), "-o", ll_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success(), "llvm codegen failed: {}", String::from_utf8_lossy(&output.stderr));
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
        .args(["llvm-build", sbx_path.to_str().unwrap(), "-o", bin_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(output.status.success(), "llvm-build failed: {}", String::from_utf8_lossy(&output.stderr));
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
    assert!(output.contains("\"hello\""), "stringify_string failed: {}", output);
    assert!(output.contains("true"), "stringify_bool true failed: {}", output);
    assert!(output.contains("false"), "stringify_bool false failed: {}", output);
    assert!(output.contains("3.141"), "parse_float failed: {}", output);
    assert!(output.contains("Alice"), "parse_string failed: {}", output);
    // has_key returns 1 for true, 0 for false
    assert!(output.contains("\n1\n") || output.contains("1\n"), "has_key present failed: {}", output);
    assert!(output.contains("\n0\n") || output.contains("\n0"), "has_key absent failed: {}", output);
    // array_len
    assert!(output.contains("\n3\n"), "array_len [10,20,30] failed: {}", output);
    assert!(output.contains("\n0\n"), "array_len [] failed: {}", output);
    assert!(output.contains("\n1\n"), "array_len [42] failed: {}", output);
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
    assert!(stdout.contains("1\n"), "Expected count 1 after delete: {}", stdout);
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
    assert!(output.contains("application/json"), "Missing Content-Type: {}", output);
    assert!(output.contains("sandbox123"), "Missing X-Custom: {}", output);
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
        if let Ok(mut s) =
            std::net::TcpStream::connect_timeout(&"127.0.0.1:8080".parse().unwrap(), Duration::from_millis(500))
        {
            let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = s.write_all(b"GET /hello HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
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
    assert!(resp1.contains("\"/hello\""), "First request failed: {}", resp1);

    // Make second request (proves it handles multiple)
    let resp2 = http_get("127.0.0.1:8077", "/world");
    assert!(resp2.contains("\"/world\""), "Second request failed: {}", resp2);

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
    let req = format!("GET {} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n", path);
    s.write_all(req.as_bytes()).unwrap();
    let mut buf = Vec::new();
    s.read_to_end(&mut buf).unwrap();
    String::from_utf8_lossy(&buf).to_string()
}

#[test]
fn test_llvm_backend_hello() {
    let out = llvm_build_and_run(r#"
fn main() {
    print("Hello LLVM!")
}
"#);
    assert_eq!(out.trim(), "Hello LLVM!");
}

#[test]
fn test_llvm_backend_arithmetic() {
    let out = llvm_build_and_run(r#"
fn add(a: i64, b: i64) -> i64 {
    return a + b
}

fn main() {
    let result = add(10, 20)
    print(result)
}
"#);
    assert_eq!(out.trim(), "30");
}

#[test]
fn test_llvm_backend_enums() {
    let ir = compile_to_llvm(r#"
enum Color {
    Red,
    Green,
    Blue
}

fn main() {
    let c = Color::Red
    print(c)
}
"#);
    assert!(ir.contains("@.tag.Color.Red = constant i64 0"), "Red should be tag 0");
    assert!(ir.contains("@.tag.Color.Green = constant i64 1"), "Green should be tag 1");
    assert!(ir.contains("@.tag.Color.Blue = constant i64 2"), "Blue should be tag 2");
}

#[test]
fn test_llvm_backend_if_else() {
    let out = llvm_build_and_run(r#"
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
"#);
    assert_eq!(out.trim(), "5");
}

#[test]
fn test_llvm_backend_while_loop() {
    let out = llvm_build_and_run(r#"
fn main() {
    let i = 0
    let sum = 0
    while i < 5 {
        sum = sum + i
        i = i + 1
    }
    print(sum)
}
"#);
    assert_eq!(out.trim(), "10");
}

#[test]
fn test_llvm_backend_struct() {
    let out = llvm_build_and_run(r#"
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
"#);
    assert_eq!(out.trim(), "25");
}

#[test]
fn test_llvm_backend_struct_fields() {
    let out = llvm_build_and_run(r#"
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
"#);
    assert_eq!(out.trim(), "15");
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
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
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
    assert!(out.contains("21"), "should evaluate triple(7)=21, got: {}", out);
    assert!(out.contains("defined"), "should confirm definition, got: {}", out);
}

#[test]
fn test_repl_multiline_function() {
    let out = run_repl("fn fib(n: i64) -> i64 {\n    if n <= 1 {\n        return n\n    } else {\n        return fib(n - 1) + fib(n - 2)\n    }\n}\nfib(10)\n:q\n");
    assert!(out.contains("55"), "should evaluate fib(10)=55, got: {}", out);
}

#[test]
fn test_repl_print_statement() {
    let out = run_repl("print(42)\n:q\n");
    assert!(out.contains("42"), "should print 42, got: {}", out);
}

#[test]
fn test_repl_reset_command() {
    let out = run_repl("10\n:reset\n20\n:q\n");
    assert!(out.contains("Reset"), "should show reset confirmation, got: {}", out);
}


#[test]
fn test_lambda_basic() {
    let (output, ok) = compile_and_run(r#"
fn apply(f: Fn(i64) -> i64, x: i64) -> i64 {
    return f(x)
}

fn main() {
    let dbl = |x: i64| -> i64 { return x * 2 }
    print(apply(dbl, 5))
}
"#);
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("10"), "Expected 10, got: {}", output);
}

#[test]
fn test_lambda_inline() {
    let (output, ok) = compile_and_run(r#"
fn apply(f: Fn(i64) -> i64, x: i64) -> i64 {
    return f(x)
}

fn main() {
    print(apply(|x: i64| -> i64 { return x * x }, 7))
}
"#);
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("49"), "Expected 49, got: {}", output);
}

#[test]
fn test_lambda_multiple() {
    let (output, ok) = compile_and_run(r#"
fn apply(f: Fn(i64) -> i64, x: i64) -> i64 {
    return f(x)
}

fn main() {
    let add_one = |x: i64| -> i64 { return x + 1 }
    let times_two = |x: i64| -> i64 { return x * 2 }
    print(apply(add_one, 5))
    print(apply(times_two, 5))
}
"#);
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("6"), "Expected 6, got: {}", output);
    assert!(output.trim().contains("10"), "Expected 10, got: {}", output);
}

// ── v2.0: Range, FString, use, function reference tests ──

#[test]
fn test_range_for_loop() {
    let (output, ok) = compile_and_run(r#"
fn main() {
    let mut sum: i64 = 0
    for i in 0..5 {
        sum = sum + i
    }
    print(sum)
}
"#);
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("10"), "Expected 10 (0+1+2+3+4), got: {}", output);
}

#[test]
fn test_range_inclusive() {
    let (output, ok) = compile_and_run(r#"
fn main() {
    let mut sum: i64 = 0
    for i in 0..=5 {
        sum = sum + i
    }
    print(sum)
}
"#);
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("15"), "Expected 15 (0+1+2+3+4+5), got: {}", output);
}

#[test]
fn test_range_expression() {
    let (output, ok) = compile_and_run(r#"
fn main() {
    for i in 3..7 {
        print(i)
    }
}
"#);
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("3"), "Expected 3, got: {}", output);
    assert!(output.contains("4"), "Expected 4, got: {}", output);
    assert!(output.contains("5"), "Expected 5, got: {}", output);
    assert!(output.contains("6"), "Expected 6, got: {}", output);
    assert!(!output.contains("7"), "Should not contain 7 (exclusive), got: {}", output);
}

#[test]
fn test_fstring_simple() {
    let (output, ok) = compile_and_run(r#"
fn main() {
    let name = "World"
    let greeting = f"Hello, {name}!"
    print(greeting)
}
"#);
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("Hello, World!"), "Expected 'Hello, World!', got: {}", output);
}

#[test]
fn test_fstring_expression() {
    let (output, ok) = compile_and_run(r#"
fn main() {
    let msg = f"1 + 2 = {1 + 2}"
    print(msg)
}
"#);
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("1 + 2 = 3"), "Expected '1 + 2 = 3', got: {}", output);
}

#[test]
fn test_fstring_multiple_interpolations() {
    let (output, ok) = compile_and_run(r#"
fn main() {
    let a = 10
    let b = 20
    let msg = f"{a} + {b} = {10 + 20}"
    print(msg)
}
"#);
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.contains("10 + 20 = 30"), "Expected '10 + 20 = 30', got: {}", output);
}

#[test]
fn test_use_single_import() {
    let (output, ok) = compile_and_run(r#"
mod math {
    fn add(a: i64, b: i64) -> i64 {
        return a + b
    }
}

use math::add;

fn main() {
    print(add(3, 4))
}
"#);
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("7"), "Expected 7, got: {}", output);
}

#[test]
fn test_use_wildcard() {
    let (output, ok) = compile_and_run(r#"
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
"#);
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("10"), "Expected 10, got: {}", output);
    assert!(output.trim().contains("15"), "Expected 15, got: {}", output);
}

#[test]
fn test_function_reference() {
    let (output, ok) = compile_and_run(r#"
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
"#);
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("7"), "Expected 7, got: {}", output);
}

#[test]
fn test_function_as_callback() {
    let (output, ok) = compile_and_run(r#"
fn double(x: i64) -> i64 {
    return x * 2
}

fn apply(f: Fn(i64) -> i64, x: i64) -> i64 {
    return f(x)
}

fn main() {
    print(apply(double, 7))
}
"#);
    assert!(ok, "Compilation failed: {}", output);
    assert!(output.trim().contains("14"), "Expected 14, got: {}", output);
}

