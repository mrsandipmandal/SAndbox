<div align="center">

# 🏖️ Sandbox

### The programming language where **financial correctness** is a language feature

<br>

[![CI](https://github.com/mrsandipmandal/SAndbox/actions/workflows/ci.yml/badge.svg)](https://github.com/mrsandipmandal/SAndbox/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org/)
[![Release](https://img.shields.io/badge/Release-v0.4.0-green.svg)](https://github.com/mrsandipmandal/SAndbox/releases)
[![Tests](https://img.shields.io/badge/Tests-31%20passing-brightgreen.svg)](https://github.com/mrsandipmandal/SAndbox/actions)

<br>

**Sandbox** catches currency mismatches and decimal rounding errors at compile time,
while remaining general-purpose enough for web, backend, desktop, and embedded apps.

<br>

```
let salary: Money<INR> = 50000 INR
let tax: Money<INR> = 7500 INR
let total = salary + tax     // ✅ Same currency — works
let bad = salary + 100 USD   // ❌ Compile error: Currency mismatch!
```

<br>

[Getting Started](#-quick-start) • [Examples](#-examples) • [Language Tour](#-language-tour) • [Contributing](#-contributing) • [Roadmap](#-roadmap)

</div>

---

## 🔥 Why Sandbox?

| Problem | Traditional Languages | Sandbox |
|---------|----------------------|---------|
| `Money + Wrong Currency` | Runtime bug, lost money | **Compile-time error** |
| `0.1 + 0.2` | `0.30000000000000004` | **Exactly `0.30`** |
| `float` for prices | Rounding errors | **Exact Decimal type** |
| Thread safety | Data races | **Memory safe by default** |

### ✨ Key Features

- **💰 Type-safe Money** — `Money<INR>`, `Money<USD>` with compile-time currency enforcement
- **🧮 Exact Decimal** — No floating-point rounding errors in financial calculations
- **📏 Unit System** — Physical units with compile-time dimensional analysis (`100 kg`, `5 meter`)
- **🔒 Memory Safe** — Safe by default, `unsafe` only when you opt in
- **⚡ C Transpilation** — Compiles to C, then to native binary via GCC
- **🌐 WebAssembly** — Generate .wat text format for browser/edge deployment
- **📝 Simple Syntax** — Clean, readable, low-ceremony
- **🏗️ Structs & Functions** — First-class support with type annotations
- **🔄 Control Flow** — `if/else`, `while`, `for...in` loops
- **🎯 Type Inference** — `let x = 5` infers `i64` automatically
- **❌ Error Handling** — `Result<T, E>`, `Ok()`, `Err()`, `?` operator
- **📦 Modules** — `mod name { ... }` for code organization
- **📚 Standard Library** — Built-in `math`, `string`, `array` modules
- **📦 Package Manager** — `sandbox add`, `sandbox install`, `sandbox tree`
- **🎨 Formatter** — `sandbox fmt` for consistent code style

---

## 🚀 Quick Start

### Install

```bash
git clone https://github.com/mrsandipmandal/SAndbox.git
cd SAndbox
cargo build --release
cargo install --path .
```

### Your First Program

```bash
# Create hello.sbx
cat > hello.sbx << 'EOF'
fn main() {
    print("Hello, Sandbox! 🏖️")
}
EOF

# Run it
sandbox run hello.sbx
# Output: Hello, Sandbox! 🏖️
```

### Initialize a Project

```bash
sandbox init my-bank-app
cd my-bank-app
sandbox run main.sbx
```

---

## 📚 Examples

### 💰 Money Type — Compile-time Currency Safety

```sbx
fn main() {
    let salary: Money<INR> = 50000 INR
    let tax: Money<INR> = 7500 INR
    let total = salary + tax
    print(total)  // 57500.0000
}
```

**What happens if you try to add different currencies?**

```sbx
let salary: Money<INR> = 50000 INR
let usd: Money<USD> = 100 USD
let bad = salary + usd  // ❌ Compile error: Currency mismatch: Money<INR> + Money<USD>
```

### 🔄 Fibonacci — Recursion

```sbx
fn fib(n: i64) -> i64 {
    if n <= 1 {
        return n
    }
    let a = fib(n - 1)
    let b = fib(n - 2)
    return a + b
}

fn main() {
    for i in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9] {
        print(fib(i))
    }
}
```

### 🏗️ Structs — Data Modeling

```sbx
struct Account {
    id: i64,
    name: string,
    balance: Money<INR>,
}

fn main() {
    let acc = Account {
        id: 1,
        name: "Alice",
        balance: 10000 INR,
    }
    print(acc.name)      // Alice
    print(acc.balance)   // 10000.0000
}
```

### ❌ Error Handling — Result Type

```sbx
fn divide(a: i64, b: i64) -> Result<i64, string> {
    if b == 0 {
        return Err("Division by zero")
    }
    return Ok(a / b)
}

fn main() {
    let result = divide(10, 2)
    print(result)  // 5
}
```

### 📦 Modules — Code Organization

```sbx
mod math {
    fn add(a: i64, b: i64) -> i64 {
        return a + b
    }

    fn multiply(a: i64, b: i64) -> i64 {
        return a * b
    }
}

fn main() {
    print(math::add(3, 4))       // 7
    print(math::multiply(3, 4))  // 12
}
```

---

## 🗺️ Language Tour

### Types

```sbx
let x: i64 = 42              // 64-bit integer
let pi: f64 = 3.14           // 64-bit float
let active: bool = true      // boolean
let name: string = "Sandbox" // string
let price: Money<INR> = 100 INR  // money
```

### Functions

```sbx
fn add(a: i64, b: i64) -> i64 {
    return a + b
}
```

### Control Flow

```sbx
// If/else
if x > 5 {
    print("big")
} else {
    print("small")
}

// While loop
let mut i: i64 = 0
while i < 10 {
    print(i)
    i = i + 1
}

// For loop
for item in [1, 2, 3] {
    print(item)
}
```

### Error Handling

```sbx
fn parse_age(input: string) -> Result<i64, string> {
    // ... parse logic
    return Ok(25)
}

// Use ? to propagate errors
fn get_age() -> Result<i64, string> {
    let age = parse_age("25")?
    return Ok(age)
}
```

### Standard Library

```sbx
// Math functions
let root = math::sqrt(25.0)    // 5.0
let m = math::max(10.5, 20.3)  // 20.3
let p = math::pow(2.0, 10.0)  // 1024.0

// String functions
let full = string::concat("Hello", "World")  // "HelloWorld"
let len = string::length("Sandbox")          // 7
let eq = string::equals("abc", "abc")        // true
```

### Unit System

```sbx
let weight: kg = 100 kg
let half = weight / 2           // 50 kg
let distance: meter = 500 meter
let time: second = 10 second
let area = 5 meter * 3 meter    // 15 meter·meter
```

### WebAssembly Target

```bash
sandbox build file.sbx --target wasm   # Generate .wat + .wasm
sandbox wasm file.sbx -o output.wat    # Generate .wat only
```

### Package Management

```bash
sandbox init myproject        # Create project with sandbox.toml
sandbox add serde --version ^1.0  # Add dependency
sandbox install               # Install all dependencies
sandbox tree                  # Show dependency tree
```

---

## 🛠️ CLI Commands

```bash
sandbox init myproject        # Initialize new project
sandbox run file.sbx          # Compile and run
sandbox build file.sbx        # Build native binary
sandbox build file.sbx -o myapp  # Build with custom name
sandbox build file.sbx --target wasm  # Build to WebAssembly
sandbox check file.sbx        # Type-check only
sandbox fmt file.sbx          # Format source file
sandbox fmt --check file.sbx  # Check formatting
sandbox add <pkg>             # Add dependency
sandbox install               # Install dependencies
sandbox tree                  # Show dependency tree
sandbox wasm file.sbx         # Generate .wat file
```

---

## 🧪 Testing

```bash
cargo test                   # Run all 31 tests
cargo clippy                 # Lint (zero warnings)
cargo fmt --check            # Check formatting
```

---

## 📁 Project Structure

```
src/
├── main.rs           # CLI entry point (clap)
├── token.rs          # Token definitions
├── lexer.rs          # Source → Tokens
├── ast.rs            # AST node definitions
├── parser.rs         # Tokens → AST (recursive descent)
├── typechecker.rs    # Type checking + stdlib registration
├── codegen.rs        # AST → C code
├── stdlib.rs         # Standard library (math, string, array)
├── wasmgen.rs        # WebAssembly .wat codegen
└── compiler.rs       # Pipeline orchestration

examples/
├── hello.sbx         # Hello World
├── fibonacci.sbx     # Recursive fibonacci
├── struct_demo.sbx   # Structs + functions
├── money.sbx         # Money type demo
├── math_demo.sbx     # Math stdlib functions
├── string_demo.sbx   # String stdlib functions
├── sorting_v3.sbx    # Array operations
├── units_demo.sbx    # Unit system demo
└── wasm_demo.sbx     # WebAssembly target demo

tests/
└── integration.rs    # 31 end-to-end tests
```

---

## 🗺️ Roadmap

### v0.1 ✅ — Foundation
- [x] Core types: `i64`, `f64`, `bool`, `string`
- [x] Money type with currency safety
- [x] Functions, structs, control flow
- [x] C transpilation backend
- [x] CLI toolchain

### v0.2 ✅ — Error Handling
- [x] Error handling (`Result`, `?` operator)
- [x] Module system (`mod`, `::`)
- [x] Project init (`sandbox init`)

### v0.3 ✅ — Standard Library & Package Manager
- [x] Standard library: `math`, `string`, `array` modules
- [x] Package manager: `sandbox add`, `sandbox install`, `sandbox tree`
- [x] Formatter: `sandbox fmt`, `sandbox fmt --check`
- [x] String concatenation with `+` operator
- [ ] LLVM backend for native optimization

### v0.4 ✅ — Units, Decimal & WebAssembly
- [x] Unit system with compile-time dimensional analysis
- [x] Decimal type with exact i128 arithmetic
- [x] WebAssembly .wat codegen backend
- [ ] HTTP / JSON standard library
- [ ] LLVM backend for native optimization

### v1.0 — Production Ready
- [ ] Self-hosting compiler
- [ ] Database integration
- [ ] Ledger / double-entry accounting
- [ ] IDE support (LSP)
- [ ] Package registry (`registry.sandbox.dev`)

---

## 🤝 Contributing

We love contributions! Whether it's:

- 🐛 **Bug reports** — Found an issue? Open one!
- 💡 **Feature ideas** — Have a suggestion? Share it!
- 📝 **Documentation** — Help others learn
- 🧪 **Tests** — Improve coverage
- 🔧 **Code** — Fix bugs or add features

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

### Good First Issues

Looking for a place to start? Check out our [Good First Issues](https://github.com/mrsandipmandal/SAndbox/labels/good%20first%20issue)!

---

## 📄 License

This project is licensed under the MIT License — see [LICENSE](LICENSE) for details.

---

## 🙏 Acknowledgments

- Inspired by Rust's memory safety, Go's simplicity, and the need for financial correctness
- Built with [Rust](https://www.rust-lang.org/) and [GCC](https://gcc.gnu.org/)
- Thanks to all [contributors](https://github.com/mrsandipmandal/SAndbox/graphs/contributors)

---

<div align="center">

**⭐ Star this repo if you find Sandbox interesting!**

[![Star History Chart](https://api.star-history.com/svg?repos=mrsandipmandal/SAndbox&type=Date)](https://star-history.com/#mrsandipmandal/SAndbox&Date)

</div>
