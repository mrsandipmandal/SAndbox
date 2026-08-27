use crate::ast::*;
use crate::stdlib;
use anyhow::{anyhow, Result};
use std::collections::HashMap;

pub struct TypeChecker {
    structs: HashMap<String, Vec<Field>>,
    functions: HashMap<String, (Vec<Type>, Option<Type>)>,
    scopes: Vec<HashMap<String, Type>>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            structs: HashMap::new(),
            functions: HashMap::new(),
            scopes: vec![HashMap::new()],
        }
    }

    pub fn check(&mut self, program: &Program) -> Result<()> {
        // Register stdlib builtins
        for (name, sig) in stdlib::builtins() {
            let param_tys: Vec<Type> = sig.params.iter().map(|p| p.1.clone()).collect();
            self.functions.insert(name, (param_tys, Some(sig.ret)));
        }

        // First pass: register all struct definitions
        for item in &program.items {
            self.register_structs(item, "");
        }

        // Second pass: register all function signatures
        for item in &program.items {
            self.register_functions(item, "");
        }

        // Third pass: type check function bodies
        for item in &program.items {
            if let TopLevel::FnDef {
                name, params, body, ..
            } = item
            {
                self.scopes.push(HashMap::new());
                for p in params {
                    self.scopes
                        .last_mut()
                        .unwrap()
                        .insert(p.name.clone(), p.ty.clone());
                }
                self.check_block(body)?;
                self.scopes.pop();
                println!("  ✓ Function '{}' type-checked", name);
            }
        }

        println!("  ✓ All type checks passed");
        Ok(())
    }

    fn check_block(&mut self, stmts: &[Stmt]) -> Result<()> {
        for stmt in stmts {
            self.check_stmt(stmt)?;
        }
        Ok(())
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
                let val_ty = self.check_expr(value)?;
                if let Some(expected) = ty {
                    if &val_ty != expected {
                        return Err(anyhow!(
                            "Type mismatch: expected '{}', got '{}' for '{}'",
                            expected,
                            val_ty,
                            name
                        ));
                    }
                }
                self.scopes.last_mut().unwrap().insert(name.clone(), val_ty);
            }
            Stmt::Assign { name, value } => {
                let val_ty = self.check_expr(value)?;
                let var_ty = self.lookup_var(name)?;
                if val_ty != var_ty {
                    return Err(anyhow!(
                        "Type mismatch in assignment: expected '{}', got '{}' for '{}'",
                        var_ty,
                        val_ty,
                        name
                    ));
                }
            }
            Stmt::If {
                condition,
                then,
                else_,
            } => {
                let cond_ty = self.check_expr(condition)?;
                if cond_ty != Type::Bool {
                    return Err(anyhow!("If condition must be bool, got '{}'", cond_ty));
                }
                self.check_block(then)?;
                if let Some(else_body) = else_ {
                    self.check_block(else_body)?;
                }
            }
            Stmt::While { condition, body } => {
                let cond_ty = self.check_expr(condition)?;
                if cond_ty != Type::Bool {
                    return Err(anyhow!("While condition must be bool, got '{}'", cond_ty));
                }
                self.check_block(body)?;
            }
            Stmt::For {
                variable,
                iterable,
                body,
            } => {
                let iter_ty = self.check_expr(iterable)?;
                let elem_ty = match &iter_ty {
                    Type::Array(inner) => inner.as_ref().clone(),
                    _ => return Err(anyhow!("For loop requires array, got '{}'", iter_ty)),
                };
                self.scopes.push(HashMap::new());
                self.scopes
                    .last_mut()
                    .unwrap()
                    .insert(variable.clone(), elem_ty);
                self.check_block(body)?;
                self.scopes.pop();
            }
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.check_expr(e)?;
                }
            }
            Stmt::Print(expr) => {
                self.check_expr(expr)?;
            }
            Stmt::ExprStmt(expr) => {
                self.check_expr(expr)?;
            }
        }
        Ok(())
    }

    fn check_expr(&self, expr: &Expr) -> Result<Type> {
        match expr {
            Expr::Int(_) => Ok(Type::I64),
            Expr::Float(_) => Ok(Type::F64),
            Expr::Str(_) => Ok(Type::String),
            Expr::Bool(_) => Ok(Type::Bool),
            Expr::MoneyLiteral { currency, .. } => Ok(Type::Money(currency.clone())),
            Expr::DecimalLiteral(_) => Ok(Type::Decimal),
            Expr::Ident(name) => self.lookup_var(name),
            Expr::ArrayLiteral(elems) => {
                if elems.is_empty() {
                    return Ok(Type::Array(Box::new(Type::I64)));
                }
                let first = self.check_expr(&elems[0])?;
                for e in &elems[1..] {
                    let ty = self.check_expr(e)?;
                    if ty != first {
                        return Err(anyhow!("Array elements must have same type"));
                    }
                }
                Ok(Type::Array(Box::new(first)))
            }
            Expr::BinaryOp { op, left, right } => {
                let lt = self.check_expr(left)?;
                let rt = self.check_expr(right)?;
                self.check_binop(op, &lt, &rt)
            }
            Expr::UnaryOp { op, expr } => {
                let ty = self.check_expr(expr)?;
                match op {
                    UnOp::Neg => {
                        if ty != Type::I64 && ty != Type::F64 {
                            return Err(anyhow!("Cannot negate '{}'", ty));
                        }
                        Ok(ty)
                    }
                    UnOp::Not => {
                        if ty != Type::Bool {
                            return Err(anyhow!("Cannot apply '!' to '{}'", ty));
                        }
                        Ok(Type::Bool)
                    }
                }
            }
            Expr::Call { name, args } => {
                // Try user-defined first, then stdlib builtins
                let (param_tys, ret_ty): (Vec<Type>, Option<Type>) = if let Some(sig) =
                    self.functions.get(name).or_else(|| {
                        name.rfind("::")
                            .map(|i| &name[i + 2..])
                            .and_then(|short| self.functions.get(short))
                    }) {
                    sig.clone()
                } else if let Some(b) = stdlib::builtins().get(name.as_str()) {
                    (
                        b.params.iter().map(|p| p.1.clone()).collect(),
                        Some(b.ret.clone()),
                    )
                } else {
                    return Err(anyhow!("Unknown function '{}'", name));
                };

                if args.len() != param_tys.len() {
                    return Err(anyhow!(
                        "Function '{}' expects {} args, got {}",
                        name,
                        param_tys.len(),
                        args.len()
                    ));
                }
                for (i, (arg, expected)) in args.iter().zip(&param_tys).enumerate() {
                    let arg_ty = self.check_expr(arg)?;
                    if arg_ty != *expected {
                        return Err(anyhow!(
                            "Arg {} of '{}': expected '{}', got '{}'",
                            i + 1,
                            name,
                            expected,
                            arg_ty
                        ));
                    }
                }
                Ok(ret_ty.unwrap_or(Type::Void))
            }
            Expr::StructLiteral { name, fields } => {
                let struct_fields = self
                    .structs
                    .get(name)
                    .ok_or_else(|| anyhow!("Unknown struct '{}'", name))?;
                for (fname, fval) in fields {
                    let sf = struct_fields
                        .iter()
                        .find(|f| &f.name == fname)
                        .ok_or_else(|| anyhow!("Unknown field '{}' in struct '{}'", fname, name))?;
                    let val_ty = self.check_expr(fval)?;
                    if val_ty != sf.ty {
                        return Err(anyhow!(
                            "Field '{}.{}': expected '{}', got '{}'",
                            name,
                            fname,
                            sf.ty,
                            val_ty
                        ));
                    }
                }
                Ok(Type::Custom(name.clone()))
            }
            Expr::FieldAccess { target, field } => {
                let ty = self.check_expr(target)?;
                match &ty {
                    Type::Custom(name) => {
                        let struct_fields = self
                            .structs
                            .get(name)
                            .ok_or_else(|| anyhow!("Unknown struct '{}'", name))?;
                        let sf = struct_fields
                            .iter()
                            .find(|f| &f.name == field)
                            .ok_or_else(|| anyhow!("Unknown field '{}' in '{}'", field, name))?;
                        Ok(sf.ty.clone())
                    }
                    _ => Err(anyhow!("Cannot access field '{}' on '{}'", field, ty)),
                }
            }
            Expr::Index { target, index } => {
                let target_ty = self.check_expr(target)?;
                let index_ty = self.check_expr(index)?;
                if index_ty != Type::I64 {
                    return Err(anyhow!("Array index must be i64, got '{}'", index_ty));
                }
                match &target_ty {
                    Type::Array(inner) => Ok(inner.as_ref().clone()),
                    _ => Err(anyhow!("Cannot index into '{}'", target_ty)),
                }
            }
            Expr::OkExpr(value) => {
                let val_ty = self.check_expr(value)?;
                Ok(Type::Result(Box::new(val_ty), Box::new(Type::String)))
            }
            Expr::ErrExpr(error) => {
                let err_ty = self.check_expr(error)?;
                Ok(Type::Result(Box::new(Type::Void), Box::new(err_ty)))
            }
            Expr::PanicExpr(msg) => {
                let msg_ty = self.check_expr(msg)?;
                if msg_ty != Type::String {
                    return Err(anyhow!("panic! requires string message, got '{}'", msg_ty));
                }
                Ok(Type::Void)
            }
            Expr::TryExpr(expr) => {
                let ty = self.check_expr(expr)?;
                match &ty {
                    Type::Result(ok, _err) => Ok(ok.as_ref().clone()),
                    _ => Err(anyhow!("? operator requires Result type, got '{}'", ty)),
                }
            }
        }
    }

    fn check_binop(&self, op: &BinOp, lt: &Type, rt: &Type) -> Result<Type> {
        match op {
            BinOp::Add | BinOp::Sub => {
                if lt == rt {
                    return Ok(lt.clone());
                }
                if let (Type::Money(lc), Type::Money(rc)) = (lt, rt) {
                    if lc != rc {
                        return Err(anyhow!("Currency mismatch: Money<{}> + Money<{}>", lc, rc));
                    }
                    return Ok(Type::Money(lc.clone()));
                }
                // String concatenation
                if let (Type::String, Type::String) = (lt, rt) {
                    return Ok(Type::String);
                }
                Err(anyhow!("Cannot apply {:?} to '{}' and '{}'", op, lt, rt))
            }
            BinOp::Mul | BinOp::Div => {
                if lt == rt {
                    return Ok(lt.clone());
                }
                if let (Type::Money(c), Type::F64) | (Type::F64, Type::Money(c)) = (lt, rt) {
                    return Ok(Type::Money(c.clone()));
                }
                if let (Type::Money(c), Type::I64) | (Type::I64, Type::Money(c)) = (lt, rt) {
                    return Ok(Type::Money(c.clone()));
                }
                Err(anyhow!("Cannot apply {:?} to '{}' and '{}'", op, lt, rt))
            }
            BinOp::Mod => {
                if lt == rt {
                    return Ok(lt.clone());
                }
                Err(anyhow!("Cannot apply {:?} to '{}' and '{}'", op, lt, rt))
            }
            BinOp::Eq | BinOp::Neq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
                if lt == rt {
                    Ok(Type::Bool)
                } else {
                    Err(anyhow!("Cannot compare '{}' and '{}'", lt, rt))
                }
            }
            BinOp::And | BinOp::Or => {
                if *lt == Type::Bool && *rt == Type::Bool {
                    Ok(Type::Bool)
                } else {
                    Err(anyhow!("Logical operators require bool operands"))
                }
            }
        }
    }

    fn lookup_var(&self, name: &str) -> Result<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Ok(ty.clone());
            }
        }
        Err(anyhow!("Undefined variable '{}'", name))
    }

    fn register_structs(&mut self, item: &TopLevel, prefix: &str) {
        match item {
            TopLevel::StructDef { name, fields } => {
                let full_name = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}::{}", prefix, name)
                };
                self.structs.insert(full_name, fields.clone());
            }
            TopLevel::ModuleDef { name, items } => {
                let new_prefix = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}::{}", prefix, name)
                };
                for item in items {
                    self.register_structs(item, &new_prefix);
                }
            }
            _ => {}
        }
    }

    fn register_functions(&mut self, item: &TopLevel, prefix: &str) {
        match item {
            TopLevel::FnDef {
                name, params, ret, ..
            } => {
                let param_tys: Vec<Type> = params.iter().map(|p| p.ty.clone()).collect();
                let full_name = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}::{}", prefix, name)
                };
                self.functions.insert(full_name, (param_tys, ret.clone()));
            }
            TopLevel::ModuleDef { name, items } => {
                let new_prefix = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}::{}", prefix, name)
                };
                for item in items {
                    self.register_functions(item, &new_prefix);
                }
            }
            _ => {}
        }
    }
}
