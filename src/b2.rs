//! B2: sized/unsigned integer types — i64-at-rest desugaring.
//!
//! Design decision (PROJECT_MEMORY.md): narrow ints (`u8..u64`, `i8..i32`,
//! `usize`) live as `i64` at rest in every backend. Arithmetic, comparisons and
//! builtin calls all run at i64. The width/sign only matters at *typed stores*:
//! assignment to a variable/parameter/return position declared with a narrow
//! type wraps the value to that width. This pass makes that explicit by
//! rewriting the AST after typechecking (so it never interacts with type
//! inference) into the same `Expr::Cast` the `as` operator produces:
//!
//!   let x: u8 = <expr>   →  let x: u8 = <expr> as u8
//!   x = <expr>           →  x = <expr> as <declared type of x>
//!   return <expr>        →  return <expr> as <fn ret type>   (narrow rets only)
//!
//! All three backends then implement exactly one wrapping primitive
//! (`Expr::Cast`), keeping C / LLVM / interpreter bit-identical by construction.

use crate::ast::{Expr, FStringPart, Pattern, Program, Stmt, TopLevel, Type};
use std::collections::HashMap;

/// Rewrite `program` in place, wrapping every typed store with a cast to the
/// stored type. Run after typechecking, before code generation.
pub fn desugar_typed_stores(program: &mut Program) {
    for item in &mut program.items {
        rewrite_top_level(item);
    }
}

fn rewrite_top_level(item: &mut TopLevel) {
    match item {
        TopLevel::FnDef {
            params, ret, body, ..
        }
        | TopLevel::AsyncFnDef {
            params, ret, body, ..
        } => {
            rewrite_fn(params, ret, body);
        }
        TopLevel::ImplDef { methods, .. } => {
            for m in methods {
                rewrite_top_level(m);
            }
        }
        TopLevel::TestDef { body, .. } => {
            *body = rewrite_stmts(body, &HashMap::new(), &Type::Void);
        }
        _ => {}
    }
}

fn rewrite_fn(params: &mut [crate::ast::Param], ret: &mut Option<Type>, body: &mut Vec<Stmt>) {
    let mut vars: HashMap<String, Type> = params
        .iter()
        .map(|p| (p.name.clone(), p.ty.clone()))
        .collect();
    let ret_ty = ret.clone().unwrap_or(Type::Void);
    // Defaults are evaluated before the fn body with earlier params in scope.
    for p in params.iter_mut() {
        if let Some(d) = p.default.as_mut() {
            *d = rewrite_expr(d, &vars, &Type::Void);
        }
        vars.insert(p.name.clone(), p.ty.clone());
    }
    *body = rewrite_stmts(body, &vars, &ret_ty);
}

fn rewrite_stmts(stmts: &[Stmt], vars: &HashMap<String, Type>, ret: &Type) -> Vec<Stmt> {
    // Thread the variable map sequentially: a typed `let` must be visible to
    // the Assign statements after it (assignment wraps to the DECLARED type).
    let mut vars = vars.clone();
    stmts
        .iter()
        .map(|s| rewrite_stmt(s, &mut vars, ret))
        .collect()
}

fn rewrite_stmt(stmt: &Stmt, vars: &mut HashMap<String, Type>, ret: &Type) -> Stmt {
    match stmt {
        Stmt::Let {
            name,
            ty,
            value,
            mutable,
        } => {
            let value = rewrite_expr(value, vars, ret);
            // Typed let: the declared type decides the store wrap.
            let value = match ty {
                Some(t) if is_narrow(t) => wrap_store(value, t),
                _ => value,
            };
            if let Some(t) = ty {
                vars.insert(name.clone(), t.clone());
            } else {
                vars.remove(name);
            }
            Stmt::Let {
                name: name.clone(),
                ty: ty.clone(),
                value,
                mutable: *mutable,
            }
        }
        Stmt::Assign { name, value } => {
            let value = rewrite_expr(value, vars, ret);
            let value = match vars.get(name) {
                Some(t) if is_narrow(t) => wrap_store(value, t),
                _ => value,
            };
            Stmt::Assign {
                name: name.clone(),
                value,
            }
        }
        Stmt::If {
            condition,
            then,
            else_,
        } => Stmt::If {
            condition: rewrite_expr(condition, vars, ret),
            // Branch bodies scope their own lets: rewrite_stmts clones, so
            // lets inside a branch don't leak to following siblings.
            then: rewrite_stmts(then, vars, ret),
            else_: else_.as_ref().map(|e| rewrite_stmts(e, vars, ret)),
        },
        Stmt::IfLet {
            pattern,
            value,
            then,
            else_,
        } => Stmt::IfLet {
            pattern: pattern.clone(),
            value: Box::new(rewrite_expr(value, vars, ret)),
            then: rewrite_stmts(then, vars, ret),
            else_: else_.as_ref().map(|e| rewrite_stmts(e, vars, ret)),
        },
        Stmt::While { condition, body } => Stmt::While {
            condition: rewrite_expr(condition, vars, ret),
            body: rewrite_stmts(body, vars, ret),
        },
        Stmt::For {
            variable,
            iterable,
            body,
        } => {
            // Loop variables are fresh i64 bindings; untyped lets don't wrap.
            let mut vars2 = vars.clone();
            vars2.remove(variable);
            Stmt::For {
                variable: variable.clone(),
                iterable: rewrite_expr(iterable, vars, ret),
                body: rewrite_stmts(body, &vars2, ret),
            }
        }
        Stmt::Return(Some(e)) => {
            let e = rewrite_expr(e, vars, ret);
            if is_narrow(ret) {
                Stmt::Return(Some(wrap_store(e, ret)))
            } else {
                Stmt::Return(Some(e))
            }
        }
        Stmt::Return(None) => Stmt::Return(None),
        Stmt::ExprStmt(e) => Stmt::ExprStmt(rewrite_expr(e, vars, ret)),
        Stmt::Print(e) => Stmt::Print(rewrite_expr(e, vars, ret)),
        Stmt::Break | Stmt::Continue => stmt.clone(),
    }
}

fn rewrite_expr(expr: &Expr, vars: &HashMap<String, Type>, ret: &Type) -> Expr {
    match expr {
        Expr::BinaryOp { op, left, right } => Expr::BinaryOp {
            op: op.clone(),
            left: Box::new(rewrite_expr(left, vars, ret)),
            right: Box::new(rewrite_expr(right, vars, ret)),
        },
        Expr::UnaryOp { op, expr } => Expr::UnaryOp {
            op: op.clone(),
            expr: Box::new(rewrite_expr(expr, vars, ret)),
        },
        Expr::Call {
            name,
            type_args,
            args,
        } => Expr::Call {
            name: name.clone(),
            type_args: type_args.clone(),
            args: rewrite_all(args, vars, ret),
        },
        Expr::MethodCall {
            target,
            method,
            args,
        } => Expr::MethodCall {
            target: Box::new(rewrite_expr(target, vars, ret)),
            method: method.clone(),
            args: rewrite_all(args, vars, ret),
        },
        Expr::ArrayLiteral(elems) => Expr::ArrayLiteral(rewrite_all(elems, vars, ret)),
        Expr::MapLiteral(pairs) => Expr::MapLiteral(
            pairs
                .iter()
                .map(|(k, v)| (rewrite_expr(k, vars, ret), rewrite_expr(v, vars, ret)))
                .collect(),
        ),
        Expr::Index { target, index } => Expr::Index {
            target: Box::new(rewrite_expr(target, vars, ret)),
            index: Box::new(rewrite_expr(index, vars, ret)),
        },
        Expr::FieldAccess { target, field } => Expr::FieldAccess {
            target: Box::new(rewrite_expr(target, vars, ret)),
            field: field.clone(),
        },
        Expr::StructLiteral {
            name,
            type_args,
            fields,
        } => Expr::StructLiteral {
            name: name.clone(),
            type_args: type_args.clone(),
            fields: fields
                .iter()
                .map(|(f, e)| (f.clone(), rewrite_expr(e, vars, ret)))
                .collect(),
        },
        Expr::Match { scrutinee, arms } => Expr::Match {
            scrutinee: Box::new(rewrite_expr(scrutinee, vars, ret)),
            arms: arms
                .iter()
                .map(|arm| crate::ast::MatchArm {
                    pattern: arm.pattern.clone(),
                    guard: arm
                        .guard
                        .as_ref()
                        .map(|g| Box::new(rewrite_expr(g, vars, ret))),
                    body: rewrite_stmts(&arm.body, vars, ret),
                })
                .collect(),
        },
        Expr::Lambda {
            params,
            ret: lret,
            body,
        } => {
            // Lambdas are typed-checked as closures; body sees only its params.
            let lv: HashMap<String, Type> = params
                .iter()
                .map(|p| (p.name.clone(), p.ty.clone()))
                .collect();
            let lret2 = lret.clone().unwrap_or(Type::Void);
            Expr::Lambda {
                params: params.clone(),
                ret: lret.clone(),
                body: rewrite_stmts(body, &lv, &lret2),
            }
        }
        Expr::UnitLiteral { value, unit } => Expr::UnitLiteral {
            value: Box::new(rewrite_expr(value, vars, ret)),
            unit: unit.clone(),
        },
        Expr::OkExpr(e) => Expr::OkExpr(Box::new(rewrite_expr(e, vars, ret))),
        Expr::ErrExpr(e) => Expr::ErrExpr(Box::new(rewrite_expr(e, vars, ret))),
        Expr::SomeExpr(e) => Expr::SomeExpr(Box::new(rewrite_expr(e, vars, ret))),
        Expr::PanicExpr(e) => Expr::PanicExpr(Box::new(rewrite_expr(e, vars, ret))),
        Expr::TryExpr(e) => Expr::TryExpr(Box::new(rewrite_expr(e, vars, ret))),
        Expr::Await(e) => Expr::Await(Box::new(rewrite_expr(e, vars, ret))),
        Expr::AssertExpr { condition, message } => Expr::AssertExpr {
            condition: Box::new(rewrite_expr(condition, vars, ret)),
            message: message
                .as_ref()
                .map(|m| Box::new(rewrite_expr(m, vars, ret))),
        },
        Expr::AssertEqExpr {
            left,
            right,
            message,
        } => Expr::AssertEqExpr {
            left: Box::new(rewrite_expr(left, vars, ret)),
            right: Box::new(rewrite_expr(right, vars, ret)),
            message: message
                .as_ref()
                .map(|m| Box::new(rewrite_expr(m, vars, ret))),
        },
        Expr::EnumVariant {
            enum_name,
            type_args,
            variant,
            payload,
        } => Expr::EnumVariant {
            enum_name: enum_name.clone(),
            type_args: type_args.clone(),
            variant: variant.clone(),
            payload: payload
                .as_ref()
                .map(|p| Box::new(rewrite_expr(p, vars, ret))),
        },
        Expr::Range {
            start,
            end,
            inclusive,
        } => Expr::Range {
            start: Box::new(rewrite_expr(start, vars, ret)),
            end: Box::new(rewrite_expr(end, vars, ret)),
            inclusive: *inclusive,
        },
        Expr::Cast { expr, ty } => Expr::Cast {
            expr: Box::new(rewrite_expr(expr, vars, ret)),
            ty: ty.clone(),
        },
        // Leaves and f-strings carry no typed stores.
        _ => expr.clone(),
    }
}

fn rewrite_all(exprs: &[Expr], vars: &HashMap<String, Type>, ret: &Type) -> Vec<Expr> {
    exprs.iter().map(|e| rewrite_expr(e, vars, ret)).collect()
}

/// C-backend semantics, made shared: a function whose final statement is an
/// expression statement returns that expression's value. Desugar it into an
/// explicit `return` so every backend (C, LLVM, interpreter, wasm) agrees.
/// Runs after type checking, alongside the typed-store desugar.
pub fn implicit_returns(program: &mut Program) {
    for item in program.items.iter_mut() {
        match item {
            TopLevel::FnDef { body, ret, .. } => {
                // Only fns with a declared return type get the implicit-return
                // rewrite: in unannotated fns the C backend may emit the final
                // call as a void statement, where `return <void call>` would
                // not compile.
                make_last_stmt_return(body, ret.is_some());
                make_value_if_return(body, ret.is_some());
            }
            TopLevel::ImplDef { methods, .. } => {
                for m in methods.iter_mut() {
                    if let TopLevel::FnDef { body, ret, .. } = m {
                        make_last_stmt_return(body, ret.is_some());
                        make_value_if_return(body, ret.is_some());
                    }
                }
            }
            _ => {}
        }
    }
}

fn make_last_stmt_return(body: &mut Vec<Stmt>, has_ret: bool) {
    if !has_ret {
        return;
    }
    if let Some(Stmt::ExprStmt(_)) = body.last() {
        if let Some(Stmt::ExprStmt(e)) = body.pop() {
            body.push(Stmt::Return(Some(e)));
        }
    }
}

/// A trailing `if` statement whose branches both produce values is a value-yielding
/// conditional (the only recursion-friendly form the language has, since there is
/// no `Expr::If`). With a declared return type, rewrite each branch's final
/// value-yielding statement into an explicit `return` so every backend agrees.
fn make_value_if_return(body: &mut [Stmt], has_ret: bool) {
    if !has_ret {
        return;
    }
    let Some(Stmt::If { then, else_, .. }) = body.last_mut() else {
        return;
    };
    let Some(else_body) = else_ else {
        return;
    };
    if force_return(then) {
        force_return(else_body);
    }
}

/// Convert the final value-yielding statement of a branch into `return`.
/// Returns false when the branch does not (or cannot) produce a trailing value,
/// in which case the whole rewrite is abandoned.
fn force_return(branch: &mut Vec<Stmt>) -> bool {
    match branch.last_mut() {
        Some(Stmt::ExprStmt(_)) => {
            if let Some(Stmt::ExprStmt(e)) = branch.pop() {
                branch.push(Stmt::Return(Some(e)));
            }
            true
        }
        Some(Stmt::If { then, else_, .. }) => {
            let Some(else_body) = else_ else {
                return false;
            };
            force_return(then) && force_return(else_body)
        }
        _ => false,
    }
}

/// A typed store for a narrow int: wrap via the same cast `as` produces.
fn wrap_store(value: Expr, ty: &Type) -> Expr {
    Expr::Cast {
        expr: Box::new(value),
        ty: ty.clone(),
    }
}

fn is_narrow(ty: &Type) -> bool {
    matches!(ty, Type::Int(_))
}

// ── Block scoping (branch-scoping fix) ──
//
// Rule (Rust-style, PROJECT_MEMORY.md): `let` binds in the innermost block,
// an inner block may shadow an outer name, and using a name outside its
// block is a typechecker error. Before this pass the AST had no notion of
// blocks, so a shadowing `let` mutated the outer binding in every backend
// (C reused the C symbol, LLVM reused the alloca, the interpreter overwrote
// the map entry) — `let y = 5 { let y = 7 } print(y)` printed 7 7.
//
// This pass gives every shadowing declaration a distinct internal name and
// rewrites the reads that refer to it, so all four backends keep seeing
// plain distinct bindings. Scopes never leak: each block body is rewritten
// against a snapshot of the parent's mapping.

use std::cell::Cell;

thread_local! {
    static SHADOW_COUNTER: Cell<usize> = const { Cell::new(0) };
}

/// Rewrite `program` in place, renaming shadowing declarations to fresh
/// internal names. Run after typechecking, alongside the other b2 passes.
pub fn resolve_block_scoping(program: &mut Program) {
    SHADOW_COUNTER.with(|c| c.set(0));
    for item in &mut program.items {
        match item {
            TopLevel::FnDef { params, body, .. } | TopLevel::AsyncFnDef { params, body, .. } => {
                let mut scope: HashMap<String, String> = params
                    .iter()
                    .map(|p| (p.name.clone(), p.name.clone()))
                    .collect();
                *body = scope_stmts(std::mem::take(body), &mut scope);
            }
            TopLevel::ImplDef { methods, .. } => {
                for m in methods {
                    if let TopLevel::FnDef { params, body, .. } = m {
                        let mut scope: HashMap<String, String> = params
                            .iter()
                            .map(|p| (p.name.clone(), p.name.clone()))
                            .collect();
                        *body = scope_stmts(std::mem::take(body), &mut scope);
                    }
                }
            }
            TopLevel::TestDef { body, .. } => {
                let mut scope = HashMap::new();
                *body = scope_stmts(std::mem::take(body), &mut scope);
            }
            _ => {}
        }
    }
}

fn fresh_shadow(base: &str) -> String {
    let n = SHADOW_COUNTER.with(|c| {
        let n = c.get();
        c.set(n + 1);
        n
    });
    format!("{}__s{}", base, n)
}

fn scope_stmts(stmts: Vec<Stmt>, scope: &mut HashMap<String, String>) -> Vec<Stmt> {
    let mut out = Vec::with_capacity(stmts.len());
    for s in stmts {
        out.push(scope_stmt(s, scope));
    }
    out
}

fn scope_stmt(stmt: Stmt, scope: &mut HashMap<String, String>) -> Stmt {
    match stmt {
        Stmt::Let {
            name,
            ty,
            value,
            mutable,
        } => {
            let value = scope_expr(value, scope);
            // A redeclaration of a live name is a shadow: bind a fresh
            // internal name. First declaration keeps the source name.
            let final_name = if scope.contains_key(&name) {
                fresh_shadow(&name)
            } else {
                name.clone()
            };
            scope.insert(name, final_name.clone());
            Stmt::Let {
                name: final_name,
                ty,
                value,
                mutable,
            }
        }
        Stmt::Assign { name, value } => {
            let value = scope_expr(value, scope);
            let name = scope.get(&name).cloned().unwrap_or(name);
            Stmt::Assign { name, value }
        }
        Stmt::If {
            condition,
            then,
            else_,
        } => {
            let condition = scope_expr(condition, scope);
            let mut inner = scope.clone();
            let then = scope_stmts(then, &mut inner);
            let else_ = else_.map(|b| {
                let mut einner = scope.clone();
                scope_stmts(b, &mut einner)
            });
            Stmt::If {
                condition,
                then,
                else_,
            }
        }
        Stmt::IfLet {
            pattern,
            value,
            then,
            else_,
        } => {
            let value = Box::new(scope_expr((*value).clone(), scope));
            let mut inner = scope.clone();
            if let Pattern::Variable(name) = &pattern {
                inner.insert(name.clone(), name.clone());
            }
            let then = scope_stmts(then, &mut inner);
            let else_ = else_.map(|b| {
                let mut einner = scope.clone();
                scope_stmts(b, &mut einner)
            });
            Stmt::IfLet {
                pattern,
                value,
                then,
                else_,
            }
        }
        Stmt::While { condition, body } => {
            // The condition is evaluated in the ENCLOSING scope, before any
            // body declaration exists (matches the typechecker/interpreter):
            // rewrite it against `scope`, not the body's shadowed view, or a
            // body `let` on the same name would redirect the condition to an
            // uninitialized fresh binding and the loop would never terminate.
            let condition = scope_expr(condition, scope);
            let mut inner = scope.clone();
            let body = scope_stmts(body, &mut inner);
            Stmt::While { condition, body }
        }
        Stmt::For {
            variable,
            iterable,
            body,
        } => {
            let iterable = scope_expr(iterable, scope);
            let mut inner = scope.clone();
            // The loop variable shadows any outer binding of the same name.
            let var_final = if inner.contains_key(&variable) {
                fresh_shadow(&variable)
            } else {
                variable.clone()
            };
            inner.insert(variable, var_final.clone());
            let body = scope_stmts(body, &mut inner);
            Stmt::For {
                variable: var_final,
                iterable,
                body,
            }
        }
        Stmt::ExprStmt(e) => Stmt::ExprStmt(scope_expr(e, scope)),
        Stmt::Print(e) => Stmt::Print(scope_expr(e, scope)),
        Stmt::Return(Some(e)) => Stmt::Return(Some(scope_expr(e, scope))),
        Stmt::Break | Stmt::Continue | Stmt::Return(None) => stmt,
    }
}

fn scope_expr(expr: Expr, scope: &mut HashMap<String, String>) -> Expr {
    match expr {
        Expr::Ident(name) => Expr::Ident(scope.get(&name).cloned().unwrap_or(name)),
        Expr::BinaryOp { op, left, right } => Expr::BinaryOp {
            op,
            left: Box::new(scope_expr(*left, scope)),
            right: Box::new(scope_expr(*right, scope)),
        },
        Expr::UnaryOp { op, expr } => Expr::UnaryOp {
            op,
            expr: Box::new(scope_expr(*expr, scope)),
        },
        Expr::Cast { expr, ty } => Expr::Cast {
            expr: Box::new(scope_expr(*expr, scope)),
            ty,
        },
        Expr::Call {
            name,
            type_args,
            args,
        } => Expr::Call {
            name,
            type_args,
            args: args.into_iter().map(|a| scope_expr(a, scope)).collect(),
        },
        Expr::MethodCall {
            target,
            method,
            args,
        } => Expr::MethodCall {
            target: Box::new(scope_expr(*target, scope)),
            method,
            args: args.into_iter().map(|a| scope_expr(a, scope)).collect(),
        },
        Expr::ArrayLiteral(elems) => {
            Expr::ArrayLiteral(elems.into_iter().map(|e| scope_expr(e, scope)).collect())
        }
        Expr::MapLiteral(pairs) => Expr::MapLiteral(
            pairs
                .into_iter()
                .map(|(k, v)| (scope_expr(k, scope), scope_expr(v, scope)))
                .collect(),
        ),
        Expr::Index { target, index } => Expr::Index {
            target: Box::new(scope_expr(*target, scope)),
            index: Box::new(scope_expr(*index, scope)),
        },
        Expr::FieldAccess { target, field } => Expr::FieldAccess {
            target: Box::new(scope_expr(*target, scope)),
            field,
        },
        Expr::StructLiteral {
            name,
            type_args,
            fields,
        } => Expr::StructLiteral {
            name,
            type_args,
            fields: fields
                .into_iter()
                .map(|(f, e)| (f, scope_expr(e, scope)))
                .collect(),
        },
        Expr::Match { scrutinee, arms } => Expr::Match {
            scrutinee: Box::new(scope_expr(*scrutinee, scope)),
            arms: arms
                .into_iter()
                .map(|arm| {
                    let mut inner = scope.clone();
                    if let Pattern::Variable(n) = &arm.pattern {
                        inner.insert(n.clone(), n.clone());
                    }
                    crate::ast::MatchArm {
                        pattern: arm.pattern,
                        guard: arm.guard.map(|g| Box::new(scope_expr(*g, &mut inner))),
                        body: scope_stmts(arm.body, &mut inner),
                    }
                })
                .collect(),
        },
        Expr::Lambda { params, ret, body } => {
            let mut inner: HashMap<String, String> = params
                .iter()
                .map(|p| (p.name.clone(), p.name.clone()))
                .collect();
            Expr::Lambda {
                params,
                ret,
                body: scope_stmts(body, &mut inner),
            }
        }
        Expr::UnitLiteral { value, unit } => Expr::UnitLiteral {
            value: Box::new(scope_expr(*value, scope)),
            unit,
        },
        Expr::OkExpr(e) => Expr::OkExpr(Box::new(scope_expr(*e, scope))),
        Expr::ErrExpr(e) => Expr::ErrExpr(Box::new(scope_expr(*e, scope))),
        Expr::SomeExpr(e) => Expr::SomeExpr(Box::new(scope_expr(*e, scope))),
        Expr::NoneExpr => Expr::NoneExpr,
        Expr::PanicExpr(e) => Expr::PanicExpr(Box::new(scope_expr(*e, scope))),
        Expr::TryExpr(e) => Expr::TryExpr(Box::new(scope_expr(*e, scope))),
        Expr::AssertExpr { condition, message } => Expr::AssertExpr {
            condition: Box::new(scope_expr(*condition, scope)),
            message: message.map(|m| Box::new(scope_expr(*m, scope))),
        },
        Expr::AssertEqExpr {
            left,
            right,
            message,
        } => Expr::AssertEqExpr {
            left: Box::new(scope_expr(*left, scope)),
            right: Box::new(scope_expr(*right, scope)),
            message: message.map(|m| Box::new(scope_expr(*m, scope))),
        },
        Expr::EnumVariant {
            enum_name,
            type_args,
            variant,
            payload,
        } => Expr::EnumVariant {
            enum_name,
            type_args,
            variant,
            payload: payload.map(|p| Box::new(scope_expr(*p, scope))),
        },
        Expr::Range {
            start,
            end,
            inclusive,
        } => Expr::Range {
            start: Box::new(scope_expr(*start, scope)),
            end: Box::new(scope_expr(*end, scope)),
            inclusive,
        },
        Expr::FString(parts) => Expr::FString(
            parts
                .into_iter()
                .map(|p| match p {
                    FStringPart::Literal(l) => FStringPart::Literal(l),
                    FStringPart::Expr(e) => FStringPart::Expr(Box::new(scope_expr(*e, scope))),
                })
                .collect(),
        ),
        // Leaves
        other => other,
    }
}
