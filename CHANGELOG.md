# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.4.0] - 2026-08-27

### Added

- **Unit System** — Physical units with compile-time dimensional analysis
  - Unit literals: `100 kg`, `5 meter`, `10 second`, `2.5 kW`
  - Unit types: `kg`, `meter`, `second`, `watt`, `celsius`, `byte`, etc.
  - Unit arithmetic: same-unit add/sub, scalar multiply/divide
  - Composite units: `meter·meter` for area, dimensionless ratios
  - Compile-time mismatch detection: `kg + meter` → error
- **Decimal Type** — Exact decimal arithmetic with i128 backend
  - 18-digit precision (scale 10^18)
  - Compatible with Int and Float literals
- **WebAssembly Backend** — Generate .wat text format
  - `sandbox wasm file.sbx -o output.wat`
  - `sandbox build file.sbx --target wasm`
  - Supports: functions, if/else, while loops, arithmetic, function calls
  - String data section, memory export, main function export
- **CLI**: `sandbox build --target wasm` and `sandbox wasm` commands
- 7 new integration tests (31 total)
- 2 new examples: units_demo, wasm_demo

### Changed

- Version bumped to 0.4.0
- AST: Added `UnitLiteral` expression and `Unit` type
- Type checker: Unit dimensional analysis, Decimal compatibility
- Codegen: Unit literals emit raw values, Decimal uses i128 scaling

## [0.3.0] - 2026-08-27

### Added

- **Standard Library** — Built-in `math`, `string`, `array` modules
  - `math::abs`, `math::max`, `math::min`, `math::sqrt`, `math::pow`, `math::floor`, `math::ceil`, `math::log`, `math::log2`, `math::log10`
  - `string::length`, `string::concat`, `string::substring`, `string::equals`
  - `array::len`, `array::push`, `array::sort`
- **Package Manager** — `sandbox.toml` manifest with `[dependencies]` section
  - `sandbox add <pkg>` — Add dependency to sandbox.toml
  - `sandbox install` — Install all dependencies (placeholder for registry)
  - `sandbox tree` — Show dependency tree
- **Formatter** — `sandbox fmt` and `sandbox fmt --check` for .sbx files
- 8 new integration tests (24 total)

## [0.2.0] - 2026-08-27

### Added

- **Result Type** — `Result<T, E>` for error handling
- **Ok/Err Constructors** — `Ok(value)` and `Err(error)` expressions
- **? Operator** — Error propagation with `expr?`
- **panic! Macro** — Runtime error with `panic!("message")`
- **Module System** — `mod name { ... }` for code organization
- **Module Calls** — `module::function()` syntax
- **sandbox init** — Initialize new project with `sandbox.toml` and `main.sbx`
- 4 new integration tests (16 total)

## [0.1.0] - 2026-08-27

### Added

- **Lexer** — Tokenizer supporting all language tokens
- **Parser** — Recursive descent parser with precedence climbing
- **AST** — Complete abstract syntax tree definitions
- **Type Checker** — Type inference and validation
- **Money Type** — `Money<INR>`, `Money<USD>` with compile-time currency safety
- **C Code Generation** — Transpile to C, compile with GCC
- **CLI** — `sandbox run`, `sandbox build`, `sandbox check` commands
- **Structs** — User-defined struct types with field access
- **Functions** — Functions with type annotations and return types
- **Control Flow** — `if/else`, `while`, `for...in` loops
- **Variables** — `let`, `mut`, assignment
- **Arrays** — Array literals and indexing
- **Examples** — hello, fibonacci, struct_demo, money
- **Tests** — 12 end-to-end integration tests
- **CI/CD** — GitHub Actions for tests, linting, formatting, and releases
