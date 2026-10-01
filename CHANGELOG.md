# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

> Versioning note: releases before 0.0.5 used a different, abandoned
> scheme (tags v0.2.0 … v1.1.0, Cargo versions up to 1.2.0, none of
> which were all published). Starting with **0.0.5** the project
> follows the GitHub Releases sequence v0.0.1 … v0.0.4. See
> [RELEASE.md](RELEASE.md) for the versioning convention, including
> the patch-999 carry rule (`X.Y.999 → X.(Y+1).0`).

## [0.0.5] - 2026-09-30
## [0.0.6] - 2026-10-01

### Added

- (describe user-visible additions here)

### Fixed

- (describe fixes here)


### Added

- **Package registry server** (`registry/`): publish/fetch packages with
  SQLite storage, download counts, rate limiting, an HTML dashboard and a
  Docker image (ghcr.io, auto-built and smoke-checked on every master push,
  with an SSH deploy job gated on the `production` environment).
- **Browser playground** at `/playground`: compiles and runs Sandbox
  entirely in-page (playground-compiler → wasm32 artifact, in-page WAT
  encoder, packages fetched from the registry API).
- **Total division** (completes B2): `/` and `%` are total and identical
  across all four backends — `x/0 → 0`, `x%0 → 0`, `i64::MIN / -1` wraps,
  `i64::MIN % -1 → 0` — via shared `sbx_gdiv`/`sbx_grem` runtime helpers
  (C/LLVM), wrapping interpreter ops, and guarded wasm helpers.
- **Release pipeline verification**: per-target built-binary checks
  (existence, sane size, ELF/Mach-O/PE architecture match) and a
  whole-release artifact-completeness gate before checksums are
  generated, both reported in the job step summary.
- `RELEASE.md` and `scripts/release.sh`: one-command release flow
  (bump + changelog + commit + tag) with the documented 999-carry rule.

### Fixed

- Playground division helpers: a mis-nested `(else …)` inside the outer
  `(then …)` (still paren-balanced) and a bare value tail that the page
  encoder's fall-through rule turned into a runtime `unreachable` trap —
  division programs now run in the browser end to end.
- `sandbox --version` now reports the real crate version instead of a
  stale hardcoded string.
- release.yml: retired `macos-13` runner label (x86_64 macOS builds were
  silently dead) replaced with `macos-15-intel`.

### CI / tooling

- New `workflow-lint` job: actionlint (pinned, checksum-verified) over
  all workflows on every push/PR; mirrored as Gate 0 in ci-local.sh.
- Parity job runs the whole integration suite with wabt + node on PATH,
  so the tooling-gated wasm rows (wasm CLI pipeline: gen → wat2wasm →
  run) execute in CI instead of silently skipping.
- Playground job runs the structural WAT nesting unit tests; the smoke
  script covers total division end-to-end through the page encoder.
- Division-program WAT added to the encoder-parity corpus (wat2wasm
  byte-identity for `$sbx_gdiv`/`$sbx_grem`).

## [1.2.0] - 2026-09-01

### Added

- **LLVM Backend Feature Parity** — The LLVM IR backend now supports the full
  Sandbox language feature set, matching the C/GCC backend:
  - Money literals & arithmetic (scaled i64 ×10000)
  - Decimal type (i128 scaled ×10^18)
  - Option/Some/None with match patterns (Some/None)
  - Closure capture (free-variable detection, captures passed as extra params)
  - Async/await (sync-async model via shared C runtime)
  - Impl blocks, trait methods, and module functions
  - if-let, ranges, f-strings, assert/assert_eq
  - Full binary operators (mod, <=, >=, and, or)
  - Method calls (string methods + struct methods)
  - Array literals and indexing
  - Panic, error, try, and assert expressions

- **LLVM Backend Runtime Linking** — `sandbox llvm-build` now emits the shared
  C runtime (`sbx_runtime.c`) alongside the `.ll` file and compiles both with
  clang. The runtime `static` functions are exported so they link correctly.
  `sandbox llvm` (IR-only mode) also emits the runtime `.c` with a compile hint.

- 12 new LLVM integration tests mirroring the C backend tests:
  money, if-let, range-for, f-string, option-match, none-match,
  closure-capture, async-await, modulo, le/ge, assert_eq, impl-method

### Changed

- Version bumped to 1.2.0
- README roadmap: LLVM backend and HTTP/JSON marked complete; Package registry
  marked complete; new v1.2 section added

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
