# Contributing to Sandbox

Thank you for your interest in contributing to Sandbox! This document provides guidelines and information for contributors.

## 🚀 Getting Started

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [GCC](https://gcc.gnu.org/) (for C transpilation)
- [Git](https://git-scm.com/)

### Setup

```bash
# Fork and clone the repository
git clone https://github.com/YOUR_USERNAME/sandbox.git
cd sandbox

# Build the project
cargo build

# Run tests
cargo test
```

## 📝 How to Contribute

### Reporting Bugs

1. Check if the bug already exists in [Issues](https://github.com/sandbox-lang/sandbox/issues)
2. If not, create a new issue with:
   - A clear, descriptive title
   - Steps to reproduce
   - Expected vs actual behavior
   - Your environment (OS, Rust version)

### Suggesting Features

1. Check existing issues and discussions
2. Open a new issue with the `enhancement` label
3. Describe the feature, use case, and expected syntax

### Submitting Changes

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes
4. Add tests if applicable
5. Ensure all checks pass:
   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test
   ```
6. Commit with a clear message
7. Push and create a Pull Request

## 🎯 Development Guidelines

### Code Style

- Follow Rust standard formatting (`cargo fmt`)
- Keep functions focused and small
- Use meaningful variable names
- Add comments for complex logic
- Prefer `?` over `.unwrap()` in library code

### Commit Messages

Use clear, descriptive commit messages:

```
Add Money type currency validation

- Implement compile-time currency mismatch detection
- Add tests for Money<INR> + Money<USD> error
- Update parser to handle currency literals
```

### Testing

- Add tests for new features
- Ensure existing tests pass
- Test edge cases
- Use descriptive test names

```rust
#[test]
fn test_money_currency_mismatch() {
    // Test that Money<INR> + Money<USD> produces compile error
}
```

### Architecture

The compiler follows this pipeline:

```
Source → Lexer → Parser → Type Checker → Codegen → C → GCC → Binary
```

When adding features, consider which layer needs changes:

- **Lexer** (`src/lexer.rs`): New tokens, keywords
- **Parser** (`src/parser.rs`): New syntax, expressions
- **Type Checker** (`src/typechecker.rs`): Type rules, validation
- **Codegen** (`src/codegen.rs`): C code generation

## 🏷️ Labels

- `bug` — Something isn't working
- `enhancement` — New feature or improvement
- `documentation` — Documentation changes
- `good first issue` — Good for newcomers
- `help wanted` — Extra attention is needed

## 📞 Questions?

- Open a [Discussion](https://github.com/sandbox-lang/sandbox/discussions)
- Ask in the issue

## 📜 License

By contributing, you agree that your contributions will be licensed under the MIT License.
