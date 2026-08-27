# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [1.0.0] - 2026-08-27

### Added

- **Ledger DSL** — Double-entry accounting as a first-class language feature
  - `ledger` keyword for transaction definitions
  - `debit` and `credit` sides with account and amount
  - Compile-time balance validation (debits must equal credits)
  - `__validate_<name>()` functions generated for runtime checks
- **Database DSL** — SQL-like database operations as language features
  - `database` keyword for database definitions
  - `table` keyword for schema definitions with typed columns
  - `query` keyword for SQL-like queries (SELECT, INSERT, UPDATE, DELETE)
  - Compile-time table reference validation
  - Query functions generated with proper C types
- **LSP Server** — Language Server Protocol support for IDEs
  - `sandbox lsp` command starts the LSP server
  - Diagnostics: real-time error reporting as you type
  - Completion: keywords, types, stdlib functions
  - Hover: type information on mouse hover
  - Compatible with VS Code, Neovim, Emacs, and other LSP clients
- **Self-Hosting Compiler** — Sandbox program that compiles Sandbox subset
  - `examples/selfhost_compiler.sbx` demonstrates compiler writing in Sandbox
  - Compiles let, print, return, assignment, if/else to C
- **Extended Standard Library**
  - `string::trim`, `string::starts_with`, `string::contains`, `string::find`
- 4 new integration tests (35 total)
- 3 new examples: ledger_demo, database_demo, selfhost_compiler

### Changed

- Version bumped to 1.0.0
- AST: Added LedgerDef, DatabaseDef, TableDef, QueryDef, QueryKind nodes
- Parser: Ledger and Database DSL parsing
- Type checker: Ledger balance validation, Database table/query validation
- Codegen: Ledger validation functions, Database query functions

## [0.4.0] - 2026-08-27

### Added

- **Unit System** — Physical units with compile-time dimensional analysis
- **Decimal Type** — Exact decimal arithmetic with i128 backend
- **WebAssembly Backend** — Generate .wat text format
- 7 new integration tests (31 total)

## [0.3.0] - 2026-08-27

### Added

- **Standard Library** — Built-in `math`, `string`, `array` modules
- **Package Manager** — `sandbox.toml` manifest with dependencies
- **Formatter** — `sandbox fmt` and `sandbox fmt --check`
- 8 new integration tests (24 total)

## [0.2.0] - 2026-08-27

### Added

- **Result Type** — `Result<T, E>` for error handling
- **Module System** — `mod name { ... }` for code organization
- **sandbox init** — Initialize new project
- 4 new integration tests (16 total)

## [0.1.0] - 2026-08-27

### Added

- **Core Language** — Lexer, Parser, Type Checker, C Code Generation
- **Money Type** — `Money<INR>`, `Money<USD>` with compile-time currency safety
- **CLI** — `sandbox run`, `sandbox build`, `sandbox check`
- **CI/CD** — GitHub Actions for tests, linting, formatting, and releases
- 12 end-to-end integration tests
