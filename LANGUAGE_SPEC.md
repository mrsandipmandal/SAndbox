# 📖 Sandbox Language Specification v0.2

This document describes the syntax and semantics of the Sandbox programming language.

---

## 1. Lexical Structure

### Comments

```
// This is a line comment
```

### Identifiers

```
identifier = (letter | '_') (letter | digit | '_')*
letter = 'a'..'z' | 'A'..'Z'
digit = '0'..'9'
```

### Keywords

```
let mut fn struct if else while for in return print
Result Ok err panic mod use
```

### Literals

```
integer = digit+
float = digit+ '.' digit+
string = '"' characters '"'
bool = 'true' | 'false'
money = integer currency | float currency
```

### Currencies

```
INR | USD | EUR | GBP | JPY | CNY | BDT
```

---

## 2. Types

### Primitive Types

| Type | Description | Size |
|------|-------------|------|
| `i64` | 64-bit signed integer | 8 bytes |
| `f64` | 64-bit IEEE 754 float | 8 bytes |
| `bool` | Boolean (true/false) | 1 byte |
| `string` | String literal | varies |

### Financial Types

| Type | Description | Storage |
|------|-------------|---------|
| `Money<C>` | Currency-tagged money | `i64` (scaled ×10000) |
| `Decimal` | Exact decimal | `i64` (scaled ×10000) |

### Composite Types

| Type | Description |
|------|-------------|
| `[T]` | Array of T |
| `Struct` | User-defined struct |
| `Result<T, E>` | Error handling |

---

## 3. Expressions

### Arithmetic

```
a + b    // Addition
a - b    // Subtraction
a * b    // Multiplication
a / b    // Division
a % b    // Modulo
```

### Comparison

```
a == b   // Equal
a != b   // Not equal
a < b    // Less than
a > b    // Greater than
a <= b   // Less or equal
a >= b   // Greater or equal
```

### Logical

```
a && b   // Logical AND
a || b   // Logical OR
!a       // Logical NOT
```

### Money Operations

```
Money<C> + Money<C>     // ✅ Same currency
Money<C> + Money<D>     // ❌ Currency mismatch
Money<C> * f64          // ✅ Percentage
Money<C> * i64          // ✅ Quantity
```

---

## 4. Statements

### Variable Declaration

```
let x: i64 = 42          // Immutable
let mut y: i64 = 42      // Mutable
let z = 42               // Type inferred
```

### Assignment

```
x = 10
y = x + 1
```

### Function Call

```
print("Hello")
add(1, 2)
```

### Return

```
return value
return
```

---

## 5. Control Flow

### If/Else

```
if condition {
    // then
} else {
    // else
}
```

### While Loop

```
while condition {
    // body
}
```

### For Loop

```
for item in [1, 2, 3] {
    print(item)
}
```

---

## 6. Functions

### Definition

```
fn name(param: Type, ...) -> ReturnType {
    // body
    return value
}
```

### Examples

```
fn add(a: i64, b: i64) -> i64 {
    return a + b
}

fn greet(name: string) {
    print("Hello, " + name)
}
```

---

## 7. Structs

### Definition

```
struct Name {
    field: Type,
    ...
}
```

### Instantiation

```
let s = Name {
    field: value,
}
```

### Field Access

```
s.field
```

---

## 8. Enums & Pattern Matching

### Enum Definition

```
enum Color {
    Red,
    Green,
    Blue,
}

enum Shape {
    Circle(f64),
    Square(f64),
    Point,
}
```

### Enum Variants

```
let c = Color::Red          // unit variant
let s = Shape::Circle(5.0)  // payload variant
```

### Pattern Matching

```
match c {
    Color::Red => 1,
    Color::Green => 2,
    Color::Blue => 3,
    _ => 0,
}

match s {
    Shape::Circle(r) => r * r * 3.14,
    Shape::Square(s) => s * s,
    Shape::Point => 0.0,
}
```

### Pattern Types

| Pattern | Example | Description |
|---------|---------|-------------|
| Enum variant | `Color::Red` | Matches a specific variant |
| Enum + binding | `Shape::Circle(r)` | Matches variant and binds payload |
| Integer literal | `42` | Matches a specific integer |
| Bool literal | `true` | Matches a specific bool |
| String literal | `"hello"` | Matches a specific string |
| Variable | `x` | Binds any value to `x` |
| Wildcard | `_` | Matches anything (no binding) |

---

## 10. Error Handling

### Result Type

```
fn divide(a: i64, b: i64) -> Result<i64, string> {
    if b == 0 {
        return Err("Division by zero")
    }
    return Ok(a / b)
}
```

### ? Operator

```
fn calculate() -> Result<i64, string> {
    let x = divide(10, 2)?
    return Ok(x * 2)
}
```

### panic!

```
panic!("Fatal error occurred")
```

---

## 9. Modules

### Definition

```
mod name {
    fn function() { ... }
    struct Type { ... }
}
```

### Usage

```
name::function()
name::Type { ... }
```

---

## 10. Type Rules

### Money Type Safety

```
Money<INR> + Money<INR>   → Money<INR>  ✅
Money<INR> + Money<USD>   → Error       ❌
Money<INR> * i64          → Money<INR>  ✅
Money<INR> * f64          → Money<INR>  ✅
Money<INR> + i64          → Error       ❌
```

### Array Type Safety

```
[1, 2, 3]                 → [i64]       ✅
[1, "a"]                  → Error       ❌
```

### Function Type Checking

- All arguments must match parameter types
- Return type must match declared type
- Unknown functions cause compile error

---

## 11. Semantic Rules

### Scope

- Variables are block-scoped
- Inner scopes can access outer variables
- Variables must be defined before use

### Mutability

- `let` creates immutable binding
- `let mut` creates mutable binding
- Immutable variables cannot be reassigned

### Memory Safety

- No null pointers
- No use-after-free
- No data races (in safe code)
- `unsafe` blocks for system programming

---

## 12. Compilation

### Pipeline

```
Source → Lexer → Parser → Type Checker → Codegen → C → GCC → Binary
```

### Targets

- Linux x86_64 (primary)
- Linux ARM64 (planned)
- WebAssembly (planned)

---

## Appendix: Grammar

```
program     → top_level*
top_level   → fn_def | struct_def | mod_def | use_stmt
fn_def      → 'fn' IDENT '(' params? ')' ('->' type)? block
struct_def  → 'struct' IDENT '{' fields '}'
mod_def     → 'mod' IDENT '{' top_level* '}'
use_stmt    → 'use' path ';'

params      → param (',' param)*
param       → IDENT ':' type
fields      → field (',' field)*
field       → IDENT ':' type

type        → 'i64' | 'f64' | 'bool' | 'string'
            | 'Money<' currency '>' | 'Decimal'
            | '[' type ']' | IDENT
            | 'Result<' type ',' type '>'

stmt        → let_stmt | assign | if_stmt | while_stmt
            | for_stmt | return_stmt | print_stmt | expr_stmt
let_stmt    → 'let' 'mut'? IDENT (':' type)? '=' expr
assign      → IDENT '=' expr
if_stmt     → 'if' expr block ('else' (if_stmt | block))?
while_stmt  → 'while' expr block
for_stmt    → 'for' IDENT 'in' expr block
return_stmt → 'return' expr?
print_stmt  → 'print' '(' expr ')'
expr_stmt   → expr

expr        → comparison
comparison  → addition (('==' | '!=' | '<' | '>' | '<=' | '>=') addition)*
addition    → multiplication (('+' | '-') multiplication)*
multiplication → unary (('*' | '/' | '%') unary)*
unary       → '-' unary | postfix
postfix     → primary ('.' IDENT | '[' expr ']' | '(' args? ')')*
primary     → INT | FLOAT | STRING | BOOL | MONEY
            | 'Ok' '(' expr ')' | 'Err' '(' expr ')'
            | 'panic' '(' expr ')' | IDENT | array | '(' expr ')'

array       → '[' (expr (',' expr)*)? ']'
args        → expr (',' expr)*
currency    → 'INR' | 'USD' | 'EUR' | 'GBP' | 'JPY' | 'CNY' | 'BDT'
```
