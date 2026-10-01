<div align="center">

# 🏖️ Sandbox

### The programming language that checks your **money math at compile time**

<br>

[![CI](https://github.com/mrsandipmandal/SAndbox/actions/workflows/ci.yml/badge.svg)](https://github.com/mrsandipmandal/SAndbox/actions)
[![Release](https://img.shields.io/badge/Release-v0.0.6-green.svg)](https://github.com/mrsandipmandal/SAndbox/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org/)
[![Tests](https://img.shields.io/badge/Tests-235%20integration%20passing-brightgreen.svg)](https://github.com/mrsandipmandal/SAndbox/actions)

<br>

**Sandbox** rejects currency mismatches, unbalanced ledgers, and dimensional
errors before your program ever runs — while staying general-purpose enough
for web servers, backend services, and WebAssembly.

Try it in the browser: **[Live Playground](https://mrsandipmandal.github.io/SAndbox/registry/static/playground/playground.html)** —
compile and run Sandbox in-page, no install needed.

<br>

```
let salary: Money<INR> = 50000 INR
let tax: Money<INR> = 7500 INR
let total = salary + tax     // ✅ same currency — works
let bad = salary + 100 USD   // ❌ Currency mismatch: Money<INR> + Money<USD>
```

<br>

[Quick Start](#-quick-start) • [Examples](#-examples) • [Language Tour](#-language-tour) • [Backends](#️-three-execution-backends) • [Roadmap](#️-roadmap) • [Contributing](#-contributing)

</div>

---

## 🔥 Why Sandbox?

| Problem | Traditional Languages | Sandbox |
|---------|----------------------|---------|
| `Money<INR> + Money<USD>` | Silently compiles, wrong books | **Compile-time error** |
| Ledger that doesn't balance | Found at month-end close | **Rejected before it runs** |
| `100 kg + 5 meter` | Runtime nonsense | **Compile-time dimensional analysis** |
| Server that dies on one request | Crash loops | **HTTP served from the language** |
| Browser deployment | Rewrite in JS | **Compile to WebAssembly (`.wat`/`.wasm`)** |

### ✨ Key Features

- **💰 Type-safe Money** — `Money<INR>`, `Money<USD>` with compile-time currency enforcement (fixed-point, scale 10⁻⁴)
- **📒 Ledger DSL** — double-entry accounting; an unbalanced ledger is a compile-time error
- **📏 Unit System** — physical units with compile-time dimensional analysis (`100 kg`, `5 meter`, `meter·meter` areas)
- **🌐 WebAssembly** — first-class `.wat`/`.wasm` codegen target; run the same program in the browser
- **🖥️ HTTP & JSON built in** — `http::get/post/put/patch/delete`, request servers (`http::serve`, `http::serve_once`)
- **⚡ Three backends** — C (via GCC), tree-walking interpreter, LLVM IR (`sandbox llvm-build`)
- **❌ Error Handling** — `Result<T, E>`, `Ok()`/`Err()`, and the `?` operator
- **🧩 Enums & Pattern Matching** — algebraic data types with `match` and wildcard arms
- **🔧 IDE Support** — LSP server with diagnostics, completion, and hover
- **📦 Package Manager + Registry** — `sandbox add/install/vendor/tree`, signed packages (ed25519), dependency resolution
- **🎨 Formatter** — `sandbox fmt` / `sandbox fmt --check`
- **🔁 REPL** — incremental evaluation with tab completion and history search
- **📝 f-strings** — `f"hello {name}, {a + b}"`

---

## 🚀 Quick Start

### Install

**Prebuilt binaries** (Linux x86_64/aarch64, macOS x86_64/aarch64, Windows x86_64) from
[Releases](https://github.com/mrsandipmandal/SAndbox/releases) — every archive is
published with a `SHA256SUMS.txt` you can verify with `sha256sum -c`.

**From source:**

```bash
git clone https://github.com/mrsandipmandal/SAndbox.git
cd SAndbox
cargo build --release
cargo install --path .
```

### Your First Program

```bash
cat > hello.sbx << 'EOF'
fn main() {
    print("Hello, Sandbox! 🏖️")
}
EOF

sandbox run hello.sbx
# Hello, Sandbox! 🏖️
```

### Initialize a Project

```bash
sandbox init my-bank-app
cd my-bank-app
sandbox run main.sbx
# Hello, my-bank-app!
```

`sandbox init` scaffolds `main.sbx`, `sandbox.toml`, and a `src/` directory.

---

## 📚 Examples

### 💰 Money — compile-time currency safety

```sbx
fn main() {
    let salary: Money<INR> = 50000 INR
    let tax: Money<INR> = 7500 INR
    let total = salary + tax
    print(total)  // 575000000 — Money is fixed-point at scale 10^-4
}
```

Add the wrong currency and it never compiles:

```
Error: Currency mismatch: Money<INR> + Money<USD>
```

### 🧾 Ledger — balance enforced at compile time

```sbx
ledger Transfer {
    debit  checking: 10000
    credit savings: 10000
}
```

A ledger whose debits don't equal its credits is rejected before the program
runs — see [examples/ledger_demo.sbx](examples/ledger_demo.sbx).

### 📝 f-strings and interpolation

```sbx
fn main() {
    let name = "Sandbox"
    print(f"hello {name}, {1 + 2}")
    // hello Sandbox, 3
}
```

### 🔄 Fibonacci — recursion

```sbx
fn fib(n: i64) -> i64 {
    if n <= 1 {
        return n
    }
    return fib(n - 1) + fib(n - 2)
}

fn main() {
    for i in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9] {
        print(fib(i))
    }
}
```

### 🏗️ Structs — data modeling

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

### ❌ Error Handling — Result and `?`

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

### 🧩 Enums & Pattern Matching

```sbx
enum Color { Red, Green, Blue }

fn code(c: Color) -> i64 {
    match c {
        Color::Red => 1,
        Color::Green => 2,
        Color::Blue => 3,
        _ => 0,
    }
}

fn main() {
    print(code(Color::Green))  // 2
}
```

### 📦 Modules

```sbx
mod math {
    fn add(a: i64, b: i64) -> i64 {
        return a + b
    }
}

fn main() {
    print(math::add(3, 4))  // 7
}
```

---

## 📚 Language Tour

### Types

```sbx
let x: i64 = 42                   // 64-bit integer
let pi: f64 = 3.14                // 64-bit float
let active: bool = true           // boolean
let name: string = "Sandbox"      // string
let price: Money<INR> = 100 INR   // fixed-point money
let d: Decimal = 42               // exact decimal (experimental — see roadmap)
```

### Control Flow

```sbx
if x > 5 {
    print("big")
} else {
    print("small")
}

let mut i: i64 = 0
while i < 10 {
    i = i + 1
}

for item in [1, 2, 3] {
    print(item)
}
```

### Standard Library

```sbx
let root = math::sqrt(25.0)                    // 5
let bigger = math::max(10.5, 20.3)             // 20.3
let full = string::concat("Hello", "World")    // HelloWorld
let n = string::length("Sandbox")              // 7

let user = json::parse_object("{\"name\":\"Alice\",\"age\":30}")
print(json::map_get(user, "name"))             // Alice

let resp = http::get("http://example.com")
print(http::status_code(resp))                 // 200
```

### HTTP Servers — in the language

```sbx
// Handles one request per connection, then returns the response.
fn handler(path: string) -> string {
    return f"path: {path}"
}

http::serve(8080, "handler", 0)  // listens forever
// http::serve_once(8080, "handler", 0) — one request, then exit
```

Live examples: [examples/http_server_demo.sbx](examples/http_server_demo.sbx)
(headers, forms, cookies, templating, 404s) and
[examples/web_app.sbx](examples/web_app.sbx) (a complete web app with
routing, cookies, and HTML escaping).

### Units

```sbx
let weight: kg = 100 kg
let half = weight / 2            // 50 kg
let area = 5 meter * 3 meter     // 15 meter·meter
```

---

## ⚙️ Three Execution Backends

The same program runs through three independent backends — and the test
suite checks they agree:

| Backend | Command | Notes |
|---------|---------|-------|
| **C** (default) | `sandbox run app.sbx` | AST → C → GCC native binary |
| **Interpreter** | `sandbox interpret app.sbx` | No compilation, direct execution |
| **LLVM** | `sandbox llvm app.sbx` → `sandbox llvm-build app.sbx` | AST → LLVM IR → clang native binary |
| **WebAssembly** | `sandbox build app.sbx --target wasm` | `.wat` + `.wasm` for browser/edge |

Binary parity (C vs LLVM vs interpreter vs wasm) is enforced continuously by
[tests/parity.rs](tests/parity.rs) in CI.

---

## 🛠️ CLI Commands

```
sandbox run file.sbx            Compile and run
sandbox build file.sbx          Build native binary (-o to name it, --target wasm)
sandbox check file.sbx          Type-check only
sandbox interpret file.sbx      Run through the interpreter
sandbox test file.sbx           Run tests defined in a .sbx file
sandbox fmt file.sbx            Format (--check to only verify)
sandbox doc file.sbx            Generate documentation
sandbox wasm file.sbx           Emit WebAssembly text (.wat)
sandbox llvm file.sbx           Emit LLVM IR (.ll)
sandbox llvm-build file.sbx     Build native binary via LLVM/clang
sandbox init myproject          Scaffold a new project
sandbox add <pkg>               Add a dependency (supports --version)
sandbox install                 Install dependencies from sandbox.toml
sandbox vendor                  Vendor dependencies into .sandbox/vendor/
sandbox tree                    Show the dependency tree
sandbox pkg <subcommand>        Package registry operations
sandbox lsp                     Start the LSP server (IDE support)
sandbox repl                    Interactive REPL
```

---

## 🧪 Testing & CI

```bash
cargo test                       # 235 integration tests
cargo clippy                     # lint (zero warnings enforced)
cargo fmt --check                # formatting
bash scripts/ci-local.sh         # full 16-gate CI suite, locally
```

CI runs workflow lint, build, tests, backend **parity**, wasm integration
(through wat2wasm + Node), registry build/tests, playground-compiler unit
tests, binary smoke, and encoder parity — the same gates
[scripts/ci-local.sh](scripts/ci-local.sh) runs locally.

---

## 📁 Project Structure

```
src/
├── main.rs           # CLI entry point (clap)
├── token.rs          # Token definitions
├── lexer.rs          # Source → Tokens
├── ast.rs            # AST node definitions
├── parser.rs         # Tokens → AST (recursive descent)
├── typechecker.rs    # Type checking + currency/unit/ledger validation
├── codegen.rs        # AST → C code
├── interpreter.rs    # Tree-walking interpreter
├── llvmgen.rs        # AST → LLVM IR
├── wasmgen.rs        # AST → WebAssembly (.wat)
├── stdlib.rs         # C runtime + stdlib (math, string, array, json, http)
├── fmt.rs            # Formatter
├── lsp.rs            # Language Server Protocol server
├── repl.rs           # Interactive REPL
├── registry_client.rs # Package registry client
├── diagnostic.rs     # Error diagnostics
├── b2.rs             # Semantics workstream B2 helpers
└── compiler.rs       # Pipeline orchestration

examples/             # 20 runnable examples (hello, fibonacci, money, ledger,
                      #   database, http_server_demo, web_app, wasm_demo, …)

tests/
├── integration.rs    # 235 end-to-end tests
└── parity.rs         # Backend agreement checks

playground-compiler/  # The compiler built to wasm32 for the in-browser playground
registry/             # Self-hostable package registry server + playground assets
scripts/ci-local.sh   # The entire CI suite, runnable locally
```

---

## 🗺️ Roadmap

### ✅ Shipped (verified by the test suite)

- [x] Core types, functions, structs, control flow, type inference
- [x] `Money<CUR>` with compile-time currency enforcement
- [x] Ledger DSL with compile-time balance validation
- [x] Unit system with dimensional analysis
- [x] `Result<T, E>` + `?` operator, enums + `match` with string/float/
      payload arms (C, LLVM, and interpreter backends), modules, f-strings
- [x] `math` / `string` / `array` / `json` / `http` standard library
- [x] Three backends (C, interpreter, LLVM) + WebAssembly target, with parity tests
- [x] Package manager + self-hostable registry (ed25519 signing, vendoring)
- [x] Formatter, LSP, REPL, `sandbox doc`
- [x] In-browser playground (compiler compiled to wasm, runs on the page)

### 🚧 Known Limitations

Honest list — each of these reproduces at HEAD:

- **Index assignment (`arr[i] = x`) is not yet supported** — see
  `examples/sorting.sbx`.
- **`print` on Money/Decimal shows the raw scaled integer** (50000 INR →
  `500000000`); formatted money output is on the roadmap.
- **`Decimal` is experimental**: fractional decimal literals don't survive
  codegen yet on any backend.

### 🔜 Next

- Formatted money/decimal output (`57500.0000` instead of scaled integers)
- Index assignment and full slice support
- Stabilize `Decimal` across all four backends
- Grow the standard library and registry package ecosystem

---

## 🤝 Contributing

We love contributions! Whether it's:

- 🐛 **Bug reports** — Found an issue? Open one!
- 💡 **Feature ideas** — Have a suggestion? Share it!
- 📝 **Documentation** — Help others learn
- 🧪 **Tests** — Improve coverage
- 🔧 **Code** — Fix bugs or add features

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines. A great first
contribution: pick anything from **Known Limitations** above — each one has
a reproducing example checked into the repo.

---

## 📄 License

This project is licensed under the MIT License — see [LICENSE](LICENSE) for details.

---

## 🙏 Acknowledgments

- Inspired by Rust's type discipline, and by every postmortem that started with a currency bug
- Built with [Rust](https://www.rust-lang.org/), [GCC](https://gcc.gnu.org/), and [LLVM](https://llvm.org/)
- Thanks to all [contributors](https://github.com/mrsandipmandal/SAndbox/graphs/contributors)

---

<div align="center">

**⭐ Star this repo if you find Sandbox interesting!**

[![Star History Chart](https://api.star-history.com/svg?repos=mrsandipmandal/SAndbox&type=Date)](https://star-history.com/#mrsandipmandal/SAndbox&Date)

</div>
