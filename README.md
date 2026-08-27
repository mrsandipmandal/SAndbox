<div align="center">

# 🏖️ Sandbox

**A memory-safe, financially-safe, general-purpose programming language**

[![CI](https://github.com/sandbox-lang/sandbox/actions/workflows/ci.yml/badge.svg)](https://github.com/sandbox-lang/sandbox/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org/)
[![Release](https://img.shields.io/github/v/release/sandbox-lang/sandbox)](https://github.com/sandbox-lang/sandbox/releases)

</div>

---

Sandbox is a systems programming language designed with **financial correctness as a language feature**. It catches currency mismatches and decimal rounding errors at compile time, while remaining general-purpose enough for web, backend, desktop, and embedded applications.

## ✨ Features

- **Type-safe Money** — `Money<INR>`, `Money<USD>` with compile-time currency enforcement
- **Exact Decimal** — No floating-point rounding errors in financial calculations
- **Memory Safe** — Safe by default, `unsafe` only when you opt in
- **C Transpilation** — Compiles to C, then to native binary via GCC
- **Simple Syntax** — Clean, readable, low-ceremony
- **Structs & Functions** — First-class support with type annotations
- **Control Flow** — `if/else`, `while`, `for...in` loops
- **Type Inference** — `let x = 5` infers `i64` automatically
- **Error Handling** — `Result<T, E>`, `Ok()`, `Err()`, `?` operator
- **Modules** — `mod name { ... }` for code organization
- **Project Init** — `sandbox init` to scaffold new projects

## 🚀 Quick Start

### Install

```bash
# From source
git clone https://github.com/sandbox-lang/sandbox.git
cd sandbox
cargo build --release
cargo install --path .
```

### Hello World

```bash
# Create hello.sbx
cat > hello.sbx << 'EOF'
fn main() {
    print("Hello, Sandbox!")
}
EOF

# Run it
sandbox run hello.sbx
```

### Money Type

```bash
cat > money.sbx << 'EOF'
fn main() {
    let salary: Money<INR> = 50000 INR
    let tax: Money<INR> = 7500 INR
    let total = salary + tax
    print(total)
}
EOF

sandbox run money.sbx
```

## 📖 Language Syntax

### Types

```sbx
let x: i64 = 42           // 64-bit integer
let pi: f64 = 3.14        // 64-bit float
let active: bool = true    // boolean
let name: string = "Sandbox" // string
```

### Money (Financial Types)

```sbx
let salary: Money<INR> = 50000 INR
let tax: Money<INR> = 7500 INR

// ✅ Same currency — works
let total = salary + tax

// ❌ Different currencies — compile error
// let bad = salary + 100 USD
```

### Functions

```sbx
fn add(a: i64, b: i64) -> i64 {
    return a + b
}

fn calculate_tax(income: Money<INR>) -> Money<INR> {
    return income * 0.30
}
```

### Structs

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
    print(acc.name)
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

## 🛠️ CLI Commands

```bash
sandbox init myproject     # Initialize new project
sandbox run file.sbx       # Compile and run
sandbox build file.sbx     # Build native binary
sandbox build file.sbx -o myapp  # Build with custom name
sandbox check file.sbx     # Type-check only (no compilation)
```

## 🧪 Testing

```bash
cargo test                 # Run all tests
cargo clippy               # Lint
cargo fmt --check          # Check formatting
```

## 📁 Project Structure

```
src/
├── main.rs           # CLI entry point
├── token.rs          # Token definitions
├── lexer.rs          # Source → Tokens
├── ast.rs            # AST node definitions
├── parser.rs         # Tokens → AST
├── typechecker.rs    # Type checking + currency validation
├── codegen.rs        # AST → C code
└── compiler.rs       # Pipeline orchestration
examples/
├── hello.sbx         # Hello World
├── fibonacci.sbx     # Recursive fibonacci
├── struct_demo.sbx   # Structs + functions
└── money.sbx         # Money type demo
tests/
└── integration.rs    # 16 end-to-end tests
```

## 🗺️ Roadmap

### v0.1 ✅
- [x] Core types: `i64`, `f64`, `bool`, `string`
- [x] Money type with currency safety
- [x] Functions, structs, control flow
- [x] C transpilation backend
- [x] CLI toolchain

### v0.2 ✅
- [x] Error handling (`Result`, `?` operator)
- [x] Modules and imports
- [x] Project init (`sandbox init`)

### v0.3
- [ ] Package manager (`sandbox add`)
- [ ] Standard library (math, http, json)
- [ ] LLVM backend for native optimization
- [ ] WebAssembly target
- [ ] Decimal type with exact arithmetic
- [ ] Unit system (`10 kg`, `5 meter`)

### v0.4
- [ ] LLVM backend for native optimization
- [ ] WebAssembly target
- [ ] Decimal type with exact arithmetic
- [ ] Unit system (`10 kg`, `5 meter`)

### v1.0
- [ ] Self-hosting compiler
- [ ] WebAssembly frontend
- [ ] Database integration
- [ ] Ledger / double-entry accounting
- [ ] IDE support (LSP)
- [ ] Self-hosting compiler
- [ ] WebAssembly frontend
- [ ] Database integration
- [ ] Ledger / double-entry accounting
- [ ] IDE support (LSP)

## 🤝 Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## 📄 License

This project is licensed under the MIT License — see [LICENSE](LICENSE) for details.

## 🙏 Acknowledgments

- Inspired by Rust's memory safety, Go's simplicity, and the need for financial correctness in programming languages
- Built with [Rust](https://www.rust-lang.org/) and [GCC](https://gcc.gnu.org/)
