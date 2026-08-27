# 🔒 Security Policy

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 0.2.x   | ✅ Yes    |
| 0.1.x   | ❌ No     |

## Reporting a Vulnerability

If you discover a security vulnerability in Sandbox, please report it responsibly.

**Please do NOT open a public GitHub issue for security vulnerabilities.**

Instead, please email: **security@sandbox-lang.dev** (or open a private issue)

### What to include:

- Description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

### What to expect:

- Acknowledgment within 48 hours
- Status update within 1 week
- Credit in release notes (unless you prefer anonymity)

## Security Considerations

### Memory Safety

Sandbox is designed to be memory-safe by default:

- No use-after-free
- No double-free
- No data races
- No buffer overflows (in safe code)

Use `unsafe` blocks only when absolutely necessary.

### Financial Safety

Sandbox provides compile-time guarantees for financial calculations:

- Currency type enforcement
- Exact decimal arithmetic
- No floating-point rounding errors

### Compiler Security

The compiler generates C code that is compiled with GCC:

- Generated code is temporary and cleaned up after execution
- No user-controlled code execution paths
- Sandboxed compilation environment

## Best Practices

When using Sandbox for financial applications:

1. Always use `Money<Currency>` type for monetary values
2. Never use `f64` for financial calculations
3. Validate all external inputs
4. Use `Result` type for error handling
5. Audit generated C code for sensitive applications

## Updates

Security updates will be released as patch versions:

- `0.2.1`, `0.2.2`, etc.

Follow the repository for security announcements.
