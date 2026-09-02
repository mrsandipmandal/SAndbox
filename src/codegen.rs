use crate::ast::*;
use crate::stdlib;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt::Write;

const MONEY_SCALE: i64 = 10000;
const DECIMAL_SCALE: i64 = 1_000_000_000_000_000_000; // 10^18

pub struct CodeGen {
    output: String,
    indent: usize,
    var_types: HashMap<String, String>,
    var_counter: Cell<usize>,
    enums: HashMap<String, Vec<EnumVariantDef>>,
    fn_returns: HashMap<String, String>,
    fn_defaults: HashMap<String, Vec<Option<Expr>>>,
    lambda_captures: HashMap<String, Vec<String>>,
    async_fns: std::collections::HashSet<String>,
    var_to_lambda: HashMap<String, String>,
    fn_sigs: HashMap<String, (Vec<String>, String)>, // (param C types, return C type)
    lambda_counter: Cell<usize>,
    #[allow(clippy::type_complexity)]
    pending_lambdas: RefCell<Vec<(String, Vec<Param>, Option<Type>, Vec<Stmt>, Vec<String>)>>,
    lambda_return_idx: Cell<usize>,
    /// Monomorphized function names already generated
    mono_fns: std::collections::HashSet<String>,
    /// Maps "funcname__type1_type2" -> monomorphized C function name
    mono_map: std::collections::HashMap<String, String>,
    /// Stores function definitions for monomorphization
    fn_defs: HashMap<String, crate::ast::TopLevel>,
    /// Pending monomorphizations to emit after main generation
    pending_mono: RefCell<Vec<(String, crate::ast::TopLevel, Vec<crate::ast::Type>)>>,
    /// Generic struct definitions: name -> (type_params, fields)
    generic_structs: HashMap<String, (Vec<String>, Vec<Field>)>,
    /// Monomorphized struct typedefs already generated
    mono_structs: std::collections::HashSet<String>,
    /// Pending struct monomorphizations: (mono_name, original_name, type_args)
    pending_mono_structs: RefCell<Vec<(String, String, Vec<Type>)>>,
    test_filter: Option<String>,
}

impl CodeGen {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
            var_counter: Cell::new(0),
            enums: HashMap::new(),
            fn_returns: HashMap::new(),
            fn_defaults: HashMap::new(),
            lambda_captures: HashMap::new(),
            async_fns: std::collections::HashSet::new(),
            var_to_lambda: HashMap::new(),
            fn_sigs: HashMap::new(),
            var_types: HashMap::new(),
            lambda_counter: Cell::new(0),
            pending_lambdas: RefCell::new(Vec::new()),
            lambda_return_idx: Cell::new(0),
            mono_fns: std::collections::HashSet::new(),
            mono_map: std::collections::HashMap::new(),
            fn_defs: HashMap::new(),
            pending_mono: RefCell::new(Vec::new()),
            generic_structs: HashMap::new(),
            mono_structs: std::collections::HashSet::new(),
            pending_mono_structs: RefCell::new(Vec::new()),
            test_filter: None,
        }
    }

    pub fn generate(&mut self, program: &Program, filter: Option<&str>) -> String {
        self.test_filter = filter.map(|s| s.to_string());
        writeln!(self.output, "#include <stdio.h>").unwrap();
        writeln!(self.output, "#include <stdlib.h>").unwrap();
        writeln!(self.output, "#include <string.h>").unwrap();
        writeln!(self.output, "#include <math.h>").unwrap();
        writeln!(self.output).unwrap();

        self.output.push_str(&stdlib::c_preamble());
        writeln!(self.output).unwrap();

        // Pre-register function definitions for monomorphization
        self.preregister_fn_defs(&program.items);

        // Pre-scan for generic function calls
        self.prescan_generic_calls(&program.items);

        // Pre-register generic struct definitions (needed before prescan)
        for item in &program.items {
            if let TopLevel::StructDef { name, type_params, fields, .. } = item {
                if !type_params.is_empty() {
                    self.generic_structs.insert(name.clone(), (type_params.clone(), fields.clone()));
                }
            }
        }

        // Pre-scan for generic struct usages
        self.prescan_generic_structs(&program.items);

        // Emit monomorphized struct typedefs
        let pending_structs: Vec<_> = self.pending_mono_structs.borrow().clone();
        for (mono_name, original_name, type_args) in &pending_structs {
            self.monomorphize_struct(mono_name, original_name, type_args);
        }

        // Emit forward declarations for monomorphized functions
        let pending: Vec<_> = self.pending_mono.borrow().clone();
        for (mono_name, fn_def, type_args) in &pending {
            if let TopLevel::FnDef { params, ret, .. } = fn_def {
                let sub: std::collections::HashMap<String, crate::ast::Type> = 
                    if let TopLevel::FnDef { type_params, .. } = &fn_def {
                        type_params.iter().zip(type_args.iter())
                            .map(|(tp, concrete)| (tp.clone(), concrete.clone()))
                            .collect()
                    } else { std::collections::HashMap::new() };
                let sub_params: Vec<crate::ast::Param> = params.iter().map(|p| crate::ast::Param {
                    name: p.name.clone(),
                    ty: Self::substitute_type_codegen(&p.ty, &sub),
                    default: None,
                }).collect();
                let sub_ret = ret.as_ref().map(|t| Self::substitute_type_codegen(t, &sub));
                self.gen_fn_decl(&mono_name, &sub_params, &sub_ret);
            }
        }
        writeln!(self.output).unwrap();

        self.gen_program_items(&program.items);

        // Emit monomorphized function bodies
        let pending: Vec<_> = self.pending_mono.borrow().clone();
        for (mono_name, fn_def, type_args) in &pending {
            self.monomorphize_function(&fn_def, type_args);
        }

        // If there are tests, emit main that runs them
        let has_tests = program.items.iter().any(|item| matches!(item, TopLevel::TestDef { .. }));
        if has_tests {
            self.gen_test_main();
        }

        // Emit pending lambda definitions
        if !self.pending_lambdas.borrow().is_empty() {
            let lambdas: Vec<_> = self.pending_lambdas.borrow().clone();
            self.output.push_str("\n// Lambda definitions\n");
            for (name, params, ret, body, captures) in lambdas {
                // Add capture params
                let mut all_params = params.clone();
                for cap in &captures {
                    all_params.push(Param { name: cap.clone(), ty: Type::I64, default: None });
                }
                self.gen_fn(&name, &all_params, &ret, &body);
            }
        }
        self.output.clone()
    }

    fn preregister_fn_defs(&mut self, items: &[TopLevel]) {
        for item in items {
            if let TopLevel::FnDef { name, .. } = item {
                self.fn_defs.insert(name.clone(), item.clone());
            }
            if let TopLevel::ModuleDef { items, .. } = item {
                self.preregister_fn_defs(items);
            }
        }
    }

    /// Pre-scan AST for generic function calls and register needed monomorphizations.
    fn prescan_generic_calls(&self, items: &[TopLevel]) {
        for item in items {
            match item {
                TopLevel::FnDef { body, .. } => {
                    self.prescan_stmts(body);
                }
                TopLevel::ModuleDef { items, .. } => {
                    self.prescan_generic_calls(items);
                }
                _ => {}
            }
        }
    }

    /// Pre-scan AST for generic struct usages (StructLiteral with type_args)
    fn prescan_generic_structs(&self, items: &[TopLevel]) {
        for item in items {
            match item {
                TopLevel::FnDef { body, .. } | TopLevel::AsyncFnDef { body, .. } => {
                    self.prescan_struct_usage_in_stmts(body);
                }
                TopLevel::ImplDef { methods, .. } => {
                    for method in methods {
                        if let TopLevel::FnDef { body, .. } = method {
                            self.prescan_struct_usage_in_stmts(body);
                        }
                    }
                }
                TopLevel::ModuleDef { items, .. } => {
                    self.prescan_generic_structs(items);
                }
                _ => {}
            }
        }
    }

    fn prescan_struct_usage_in_stmts(&self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::Let { value, .. } => self.prescan_struct_usage_in_expr(value),
                Stmt::Assign { value, .. } => self.prescan_struct_usage_in_expr(value),
                Stmt::If { condition, then, else_ } => {
                    self.prescan_struct_usage_in_expr(condition);
                    self.prescan_struct_usage_in_stmts(then);
                    if let Some(e) = else_ { self.prescan_struct_usage_in_stmts(e); }
                }
                Stmt::While { condition, body } => {
                    self.prescan_struct_usage_in_expr(condition);
                    self.prescan_struct_usage_in_stmts(body);
                }
                Stmt::For { iterable, body, .. } => {
                    self.prescan_struct_usage_in_expr(iterable);
                    self.prescan_struct_usage_in_stmts(body);
                }
                Stmt::Return(Some(e)) => self.prescan_struct_usage_in_expr(e),
                Stmt::ExprStmt(e) => self.prescan_struct_usage_in_expr(e),
                Stmt::Print(e) => self.prescan_struct_usage_in_expr(e),
                _ => {}
            }
        }
    }

    fn prescan_struct_usage_in_expr(&self, expr: &Expr) {
        match expr {
            Expr::StructLiteral { name, type_args, .. } if !type_args.is_empty() => {
                self.queue_struct_mono(name, type_args);
            }
            Expr::BinaryOp { left, right, .. } => {
                self.prescan_struct_usage_in_expr(left);
                self.prescan_struct_usage_in_expr(right);
            }
            Expr::UnaryOp { expr, .. } => self.prescan_struct_usage_in_expr(expr),
            Expr::Call { args, .. } => {
                for a in args { self.prescan_struct_usage_in_expr(a); }
            }
            Expr::MethodCall { target, args, .. } => {
                self.prescan_struct_usage_in_expr(target);
                for a in args { self.prescan_struct_usage_in_expr(a); }
            }
            Expr::FieldAccess { target, .. } => self.prescan_struct_usage_in_expr(target),
            Expr::Index { target, index } => {
                self.prescan_struct_usage_in_expr(target);
                self.prescan_struct_usage_in_expr(index);
            }
            Expr::Match { scrutinee, arms } => {
                self.prescan_struct_usage_in_expr(scrutinee);
                for arm in arms { self.prescan_struct_usage_in_stmts(&arm.body); }
            }
            Expr::SomeExpr(e) | Expr::Await(e) | Expr::PanicExpr(e) | Expr::OkExpr(e) | Expr::ErrExpr(e) | Expr::TryExpr(e) => {
                self.prescan_struct_usage_in_expr(e);
            }
            Expr::FString(parts) => {
                for part in parts {
                    if let crate::ast::FStringPart::Expr(e) = part {
                        self.prescan_struct_usage_in_expr(e);
                    }
                }
            }
            _ => {}
        }
    }

    /// Queue a struct monomorphization if needed
    fn queue_struct_mono(&self, name: &str, type_args: &[Type]) {

        if let Some((type_params, _fields)) = self.generic_structs.get(name) {
            if type_params.len() == type_args.len() {
                let type_suffix: Vec<String> = type_args.iter().map(|t| Self::type_id(t)).collect();
                let mono_name = format!("{}_{}", name, type_suffix.join("_"));
                let key = format!("{}__{}", name, type_args.iter().map(|t| format!("{}", t)).collect::<Vec<_>>().join(","));
                if !self.mono_structs.contains(&key) {
                    self.pending_mono_structs.borrow_mut().push((mono_name, name.to_string(), type_args.to_vec()));
                }
            }
        }
    }

    /// Generate a monomorphized struct typedef
    fn monomorphize_struct(&mut self, mono_name: &str, original_name: &str, type_args: &[Type]) {


        if let Some((type_params, fields)) = self.generic_structs.get(original_name).cloned() {
            let key = format!("{}__{}", original_name, type_args.iter().map(|t| format!("{}", t)).collect::<Vec<_>>().join(","));
            if self.mono_structs.contains(&key) { return; }
            self.mono_structs.insert(key);

            let sub: std::collections::HashMap<String, Type> = type_params.iter()
                .zip(type_args.iter())
                .map(|(tp, concrete)| (tp.clone(), concrete.clone()))
                .collect();

            writeln!(self.output, "typedef struct {{").unwrap();
            for f in &fields {
                let concrete_ty = Self::substitute_type_codegen(&f.ty, &sub);
                writeln!(self.output, "    {} {};", self.c_type(&concrete_ty), f.name).unwrap();
            }
            writeln!(self.output, "}} {};", mono_name).unwrap();
            writeln!(self.output).unwrap();
        }
    }

    fn prescan_stmts(&self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::Let { value, .. } => self.prescan_expr(value),
                Stmt::Assign { value, .. } => self.prescan_expr(value),
                Stmt::If { condition, then, else_ } => {
                    self.prescan_expr(condition);
                    self.prescan_stmts(then);
                    if let Some(e) = else_ { self.prescan_stmts(e); }
                }
                Stmt::While { condition, body } => {
                    self.prescan_expr(condition);
                    self.prescan_stmts(body);
                }
                Stmt::Return(Some(e)) => self.prescan_expr(e),
                Stmt::ExprStmt(e) => self.prescan_expr(e),
                Stmt::Print(e) => self.prescan_expr(e),
                _ => {}
            }
        }
    }

    fn prescan_expr(&self, expr: &Expr) {
        if let Expr::Call { name, type_args, args } = expr {
            if !type_args.is_empty() {
                if let Some(fn_def) = self.fn_defs.get(name.as_str()) {
                    let type_suffix: Vec<String> = type_args.iter().map(|t| self.c_type(t)).collect();
                    let mono_name = format!("{}_{}", Self::c_mangle(name), type_suffix.join("_"));
                    let key = format!("{}__{}",
                        if let TopLevel::FnDef { name, .. } = fn_def { name } else { "" },
                        type_args.iter().map(|t| format!("{}", t)).collect::<Vec<_>>().join(","));
                    if !self.mono_fns.contains(&key) {
                        self.pending_mono.borrow_mut().push((mono_name, fn_def.clone(), type_args.clone()));
                    }
                }
            }
            for a in args { self.prescan_expr(a); }
        }
    }

    /// Register module::function signatures
    fn register_module_fn_sigs(&mut self, items: &[TopLevel]) {
        for item in items {
            if let TopLevel::ModuleDef { name: mod_name, items: sub_items, .. } = item {
                for sub in sub_items {
                    if let TopLevel::FnDef { name, params, ret, .. } = sub {
                        let c_ret = ret.as_ref().map_or("void".to_string(), |t| self.c_type(t));
                        let c_params: Vec<String> = params.iter().map(|p| self.c_type(&p.ty)).collect();
                        // Register qualified name (math_add)
                        let qualified = format!("{}_{}", mod_name, name);
                        self.fn_returns.insert(qualified.clone(), c_ret.clone());
                        self.fn_sigs.insert(qualified, (c_params.clone(), c_ret.clone()));
                        // Also register unqualified name for `use` imports (mangle if needed)
                        let unqual_c = Self::c_mangle(name);
                        self.fn_returns.insert(name.clone(), c_ret.clone());
                        self.fn_sigs.insert(name.clone(), (c_params.clone(), c_ret.clone()));
                        if unqual_c != *name {
                            self.fn_returns.insert(unqual_c.clone(), c_ret.clone());
                            self.fn_sigs.insert(unqual_c, (c_params, c_ret));
                        }
                    }
                }
            }
        }
    }

    fn gen_program_items(&mut self, items: &[TopLevel]) {
        // Forward-declare test jmp_buf if there are test functions
        let has_tests = items.iter().any(|item| matches!(item, TopLevel::TestDef { .. }));
        if has_tests {
            writeln!(self.output, "jmp_buf __test_jmp;").unwrap();
            writeln!(self.output).unwrap();
        }

        // Enums — generate tag constants and register for type mapping
        for item in items {
            if let TopLevel::EnumDef { name, variants, .. } = item {
                self.enums.insert(name.clone(), variants.clone());
                self.gen_enum(name, variants);
            }
        }
        // Structs — register generic structs, generate concrete ones
        for item in items {
            if let TopLevel::StructDef { name, type_params, fields, .. } = item {
                if !type_params.is_empty() {
                    // Store generic struct definition for later monomorphization
                    self.generic_structs.insert(name.clone(), (type_params.clone(), fields.clone()));
                } else {
                    self.gen_struct(name, fields);
                }
            }
        }
        // v1.0: Database table structs
        for item in items {
            if let TopLevel::DatabaseDef(db) = item {
                for table in &db.tables {
                    self.gen_database_table_struct(&table.name, &table.columns);
                }
            }
        }
        // v1.0: Ledger validation functions
        for item in items {
            if let TopLevel::LedgerDef(ledger) = item {
                self.gen_ledger_validate_fn(ledger);
            }
        }
        // v1.0: Database query functions
        for item in items {
            if let TopLevel::DatabaseDef(db) = item {
                for query in &db.queries {
                    self.gen_database_query_fn(&db.name, query);
                }
            }
        }
        // Track async function names
        for item in items {
            if let TopLevel::AsyncFnDef { name, .. } = item {
                self.async_fns.insert(name.clone());
            }
        }
        // Register function signatures for type inference and function references
        // Also register module-qualified names
        self.register_module_fn_sigs(items);
        for item in items {
            match item {
                TopLevel::FnDef { name, params, ret, .. }
                | TopLevel::AsyncFnDef { name, params, ret, .. } => {
                    let c_ret = ret.as_ref().map_or("void".to_string(), |t| self.c_type(t));
                    let c_params: Vec<String> = params.iter().map(|p| self.c_type(&p.ty)).collect();
                    // Store defaults for call-site filling
                    let defaults: Vec<Option<Expr>> = params.iter().map(|p| p.default.clone()).collect();
                    if defaults.iter().any(|d| d.is_some()) {
                        self.fn_defaults.insert(name.clone(), defaults);
                    }
                    self.fn_returns.insert(name.clone(), c_ret.clone());
                    self.fn_sigs.insert(name.clone(), (c_params.clone(), c_ret.clone()));
                    let c_name = Self::c_mangle(name);
                    if c_name != *name {
                        self.fn_returns.insert(c_name.clone(), c_ret.clone());
                        self.fn_sigs.insert(c_name, (c_params, c_ret));
                    }
                }
                TopLevel::ImplDef { type_name, methods, .. } => {
                    for method in methods {
                        if let TopLevel::FnDef { name, params, ret, .. } = method {
                            let mangled = format!("{}_{}", type_name, name);
                            let c_ret = ret.as_ref().map_or("void".to_string(), |t| self.c_type(t));
                            let c_params: Vec<String> = params.iter().map(|p| self.c_type(&p.ty)).collect();
                            let defaults: Vec<Option<Expr>> = params.iter().map(|p| p.default.clone()).collect();
                            if defaults.iter().any(|d| d.is_some()) {
                                self.fn_defaults.insert(mangled.clone(), defaults);
                            }
                            self.fn_returns.insert(mangled.clone(), c_ret.clone());
                            self.fn_sigs.insert(mangled, (c_params, c_ret));
                        }
                    }
                }
                _ => {}
            }
        }
        // Pre-scan: discover all lambda expressions and register them
        self.prescan_lambdas(items);
        // Emit lambda forward declarations before function declarations
        if !self.pending_lambdas.borrow().is_empty() {
            let lambdas: Vec<_> = self.pending_lambdas.borrow().clone();
            for (name, params, ret, _, captures) in &lambdas {
                // Lambda params = original params + capture params (long type)
                let mut all_params = params.clone();
                for cap in captures {
                    all_params.push(Param { name: cap.clone(), ty: Type::I64, default: None });
                }
                self.gen_fn_decl(name, &all_params, ret);
            }
        }
        self.gen_fn_decls(items);
        writeln!(self.output).unwrap();
        self.gen_fn_bodies(items);

        // Generate test runner if there are test functions
        let test_names: Vec<String> = items.iter().filter_map(|item| {
            if let TopLevel::TestDef { name, .. } = item {
                Some(format!("__test_{}", Self::c_mangle(name)))
            } else {
                None
            }
        }).collect();
        if !test_names.is_empty() {
            self.gen_test_runner(&test_names);
        }
    }

    fn prescan_lambdas(&mut self, items: &[TopLevel]) {
        for item in items {
            match item {
                TopLevel::FnDef { body, .. } | TopLevel::AsyncFnDef { body, .. } => {
                    self.prescan_lambdas_in_stmts(body);
                }
                TopLevel::ImplDef { methods, .. } => {
                    for method in methods {
                        if let TopLevel::FnDef { body, .. } = method {
                            self.prescan_lambdas_in_stmts(body);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn prescan_lambdas_in_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::Let { name, value, .. } => {
                    // If value is a lambda, map variable name to lambda name
                    if let Expr::Lambda { .. } = value {
                        let lambda_idx = self.lambda_counter.get();
                        let lambda_name = format!("__lambda_{}", lambda_idx);
                        self.var_to_lambda.insert(name.clone(), lambda_name);
                    }
                    self.prescan_lambdas_in_expr(value);
                }
                Stmt::Assign { value, .. } => self.prescan_lambdas_in_expr(value),
                Stmt::If {
                    condition,
                    then,
                    else_,
                } => {
                    self.prescan_lambdas_in_expr(condition);
                    self.prescan_lambdas_in_stmts(then);
                    if let Some(e) = else_ {
                        self.prescan_lambdas_in_stmts(e);
                    }
                }
                Stmt::While { condition, body } => {
                    self.prescan_lambdas_in_expr(condition);
                    self.prescan_lambdas_in_stmts(body);
                }
                Stmt::Return(Some(e)) => self.prescan_lambdas_in_expr(e),
                Stmt::ExprStmt(e) => self.prescan_lambdas_in_expr(e),
                Stmt::Print(e) => self.prescan_lambdas_in_expr(e),
                _ => {}
            }
        }
    }

    fn prescan_lambdas_in_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Lambda { params, ret, body } => {
                let lambda_name = format!("__lambda_{}", self.lambda_counter.get());
                self.lambda_counter.set(self.lambda_counter.get() + 1);
                // Find free variables (captures) in the lambda body
                let param_names: std::collections::HashSet<String> =
                    params.iter().map(|p| p.name.clone()).collect();
                let mut captures = Vec::new();
                Self::find_captures_in_stmts(body, &param_names, &mut captures);
                // Deduplicate captures
                captures.sort();
                captures.dedup();
                // Store captures in separate map keyed by lambda name
                self.lambda_captures.insert(lambda_name.clone(), captures.clone());
                self.pending_lambdas.borrow_mut().push((
                    lambda_name,
                    params.clone(),
                    ret.clone(),
                    body.clone(),
                    captures,
                ));
            }
            Expr::BinaryOp { left, right, .. } => {
                self.prescan_lambdas_in_expr(left);
                self.prescan_lambdas_in_expr(right);
            }
            Expr::UnaryOp { expr, .. } => self.prescan_lambdas_in_expr(expr),
            Expr::Call { args, .. } => {
                for a in args {
                    self.prescan_lambdas_in_expr(a);
                }
            }
            Expr::Match { scrutinee, arms } => {
                self.prescan_lambdas_in_expr(scrutinee);
                for arm in arms {
                    self.prescan_lambdas_in_stmts(&arm.body);
                }
            }
            _ => {}
        }
    }
/// Find free variables (captures) in a lambda body
    fn find_captures_in_stmts(
        stmts: &[Stmt],
        local_scope: &std::collections::HashSet<String>,
        captures: &mut Vec<String>,
    ) {
        for stmt in stmts {
            match stmt {
                Stmt::Let { name, value, .. } => {
                    Self::find_captures_in_expr(value, local_scope, captures);
                    let mut scope = local_scope.clone();
                    scope.insert(name.clone());
                }
                Stmt::Assign { name, value } => {
                    if !local_scope.contains(name) {
                        captures.push(name.clone());
                    }
                    Self::find_captures_in_expr(value, local_scope, captures);
                }
                Stmt::If { condition, then, else_ } => {
                    Self::find_captures_in_expr(condition, local_scope, captures);
                    Self::find_captures_in_stmts(then, local_scope, captures);
                    if let Some(e) = else_ { Self::find_captures_in_stmts(e, local_scope, captures); }
                }
                Stmt::While { condition, body } => {
                    Self::find_captures_in_expr(condition, local_scope, captures);
                    Self::find_captures_in_stmts(body, local_scope, captures);
                }
                Stmt::For { variable, iterable, body } => {
                    Self::find_captures_in_expr(iterable, local_scope, captures);
                    let mut inner = local_scope.clone();
                    inner.insert(variable.clone());
                    Self::find_captures_in_stmts(body, &inner, captures);
                }
                Stmt::Return(Some(e)) => Self::find_captures_in_expr(e, local_scope, captures),
                Stmt::Print(e) => Self::find_captures_in_expr(e, local_scope, captures),
                Stmt::ExprStmt(e) => Self::find_captures_in_expr(e, local_scope, captures),
                Stmt::IfLet { value, then, else_, .. } => {
                    Self::find_captures_in_expr(value, local_scope, captures);
                    Self::find_captures_in_stmts(then, local_scope, captures);
                    if let Some(e) = else_ { Self::find_captures_in_stmts(e, local_scope, captures); }
                }
                _ => {}
            }
        }
    }

    fn find_captures_in_expr(
        expr: &Expr,
        local_scope: &std::collections::HashSet<String>,
        captures: &mut Vec<String>,
    ) {
        match expr {
            Expr::Ident(name) => {
                if !local_scope.contains(name) && !name.starts_with("__") {
                    captures.push(name.clone());
                }
            }
            Expr::BinaryOp { left, right, .. } => {
                Self::find_captures_in_expr(left, local_scope, captures);
                Self::find_captures_in_expr(right, local_scope, captures);
            }
            Expr::UnaryOp { expr, .. } => Self::find_captures_in_expr(expr, local_scope, captures),
            Expr::Call { args, .. } => {
                for a in args { Self::find_captures_in_expr(a, local_scope, captures); }
            }
            Expr::MethodCall { target, args, .. } => {
                Self::find_captures_in_expr(target, local_scope, captures);
                for a in args { Self::find_captures_in_expr(a, local_scope, captures); }
            }
            Expr::FieldAccess { target, .. } => Self::find_captures_in_expr(target, local_scope, captures),
            Expr::Index { target, index } => {
                Self::find_captures_in_expr(target, local_scope, captures);
                Self::find_captures_in_expr(index, local_scope, captures);
            }
            Expr::Match { scrutinee, arms } => {
                Self::find_captures_in_expr(scrutinee, local_scope, captures);
                for arm in arms { Self::find_captures_in_stmts(&arm.body, local_scope, captures); }
            }
            Expr::SomeExpr(e) | Expr::Await(e) | Expr::PanicExpr(e) | Expr::OkExpr(e) | Expr::ErrExpr(e) | Expr::TryExpr(e) => {
                Self::find_captures_in_expr(e, local_scope, captures);
            }
            Expr::NoneExpr => {}
            Expr::FString(parts) => {
                for part in parts {
                    if let crate::ast::FStringPart::Expr(e) = part {
                        Self::find_captures_in_expr(e, local_scope, captures);
                    }
                }
            }
            _ => {}
        }
    }

    fn gen_fn_decls(&mut self, items: &[TopLevel]) {
        for item in items {
            match item {
                TopLevel::FnDef { name, type_params, params, ret, .. }
                | TopLevel::AsyncFnDef { name, type_params, params, ret, .. } => {
                    if !type_params.is_empty() { continue; }
                    let c_name = Self::c_mangle(name);
                    self.gen_fn_decl(&c_name, params, ret);
                }
                TopLevel::ImplDef { type_name, methods, .. } => {
                    for method in methods {
                        if let TopLevel::FnDef { name, params, ret, .. } = method {
                            let mangled = format!("{}_{}", type_name, name);
                            let sub: std::collections::HashMap<String, Type> = vec![
                                ("Self".to_string(), Type::custom(&type_name)),
                            ].into_iter().collect();
                            let resolved_params: Vec<Param> = params.iter().map(|p| Param {
                                name: p.name.clone(),
                                ty: Self::substitute_type_codegen(&p.ty, &sub),
                                default: p.default.clone(),
                            }).collect();
                            let resolved_ret = ret.as_ref().map(|t| Self::substitute_type_codegen(t, &sub));
                            self.gen_fn_decl(&mangled, &resolved_params, &resolved_ret);
                        }
                    }
                }
                TopLevel::TestDef { name, .. } => {
                    let fn_name = format!("__test_{}", Self::c_mangle(name));
                    writeln!(self.output, "int {}();", fn_name).unwrap();
                }
                TopLevel::ModuleDef { name: mod_name, items, .. } => {
                    for sub in items {
                        match sub {
                            TopLevel::FnDef { name, type_params, params, ret, .. } => {
                                if !type_params.is_empty() { continue; }
                                let qualified = format!("{}_{}", mod_name, name);
                                let c_name = Self::c_mangle(&qualified);
                                self.gen_fn_decl(&c_name, params, ret);
                            }
                            TopLevel::ModuleDef { .. } => {
                                // Nested module — recurse (for now, flatten)
                                self.gen_fn_decls(&[sub.clone()]);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn gen_fn_bodies(&mut self, items: &[TopLevel]) {
        for item in items {
            match item {
                TopLevel::FnDef { name, type_params, params, ret, body, .. }
                | TopLevel::AsyncFnDef { name, type_params, params, ret, body, .. } => {
                    if !type_params.is_empty() { continue; }
                    let c_name = Self::c_mangle(name);
                    self.gen_fn(&c_name, params, ret, body);
                }
                TopLevel::ImplDef { type_name, methods, .. } => {
                    for method in methods {
                        if let TopLevel::FnDef { name, params, ret, body, .. } = method {
                            let mangled = format!("{}_{}", type_name, name);
                            // Substitute Self -> concrete type in params
                            let sub: std::collections::HashMap<String, Type> = vec![
                                ("Self".to_string(), Type::custom(&type_name)),
                            ].into_iter().collect();
                            let resolved_params: Vec<Param> = params.iter().map(|p| Param {
                                name: p.name.clone(),
                                ty: Self::substitute_type_codegen(&p.ty, &sub),
                                default: p.default.clone(),
                            }).collect();
                            let resolved_ret = ret.as_ref().map(|t| Self::substitute_type_codegen(t, &sub));
                            self.gen_fn(&mangled, &resolved_params, &resolved_ret, body);
                        }
                    }
                }
                TopLevel::TestDef { name, body, .. } => {
                    let fn_name = format!("__test_{}", Self::c_mangle(name));
                    self.gen_test_fn(&fn_name, name, body);
                }
                TopLevel::ModuleDef { name: mod_name, items, .. } => {
                    for sub in items {
                        match sub {
                            TopLevel::FnDef { name, type_params, params, ret, body, .. } => {
                                if !type_params.is_empty() { continue; }
                                // Generate with qualified name
                                let qualified = format!("{}_{}", mod_name, name);
                                let c_name = Self::c_mangle(&qualified);
                                self.gen_fn(&c_name, params, ret, body);
                                // Also generate unqualified name for `use` imports (mangle if needed)
                                let unqual_c_name = Self::c_mangle(name);
                                self.gen_fn(&unqual_c_name, params, ret, body);
                            }
                            TopLevel::ModuleDef { .. } => {
                                self.gen_fn_bodies(&[sub.clone()]);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn gen_test_fn(&mut self, fn_name: &str, test_name: &str, body: &[Stmt]) {
        writeln!(self.output, "int {}() {{", fn_name).unwrap();
        self.indent += 1;
        self.write_indent();
        writeln!(self.output, "if (setjmp(__test_jmp) == 0) {{").unwrap();
        self.indent += 1;
        for stmt in body {
            self.gen_stmt(stmt);
        }
        self.write_indent();
        writeln!(self.output, "return 0;").unwrap();
        self.indent -= 1;
        self.write_indent();
        writeln!(self.output, "}} else {{").unwrap();
        self.indent += 1;
        self.write_indent();
        writeln!(self.output, "return 1;").unwrap();
        self.indent -= 1;
        self.write_indent();
        writeln!(self.output, "}}").unwrap();
        self.indent -= 1;
        writeln!(self.output, "}}").unwrap();
        writeln!(self.output).unwrap();
    }

    fn gen_test_runner(&mut self, test_names: &[String]) {
        writeln!(self.output, "int __run_tests() {{").unwrap();
        self.indent += 1;
        self.write_indent();
        writeln!(self.output, "int passed = 0;").unwrap();
        self.write_indent();
        writeln!(self.output, "int failed = 0;").unwrap();
        self.write_indent();
        writeln!(self.output, "int skipped = 0;").unwrap();
        self.write_indent();
        writeln!(self.output, "struct timespec __t_start, __t_end;").unwrap();
        self.write_indent();
        writeln!(self.output, "double __total_ms = 0;").unwrap();

        let filter_clone = self.test_filter.clone();
        if let Some(ref filter) = filter_clone {
            self.write_indent();
            writeln!(self.output, "const char* __filter = \"{}\";", filter).unwrap();
        }

        self.write_indent();
        writeln!(self.output, "printf(\"\\n\");").unwrap();

        // Count how many tests actually run (respecting filter)
        let matching_count = if let Some(ref filt) = filter_clone {
            test_names.iter().filter(|n| {
                let dn = n.strip_prefix("__test_").unwrap_or(n);
                dn.contains(filt.as_str())
            }).count()
        } else {
            test_names.len()
        };

        self.write_indent();
        writeln!(self.output, "printf(\"running {} test(s)\\n\");", matching_count).unwrap();
        self.write_indent();
        writeln!(self.output, "printf(\"\\n\");").unwrap();

        for name in test_names {
            let display_name = name.strip_prefix("__test_").unwrap_or(name);

            // Emit filter check if filter is set
            if filter_clone.is_some() {
                self.write_indent();
                writeln!(self.output, "if (strstr(\"{}\", __filter) == NULL) {{ skipped++; goto __skip_{}; }}", display_name, Self::c_mangle(display_name)).unwrap();
            }

            self.write_indent();
            writeln!(self.output, "clock_gettime(CLOCK_MONOTONIC, &__t_start);").unwrap();
            self.write_indent();
            writeln!(self.output, "if ({name}() == 0) {{", name=name).unwrap();
            self.indent += 1;
            self.write_indent();
            writeln!(self.output, "clock_gettime(CLOCK_MONOTONIC, &__t_end);").unwrap();
            self.write_indent();
            writeln!(self.output, "double __ms = (__t_end.tv_sec - __t_start.tv_sec) * 1000.0 + (__t_end.tv_nsec - __t_start.tv_nsec) / 1000000.0;").unwrap();
            self.write_indent();
            writeln!(self.output, "__total_ms += __ms;").unwrap();
            self.write_indent();
            writeln!(self.output, "printf(\"  \\x1b[32m\\u2713\\x1b[0m {} (\\x1b[90m%.1fms\\x1b[0m)\\n\", __ms);", display_name).unwrap();
            self.write_indent();
            writeln!(self.output, "passed++;").unwrap();
            self.indent -= 1;
            self.write_indent();
            writeln!(self.output, "}} else {{").unwrap();
            self.indent += 1;
            self.write_indent();
            writeln!(self.output, "clock_gettime(CLOCK_MONOTONIC, &__t_end);").unwrap();
            self.write_indent();
            writeln!(self.output, "double __ms = (__t_end.tv_sec - __t_start.tv_sec) * 1000.0 + (__t_end.tv_nsec - __t_start.tv_nsec) / 1000000.0;").unwrap();
            self.write_indent();
            writeln!(self.output, "__total_ms += __ms;").unwrap();
            self.write_indent();
            writeln!(self.output, "printf(\"  \\x1b[31m\\u2717\\x1b[0m {} (\\x1b[90m%.1fms\\x1b[0m)\\n\", __ms);", display_name).unwrap();
            self.write_indent();
            writeln!(self.output, "failed++;").unwrap();
            self.indent -= 1;
            self.write_indent();
            writeln!(self.output, "}}").unwrap();

            // Emit skip label
            if filter_clone.is_some() {
                self.write_indent();
                writeln!(self.output, "__skip_{}: ;", Self::c_mangle(display_name)).unwrap();
            }
        }

        self.write_indent();
        writeln!(self.output, "printf(\"\\n\");").unwrap();
        self.write_indent();
        writeln!(self.output, "if (failed == 0) {{").unwrap();
        self.indent += 1;
        self.write_indent();
        writeln!(self.output, "if (skipped > 0) {{ printf(\"  \\x1b[33m%d skipped\\x1b[0m, \\x1b[32m%d passed\\x1b[0m in \\x1b[90m%.1fms\\x1b[0m\\n\", skipped, passed, __total_ms); }} else {{ printf(\"  \\x1b[32m%d passed\\x1b[0m in \\x1b[90m%.1fms\\x1b[0m\\n\", passed, __total_ms); }}").unwrap();
        self.indent -= 1;
        self.write_indent();
        writeln!(self.output, "}} else {{").unwrap();
        self.indent += 1;
        self.write_indent();
        writeln!(self.output, "printf(\"  \\x1b[31m%d failed\\x1b[0m, \\x1b[32m%d passed\\x1b[0m in \\x1b[90m%.1fms\\x1b[0m\\n\", failed, passed, __total_ms);").unwrap();
        self.indent -= 1;
        self.write_indent();
        writeln!(self.output, "}}").unwrap();
        self.write_indent();
        writeln!(self.output, "printf(\"\\n\");").unwrap();
        self.write_indent();
        writeln!(self.output, "return failed > 0 ? 1 : 0;").unwrap();
        self.indent -= 1;
        writeln!(self.output, "}}").unwrap();
        writeln!(self.output).unwrap();
    }

    fn gen_test_main(&mut self) {
        writeln!(self.output, "int main() {{ return __run_tests(); }}").unwrap();
        writeln!(self.output).unwrap();
    }

    // ── v1.0: Database table struct ──

    fn gen_database_table_struct(&mut self, name: &str, columns: &[ColumnDef]) {
        writeln!(self.output, "typedef struct {{").unwrap();
        for col in columns {
            writeln!(self.output, "    {} {};", self.c_type(&col.ty), col.name).unwrap();
        }
        writeln!(self.output, "}} {}_row;", name).unwrap();
        writeln!(self.output).unwrap();
    }

    // ── v1.0: Ledger validation function ──

    fn gen_ledger_validate_fn(&mut self, ledger: &LedgerDef) {
        let fn_name = format!("__validate_{}", ledger.name);
        writeln!(self.output, "int {}() {{", fn_name).unwrap();
        self.indent += 1;
        self.write_indent();
        writeln!(self.output, "// Ledger '{}' validation", ledger.name).unwrap();
        self.write_indent();
        writeln!(self.output, "long total_debit = 0;").unwrap();
        self.write_indent();
        writeln!(self.output, "long total_credit = 0;").unwrap();
        for entry in &ledger.entries {
            let amount = self.gen_expr(&entry.amount);
            self.write_indent();
            match entry.side {
                LedgerSide::Debit => {
                    writeln!(self.output, "total_debit += {};", amount).unwrap();
                }
                LedgerSide::Credit => {
                    writeln!(self.output, "total_credit += {};", amount).unwrap();
                }
            }
        }
        self.write_indent();
        writeln!(
            self.output,
            "if (total_debit != total_credit) {{
        fprintf(stderr, \"Ledger '{}' unbalanced: %ld != %ld\\n\", total_debit, total_credit);
        return 1;
    }}",
            ledger.name
        )
        .unwrap();
        self.write_indent();
        writeln!(self.output, "return 0;").unwrap();
        self.indent -= 1;
        writeln!(self.output, "}}").unwrap();
        writeln!(self.output).unwrap();
    }

    // ── v1.0: Database query function ──

    fn gen_database_query_fn(&mut self, db_name: &str, query: &QueryDef) {
        let fn_name = format!("{}_{}", db_name, query.name);
        let params_str = query
            .params
            .iter()
            .map(|p| self.c_param(&p.ty, &p.name))
            .collect::<Vec<_>>()
            .join(", ");
        // Collect table names from the same database
        let table_names: Vec<String> = Vec::new(); // placeholder
        let ret_str = if let Some(ret_ty) = &query.ret {
            match ret_ty {
                Type::Custom { name, .. } => {
                    // "void" is a keyword, table types get _row suffix
                    if name == "void" {
                        "void".to_string()
                    } else {
                        format!("{}_row", name)
                    }
                }
                Type::Void => "void".to_string(),
                _ => self.c_type(ret_ty),
            }
        } else {
            "void".to_string()
        };
        let _ = table_names;

        writeln!(self.output, "{} {}({}) {{", ret_str, fn_name, params_str).unwrap();
        self.indent += 1;

        match &query.kind {
            QueryKind::Select {
                columns,
                from_table,
                where_clause,
            } => {
                self.write_indent();
                writeln!(self.output, "// SELECT from {}", from_table).unwrap();
                self.write_indent();
                writeln!(
                    self.output,
                    "printf(\"SELECT {} FROM {}\\n\");",
                    columns
                        .iter()
                        .map(|c| match c {
                            SqlExpr::Column(n) => n.clone(),
                            SqlExpr::Star => "*".to_string(),
                            SqlExpr::Literal(e) => self.gen_expr(e),
                        })
                        .collect::<Vec<_>>()
                        .join(", "),
                    from_table
                )
                .unwrap();
                if let Some(wc) = where_clause {
                    self.write_indent();
                    writeln!(
                        self.output,
                        "printf(\"  WHERE condition = %ld\\n\", {});",
                        self.gen_expr(wc)
                    )
                    .unwrap();
                }
            }
            QueryKind::Insert {
                table,
                columns,
                values: _,
            } => {
                self.write_indent();
                writeln!(self.output, "// INSERT INTO {}", table).unwrap();
                self.write_indent();
                writeln!(
                    self.output,
                    "printf(\"INSERT INTO {} ({})\\n\");",
                    table,
                    columns.join(", ")
                )
                .unwrap();
            }
            QueryKind::Update {
                table,
                set_clauses,
                where_clause,
            } => {
                self.write_indent();
                writeln!(self.output, "// UPDATE {}", table).unwrap();
                let sets: Vec<String> = set_clauses
                    .iter()
                    .map(|(c, v)| format!("{} = {}", c, self.gen_expr(v)))
                    .collect();
                self.write_indent();
                writeln!(
                    self.output,
                    "printf(\"UPDATE {} SET {}\\n\");",
                    table,
                    sets.join(", ")
                )
                .unwrap();
                if let Some(wc) = where_clause {
                    self.write_indent();
                    writeln!(
                        self.output,
                        "printf(\"  WHERE condition = %ld\\n\", {});",
                        self.gen_expr(wc)
                    )
                    .unwrap();
                }
            }
            QueryKind::Delete {
                table,
                where_clause,
            } => {
                self.write_indent();
                writeln!(self.output, "// DELETE FROM {}", table).unwrap();
                self.write_indent();
                writeln!(self.output, "printf(\"DELETE FROM {}\\n\");", table).unwrap();
                if let Some(wc) = where_clause {
                    self.write_indent();
                    writeln!(
                        self.output,
                        "printf(\"  WHERE condition = %ld\\n\", {});",
                        self.gen_expr(wc)
                    )
                    .unwrap();
                }
            }
        }

        self.indent -= 1;
        writeln!(self.output, "}}").unwrap();
        writeln!(self.output).unwrap();
    }

    fn gen_fn_decl(&mut self, name: &str, params: &[Param], ret: &Option<Type>) {
        let ret_str = if name == "main" {
            "int".into()
        } else {
            ret.as_ref().map_or("void".into(), |t| self.c_type(t))
        };
        let params_str = params
            .iter()
            .map(|p| self.c_param(&p.ty, &p.name))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(self.output, "{} {}({});", ret_str, name, params_str).unwrap();
    }

    fn gen_struct(&mut self, name: &str, fields: &[Field]) {
        writeln!(self.output, "typedef struct {{").unwrap();
        for f in fields {
            writeln!(self.output, "    {} {};", self.c_type(&f.ty), f.name).unwrap();
        }
        writeln!(self.output, "}} {};", name).unwrap();
        writeln!(self.output).unwrap();
    }

    fn gen_enum(&mut self, name: &str, variants: &[EnumVariantDef]) {
        // Tag constants — each variant gets a unique integer
        for (i, v) in variants.iter().enumerate() {
            writeln!(self.output, "#define {}_{} {}L", name, v.name, i).unwrap();
        }
        writeln!(self.output).unwrap();
    }

    fn gen_fn(&mut self, name: &str, params: &[Param], ret: &Option<Type>, body: &[Stmt]) {
        let has_defaults = params.iter().any(|p| p.default.is_some());
        let fn_name = if has_defaults { format!("{}_inner", name) } else { name.to_string() };

        // Emit function signature
        let ret_str = if name == "main" {
            "int".into()
        } else {
            ret.as_ref().map_or("void".into(), |t| self.c_type(t))
        };
        let params_str = params.iter()
            .map(|p| self.c_param(&p.ty, &p.name))
            .collect::<Vec<_>>()
            .join(", ");

        self.var_types.clear();
        writeln!(self.output, "{} {}({}) {{", ret_str, fn_name, params_str).unwrap();
        self.indent += 1;

        for (i, stmt) in body.iter().enumerate() {
            let is_last = i == body.len() - 1;
            if is_last && name != "main" {
                match stmt {
                    Stmt::ExprStmt(e) => {
                        self.write_indent();
                        writeln!(self.output, "return {};", self.gen_expr(e)).unwrap();
                    }
                    Stmt::Return(_) => {
                        self.gen_stmt(stmt);
                    }
                    _ => {
                        self.gen_stmt(stmt);
                        if ret.is_some() {
                            self.write_indent();
                            writeln!(self.output, "return 0;").unwrap();
                        }
                    }
                }
            } else {
                self.gen_stmt(stmt);
            }
        }

        if name == "main" {
            self.write_indent();
            writeln!(self.output, "return 0;").unwrap();
        }

        self.indent -= 1;
        writeln!(self.output, "}}").unwrap();
        writeln!(self.output).unwrap();

        // If function has default params, emit wrapper + overloads
        if has_defaults {
            // Wrapper: same signature, delegates to _inner
            writeln!(self.output, "{} {}({}) {{", ret_str, name, params_str).unwrap();
            self.indent += 1;
            let args_str = params.iter().map(|p| p.name.clone()).collect::<Vec<_>>().join(", ");
            if ret.is_some() {
                self.write_indent();
                writeln!(self.output, "return {}_inner({});", name, args_str).unwrap();
            } else {
                self.write_indent();
                writeln!(self.output, "{}_inner({});", name, args_str).unwrap();
            }
            self.indent -= 1;
            writeln!(self.output, "}}").unwrap();
            writeln!(self.output).unwrap();

        }
    }

    fn gen_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::IfLet { pattern, value, then, else_ } => {
                let val = self.gen_expr(value);
                // Bind pattern variable
                match pattern {
                    Pattern::Variable(name) => {
                        self.var_types.insert(name.clone(), self.infer_c_type(value));
                        self.write_indent();
                        writeln!(self.output, "long {} = {};", name, val).unwrap();
                    }
                    Pattern::SomePattern { binding: Some(b) } => {
                        self.var_types.insert(b.clone(), "long".to_string());
                        self.write_indent();
                        writeln!(self.output, "long {} = {};", b, val).unwrap();
                    }
                    _ => {}
                }
                // Compute condition: SomePattern => val != 0, NonePattern => val == 0, Variable => val != 0
                let cond = match pattern {
                    Pattern::NonePattern => format!("({}) == 0", val),
                    _ => format!("({}) != 0", val),
                };
                self.write_indent();
                writeln!(self.output, "if ({}) {{", cond).unwrap();
                self.indent += 1;
                for s in then {
                    self.gen_stmt(s);
                }
                self.indent -= 1;
                if let Some(else_body) = else_ {
                    self.write_indent();
                    writeln!(self.output, "}} else {{").unwrap();
                    self.indent += 1;
                    for s in else_body {
                        self.gen_stmt(s);
                    }
                    self.indent -= 1;
                }
                self.write_indent();
                writeln!(self.output, "}}").unwrap();
            }
            Stmt::Let {
                name, ty, value, ..
            } => {
                // If assigning a lambda to a variable, generate it as a named function
                if let Expr::Lambda { params, ret, body, .. } = value {
                    let lambda_name = self.var_to_lambda.get(name.as_str()).cloned()
                        .unwrap_or_else(|| name.clone());
                    let captures = self.lambda_captures.get(&lambda_name).cloned().unwrap_or_default();
                    let mut all_params = params.clone();
                    for cap in &captures {
                        all_params.push(Param { name: cap.clone(), ty: Type::I64, default: None });
                    }
                    // Generate function with the variable's name — no assignment needed
                    self.gen_fn(name, &all_params, ret, body);
                    // Register the function signature for call-site lookup
                    let ret_c = ret.as_ref().map_or("void".into(), |t| self.c_type(t));
                    let param_tys: Vec<String> = all_params.iter().map(|p| self.c_type(&p.ty)).collect();
                    self.fn_sigs.insert(name.clone(), (param_tys, ret_c.clone()));
                    self.fn_returns.insert(name.clone(), ret_c);
                    return;
                }
                let c_ty = ty
                    .as_ref()
                    .map_or_else(|| self.infer_c_type(value), |t| self.c_type(t));
                self.var_types.insert(name.clone(), c_ty.clone());
                self.write_indent();
                if matches!(value, Expr::ArrayLiteral(_)) && c_ty.ends_with('*') {
                    let arr_ty = c_ty.trim_end_matches('*');
                    writeln!(
                        self.output,
                        "{} {}[] = {};",
                        arr_ty,
                        name,
                        self.gen_expr(value)
                    )
                    .unwrap();
                } else if c_ty.contains("(*)") {
                    // Function pointer type: `ret (*)(params)` → `ret (*name)(params)`
                    let fn_ptr = c_ty.replace("(*)", &format!("(*{})", name));
                    writeln!(self.output, "{} = {};", fn_ptr, self.gen_expr(value)).unwrap();
                } else {
                    writeln!(self.output, "{} {} = {};", c_ty, name, self.gen_expr(value)).unwrap();
                }
            }
            Stmt::Assign { name, value } => {
                self.write_indent();
                writeln!(self.output, "{} = {};", name, self.gen_expr(value)).unwrap();
            }
            Stmt::If {
                condition,
                then,
                else_,
            } => {
                self.write_indent();
                writeln!(self.output, "if ({}) {{", self.gen_expr(condition)).unwrap();
                self.indent += 1;
                for s in then {
                    self.gen_stmt(s);
                }
                self.indent -= 1;
                if let Some(else_body) = else_ {
                    self.write_indent();
                    writeln!(self.output, "}} else {{").unwrap();
                    self.indent += 1;
                    for s in else_body {
                        self.gen_stmt(s);
                    }
                    self.indent -= 1;
                }
                self.write_indent();
                writeln!(self.output, "}}").unwrap();
            }
            Stmt::While { condition, body } => {
                self.write_indent();
                writeln!(self.output, "while ({}) {{", self.gen_expr(condition)).unwrap();
                self.indent += 1;
                for s in body {
                    self.gen_stmt(s);
                }
                self.indent -= 1;
                self.write_indent();
                writeln!(self.output, "}}").unwrap();
            }
            Stmt::For {
                variable,
                iterable,
                body,
            } => {
                if let Expr::ArrayLiteral(elems) = iterable {
                    for elem in elems {
                        self.write_indent();
                        writeln!(self.output, "{{").unwrap();
                        self.indent += 1;
                        self.write_indent();
                        writeln!(self.output, "long {} = {};", variable, self.gen_expr(elem))
                            .unwrap();
                        for s in body {
                            self.gen_stmt(s);
                        }
                        self.indent -= 1;
                        self.write_indent();
                        writeln!(self.output, "}}").unwrap();
                    }
                } else if let Expr::Range {
                    start,
                    end,
                    inclusive,
                } = iterable
                {
                    let start_val = self.gen_expr(start);
                    let end_val = self.gen_expr(end);
                    self.write_indent();
                    if *inclusive {
                        writeln!(
                            self.output,
                            "for (long {} = {}; {} <= {}; {}++) {{",
                            variable, start_val, variable, end_val, variable
                        )
                        .unwrap();
                    } else {
                        writeln!(
                            self.output,
                            "for (long {} = {}; {} < {}; {}++) {{",
                            variable, start_val, variable, end_val, variable
                        )
                        .unwrap();
                    }
                    self.indent += 1;
                    for s in body {
                        self.gen_stmt(s);
                    }
                    self.indent -= 1;
                    self.write_indent();
                    writeln!(self.output, "}}").unwrap();
                } else {
                    let iter_expr = self.gen_expr(iterable);
                    self.write_indent();
                    writeln!(self.output, "long __arr[] = (long[]){};", iter_expr).unwrap();
                    self.write_indent();
                    writeln!(
                        self.output,
                        "for (long __i = 0, __len = sizeof(__arr) / sizeof(__arr[0]); __i < __len; __i++) {{"
                    )
                    .unwrap();
                    self.indent += 1;
                    self.write_indent();
                    writeln!(self.output, "long {} = __arr[__i];", variable).unwrap();
                    for s in body {
                        self.gen_stmt(s);
                    }
                    self.indent -= 1;
                    self.write_indent();
                    writeln!(self.output, "}}").unwrap();
                }
            }
            Stmt::Return(expr) => {
                self.write_indent();
                if let Some(e) = expr {
                    if let Expr::ErrExpr(msg) = e {
                        let msg_val = self.gen_expr(msg);
                        writeln!(
                            self.output,
                            "fprintf(stderr, \"Error: %s\\n\", {msg_val}); return 1;"
                        )
                        .unwrap();
                    } else {
                        writeln!(self.output, "return {};", self.gen_expr(e)).unwrap();
                    }
                } else {
                    writeln!(self.output, "return;").unwrap();
                }
            }
            Stmt::Print(expr) => {
                self.gen_print(expr);
            }
            Stmt::ExprStmt(expr) => {
                self.write_indent();
                writeln!(self.output, "{};", self.gen_expr(expr)).unwrap();
            }
        }
    }

    fn gen_print(&mut self, expr: &Expr) {
        self.write_indent();
        match expr {
            Expr::Str(_) => {
                writeln!(self.output, "printf(\"%s\\n\", {});", self.gen_expr(expr)).unwrap();
            }
            Expr::MoneyLiteral { .. } => {
                let val = self.gen_expr(expr);
                writeln!(
                    self.output,
                    "printf(\"%%ld.%%04ld\\n\", {} / {}, {} % {});",
                    val, MONEY_SCALE, val, MONEY_SCALE
                )
                .unwrap();
            }
            Expr::Float(_) => {
                writeln!(self.output, "printf(\"%f\\n\", {});", self.gen_expr(expr)).unwrap();
            }
            Expr::Bool(_) => {
                writeln!(
                    self.output,
                    "printf(\"%s\\n\", {} ? \"true\" : \"false\");",
                    self.gen_expr(expr)
                )
                .unwrap();
            }
            Expr::UnitLiteral { unit, value } => {
                let val = self.gen_expr(value);
                writeln!(self.output, "printf(\"%ld {}\\n\", (long){});", unit, val).unwrap();
            }
            Expr::Ident(name) => {
                let var_type = self.var_types.get(name.as_str()).map(|s| s.as_str());
                if var_type == Some("const char*") || var_type == Some("string") {
                    writeln!(self.output, "printf(\"%s\\n\", {});", self.gen_expr(expr)).unwrap();
                } else if var_type == Some("double") {
                    writeln!(self.output, "printf(\"%f\\n\", {});", self.gen_expr(expr)).unwrap();
                } else {
                    writeln!(
                        self.output,
                        "printf(\"%ld\\n\", (long){});",
                        self.gen_expr(expr)
                    )
                    .unwrap();
                }
            }
            Expr::FString(_) => {
                writeln!(self.output, "printf(\"%s\\n\", {});", self.gen_expr(expr)).unwrap();
            }
            _ => {
                let expr_ty = self.infer_c_type(expr);
                let val = self.gen_expr(expr);
                if expr_ty == "const char*" || expr_ty == "string" {
                    writeln!(self.output, "printf(\"%s\\n\", {});", val).unwrap();
                } else if expr_ty == "double" {
                    writeln!(self.output, "printf(\"%f\\n\", {});", val).unwrap();
                } else if expr_ty == "int" {
                    writeln!(self.output, "printf(\"%s\\n\", {} ? \"true\" : \"false\");", val).unwrap();
                } else {
                    writeln!(self.output, "printf(\"%ld\\n\", (long){});", val).unwrap();
                }
            }
        }
    }

    fn infer_c_type(&self, expr: &Expr) -> String {
        match expr {
            Expr::Int(_) => "long".into(),
            Expr::Float(_) => "double".into(),
            Expr::Bool(_) => "int".into(),
            Expr::Str(_) => "const char*".into(),
            Expr::MoneyLiteral { .. } | Expr::DecimalLiteral(_) => "long".into(),
            Expr::UnitLiteral { .. } => "long".into(),
            Expr::ArrayLiteral(elems) if !elems.is_empty() => {
                format!("{}*", self.infer_c_type(&elems[0]))
            }
            Expr::StructLiteral { name, type_args, .. } => {
                if !type_args.is_empty() && self.generic_structs.contains_key(name) {
                    let type_suffix: Vec<String> = type_args.iter().map(|t| Self::type_id(t)).collect();
                    format!("{}_{}", name, type_suffix.join("_"))
                } else {
                    name.clone()
                }
            }
            Expr::Call { name, .. } => {
                if let Some(b) = stdlib::builtins().get(name.as_str()) {
                    self.c_type(&b.ret)
                } else if let Some(ret) = self.fn_returns.get(name.as_str()) {
                    ret.clone()
                } else {
                    "long".into()
                }
            }
            Expr::Ident(name) => {
                // Check if this is a function reference (check original, ::-to-_, and mangled)
                let normalized = name.replace("::", "_");
                if let Some((params, ret)) = self
                    .fn_sigs
                    .get(name.as_str())
                    .or_else(|| self.fn_sigs.get(normalized.as_str()))
                    .or_else(|| self.fn_sigs.get(&Self::c_mangle(name)))
                {
                    let params_str = params.join(", ");
                    format!("{} (*)({})", ret, params_str)
                } else {
                    self.var_types
                        .get(name.as_str())
                        .cloned()
                        .unwrap_or_else(|| "long".to_string())
                }
            }
            Expr::EnumVariant { enum_name, .. } => {
                if self.enums.contains_key(enum_name) {
                    "sbx_enum".into()
                } else {
                    "long".into()
                }
            }
            Expr::Match { .. } => "long".into(),
            Expr::Lambda { params, ret, .. } => {
                let ret_str = ret.as_ref().map_or("void".to_string(), |t| self.c_type(t));
                let params_str: Vec<String> = params.iter().map(|p| self.c_type(&p.ty)).collect();
                format!("{} (*)({})", ret_str, params_str.join(", "))
            }
            Expr::OkExpr(val) => self.infer_c_type(val),
            Expr::ErrExpr(_) => "const char*".into(),
            Expr::Range { .. } => "sbx_range_t".into(),
            Expr::FString(_) => "const char*".into(),
            _ => "long".into(),
        }
    }

    fn gen_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::Int(n) => format!("{}", n),
            Expr::Float(n) => format!("{}", n),
            Expr::Str(s) => {
                let escaped = s
                    .replace("\\", "\\\\")
                    .replace("\"", "\\\"")
                    .replace("\n", "\\n")
                    .replace("\r", "\\r")
                    .replace("\t", "\\t");
                format!("\"{}\"", escaped)
            }
            Expr::Bool(b) => {
                if *b {
                    "1".into()
                } else {
                    "0".into()
                }
            }
            Expr::Ident(name) => {
                // If this is a function reference, mangle the name
                // Convert Point::distance_sq -> Point_distance_sq for lookups
                let normalized = name.replace("::", "_");
                if self.fn_sigs.contains_key(name.as_str())
                    || self.fn_returns.contains_key(name.as_str())
                    || self.fn_sigs.contains_key(normalized.as_str())
                    || self.fn_returns.contains_key(normalized.as_str())
                {
                    Self::c_mangle(&normalized)
                } else {
                    name.clone()
                }
            }
            Expr::MoneyLiteral { amount, currency } => {
                let scaled = (*amount * MONEY_SCALE as f64) as i64;
                format!("((long){}L) /* {} */", scaled, currency)
            }
            Expr::DecimalLiteral(s) => {
                // Parse decimal string to i128 scaled value
                if let Some((int_part, frac_part)) = s.split_once('.') {
                    let int_val: i128 = int_part.parse().unwrap_or(0);
                    let mut frac_str = frac_part.to_string();
                    // Pad or truncate to 18 digits
                    while frac_str.len() < 18 {
                        frac_str.push('0');
                    }
                    frac_str.truncate(18);
                    let frac_val: i128 = frac_str.parse().unwrap_or(0);
                    let total = int_val * DECIMAL_SCALE as i128 + frac_val;
                    format!("((__int128){}LL)", total)
                } else {
                    let val: i128 = s.parse().unwrap_or(0);
                    format!("((__int128){}LL)", val * DECIMAL_SCALE as i128)
                }
            }
            Expr::UnitLiteral { value, .. } => self.gen_expr(value),
            Expr::BinaryOp { op, left, right } => {
                let l = self.gen_expr(left);
                let r = self.gen_expr(right);
                let op_str = match op {
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    BinOp::Div => "/",
                    BinOp::Mod => "%",
                    BinOp::Eq => "==",
                    BinOp::Neq => "!=",
                    BinOp::Lt => "<",
                    BinOp::Gt => ">",
                    BinOp::Le => "<=",
                    BinOp::Ge => ">=",
                    BinOp::And => "&&",
                    BinOp::Or => "||",
                };

                if matches!(op, BinOp::Add) {
                    if let (Expr::Str(_), _) | (_, Expr::Str(_)) = (left.as_ref(), right.as_ref()) {
                        return format!("__sbx_str_concat({}, {})", l, r);
                    }
                }

                let needs_scale = self.is_money_binop(left, right);
                if needs_scale {
                    match op {
                        BinOp::Mul | BinOp::Div => {
                            return format!("({} {} {})", l, op_str, r);
                        }
                        _ => {}
                    }
                }

                format!("({} {} {})", l, op_str, r)
            }
            Expr::UnaryOp { op, expr } => {
                let e = self.gen_expr(expr);
                match op {
                    UnOp::Neg => format!("(-{})", e),
                    UnOp::Not => format!("(!{})", e),
                }
            }
            Expr::Call { name, type_args, args } => {
                // If type_args are present, queue monomorphization
                if !type_args.is_empty() {
                    if let Some(fn_def) = self.fn_defs.get(name.as_str()) {
                        let type_suffix: Vec<String> = type_args.iter().map(|t| self.c_type(t)).collect();
                        let mono_name = format!("{}_{}", Self::c_mangle(name), type_suffix.join("_"));
                        // Queue for later emission
                        self.pending_mono.borrow_mut().push((mono_name.clone(), fn_def.clone(), type_args.clone()));
                        let args_str: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
                        return format!("{}({})", mono_name, args_str.join(", "));
                    }
                }
                // Fill in default parameters if call has fewer args than params
                let filled_args: Vec<String> = if let Some(defaults) = self.fn_defaults.get(name.as_str()) {
                    if args.len() < defaults.len() {
                        let mut a: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
                        for d in &defaults[args.len()..] {
                            if let Some(ref default_expr) = d {
                                a.push(self.gen_expr(default_expr));
                            }
                        }
                        a
                    } else {
                        args.iter().map(|a| self.gen_expr(a)).collect()
                    }
                } else {
                    args.iter().map(|a| self.gen_expr(a)).collect()
                };
                // If calling a lambda with captures, append capture args
                let lambda_name = self.var_to_lambda.get(name.as_str()).cloned()
                    .unwrap_or_else(|| name.clone());
                let captures = self.lambda_captures.get(&lambda_name).cloned().unwrap_or_default();
                let mut all_args: Vec<String> = filled_args;
                for cap in &captures {
                    all_args.push(cap.clone());
                }
                let args_str: Vec<String> = all_args;
                // Sync-async: future::wait and future::is_ready are no-ops
                if name == "future::wait" || name == "future::is_ready" {
                    return args_str.first().cloned().unwrap_or_else(|| "0".to_string());
                }
                if stdlib::is_builtin(name) {
                    let c_fn = stdlib::c_name(name);
                    // v2.0: spawn / serve_once take a *function name* — emit it as a
                    // C function pointer (dropping the string quotes) instead of a literal.
                    let args_joined =
                        if matches!(name.as_str(), "spawn" | "http::serve_once" | "http::serve") {
                            let first = args_str
                                .first()
                                .cloned()
                                .unwrap_or_default()
                                .trim_matches('"')
                                .to_string();
                            if name == "http::serve_once" || name == "http::serve" {
                                let handler = args_str
                                    .get(1)
                                    .cloned()
                                    .unwrap_or_default()
                                    .trim_matches('"')
                                    .to_string();
                                format!("{}, (const char* (*)(const char*)){}", first, handler)
                            } else {
                                let rest: Vec<String> = args_str.iter().skip(1).cloned().collect();
                                if rest.is_empty() {
                                    first
                                } else {
                                    format!("{first}, {}", rest.join(", "))
                                }
                            }
                        } else {
                            args_str.join(", ")
                        };
                    format!("{}({})", c_fn, args_joined)
                } else {
                    // Convert :: to _ for impl/module method calls
                    let normalized = name.replace("::", "_");
                    let c_name = Self::c_mangle(&normalized);
                    let call = format!("{}({})", c_name, args_str.join(", "));
                    call
                }
            }
            Expr::StructLiteral { name, type_args, fields, .. } => {
                let actual_name = if !type_args.is_empty() && self.generic_structs.contains_key(name) {
                    let type_suffix: Vec<String> = type_args.iter().map(|t| Self::type_id(t)).collect();
                    format!("{}_{}", name, type_suffix.join("_"))
                } else {
                    name.clone()
                };
                let fields_str: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!(".{} = {}", k, self.gen_expr(v)))
                    .collect();
                format!("({}){{ {} }}", actual_name, fields_str.join(", "))
            }
            Expr::FieldAccess { target, field } => {
                format!("({}).{}", self.gen_expr(target), field)
            }
            Expr::Index { target, index } => {
                format!("({})[{}]", self.gen_expr(target), self.gen_expr(index))
            }
            Expr::ArrayLiteral(elems) => {
                let elems_str: Vec<String> = elems.iter().map(|e| self.gen_expr(e)).collect();
                format!("{{ {} }}", elems_str.join(", "))
            }
            Expr::OkExpr(value) => self.gen_expr(value),
            Expr::ErrExpr(error) => {
                let msg = self.gen_expr(error);
                format!("(fprintf(stderr, \"Error: %s\\n\", {msg}), exit(1))")
            }
            Expr::PanicExpr(msg) => {
                let msg_val = self.gen_expr(msg);
                format!("(fprintf(stderr, \"Panic: %s\\n\", {msg_val}), exit(1))")
            }
            Expr::TryExpr(expr) => self.gen_expr(expr),
            Expr::EnumVariant {
                enum_name,
                type_args: _,
                variant,
                payload,
            } => {
                let tag = format!("{}_{}", enum_name, variant);
                match payload {
                    Some(expr) => {
                        let val = self.gen_expr(expr);
                        // Store as double — works for both int and float payloads
                        format!(
                            "((sbx_enum){{.tag = {}, .payload = {{.d = {}}}}})",
                            tag, val
                        )
                    }
                    None => format!("((sbx_enum){{.tag = {}, .payload = {{.d = 0}}}})", tag),
                }
            }
            Expr::Match { scrutinee, arms } => {
                let sc = self.gen_expr(scrutinee);
                let sc_ty = self.infer_c_type(scrutinee);
                let idx = self.var_counter.get();
                self.var_counter.set(idx + 1);
                let tmp = format!("__m{}", idx);
                let res = format!("__r{}", idx);
                // Detect if this is an enum match by checking arm patterns
                let is_enum_match = arms.iter().any(|arm| matches!(&arm.pattern,
                    Pattern::EnumVariant { .. }
                ));
                // For Option types (long), use value directly as tag
                // For enum types (sbx_enum), use .tag
                let tag_expr = if is_enum_match {
                    format!("{}.tag", sc)
                } else {
                    sc.clone()
                };
                // GNU statement expression: ({ long __r=0; long __m=tag; if(...) __r=val; ... __r; })
                let mut c = format!("({{ double {res} = 0; long {tmp} = {tag_expr}; ");
                let mut first = true;
                for arm in arms {
                    let cond = match &arm.pattern {
                        Pattern::EnumVariant {
                            enum_name, variant, ..
                        } => {
                            format!("{tmp} == {enum_name}_{variant}")
                        }
                        Pattern::SomePattern { .. } => format!("{tmp} != 0"),
                        Pattern::NonePattern => format!("{tmp} == 0"),
                        Pattern::IntLiteral(n) => format!("{tmp} == {n}"),
                        Pattern::BoolLiteral(b) => format!("{tmp} == {}", if *b { 1 } else { 0 }),
                        _ => "1".to_string(),
                    };
                    let kw = if first { "if" } else { "else if" };
                    first = false;
                    // Check if pattern has a binding variable
                    let binding_decl = match &arm.pattern {
                        Pattern::EnumVariant {
                            binding: Some(b), ..
                        } => {
                            if is_enum_match {
                                format!("long {b} = (long){sc}.payload.d; ")
                            } else {
                                format!("long {b} = {sc}; ")
                            }
                        }
                        Pattern::SomePattern { binding: Some(b) } => {
                            format!("long {b} = {sc}; ")
                        }
                        Pattern::Variable(name) => {
                            if is_enum_match {
                                format!("long {name} = (long){sc}.payload.d; ")
                            } else {
                                format!("long {name} = {sc}; ")
                            }
                        }
                        _ => String::new(),
                    };
                    // Generate ALL statements in the arm body
                    let mut arm_code = String::new();
                    arm_code.push_str(&binding_decl);
                    for (si, s) in arm.body.iter().enumerate() {
                        let is_last = si == arm.body.len() - 1;
                        match s {
                            Stmt::ExprStmt(e) if is_last => {
                                let val = self.gen_expr(e);
                                arm_code.push_str(&format!("{res} = {val}; "));
                            }
                            Stmt::Return(Some(e)) if is_last => {
                                let val = self.gen_expr(e);
                                arm_code.push_str(&format!("{res} = {val}; "));
                            }
                            Stmt::Print(e) => {
                                let val = self.gen_expr(e);
                                let ty = self.infer_c_type(e);
                                if ty == "const char*" || ty == "string" {
                                arm_code.push_str(&format!("printf(\"%s\\n\", {val}); "));
                                } else if ty == "double" {
                                arm_code.push_str(&format!("printf(\"%f\\n\", {val}); "));
                                } else {
                                arm_code.push_str(&format!("printf(\"%ld\\n\", (long){val}); "));
                                }
                            }
                            _ => {
                                // For other statements, we can't easily inline them in a GNU expr
                                // Just ignore for now (match-as-expression use case)
                            }
                        }
                    }
                    c.push_str(&format!(
                        "{kw} ({cond}) {{ {arm_code}}} "
                    ));
                }
                c.push_str(&format!("{res}; }})"));
                c
            }
            Expr::AssertExpr { condition, message } => {
                let cond = self.gen_expr(condition);
                let msg = match message {
                    Some(m) => self.gen_expr(m),
                    None => format!("\"assertion failed\"")
                };
                format!("(({cond}) || (fprintf(stderr, \"Assert failed: %s\\n\", {msg}), longjmp(__test_jmp, 1), 0))")
            }
            Expr::AssertEqExpr { left, right, message } => {
                let l = self.gen_expr(left);
                let r = self.gen_expr(right);
                let msg = match message {
                    Some(m) => self.gen_expr(m),
                    None => format!("\"assert_eq failed\"")
                };
                format!("(({l}) == ({r}) || (fprintf(stderr, \"%s: %ld != %ld\\n\", {msg}, (long)({l}), (long)({r})), longjmp(__test_jmp, 1), 0))")
            }
            Expr::Lambda { .. } => {
                // Lambda was already registered by prescan_lambdas
                let idx = self.lambda_return_idx.get();
                self.lambda_return_idx.set(idx + 1);
                format!("__lambda_{}", idx)
            }
            Expr::Range {
                start,
                end,
                inclusive,
            } => {
                let s = self.gen_expr(start);
                let e = self.gen_expr(end);
                if *inclusive {
                    format!("sbx_range_inclusive({}, {})", s, e)
                } else {
                    format!("sbx_range({}, {})", s, e)
                }
            }
            Expr::SomeExpr(inner) => self.gen_expr(inner),
            Expr::Await(inner) => self.gen_expr(inner),
            Expr::NoneExpr => "0".to_string(),
            Expr::MethodCall { target, method, args } => {
                let target_ty = self.infer_c_type(target);
                let t = self.gen_expr(target);
                let a: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
                // String methods — map to __sbx_str_* C runtime functions
                let c_fn: String = if target_ty == "const char*" {
                    match method.as_str() {
                        "to_upper" => "__sbx_str_to_upper".to_string(),
                        "to_lower" => "__sbx_str_to_lower".to_string(),
                        "replace" => "__sbx_str_replace".to_string(),
                        "trim" => "__sbx_str_trim".to_string(),
                        "length" => "__sbx_str_len".to_string(),
                        _ => format!("__sbx_str_{}", method),
                    }
                } else {
                    // Struct method: Type_method
                    let type_name = if let Expr::Ident(name) = target.as_ref() {
                        self.var_types.get(name.as_str()).cloned().unwrap_or_else(|| name.clone())
                    } else {
                        target_ty.clone()
                    };
                    format!("{}_{}", type_name, method)
                };
                let mut all_args = vec![t];
                all_args.extend(a);
                let args_str = all_args.join(", ");
                format!("{}({})", c_fn, args_str)
            }
            Expr::FString(parts) => {
                let mut result = String::from("(const char*)__sbx_str_concat_multi(");
                let mut first = true;
                let mut count = 0;
                for part in parts {
                    match part {
                        crate::ast::FStringPart::Literal(s) => {
                            if !first {
                                result.push_str(", ");
                            }
                            result.push_str(&format!(
                                "\"{}\"",
                                s.replace('"', "\\\"").replace('\n', "\\n")
                            ));
                            first = false;
                            count += 1;
                        }
                        crate::ast::FStringPart::Expr(expr) => {
                            let expr_str = self.gen_expr(expr);
                            let expr_ty = self.infer_c_type(expr);
                            if !first {
                                result.push_str(", ");
                            }
                            // Only wrap non-string values in __sbx_to_string
                            if expr_ty == "const char*" {
                                result.push_str(&expr_str);
                            } else if expr_ty == "double" {
                                result.push_str(&format!("__sbx_to_string_f({})", expr_str));
                            } else {
                                result.push_str(&format!("__sbx_to_string({})", expr_str));
                            }
                            first = false;
                            count += 1;
                        }
                    }
                }
                // Replace the placeholder with actual count and add comma after it
                result = result.replacen(
                    "__sbx_str_concat_multi(",
                    &format!("__sbx_str_concat_multi({}, ", count),
                    1,
                );
                // The result already has the parts with commas between them, close the call
                result.push(')');
                result
            }
        }
    }

    fn is_money_binop(&self, left: &Expr, right: &Expr) -> bool {
        matches!(left, Expr::MoneyLiteral { .. }) || matches!(right, Expr::MoneyLiteral { .. })
    }


    /// Substitute type parameters with concrete types for codegen.
    fn substitute_type_codegen(ty: &Type, sub: &std::collections::HashMap<String, Type>) -> Type {
        match ty {
            Type::TypeParam(name) | Type::Custom { name, .. } if sub.contains_key(name) => {
                sub.get(name).cloned().unwrap_or_else(|| ty.clone())
            }
            Type::Array(inner) => Type::Array(Box::new(Self::substitute_type_codegen(inner, sub))),
            Type::Option(inner) => Type::Option(Box::new(Self::substitute_type_codegen(inner, sub))),
            Type::Result(ok, err) => Type::Result(
                Box::new(Self::substitute_type_codegen(ok, sub)),
                Box::new(Self::substitute_type_codegen(err, sub)),
            ),
            Type::Fn(params, ret) => Type::Fn(
                params.iter().map(|p| Self::substitute_type_codegen(p, sub)).collect(),
                Box::new(Self::substitute_type_codegen(ret, sub)),
            ),
            Type::Future(inner) => Type::Future(Box::new(Self::substitute_type_codegen(inner, sub))),
            _ => ty.clone(),
        }
    }

    /// Generate a monomorphized version of a generic function.
    fn monomorphize_function(&mut self, fn_def: &TopLevel, type_args: &[Type]) -> String {
        if let TopLevel::FnDef { name, type_params, params, ret, body, .. } = fn_def {
            // Build substitution map
            let sub: std::collections::HashMap<String, Type> = type_params.iter()
                .zip(type_args.iter())
                .map(|(tp, concrete)| (tp.clone(), concrete.clone()))
                .collect();

            // Generate monomorphized name: max__i64 -> max_i64
            let type_suffix: Vec<String> = type_args.iter().map(|t| self.c_type(t)).collect();
            let mono_name = format!("{}_{}", Self::c_mangle(name), type_suffix.join("_"));

            // Check if already generated
            let key = format!("{}__{}", name, type_args.iter().map(|t| format!("{}", t)).collect::<Vec<_>>().join(","));
            if self.mono_fns.contains(&key) {
                return mono_name;
            }
            let key_clone = key.clone();
            self.mono_fns.insert(key);
            self.mono_map.insert(key_clone, mono_name.clone());

            // Substitute types in params and return type
            let sub_params: Vec<Param> = params.iter().map(|p| Param {
                name: p.name.clone(),
                ty: Self::substitute_type_codegen(&p.ty, &sub),
                default: None,
            }).collect();
            let sub_ret = ret.as_ref().map(|t| Self::substitute_type_codegen(t, &sub));

            // Generate the monomorphized function
            self.gen_fn(&mono_name, &sub_params, &sub_ret, body);

            mono_name
        } else {
            String::new()
        }
    }

    /// Return a safe C identifier for a type (used in monomorphized struct names)
    fn type_id(ty: &Type) -> String {
        match ty {
            Type::I64 => "long".to_string(),
            Type::F64 => "double".to_string(),
            Type::Bool => "int".to_string(),
            Type::String => "string".to_string(),
            Type::Money(c) => format!("Money_{}", c),
            Type::Decimal => "decimal".to_string(),
            Type::Unit(u) => u.clone(),
            Type::Array(inner) => format!("{}_arr", Self::type_id(inner)),
            Type::Void => "void".to_string(),
            Type::Custom { name, .. } => name.clone(),
            Type::Option(inner) => format!("Option_{}", Self::type_id(inner)),
            Type::Result(ok, _) => format!("Result_{}", Self::type_id(ok)),
            Type::Fn(_, _) => "fn_ptr".to_string(),
            Type::Future(inner) => format!("Future_{}", Self::type_id(inner)),
            Type::TypeParam(name) => name.clone(),
        }
    }

    fn c_type(&self, ty: &Type) -> String {
        match ty {
            Type::I64 => "long".into(),
            Type::F64 => "double".into(),
            Type::Bool => "int".into(),
            Type::String => "const char*".into(),
            Type::Money(_) | Type::Decimal => "long".into(),
            Type::Unit(_) => "long".into(),
            Type::Array(inner) => format!("{}*", self.c_type(inner)),
            Type::Void => "void".into(),
            Type::Custom { name, type_args } => {
                if name == "Self" {
                    "long".into()
                } else if self.enums.contains_key(name) {
                    "sbx_enum".into()
                } else if !type_args.is_empty() && self.generic_structs.contains_key(name) {
                    // Generic struct with type args — use monomorphized name
                    let type_suffix: Vec<String> = type_args.iter().map(|t| Self::type_id(t)).collect();
                    let mono_name = format!("{}_{}", name, type_suffix.join("_"));
                    // Queue monomorphization if not already done
                    self.queue_struct_mono(name, type_args);
                    mono_name
                } else if self.generic_structs.contains_key(name) {
                    name.clone()
                } else {
                    name.clone()
                }
            }
            Type::Option(_) => "long".into(),  // Tagged: 0 = None, nonzero = Some(payload)
            Type::Future(inner) => format!("Future<{}>", self.c_type(inner)),
            Type::TypeParam(name) => name.clone(),
            Type::Result(ok, _) => self.c_type(ok),
            Type::Fn(params, ret) => {
                let params_str: Vec<String> = params.iter().map(|p| self.c_type(p)).collect();
                format!("{} (*)({})", self.c_type(ret), params_str.join(", "))
            }
        }
    }

    /// Format a parameter as a C declaration, handling function pointer types
    fn c_param(&self, ty: &Type, name: &str) -> String {
        if let Type::Fn(params, ret) = ty {
            let params_str: Vec<String> = params.iter().map(|p| self.c_type(p)).collect();
            format!(
                "{} (*{})({})",
                self.c_type(ret),
                name,
                params_str.join(", ")
            )
        } else {
            format!("{} {}", self.c_type(ty), name)
        }
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            write!(self.output, "    ").unwrap();
        }
    }

    /// Mangle a Sandbox name to avoid C keyword conflicts
    fn c_mangle(name: &str) -> String {
        match name {
            // C keywords and common type names that conflict
            "double" | "float" | "int" | "long" | "short" | "char" | "void" | "if" | "else"
            | "for" | "while" | "do" | "switch" | "case" | "return" | "break" | "continue"
            | "struct" | "enum" | "union" | "typedef" | "sizeof" | "static" | "extern"
            | "const" | "volatile" | "auto" | "register" | "signed" | "unsigned" | "inline"
            | "asm" | "goto" | "default" | "NULL" | "stdin" | "stdout" | "stderr" => {
                format!("sbx_{}", name)
            }
            _ => name.to_string(),
        }
    }
}
