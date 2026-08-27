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
