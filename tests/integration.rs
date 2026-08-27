use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn compile_and_run(source: &str) -> (String, bool) {
    let tmp = TempDir::new().unwrap();
    let sbx_path = tmp.path().join("test.sbx");
    fs::write(&sbx_path, source).unwrap();

    let output = Command::new("cargo")
        .args(["run", "--", "run", sbx_path.to_str().unwrap()])
        .output()
        .unwrap();

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

#[test]
fn test_init_project() {
    let tmp = TempDir::new().unwrap();
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "init",
            tmp.path().join("myproject").to_str().unwrap(),
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(output.status.success(), "Init failed: {}", stdout);
    assert!(
        tmp.path().join("myproject/sandbox.toml").exists(),
        "sandbox.toml not created"
    );
    assert!(
        tmp.path().join("myproject/main.sbx").exists(),
        "main.sbx not created"
    );
}
