use crate::ast::*;
use crate::stdlib;
use anyhow::{anyhow, Result};
use std::collections::HashMap;

pub struct TypeChecker {
    structs: HashMap<String, Vec<Field>>,
    enums: HashMap<String, Vec<EnumVariantDef>>,
    /// Generic structs: name -> (type_params, fields)
    generic_structs: HashMap<String, (Vec<crate::ast::TypeParamDef>, Vec<Field>)>,
    /// Generic enums: name -> (type_params, variants)
    generic_enums: HashMap<String, (Vec<crate::ast::TypeParamDef>, Vec<EnumVariantDef>)>,
    functions: HashMap<String, (Vec<Type>, Option<Type>)>,
    fn_defs: HashMap<String, TopLevel>,
    scopes: Vec<HashMap<String, Type>>,
    /// Compile-time constants: name -> (type, evaluated_value)
    constants: HashMap<String, (Type, i64)>,
    /// Depth of nested loops (for break/continue validation)
    loop_depth: usize,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            structs: HashMap::new(),
            enums: HashMap::new(),
            generic_structs: HashMap::new(),
            generic_enums: HashMap::new(),
            functions: HashMap::new(),
            fn_defs: HashMap::new(),
            scopes: vec![HashMap::new()],
            constants: HashMap::new(),
            loop_depth: 0,
        }
    }

    /// Substitute known constant identifiers with their integer values
    fn substitute_const_expr(expr: &Expr, constants: &HashMap<String, (Type, i64)>) -> Expr {
        match expr {
            Expr::Ident(name) => {
                if let Some((_, val)) = constants.get(name) {
                    Expr::Int(*val)
                } else {
                    expr.clone()
                }
            }
            Expr::BinaryOp { left, op, right } => {
                Expr::BinaryOp {
                    op: op.clone(),
                    left: Box::new(Self::substitute_const_expr(left, constants)),
                    right: Box::new(Self::substitute_const_expr(right, constants)),
                }
            }
            Expr::UnaryOp { op, expr } => {
                Expr::UnaryOp {
                    op: op.clone(),
                    expr: Box::new(Self::substitute_const_expr(expr, constants)),
                }
            }
            _ => expr.clone(),
        }
    }

    /// Evaluate a constant expression at compile time (simple cases only)
    fn eval_const_expr(expr: &Expr) -> Option<i64> {
        match expr {
            Expr::Int(n) => Some(*n),
            Expr::Bool(b) => Some(if *b { 1 } else { 0 }),
            Expr::BinaryOp { left, op, right } => {
                let l = Self::eval_const_expr(left)?;
                let r = Self::eval_const_expr(right)?;
                match op {
                    BinOp::Add => Some(l + r),
                    BinOp::Sub => Some(l - r),
                    BinOp::Mul => Some(l * r),
                    BinOp::Div => if r == 0 { None } else { Some(l / r) },
                    BinOp::Mod => if r == 0 { None } else { Some(l % r) },
                    _ => None,
                }
            }
            Expr::UnaryOp { op, expr } => {
                let v = Self::eval_const_expr(expr)?;
                match op {
                    UnOp::Neg => Some(-v),
                    UnOp::Not => Some(if v == 0 { 1 } else { 0 }),
                }
            }
            _ => None,
        }
    }

    /// Substitute type parameters with concrete types (static version for typechecker)

    /// Infer a type parameter from a raw type pattern and a concrete type.
    /// E.g., infer_type_param(TypeParam("T"), "T", Type::I64) => Some(Type::I64)
    fn infer_type_param(raw_pattern: &Type, param_name: &str, concrete: &Type) -> Option<Type> {
        match raw_pattern {
            Type::TypeParam(name) if name == param_name => Some(concrete.clone()),
            Type::Custom { name, type_args } if name == param_name && type_args.is_empty() => Some(concrete.clone()),
            Type::Array(inner) => {
                if let Type::Array(concrete_inner) = concrete {
                    Self::infer_type_param(inner, param_name, concrete_inner)
                } else {
                    None
                }
            }
            Type::Option(inner) => {
                if let Type::Option(concrete_inner) = concrete {
                    Self::infer_type_param(inner, param_name, concrete_inner)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn substitute_type_static(ty: &Type, sub: &std::collections::HashMap<String, Type>) -> Type {
        match ty {
            Type::TypeParam(name) => {
                sub.get(name).cloned().unwrap_or_else(|| ty.clone())
            }
            Type::Custom { name, type_args } => {
                if let Some(replacement) = sub.get(name) {
                    replacement.clone()
                } else {
                    let new_args: Vec<Type> = type_args.iter().map(|a| Self::substitute_type_static(a, sub)).collect();
                    Type::Custom { name: name.clone(), type_args: new_args }
                }
            }
            Type::Array(inner) => Type::Array(Box::new(Self::substitute_type_static(inner, sub))),
            Type::Option(inner) => Type::Option(Box::new(Self::substitute_type_static(inner, sub))),
            Type::Result(ok, err) => Type::Result(
                Box::new(Self::substitute_type_static(ok, sub)),
                Box::new(Self::substitute_type_static(err, sub)),
            ),
            Type::Fn(params, ret) => Type::Fn(
                params.iter().map(|p| Self::substitute_type_static(p, sub)).collect(),
                Box::new(Self::substitute_type_static(ret, sub)),
            ),
            Type::Future(inner) => Type::Future(Box::new(Self::substitute_type_static(inner, sub))),
            _ => ty.clone(),
        }
    }

    fn c_type_name(ty: &Type) -> String {
        match ty {
            Type::I64 => "long".to_string(),
            Type::F64 => "double".to_string(),
            Type::Bool => "int".to_string(),
            Type::String => "string".to_string(),
            Type::Money(c) => format!("Money_{}", c),
            Type::Decimal => "decimal".to_string(),
            Type::Unit(u) => u.clone(),
            Type::Array(inner) => format!("{}_arr", Self::c_type_name(inner)),
            Type::Void => "void".to_string(),
            Type::Custom { name, .. } => name.clone(),
            Type::Option(inner) => format!("Option_{}", Self::c_type_name(inner)),
            Type::Result(ok, _) => format!("Result_{}", Self::c_type_name(ok)),
            Type::Fn(_, _) => "fn_ptr".to_string(),
            Type::Future(inner) => format!("Future_{}", Self::c_type_name(inner)),
            Type::TypeParam(name) => name.clone(),
        }
    }

    pub fn check(&mut self, program: &Program) -> Result<()> {
        for (name, sig) in stdlib::builtins() {
            let param_tys: Vec<Type> = sig.params.iter().map(|p| p.1.clone()).collect();
            self.functions.insert(name, (param_tys, Some(sig.ret)));
        }

        for item in &program.items {
            self.register_structs(item, "");
        }

        for item in &program.items {
            self.register_enums(item, "");
        }

        for item in &program.items {
            self.register_functions(item, "");
        }

        // Process use imports
        for item in &program.items {
            if let TopLevel::Use { path, wildcard } = item {
                self.process_use(path, *wildcard)?;
            }
        }

        // v1.0: Register ledger validate functions
        for item in &program.items {
            if let TopLevel::LedgerDef(ledger) = item {
                let fn_name = format!("__validate_{}", ledger.name);
                self.functions.insert(fn_name, (vec![], Some(Type::I64)));
            }
        }

        // v1.0: Register database query functions
        for item in &program.items {
            if let TopLevel::DatabaseDef(db) = item {
                for query in &db.queries {
                    let fn_name = format!("{}_{}", db.name, query.name);
                    let param_tys: Vec<Type> = query.params.iter().map(|p| p.ty.clone()).collect();
                    self.functions
                        .insert(fn_name, (param_tys, query.ret.clone()));
                }
            }
        }

        // Register impl block methods
        for item in &program.items {
            if let TopLevel::ImplDef { type_name, methods, .. } = item {
                // Build Self → ConcreteType substitution
                let self_sub: HashMap<String, Type> = vec![
                    ("Self".to_string(), Type::custom(&type_name)),
                ].into_iter().collect();
                for method in methods {
                    if let TopLevel::FnDef { name, params, ret, .. } = method {
                        let param_tys: Vec<Type> = params.iter()
                            .map(|p| Self::substitute_type(&p.ty, &self_sub))
                            .collect();
                        let ret_sub = ret.as_ref().map(|t| Self::substitute_type(t, &self_sub));
                        let full_name = format!("{}_{}", type_name, name);
                        self.functions.insert(full_name, (param_tys, ret_sub));
                    }
                }
            }
        }

        // Check impl block methods
        for item in &program.items {
            if let TopLevel::ImplDef { type_name, methods, .. } = item {
                for method in methods {
                    if let TopLevel::FnDef { name, params, body, .. } = method {
                        self.scopes.push(HashMap::new());
                        for p in params {
                            self.scopes.last_mut().unwrap().insert(p.name.clone(), p.ty.clone());
                        }
                        self.check_block(body)?;
                        self.scopes.pop();
                        println!("  ✓ Method '{}::{}' type-checked", type_name, name);
                    }
                }
            }
        }

        // Process compile-time constants (multi-pass for forward references)
        for item in &program.items {
            if let TopLevel::ConstDef { name, ty, value } = item {
                // Substitute known constants in the expression
                let substituted = Self::substitute_const_expr(value, &self.constants);
                let val_ty = self.check_expr(&substituted)?;
                if let Some(evaluated) = Self::eval_const_expr(&substituted) {
                    let const_ty = ty.clone().unwrap_or(val_ty.clone());
                    self.constants.insert(name.clone(), (const_ty, evaluated));
                }
                // Also register in scope as a variable
                self.scopes.last_mut().unwrap().insert(name.clone(), val_ty);
            }
        }

        // Check test functions
        for item in &program.items {
            if let TopLevel::TestDef { name, body, .. } = item {
                self.scopes.push(HashMap::new());
                self.loop_depth += 1;
                self.check_block(body)?;
                self.loop_depth -= 1;
                self.scopes.pop();
                println!("  ✓ Test '{}' type-checked", name);
            }
        }

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
            if let TopLevel::AsyncFnDef {
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
                println!("  ✓ Async function '{}' type-checked", name);
            }
        }

        // v1.0: Validate ledgers
        for item in &program.items {
            if let TopLevel::LedgerDef(ledger) = item {
                self.validate_ledger(ledger)?;
                println!("  ✓ Ledger '{}' validated", ledger.name);
            }
        }

        // v1.0: Validate databases
        for item in &program.items {
            if let TopLevel::DatabaseDef(db) = item {
                self.validate_database(db)?;
                println!("  ✓ Database '{}' validated", db.name);
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
                    if !self.types_compatible(expected, &val_ty) {
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
                if !self.types_compatible(&var_ty, &val_ty) {
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
            Stmt::IfLet { pattern, value, then, else_ } => {
                let _val_ty = self.check_expr(value)?;
                self.scopes.push(HashMap::new());
                // Bind pattern variables (simplified: variable patterns only)
                if let Pattern::Variable(name) = pattern {
                    self.scopes.last_mut().unwrap().insert(name.clone(), _val_ty.clone());
                }
                self.check_block(then)?;
                self.scopes.pop();
                if let Some(else_body) = else_ {
                    self.check_block(else_body)?;
                }
            }
            Stmt::While { condition, body } => {
                let cond_ty = self.check_expr(condition)?;
                if cond_ty != Type::Bool {
                    return Err(anyhow!("While condition must be bool, got '{}'", cond_ty));
                }
                self.loop_depth += 1;
                self.check_block(body)?;
                self.loop_depth -= 1;
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
                self.loop_depth += 1;
                self.check_block(body)?;
                self.loop_depth -= 1;
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
            Stmt::Break | Stmt::Continue => {
                if self.loop_depth == 0 {
                    return Err(anyhow!("'break'/'continue' outside of loop"));
                }
            }
        }
        Ok(())
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<Type> {
        match expr {
            Expr::Int(_) => Ok(Type::I64),
            Expr::Float(_) => Ok(Type::F64),
            Expr::Str(_) => Ok(Type::String),
            Expr::Bool(_) => Ok(Type::Bool),
            Expr::MoneyLiteral { currency, .. } => Ok(Type::Money(currency.clone())),
            Expr::DecimalLiteral(_) => Ok(Type::Decimal),
            Expr::UnitLiteral { unit, .. } => Ok(Type::Unit(unit.clone())),
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
                        if !matches!(ty, Type::I64 | Type::F64 | Type::Decimal | Type::Unit(_)) {
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
            Expr::Call { name, type_args, args } => {
                // Special-case: len() — polymorphic over strings and arrays
                if name == "len" && type_args.is_empty() {
                    if args.len() != 1 {
                        return Err(anyhow!("'len' takes exactly 1 argument, got {}", args.len()));
                    }
                    let arg_ty = self.check_expr(&args[0])?;
                    match &arg_ty {
                        Type::String | Type::Array(_) => return Ok(Type::I64),
                        _ => return Err(anyhow!("'len' is not defined for type '{}'", arg_ty)),
                    }
                }

                let (mut param_tys, mut ret_ty): (Vec<Type>, Option<Type>) = if let Some(sig) =
                    self.functions.get(name).or_else(|| {
                        name.rfind("::")
                            .map(|i| &name[i + 2..])
                            .and_then(|short| self.functions.get(short))
                    }).or_else(|| {
                        let mangled = name.replace("::", "_");
                        self.functions.get(&mangled)
                    }) {
                    sig.clone()
                } else if let Some(b) = stdlib::builtins().get(name.as_str()) {
                    (
                        b.params.iter().map(|p| p.1.clone()).collect(),
                        Some(b.ret.clone()),
                    )
                } else if let Some(ty) = self.resolve_variable_type(name) {
                    if let Type::Fn(params, ret) = ty {
                        (params, Some(*ret))
                    } else {
                        return Err(anyhow!("'{}' is not callable (type '{}')", name, ty));
                    }
                } else {
                    return Err(anyhow!("Unknown function '{}'", name));
                };

                // Monomorphize: if type_args are provided, substitute type parameters
                if !type_args.is_empty() {
                    let lookup_name = name.clone();
                    if let Some(TopLevel::FnDef { type_params, .. }) = self.fn_defs.get(&lookup_name) {
                        if type_params.len() != type_args.len() {
                            return Err(anyhow!(
                                "Function '{}' expects {} type params, got {}",
                                name, type_params.len(), type_args.len()
                            ));
                        }
                        // Build substitution map: T → concrete type
                        let sub: HashMap<String, Type> = type_params.iter().map(|tp| tp.name.clone()).zip(type_args.iter())
                            .map(|(tp, concrete)| (tp, concrete.clone()))
                            .collect();
                        // Substitute in param types and return type
                        param_tys = param_tys.iter().map(|t| Self::substitute_type(t, &sub)).collect();
                        ret_ty = ret_ty.map(|t| Self::substitute_type(&t, &sub));

                    }
                }

                // Check arg count — allow fewer args if defaults exist
                let max_params = param_tys.len();
                let min_params = if let Some(TopLevel::FnDef { params, .. }) = self.fn_defs.get(name) {
                    params.iter().filter(|p| p.default.is_none()).count()
                } else {
                    max_params
                };
                if args.len() < min_params || args.len() > max_params {
                    return Err(anyhow!(
                        "Function '{}' expects {}-{} args, got {}",
                        name, min_params, max_params, args.len()
                    ));
                }
                for (i, (arg, expected)) in args.iter().zip(&param_tys).enumerate() {
                    let arg_ty = self.check_expr(arg)?;
                    if !self.types_compatible(expected, &arg_ty) {
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
            Expr::StructLiteral { name, type_args, fields } => {
                if !type_args.is_empty() {
                    // Generic struct literal — register concrete struct and validate fields
                    let concrete_name = format!("{}_{}", name, type_args.iter()
                        .map(|t| Self::c_type_name(t))
                        .collect::<Vec<_>>()
                        .join("_"));
                    // Look up the generic definition from generic_structs
                    if let Some((type_params, generic_fields)) = self.generic_structs.get(name).cloned() {
                        // Build substitution: T -> concrete type
                        let sub: std::collections::HashMap<String, Type> = type_params.iter()
                            .map(|tp| tp.name.clone()).zip(type_args.iter())
                            .map(|(tp, concrete)| (tp, concrete.clone()))
                            .collect();
                        // Register the concrete struct with substituted field types
                        let concrete_fields: Vec<Field> = generic_fields.iter().map(|f| {
                            let concrete_ty = Self::substitute_type_static(&f.ty, &sub);
                            Field { name: f.name.clone(), ty: concrete_ty }
                        }).collect();
                        self.structs.insert(concrete_name.clone(), concrete_fields);
                    }
                    // Validate field values
                    for (fname, fval) in fields {
                        let _ = self.check_expr(fval)?;
                    }
                    Ok(Type::custom(&concrete_name))
                } else {
                    let struct_fields = self
                        .structs
                        .get(name)
                        .cloned()
                        .ok_or_else(|| anyhow!("Unknown struct '{}'", name))?;
                    for (fname, fval) in fields {
                        let sf = struct_fields
                            .iter()
                            .find(|f| &f.name == fname)
                            .ok_or_else(|| anyhow!("Unknown field '{}' in '{}'", fname, name))?;
                        let val_ty = self.check_expr(fval)?;
                        if !self.types_compatible(&sf.ty, &val_ty) {
                            return Err(anyhow!(
                                "Field '{}.{}': expected '{}', got '{}'",
                                name,
                                fname,
                                sf.ty,
                                val_ty
                            ));
                        }
                    }
                    Ok(Type::custom(&name))
                }
            }
            Expr::FieldAccess { target, field } => {
                let ty = self.check_expr(target)?;
                match &ty {
                    Type::Custom { name, .. } => {
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
            Expr::SomeExpr(value) => {
                let val_ty = self.check_expr(value)?;
                Ok(Type::Option(Box::new(val_ty)))
            }
            Expr::NoneExpr => {
                Ok(Type::Option(Box::new(Type::Void)))
            }
            Expr::PanicExpr(msg) => {
                let msg_ty = self.check_expr(msg)?;
                if msg_ty != Type::String {
                    return Err(anyhow!("panic! requires string message, got '{}'", msg_ty));
                }
                Ok(Type::Void)
            }
            Expr::AssertExpr { condition, message } => {
                let cond_ty = self.check_expr(condition)?;
                if cond_ty != Type::Bool {
                    return Err(anyhow!("assert condition must be bool, got '{}'", cond_ty));
                }
                if let Some(msg) = message {
                    let msg_ty = self.check_expr(msg)?;
                    if msg_ty != Type::String {
                        return Err(anyhow!("assert message must be string, got '{}'", msg_ty));
                    }
                }
                Ok(Type::Void)
            }
            Expr::AssertEqExpr { left, right, message } => {
                let _left_ty = self.check_expr(left)?;
                let _right_ty = self.check_expr(right)?;
                if let Some(msg) = message {
                    let msg_ty = self.check_expr(msg)?;
                    if msg_ty != Type::String {
                        return Err(anyhow!("assert_eq message must be string, got '{}'", msg_ty));
                    }
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
            Expr::EnumVariant {
                enum_name,
                type_args,
                variant,
                payload,
            } => {
                let (has_payload_v, raw_payload_ty_v) = {
                    let ev = self
                        .enums
                        .get(enum_name)
                        .ok_or_else(|| anyhow!("Unknown enum '{}'", enum_name))?;
                    let v = ev.iter().find(|v| v.name == *variant).ok_or_else(|| {
                        anyhow!("Unknown variant '{}' in enum '{}'", variant, enum_name)
                    })?;
                    (v.payload.is_some(), v.payload.clone())
                };

                let has_payload = has_payload_v;
                let raw_payload_ty = raw_payload_ty_v;

                // If type_args are empty and enum is generic, infer from payload
                let generic_info = self.generic_enums.get(enum_name).cloned();
                let resolved_type_args = if type_args.is_empty() {
                    if let Some((type_params, _)) = &generic_info {
                        if let (Some(ref payload_expr), Some(ref raw_payload_ty_pat)) = (&payload, &raw_payload_ty) {
                            let arg_ty = self.check_expr(payload_expr)?;
                            // Try to infer type params from the payload
                            let mut sub: std::collections::HashMap<String, Type> = std::collections::HashMap::new();
                            let mut all_resolved = true;
                            for tp in type_params.iter().map(|tp| tp.name.as_str()) {
                                if let Some(concrete) = Self::infer_type_param(raw_payload_ty_pat, tp, &arg_ty) {
                                    sub.insert(tp.to_string(), concrete);
                                } else {
                                    all_resolved = false;
                                }
                            }
                            if all_resolved && sub.len() == type_params.len() {
                                type_params.iter().map(|tp| sub.get(&tp.name).cloned().unwrap_or(Type::Void)).collect()
                            } else {
                                type_args.clone()
                            }
                        } else {
                            type_args.clone()
                        }
                    } else {
                        type_args.clone()
                    }
                } else {
                    type_args.clone()
                };

                // Build substitution from resolved type args
                let sub: std::collections::HashMap<String, Type> = if let Some((type_params, _)) = &generic_info {
                    type_params.iter().map(|tp| tp.name.clone())
                        .zip(resolved_type_args.iter())
                        .map(|(tp, concrete)| (tp, concrete.clone()))
                        .collect()
                } else {
                    std::collections::HashMap::new()
                };

                // Get the expected payload type (substituted if generic)
                let expected_payload = if let Some(ref raw_ty) = raw_payload_ty {
                    if sub.is_empty() {
                        Some(raw_ty.clone())
                    } else {
                        Some(Self::substitute_type_static(raw_ty, &sub))
                    }
                } else {
                    None
                };

                match (has_payload, expected_payload, payload) {
                    (false, _, None) => Ok(if resolved_type_args.is_empty() {
                        Type::custom(&enum_name)
                    } else {
                        Type::Custom { name: enum_name.clone(), type_args: resolved_type_args }
                    }),
                    (true, Some(expected), Some(expr)) => {
                        // For generic enums without explicit type args, check against raw TypeParam
                        let arg_ty = if resolved_type_args.is_empty() {
                            self.check_expr(expr)?
                        } else {
                            let checked = self.check_expr(expr)?;
                            if !self.types_compatible(&expected, &checked) {
                                return Err(anyhow!(
                                    "Enum variant '{}::{}': expected payload '{}', got '{}'",
                                    enum_name, variant, expected, checked
                                ));
                            }
                            checked
                        };

                        // If we still have unresolved type params, infer them
                        let final_type_args = if resolved_type_args.is_empty() {
                            if let Some((type_params, _)) = &generic_info {
                                if let Some(ref raw_ty) = raw_payload_ty {
                                    let mut sub2: std::collections::HashMap<String, Type> = std::collections::HashMap::new();
                                    let mut all_resolved = true;
                                    for tp in type_params.iter().map(|tp| tp.name.as_str()) {
                                        if let Some(concrete) = Self::infer_type_param(raw_ty, tp, &arg_ty) {
                                            sub2.insert(tp.to_string(), concrete);
                                        } else {
                                            all_resolved = false;
                                        }
                                    }
                                    if all_resolved && sub2.len() == type_params.len() {
                                        type_params.iter().map(|tp| sub2.get(&tp.name).cloned().unwrap_or(Type::Void)).collect()
                                    } else {
                                        return Err(anyhow!(
                                            "Enum variant '{}::{}': cannot infer type parameter from payload '{}' (got '{}')",
                                            enum_name, variant, raw_ty, arg_ty
                                        ));
                                    }
                                } else {
                                    resolved_type_args
                                }
                            } else {
                                resolved_type_args
                            }
                        } else {
                            resolved_type_args
                        };

                        Ok(Type::Custom { name: enum_name.clone(), type_args: final_type_args })
                    }
                    (false, _, Some(_)) => {
                        Err(anyhow!("Variant '{}' does not take a payload", variant))
                    }
                    (true, _, None) => Err(anyhow!("Variant '{}' requires a payload", variant)),
                    _ => unreachable!(),
                }
            }
            Expr::Await(expr) => {
                let ty = self.check_expr(expr)?;
                match ty {
                    Type::Future(inner) => Ok(*inner),
                    _ => Err(anyhow!("Cannot await non-Future type '{}'", ty)),
                }
            }
            Expr::Match { scrutinee, arms } => {
                let scrutinee_ty = self.check_expr(scrutinee)?;
                let mut result_ty: Option<Type> = None;
                for arm in arms {
                    self.scopes.push(HashMap::new());
                    // Bind pattern variables into scope
                    match &arm.pattern {
                        Pattern::Variable(name) => {
                            self.scopes
                                .last_mut()
                                .unwrap()
                                .insert(name.clone(), scrutinee_ty.clone());
                        }
                        Pattern::EnumVariant {
                            enum_name,
                            variant,
                            binding: Some(b),
                        } => {
                            let payload_ty =
                                self.enums.get(enum_name).cloned().and_then(|variants| {
                                    variants
                                        .iter()
                                        .find(|v| v.name == *variant)?
                                        .payload
                                        .clone()
                                });
                            if let Some(ty) = payload_ty {
                                self.scopes.last_mut().unwrap().insert(b.clone(), ty);
                            }
                        }
                        Pattern::SomePattern { binding: Some(b) } => {
                            if let Type::Option(inner) = &scrutinee_ty {
                                self.scopes.last_mut().unwrap().insert(b.clone(), inner.as_ref().clone());
                            }
                        }
                        Pattern::SomePattern { binding: None } | Pattern::NonePattern => {}
                        _ => {}
                    }
                    // Typecheck guard expression if present
                    if let Some(ref guard) = arm.guard {
                        let guard_ty = self.check_expr(guard)?;
                        if !matches!(guard_ty, Type::Bool) {
                            // Allow it but warn — guard should be bool
                        }
                    }
                    self.check_block(&arm.body)?;
                    if let Some(last) = arm.body.last() {
                        let arm_ty = match last {
                            Stmt::ExprStmt(e) => self.check_expr(e)?,
                            Stmt::Return(Some(e)) => self.check_expr(e)?,
                            _ => Type::Void,
                        };
                        match &result_ty {
                            None => result_ty = Some(arm_ty),
                            Some(prev) => {
                                if !self.types_compatible(prev, &arm_ty) {
                                    return Err(anyhow!(
                                        "Match arm type mismatch: '{}' vs '{}'",
                                        prev,
                                        arm_ty
                                    ));
                                }
                            }
                        }
                    }
                    self.scopes.pop();
                }
                Ok(result_ty.unwrap_or(Type::Void))
            }
            Expr::Lambda { params, ret, body } => {
                self.scopes.push(HashMap::new());
                let mut param_tys = Vec::new();
                for p in params {
                    self.scopes
                        .last_mut()
                        .unwrap()
                        .insert(p.name.clone(), p.ty.clone());
                    param_tys.push(p.ty.clone());
                }
                for stmt in body {
                    self.check_stmt(stmt)?;
                }
                let ret_ty = ret.clone().unwrap_or(Type::I64);
                self.scopes.pop();
                Ok(Type::Fn(param_tys, Box::new(ret_ty)))
            }
            Expr::MethodCall { target, method, args } => {
                // Resolve target type
                let target_ty = self.check_expr(target)?;
                // For known types, resolve method as Type_method
                let full_name = match &target_ty {
                    Type::Custom { name: type_name, .. } => {
                        format!("{}_{}", type_name, method)
                    }
                    Type::String => {
                        format!("string_{}", method)
                    }
                    Type::Array(_) => {
                        format!("array_{}", method)
                    }
                    _ => {
                        // Try as function name directly
                        method.clone()
                    }
                };
                // Look up the function signature
                let (param_tys, ret_ty) = if let Some(sig) = self.functions.get(&full_name) {
                    sig.clone()
                } else {
                    return Err(anyhow!("Unknown method '{}' for type '{}'", method, target_ty));
                };
                // Check first param matches target type (self param)
                if let Some(first_param) = param_tys.first() {
                    if !self.types_compatible(first_param, &target_ty) {
                        return Err(anyhow!(
                            "Method '{}.{}': expected self type '{}', got '{}'",
                            target_ty, method, first_param, target_ty
                        ));
                    }
                }
                // Check remaining args
                for (i, (arg, expected)) in args.iter().zip(param_tys.iter().skip(1)).enumerate() {
                    let arg_ty = self.check_expr(arg)?;
                    if !self.types_compatible(expected, &arg_ty) {
                        return Err(anyhow!(
                            "Arg {} of '{}.{}': expected '{}', got '{}'",
                            i + 1, target_ty, method, expected, arg_ty
                        ));
                    }
                }
                Ok(ret_ty.unwrap_or(Type::Void))
            }
            Expr::Range { start, end, .. } => {
                let start_ty = self.check_expr(start)?;
                let end_ty = self.check_expr(end)?;
                if !matches!(start_ty, Type::I64) {
                    return Err(anyhow!("Range start must be i64, got '{}'", start_ty));
                }
                if !matches!(end_ty, Type::I64) {
                    return Err(anyhow!("Range end must be i64, got '{}'", end_ty));
                }
                Ok(Type::Array(Box::new(Type::I64)))
            }
            Expr::FString(parts) => {
                for part in parts {
                    if let crate::ast::FStringPart::Expr(expr) = part {
                        self.check_expr(expr)?;
                    }
                }
                Ok(Type::String)
            }
        }
    }

    /// Check if two types are compatible (same type or auto-coercion)
    fn resolve_variable_type(&self, name: &str) -> Option<Type> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.get(name) {
                return Some(ty.clone());
            }
        }
        None
    }

    /// Substitute type parameters in a type with concrete types.
    fn substitute_type(ty: &Type, sub: &HashMap<String, Type>) -> Type {
        match ty {
            Type::TypeParam(name) => {
                sub.get(name).cloned().unwrap_or_else(|| ty.clone())
            }
            // Custom("T") is used by parser for generic type params — substitute too
            Type::Custom { name, .. } => {
                sub.get(name).cloned().unwrap_or_else(|| ty.clone())
            }
            Type::Array(inner) => Type::Array(Box::new(Self::substitute_type(inner, sub))),
            Type::Option(inner) => Type::Option(Box::new(Self::substitute_type(inner, sub))),
            Type::Result(ok, err) => Type::Result(
                Box::new(Self::substitute_type(ok, sub)),
                Box::new(Self::substitute_type(err, sub)),
            ),
            Type::Fn(params, ret) => Type::Fn(
                params.iter().map(|p| Self::substitute_type(p, sub)).collect(),
                Box::new(Self::substitute_type(ret, sub)),
            ),
            Type::Future(inner) => Type::Future(Box::new(Self::substitute_type(inner, sub))),
            _ => ty.clone(),
        }
    }

    fn types_compatible(&self, expected: &Type, actual: &Type) -> bool {
        if expected == actual {
            return true;
        }
        // None (Option<void>) is compatible with any Option<T>
        if let (Type::Option(_), Type::Option(inner)) = (expected, actual) {
            if matches!(inner.as_ref(), Type::Void) {
                return true;
            }
        }
        if let (Type::Option(inner), Type::Option(_)) = (expected, actual) {
            if matches!(inner.as_ref(), Type::Void) {
                return true;
            }
        }
        // Decimal accepts Int and Float literals
        // Future<T> is compatible with i64 (handle is a long)
        matches!(
            (expected, actual),
            (Type::Decimal, Type::I64)
                | (Type::Decimal, Type::F64)
                | (Type::F64, Type::I64)
                | (Type::I64, Type::Future(_))
                | (Type::Future(_), Type::I64)
        )
    }

    fn check_binop(&self, op: &BinOp, lt: &Type, rt: &Type) -> Result<Type> {
        match op {
            BinOp::Add | BinOp::Sub => {
                if lt == rt {
                    return Ok(lt.clone());
                }
                // Money + Money (same currency)
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
                // Unit + Unit (same unit)
                if let (Type::Unit(lu), Type::Unit(ru)) = (lt, rt) {
                    if lu != ru {
                        return Err(anyhow!("Unit mismatch: {} + {}", lu, ru));
                    }
                    return Ok(Type::Unit(lu.clone()));
                }
                // Decimal + Int/Float
                if matches!(lt, Type::Decimal) && matches!(rt, Type::I64 | Type::F64) {
                    return Ok(Type::Decimal);
                }
                if matches!(rt, Type::Decimal) && matches!(lt, Type::I64 | Type::F64) {
                    return Ok(Type::Decimal);
                }
                Err(anyhow!("Cannot apply {:?} to '{}' and '{}'", op, lt, rt))
            }
            BinOp::Mul | BinOp::Div => {
                if lt == rt {
                    return Ok(lt.clone());
                }
                // Money * scalar
                if let (Type::Money(c), Type::F64) | (Type::F64, Type::Money(c)) = (lt, rt) {
                    return Ok(Type::Money(c.clone()));
                }
                if let (Type::Money(c), Type::I64) | (Type::I64, Type::Money(c)) = (lt, rt) {
                    return Ok(Type::Money(c.clone()));
                }
                // Decimal * scalar
                if matches!(lt, Type::Decimal) && matches!(rt, Type::I64 | Type::F64) {
                    return Ok(Type::Decimal);
                }
                if matches!(rt, Type::Decimal) && matches!(lt, Type::I64 | Type::F64) {
                    return Ok(Type::Decimal);
                }
                // Unit * scalar → Unit
                if let (Type::Unit(u), Type::I64) | (Type::I64, Type::Unit(u)) = (lt, rt) {
                    return Ok(Type::Unit(u.clone()));
                }
                if let (Type::Unit(u), Type::F64) | (Type::F64, Type::Unit(u)) = (lt, rt) {
                    return Ok(Type::Unit(u.clone()));
                }
                // Unit * Unit → composite (simplified: just use product notation)
                if let (Type::Unit(lu), Type::Unit(ru)) = (lt, rt) {
                    if *op == BinOp::Mul {
                        return Ok(Type::Unit(format!("{}·{}", lu, ru)));
                    }
                    // Unit / Unit → dimensionless ratio
                    return Ok(Type::F64);
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
                if self.types_compatible(lt, rt) || self.types_compatible(rt, lt) {
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
        // Check if it's a known function (function reference)
        if let Some((param_tys, ret_ty)) = self.functions.get(name) {
            let ret = ret_ty.clone().unwrap_or(Type::Void);
            return Ok(Type::Fn(param_tys.clone(), Box::new(ret)));
        }
        Err(anyhow!("Undefined variable '{}'", name))
    }

    fn register_structs(&mut self, item: &TopLevel, prefix: &str) {
        match item {
            TopLevel::StructDef { name, type_params, fields, .. } => {
                if !type_params.is_empty() {
                    let full_name = if prefix.is_empty() {
                        name.clone()
                    } else {
                        format!("{}::{}", prefix, name)
                    };
                    self.generic_structs.insert(full_name, (type_params.clone(), fields.clone()));
                }
                let full_name = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}::{}", prefix, name)
                };
                self.structs.insert(full_name, fields.clone());
            }
            TopLevel::ModuleDef { name, items, .. } => {
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

    fn register_enums(&mut self, item: &TopLevel, prefix: &str) {
        match item {
            TopLevel::EnumDef { name, type_params, variants, .. } => {
                let full_name = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}::{}", prefix, name)
                };
                if !type_params.is_empty() {
                    self.generic_enums.insert(full_name.clone(), (type_params.clone(), variants.clone()));
                }
                self.enums.insert(full_name, variants.clone());
            }
            TopLevel::ModuleDef { name, items, .. } => {
                let new_prefix = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}::{}", prefix, name)
                };
                for item in items {
                    self.register_enums(item, &new_prefix);
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
                self.functions.insert(full_name.clone(), (param_tys, ret.clone()));
                self.fn_defs.insert(full_name, item.clone());
            }
            TopLevel::AsyncFnDef {
                name, params, ret, ..
            } => {
                let param_tys: Vec<Type> = params.iter().map(|p| p.ty.clone()).collect();
                // Async fn returns Future<RetType> in the type system
                let future_ret = ret
                    .as_ref()
                    .map(|t| Type::Future(Box::new(t.clone())))
                    .unwrap_or(Type::Future(Box::new(Type::Void)));
                let full_name = if prefix.is_empty() {
                    name.clone()
                } else {
                    format!("{}::{}", prefix, name)
                };
                self.functions
                    .insert(full_name.clone(), (param_tys, Some(future_ret)));
                self.fn_defs.insert(full_name, item.clone());
            }
            TopLevel::ModuleDef { name, items, .. } => {
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

    // ── v1.0: Ledger validation ──

    fn validate_ledger(&self, ledger: &LedgerDef) -> Result<()> {
        if ledger.entries.is_empty() {
            return Err(anyhow!("Ledger '{}' has no entries", ledger.name));
        }
        // Check that debits == credits
        let mut total_debit: i64 = 0;
        let mut total_credit: i64 = 0;
        for entry in &ledger.entries {
            let amount = self.eval_literal_i64(&entry.amount)?;
            match entry.side {
                LedgerSide::Debit => total_debit += amount,
                LedgerSide::Credit => total_credit += amount,
            }
        }
        if total_debit != total_credit {
            return Err(anyhow!(
                "Ledger '{}' is unbalanced: debits ({}) != credits ({})",
                ledger.name,
                total_debit,
                total_credit
            ));
        }
        Ok(())
    }

    fn eval_literal_i64(&self, expr: &Expr) -> Result<i64> {
        match expr {
            Expr::Int(n) => Ok(*n),
            Expr::MoneyLiteral { amount, .. } => Ok(*amount as i64 * 10000),
            Expr::Ident(name) => {
                // Look up in scope
                for scope in self.scopes.iter().rev() {
                    if let Some(ty) = scope.get(name) {
                        // For money variables, return 0 (we can't resolve at check time)
                        if matches!(ty, Type::Money(_)) {
                            return Ok(0);
                        }
                    }
                }
                Ok(0)
            }
            Expr::BinaryOp { op, left, right } => {
                let l = self.eval_literal_i64(left)?;
                let r = self.eval_literal_i64(right)?;
                match op {
                    BinOp::Add => Ok(l + r),
                    BinOp::Sub => Ok(l - r),
                    BinOp::Mul => Ok(l * r),
                    BinOp::Div => Ok(if r != 0 { l / r } else { 0 }),
                    _ => Ok(0),
                }
            }
            _ => Ok(0),
        }
    }

    // ── v2.0: use imports ──

    fn process_use(&mut self, path: &[String], wildcard: bool) -> Result<()> {
        let prefix = path.join("::");
        if wildcard {
            // Import all functions from the module
            let module_prefix = format!("{}::", prefix);
            let names: Vec<String> = self
                .functions
                .keys()
                .filter(|k| k.starts_with(&module_prefix))
                .cloned()
                .collect();
            for full_name in names {
                if let Some(sig) = self.functions.get(&full_name).cloned() {
                    let short_name = full_name[module_prefix.len()..].to_string();
                    self.functions.insert(short_name, sig);
                }
            }
            // Import structs
            let struct_names: Vec<String> = self
                .structs
                .keys()
                .filter(|k| k.starts_with(&module_prefix))
                .cloned()
                .collect();
            for full_name in struct_names {
                if let Some(fields) = self.structs.get(&full_name).cloned() {
                    let short_name = full_name[module_prefix.len()..].to_string();
                    self.structs.insert(short_name, fields);
                }
            }
            // Import enums
            let enum_names: Vec<String> = self
                .enums
                .keys()
                .filter(|k| k.starts_with(&module_prefix))
                .cloned()
                .collect();
            for full_name in enum_names {
                if let Some(variants) = self.enums.get(&full_name).cloned() {
                    let short_name = full_name[module_prefix.len()..].to_string();
                    self.enums.insert(short_name, variants);
                }
            }
        } else {
            // Import a specific name
            let short_name = path
                .last()
                .ok_or_else(|| anyhow!("Empty use path"))?
                .clone();
            if let Some(sig) = self.functions.get(&prefix).cloned() {
                self.functions.insert(short_name.clone(), sig);
            } else if let Some(fields) = self.structs.get(&prefix).cloned() {
                self.structs.insert(short_name.clone(), fields);
            } else if let Some(variants) = self.enums.get(&prefix).cloned() {
                self.enums.insert(short_name.clone(), variants);
            } else {
                return Err(anyhow!(
                    "Cannot import '{}' — not found in any module",
                    prefix
                ));
            }
        }
        Ok(())
    }

    // ── v1.0: Database validation ──

    fn validate_database(&self, db: &DatabaseDef) -> Result<()> {
        // Check table names are unique
        let mut table_names = std::collections::HashSet::new();
        for table in &db.tables {
            if !table_names.insert(&table.name) {
                return Err(anyhow!(
                    "Duplicate table '{}' in database '{}'",
                    table.name,
                    db.name
                ));
            }
        }
        // Check query names are unique
        let mut query_names = std::collections::HashSet::new();
        for query in &db.queries {
            if !query_names.insert(&query.name) {
                return Err(anyhow!(
                    "Duplicate query '{}' in database '{}'",
                    query.name,
                    db.name
                ));
            }
            // Validate query references existing tables
            match &query.kind {
                QueryKind::Select { from_table, .. } => {
                    if !db.tables.iter().any(|t| &t.name == from_table) {
                        return Err(anyhow!(
                            "Query '{}' references unknown table '{}' in database '{}'",
                            query.name,
                            from_table,
                            db.name
                        ));
                    }
                }
                QueryKind::Insert { table, .. } => {
                    if !db.tables.iter().any(|t| &t.name == table) {
                        return Err(anyhow!(
                            "Query '{}' references unknown table '{}' in database '{}'",
                            query.name,
                            table,
                            db.name
                        ));
                    }
                }
                QueryKind::Update { table, .. } => {
                    if !db.tables.iter().any(|t| &t.name == table) {
                        return Err(anyhow!(
                            "Query '{}' references unknown table '{}' in database '{}'",
                            query.name,
                            table,
                            db.name
                        ));
                    }
                }
                QueryKind::Delete { table, .. } => {
                    if !db.tables.iter().any(|t| &t.name == table) {
                        return Err(anyhow!(
                            "Query '{}' references unknown table '{}' in database '{}'",
                            query.name,
                            table,
                            db.name
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}
