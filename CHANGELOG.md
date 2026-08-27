# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

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
- **String Concatenation** — `+` operator for strings generates `__sbx_str_concat`
- **Improved For-Loops** — Inline unrolled codegen for array literals
- **Variable Type Tracking** — CodeGen tracks variable C types for correct `printf` format
- **stdlib C Runtime** — String helpers, math functions via C stdlib
- **Type keyword module syntax** — `string::concat`, `math::sqrt` work with type keywords as module names
- 8 new integration tests (24 total)
- 4 new examples: math_demo, string_demo, sorting_v3, bank_transfer

### Changed

- Version bumped to 0.3.0
- Type checker registers stdlib builtins automatically
- Codegen uses `std::collections::HashMap` for variable type tracking
- For-loop with array literals generates scoped blocks (no redefinition errors)

## [0.2.0] - 2026-08-27

### Added

- **Result Type** — `Result<T, E>` for error handling
- **Ok/Err Constructors** — `Ok(value)` and `Err(error)` expressions
- **? Operator** — Error propagation with `expr?`
- **panic! Macro** — Runtime error with `panic!("message")`
- **Module System** — `mod name { ... }` for code organization
- **Module Calls** — `module::function()` syntax
- **sandbox init** — Initialize new project with `sandbox.toml` and `main.sbx`
- **use Statement** — Import syntax (parsed, not yet resolved)
- 4 new integration tests (16 total)

### Changed

- Version bumped to 0.2.0
- Type checker now handles module-prefixed functions
- Codegen flattens modules to C functions

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
