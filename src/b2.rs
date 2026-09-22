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

use crate::ast::{Expr, Program, Stmt, TopLevel, Type};
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
