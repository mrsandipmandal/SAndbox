# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

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
