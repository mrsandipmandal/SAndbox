//! In-tree interpreter for .sbx source files.
//! Provides `sandbox interpret` for instant dev feedback without C compilation.

use crate::ast;
use crate::lexer;
use crate::parser;

/// without compiling to C. Enables instant `sandbox interpret` for dev.
/// Scope captured by a lambda at creation time.
#[derive(Clone)]
struct CapturedScope {
    vars: std::collections::HashMap<String, i64>,
    str_vars: std::collections::HashMap<String, String>,
    arr_vars: std::collections::HashMap<String, Vec<i64>>,
    struct_instances: std::collections::HashMap<String, Vec<i64>>,
    struct_type_of: std::collections::HashMap<String, String>,
    enum_instances: std::collections::HashMap<String, (String, Option<String>, i64)>,
}

type LambdaMap =
    std::collections::HashMap<String, (Vec<ast::Param>, Vec<ast::Stmt>, CapturedScope)>;
type FnMap =
    std::collections::HashMap<String, (Vec<ast::Param>, Option<ast::Type>, Vec<ast::Stmt>)>;

struct InterpreterState {
    vars: std::collections::HashMap<String, i64>,
    str_vars: std::collections::HashMap<String, String>,
    arr_vars: std::collections::HashMap<String, Vec<i64>>,
    lambdas: LambdaMap,
    functions: FnMap,
    struct_fields: std::collections::HashMap<String, Vec<String>>,
    struct_instances: std::collections::HashMap<String, Vec<i64>>,
    struct_type_of: std::collections::HashMap<String, String>,
    impl_methods: FnMap,
    lambda_counter: usize,
    enum_defs: std::collections::HashMap<String, Vec<String>>,
    enum_instances: std::collections::HashMap<String, (String, Option<String>, i64)>,
}

impl InterpreterState {
    fn new() -> Self {
        Self {
            vars: std::collections::HashMap::new(),
            str_vars: std::collections::HashMap::new(),
            arr_vars: std::collections::HashMap::new(),
            lambdas: std::collections::HashMap::new(),
            functions: std::collections::HashMap::new(),
            struct_fields: std::collections::HashMap::new(),
            struct_instances: std::collections::HashMap::new(),
            struct_type_of: std::collections::HashMap::new(),
            impl_methods: std::collections::HashMap::new(),
            lambda_counter: 0,
            enum_defs: std::collections::HashMap::new(),
            enum_instances: std::collections::HashMap::new(),
        }
    }

    /// Snapshot all auto-generated keys with the given prefix.
    fn snapshot_auto_keys(&self, prefix: &str) -> std::collections::HashSet<String> {
        self.str_vars
            .keys()
            .chain(self.arr_vars.keys())
            .chain(self.lambdas.keys())
            .chain(self.struct_instances.keys())
            .chain(self.enum_instances.keys())
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }

    /// After evaluating an expression, find the newest auto key (by prefix) that
    /// wasn't in `before` and transfer it to `name` in the appropriate map.
    /// Returns true if a key was transferred.
    fn transfer_new_key(
        &mut self,
        prefix: &str,
        name: &str,
        before: &std::collections::HashSet<String>,
    ) -> bool {
        // Determine which map to look in based on prefix
        let new_key = match prefix {
            p if p.starts_with("__struct_") => self
                .struct_instances
                .keys()
                .filter(|k| k.starts_with("__struct_") && !before.contains(*k))
                .max_by(|a, b| a.cmp(b))
                .cloned(),
            p if p.starts_with("__auto_arr_") => self
                .arr_vars
                .keys()
                .filter(|k| k.starts_with("__auto_arr_") && !before.contains(*k))
                .max_by(|a, b| a.cmp(b))
                .cloned(),
            p if p.starts_with("__lambda_") => self
                .lambdas
                .keys()
                .filter(|k| k.starts_with("__lambda_") && !before.contains(*k))
                .max_by(|a, b| a.cmp(b))
                .cloned(),
            p if p.starts_with("__enum_") => self
                .enum_instances
                .keys()
                .filter(|k| k.starts_with("__enum_") && !before.contains(*k))
                .max_by(|a, b| a.cmp(b))
                .cloned(),
            _ => {
                // "__auto_"
                self.str_vars
                    .keys()
                    .filter(|k| k.starts_with("__auto_") && !before.contains(*k))
                    .max_by(|a, b| a.cmp(b))
                    .cloned()
            }
        };
        if let Some(key) = new_key {
            match prefix {
                p if p.starts_with("__struct_") => {
                    if let Some(s) = self.struct_instances.remove(&key) {
                        self.struct_instances.insert(name.to_string(), s);
                        if let Some(t) = self.struct_type_of.remove(&key) {
                            self.struct_type_of.insert(name.to_string(), t);
                        }
                        return true;
                    }
                }
                p if p.starts_with("__auto_arr_") => {
                    if let Some(a) = self.arr_vars.remove(&key) {
                        self.arr_vars.insert(name.to_string(), a);
                        return true;
                    }
                }
                p if p.starts_with("__lambda_") => {
                    if let Some(l) = self.lambdas.remove(&key) {
                        self.lambdas.insert(name.to_string(), l);
                        return true;
                    }
                }
                p if p.starts_with("__enum_") => {
                    if let Some(e) = self.enum_instances.remove(&key) {
                        self.enum_instances.insert(name.to_string(), e);
                        return true;
                    }
                }
                _ => {
                    if let Some(s) = self.str_vars.remove(&key) {
                        self.str_vars.insert(name.to_string(), s);
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Check if name is bound in any type-specific map (not plain vars).
    fn is_bound_typed(&self, name: &str) -> bool {
        self.str_vars.contains_key(name)
            || self.arr_vars.contains_key(name)
            || self.lambdas.contains_key(name)
            || self.struct_instances.contains_key(name)
            || self.enum_instances.contains_key(name)
    }

    /// Inject a captured scope into this state (for lambda invocation).
    fn inject_scope(&mut self, scope: &CapturedScope) {
        self.vars
            .extend(scope.vars.iter().map(|(k, v)| (k.clone(), *v)));
        self.str_vars
            .extend(scope.str_vars.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.arr_vars
            .extend(scope.arr_vars.iter().map(|(k, v)| (k.clone(), v.clone())));
        self.struct_instances.extend(
            scope
                .struct_instances
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );
        self.struct_type_of.extend(
            scope
                .struct_type_of
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );
        self.enum_instances.extend(
            scope
                .enum_instances
                .iter()
                .map(|(k, v)| (k.clone(), v.clone())),
        );
    }
}

pub fn interpret(source: &str, filename: &str) -> anyhow::Result<()> {
    let mut lexer = lexer::Lexer::new(source).with_source(filename);
    let tokens = lexer.tokenize()?;
    let mut parser = parser::Parser::new(tokens).with_source(source, filename);
    let program = parser.parse()?;

    let mut state = InterpreterState::new();
    for item in &program.items {
        match item {
            ast::TopLevel::FnDef {
                name,
                params,
                ret,
                body,
                ..
            } => {
                state
                    .functions
                    .insert(name.clone(), (params.clone(), ret.clone(), body.clone()));
            }
            ast::TopLevel::StructDef { name, fields, .. } => {
                state.struct_fields.insert(
                    name.clone(),
                    fields.iter().map(|f| f.name.clone()).collect(),
                );
            }
            ast::TopLevel::ImplDef {
                type_name, methods, ..
            } => {
                for method in methods {
                    if let ast::TopLevel::FnDef {
                        name,
                        params,
                        ret,
                        body,
                        ..
                    } = method
                    {
                        let key = format!("{}::{}", type_name, name);
                        state
                            .impl_methods
                            .insert(key, (params.clone(), ret.clone(), body.clone()));
                    }
                }
            }
            ast::TopLevel::EnumDef { name, variants, .. } => {
                state.enum_defs.insert(
                    name.clone(),
                    variants.iter().map(|v| v.name.clone()).collect(),
                );
            }
            _ => {}
        }
    }

    let main_fn = state
        .functions
        .get("main")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("No 'main' function found in {}", filename))?;

    exec_block(&main_fn.2, &mut state)?;
    Ok(())
}

fn exec_block(stmts: &[ast::Stmt], state: &mut InterpreterState) -> anyhow::Result<Option<i64>> {
    const BREAK_SENTINEL: i64 = -999999;
    const CONTINUE_SENTINEL: i64 = -999998;

    for stmt in stmts {
        match stmt {
            ast::Stmt::Let { name, value, .. } => {
                // Snapshot auto-keys before eval, clear old bindings, evaluate, transfer new keys
                let str_before = state.snapshot_auto_keys("__auto_");
                let arr_before = state.snapshot_auto_keys("__auto_arr_");
                let lam_before = state.snapshot_auto_keys("__lambda_");
                let struct_before = state.snapshot_auto_keys("__struct_");
                let enum_before = state.snapshot_auto_keys("__enum_");
                let int_val = eval_expr(value, state)?;
                state.str_vars.remove(name);
                state.arr_vars.remove(name);
                state.lambdas.remove(name);
                state.transfer_new_key("__auto_", name, &str_before);
                state.transfer_new_key("__auto_arr_", name, &arr_before);
                state.transfer_new_key("__lambda_", name, &lam_before);
                state.transfer_new_key("__struct_", name, &struct_before);
                state.transfer_new_key("__enum_", name, &enum_before);
                if !state.is_bound_typed(name) {
                    state.vars.insert(name.clone(), int_val);
                }
                // Clean up leaked intermediate auto keys
                let leaked: Vec<String> = state
                    .str_vars
                    .keys()
                    .filter(|k| k.starts_with("__auto_") && **k != *name)
                    .cloned()
                    .collect();
                for k in leaked {
                    state.str_vars.remove(&k);
                }
            }
            ast::Stmt::Assign { name, value } => {
                // Snapshot, eval (old value still accessible), clear old bindings, transfer new
                let str_before = state.snapshot_auto_keys("__auto_");
                let arr_before = state.snapshot_auto_keys("__auto_arr_");
                let struct_before = state.snapshot_auto_keys("__struct_");
                let enum_before = state.snapshot_auto_keys("__enum_");
                let int_val = eval_expr(value, state)?;
                state.str_vars.remove(name);
                state.arr_vars.remove(name);
                state.lambdas.remove(name);
                state.struct_instances.remove(name);
                state.struct_type_of.remove(name);
                state.enum_instances.remove(name);
                state.transfer_new_key("__auto_", name, &str_before);
                state.transfer_new_key("__auto_arr_", name, &arr_before);
                state.transfer_new_key("__struct_", name, &struct_before);
                state.transfer_new_key("__enum_", name, &enum_before);
                if !state.is_bound_typed(name) {
                    state.vars.insert(name.clone(), int_val);
                }
                let leaked: Vec<String> = state
                    .str_vars
                    .keys()
                    .filter(|k| k.starts_with("__auto_") && **k != *name)
                    .cloned()
                    .collect();
                for k in leaked {
                    state.str_vars.remove(&k);
                }
            }
            ast::Stmt::Print(expr) => {
                // Smart print: resolve strings/arrays from idents, eval everything else
                match expr {
                    ast::Expr::Ident(n) => {
                        if let Some(s) = state.str_vars.get(n) {
                            println!("{}", s);
                        } else if let Some(arr) = state.arr_vars.get(n) {
                            let elems: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                            println!("[{}]", elems.join(", "));
                        } else if let Some((params, _body, _cap)) = state.lambdas.get(n).cloned() {
                            // Print function pointer for lambdas
                            print!("<fn|");
                            for (i, p) in params.iter().enumerate() {
                                if i > 0 {
                                    print!(", ");
                                }
                                print!("{}: {}", p.name, p.ty);
                            }
                            println!("|>");
                        } else {
                            let val = state.vars.get(n.as_str()).unwrap_or(&0);
                            println!("{}", val);
                        }
                    }
                    ast::Expr::Str(s) => println!("{}", s),
                    // C backend prints bool literals as true/false
                    ast::Expr::Bool(b) => println!("{}", if *b { "true" } else { "false" }),
                    ast::Expr::ArrayLiteral(elems) => {
                        let vals: Vec<String> = elems
                            .iter()
                            .map(|e| match e {
                                ast::Expr::Str(s) => s.to_string(),
                                ast::Expr::Int(n) => n.to_string(),
                                ast::Expr::Bool(b) => {
                                    if *b {
                                        "true".to_string()
                                    } else {
                                        "false".to_string()
                                    }
                                }
                                _ => "?".to_string(),
                            })
                            .collect();
                        println!("[{}]", vals.join(", "));
                    }
                    ast::Expr::Call {
                        name,
                        type_args: _,
                        args,
                    } => {
                        if name == "print" {
                            if let Some(first) = args.first() {
                                // C backend prints bool literals as true/false
                                if let ast::Expr::Bool(b) = first {
                                    println!("{}", if *b { "true" } else { "false" });
                                    return Ok(Some(0));
                                }
                                // Snapshot str_vars before to detect new strings
                                let str_before: std::collections::HashSet<String> =
                                    state.str_vars.keys().cloned().collect();
                                let val = eval_expr(first, state)?;
                                // Check if a new string was created (from concat, etc.)
                                let new_str = state
                                    .str_vars
                                    .keys()
                                    .filter(|k| !str_before.contains(*k))
                                    .max_by(|a, b| a.cmp(b))
                                    .cloned();
                                if let Some(key) = new_str {
                                    if let Some(s) = state.str_vars.get(&key) {
                                        println!("{}", s);
                                    } else {
                                        println!("{}", val);
                                    }
                                } else if let ast::Expr::Ident(n) = first {
                                    if let Some(s) = state.str_vars.get(n) {
                                        println!("{}", s);
                                    } else if let Some(arr) = state.arr_vars.get(n) {
                                        let elems: Vec<String> =
                                            arr.iter().map(|v| v.to_string()).collect();
                                        println!("[{}]", elems.join(", "));
                                    } else {
                                        println!("{}", val);
                                    }
                                } else {
                                    println!("{}", val);
                                }
                            }
                        } else {
                            let str_before: std::collections::HashSet<String> =
                                state.str_vars.keys().cloned().collect();
                            let arr_before: std::collections::HashSet<String> =
                                state.arr_vars.keys().cloned().collect();
                            let val = eval_expr(expr, state)?;
                            if val != 0 {
                                println!("{}", val);
                            } else {
                                let new_str = state
                                    .str_vars
                                    .keys()
                                    .filter(|k| !str_before.contains(*k))
                                    .max_by(|a, b| a.cmp(b))
                                    .cloned();
                                if let Some(key) = new_str {
                                    if let Some(s) = state.str_vars.get(&key) {
                                        println!("{}", s);
                                    } else {
                                        println!("{}", val);
                                    }
                                } else {
                                    let new_arr = state
                                        .arr_vars
                                        .keys()
                                        .filter(|k| !arr_before.contains(*k))
                                        .max_by(|a, b| a.cmp(b))
                                        .cloned();
                                    if let Some(key) = new_arr {
                                        if let Some(arr) = state.arr_vars.get(&key) {
                                            let elems: Vec<String> =
                                                arr.iter().map(|v| v.to_string()).collect();
                                            println!("[{}]", elems.join(", "));
                                        } else {
                                            println!("{}", val);
                                        }
                                    } else {
                                        println!("{}", val);
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        // Snapshot to detect intermediate strings/arrays from expression evaluation
                        let str_before: std::collections::HashSet<String> =
                            state.str_vars.keys().cloned().collect();
                        let arr_before: std::collections::HashSet<String> =
                            state.arr_vars.keys().cloned().collect();
                        let val = eval_expr(expr, state)?;
                        let new_str = state
                            .str_vars
                            .keys()
                            .filter(|k| !str_before.contains(*k))
                            .max_by(|a, b| a.cmp(b))
                            .cloned();
                        if let Some(key) = new_str {
                            if let Some(s) = state.str_vars.get(&key) {
                                println!("{}", s);
                            } else {
                                println!("{}", val);
                            }
                        } else {
                            let new_arr = state
                                .arr_vars
                                .keys()
                                .filter(|k| !arr_before.contains(*k))
                                .max_by(|a, b| a.cmp(b))
                                .cloned();
                            if let Some(key) = new_arr {
                                if let Some(arr) = state.arr_vars.get(&key) {
                                    let elems: Vec<String> =
                                        arr.iter().map(|v| v.to_string()).collect();
                                    println!("[{}]", elems.join(", "));
                                } else {
                                    println!("{}", val);
                                }
                            } else {
                                println!("{}", val);
                            }
                        }
                    }
                }
            }
            ast::Stmt::Return(Some(expr)) => {
                let val = eval_expr(expr, state)?;
                return Ok(Some(val));
            }
            ast::Stmt::Return(None) => return Ok(Some(0)),
            ast::Stmt::Break => return Ok(Some(BREAK_SENTINEL)),
            ast::Stmt::Continue => return Ok(Some(CONTINUE_SENTINEL)),
            ast::Stmt::If {
                condition,
                then,
                else_,
            } => {
                let cond = eval_expr(condition, state)?;
                if cond != 0 {
                    if let Some(val) = exec_block(then, state)? {
                        return Ok(Some(val));
                    }
                } else if let Some(else_body) = else_ {
                    if let Some(val) = exec_block(else_body, state)? {
                        return Ok(Some(val));
                    }
                }
            }
            ast::Stmt::While { condition, body } => loop {
                let cond = eval_expr(condition, state)?;
                if cond == 0 {
                    break;
                }
                match exec_block(body, state)? {
                    Some(BREAK_SENTINEL) => break,
                    Some(CONTINUE_SENTINEL) => continue,
                    Some(val) => return Ok(Some(val)),
                    None => {}
                }
            },
            ast::Stmt::For {
                variable,
                iterable,
                body,
            } => {
                // Check if iterable is a string
                if let ast::Expr::Ident(n) = iterable {
                    if let Some(s) = state.str_vars.get(n).cloned() {
                        let chars: Vec<i64> = s.bytes().map(|b| b as i64).collect();
                        for c in chars {
                            state.vars.insert(variable.clone(), c);
                            state.str_vars.remove(variable); // shadow string with char
                            match exec_block(body, state)? {
                                Some(BREAK_SENTINEL) => break,
                                Some(CONTINUE_SENTINEL) => continue,
                                Some(val) => return Ok(Some(val)),
                                None => {}
                            }
                        }
                        continue;
                    }
                }
                if let ast::Expr::Str(s) = iterable {
                    let chars: Vec<i64> = s.bytes().map(|b| b as i64).collect();
                    for c in chars {
                        state.vars.insert(variable.clone(), c);
                        match exec_block(body, state)? {
                            Some(BREAK_SENTINEL) => break,
                            Some(CONTINUE_SENTINEL) => continue,
                            Some(val) => return Ok(Some(val)),
                            None => {}
                        }
                    }
                    continue;
                }
                // Named array variable iteration
                if let ast::Expr::Ident(n) = iterable {
                    if let Some(arr) = state.arr_vars.get(n).cloned() {
                        for v in arr {
                            state.vars.insert(variable.clone(), v);
                            state.arr_vars.remove(variable); // shadow array with element
                            match exec_block(body, state)? {
                                Some(BREAK_SENTINEL) => break,
                                Some(CONTINUE_SENTINEL) => continue,
                                Some(val) => return Ok(Some(val)),
                                None => {}
                            }
                        }
                        continue;
                    }
                }
                // Regular numeric range
                let count = eval_expr(iterable, state)?;
                for i in 0..count {
                    state.vars.insert(variable.clone(), i);
                    match exec_block(body, state)? {
                        Some(BREAK_SENTINEL) => break,
                        Some(CONTINUE_SENTINEL) => continue,
                        Some(val) => return Ok(Some(val)),
                        None => {}
                    }
                }
            }
            ast::Stmt::ExprStmt(expr) => {
                eval_expr(expr, state)?;
            }
            ast::Stmt::IfLet {
                value, then, else_, ..
            } => {
                let val = eval_expr(value, state)?;
                if val != 0 {
                    if let Some(result) = exec_block(then, state)? {
                        return Ok(Some(result));
                    }
                } else if let Some(else_body) = else_ {
                    if let Some(result) = exec_block(else_body, state)? {
                        return Ok(Some(result));
                    }
                }
            }
        }
    }
    Ok(None)
}

/// Resolve the string value of an expression, if it has one.
fn resolve_str(expr: &ast::Expr, state: &InterpreterState) -> Option<String> {
    match expr {
        ast::Expr::Ident(n) => state.str_vars.get(n).cloned(),
        ast::Expr::Str(s) => Some(s.clone()),
        _ => None,
    }
}

/// Resolve the array value of an expression, if it has one.
fn resolve_arr(expr: &ast::Expr, state: &InterpreterState) -> Option<Vec<i64>> {
    match expr {
        ast::Expr::Ident(n) => state.arr_vars.get(n).cloned(),
        _ => None,
    }
}

fn eval_expr(expr: &ast::Expr, state: &mut InterpreterState) -> anyhow::Result<i64> {
    match expr {
        ast::Expr::Int(n) => Ok(*n),
        ast::Expr::Float(n) => Ok(*n as i64),
        ast::Expr::Bool(b) => Ok(if *b { 1 } else { 0 }),
        ast::Expr::Str(s) => {
            // Store with auto key — exec_block will rename it to the variable
            let key = format!("__auto_{}", state.str_vars.len());
            state.str_vars.insert(key, s.clone());
            Ok(0)
        }
        ast::Expr::Ident(name) => {
            // Check strings first, then arrays, then integers
            if state.str_vars.contains_key(name)
                || state.arr_vars.contains_key(name)
                || state.lambdas.contains_key(name)
            {
                Ok(0) // these types use 0 as the integer representation
            } else {
                Ok(*state.vars.get(name.as_str()).unwrap_or(&0))
            }
        }
        ast::Expr::ArrayLiteral(elems) => {
            let mut vals = Vec::new();
            for e in elems {
                let v = eval_expr(e, state)?;
                vals.push(v);
            }
            let key = format!("__auto_arr_{}", state.arr_vars.len());
            state.arr_vars.insert(key, vals);
            Ok(0)
        }
        ast::Expr::StructLiteral { name, fields, .. } => {
            // Evaluate struct literal: Point { x: 1, y: 2 }
            let field_defs = state.struct_fields.get(name).cloned().unwrap_or_default();
            let mut values = vec![0i64; field_defs.len()];
            for (fname, fval) in fields {
                if let Some(idx) = field_defs.iter().position(|f| f == fname) {
                    values[idx] = eval_expr(fval, state)?;
                }
            }
            let key = format!("__struct_{}", state.struct_instances.len());
            state.struct_type_of.insert(key.clone(), name.clone());
            state.struct_instances.insert(key, values);
            Ok(0)
        }
        ast::Expr::EnumVariant {
            enum_name,
            variant,
            payload,
            ..
        } => {
            // Evaluate an enum variant instance: Color::Green or Maybe::Has(42)
            let valid = state
                .enum_defs
                .get(enum_name)
                .map(|vs| vs.iter().any(|v| v == variant))
                .unwrap_or(false);
            if !valid {
                return Err(anyhow::anyhow!(
                    "Unknown enum variant {}::{}",
                    enum_name,
                    variant
                ));
            }
            let payload_val = match payload {
                Some(p) => eval_expr(p, state)?,
                None => 0,
            };
            let key = format!("__enum_{}", state.enum_instances.len());
            state
                .enum_instances
                .insert(key, (variant.clone(), Some(enum_name.clone()), payload_val));
            Ok(0)
        }
        ast::Expr::Match { scrutinee, arms } => {
            // Match-as-expression: return the matched arm's value.
            // Scrutinee may be an enum instance stored under a name or auto key.
            let scrut_key: Option<String> = match &**scrutinee {
                ast::Expr::Ident(n) => {
                    if state.enum_instances.contains_key(n.as_str()) {
                        Some(n.clone())
                    } else {
                        // Snapshot auto keys, eval, find new enum instance
                        let before: std::collections::HashSet<String> =
                            state.enum_instances.keys().cloned().collect();
                        eval_expr(scrutinee, state)?;
                        state
                            .enum_instances
                            .keys()
                            .filter(|k| !before.contains(*k))
                            .max_by(|a, b| a.cmp(b))
                            .cloned()
                    }
                }
                _ => {
                    let before: std::collections::HashSet<String> =
                        state.enum_instances.keys().cloned().collect();
                    eval_expr(scrutinee, state)?;
                    state
                        .enum_instances
                        .keys()
                        .filter(|k| !before.contains(*k))
                        .max_by(|a, b| a.cmp(b))
                        .cloned()
                }
            };
            let scrut_instance = scrut_key
                .as_ref()
                .and_then(|k| state.enum_instances.get(k).cloned());

            // Resolve scrutinee scalar/string/arr values for literal & variable patterns
            let scrut_val = eval_expr(scrutinee, state)?;
            let scrut_str = resolve_str(scrutinee, state);
            let scrut_arr = resolve_arr(scrutinee, state);

            for arm in arms {
                // Pattern check
                let matched: Option<Option<i64>> = match &arm.pattern {
                    ast::Pattern::EnumVariant {
                        variant, binding, ..
                    } => {
                        match &scrut_instance {
                            Some((sv, _, sp)) if sv == variant => {
                                // Bind payload if requested
                                if let Some(b) = binding {
                                    state.vars.insert(b.clone(), *sp);
                                }
                                Some(Some(*sp))
                            }
                            _ => None,
                        }
                    }
                    ast::Pattern::SomePattern { binding } => match &scrut_instance {
                        Some((sv, se, sp)) if se.is_none() && sv != "None" => {
                            if let Some(b) = binding {
                                state.vars.insert(b.clone(), *sp);
                            }
                            Some(Some(*sp))
                        }
                        _ => None,
                    },
                    ast::Pattern::NonePattern => match &scrut_instance {
                        Some((sv, _, _)) if sv == "None" => Some(None),
                        _ => None,
                    },
                    ast::Pattern::IntLiteral(n) => {
                        if scrut_val == *n {
                            Some(None)
                        } else {
                            None
                        }
                    }
                    ast::Pattern::BoolLiteral(b) => {
                        let bv = if *b { 1i64 } else { 0i64 };
                        if scrut_val == bv {
                            Some(None)
                        } else {
                            None
                        }
                    }
                    ast::Pattern::StrLiteral(s) => {
                        if scrut_str.as_deref() == Some(s.as_str()) {
                            Some(None)
                        } else {
                            None
                        }
                    }
                    ast::Pattern::Wildcard => Some(None),
                    ast::Pattern::Variable(v) => {
                        // Bind the scrutinee value to the pattern variable
                        if let Some(s) = &scrut_str {
                            state.str_vars.insert(v.clone(), s.clone());
                        } else if let Some(a) = &scrut_arr {
                            state.arr_vars.insert(v.clone(), a.clone());
                        } else {
                            state.vars.insert(v.clone(), scrut_val);
                        }
                        Some(None)
                        // variable patterns match unconditionally
                    }
                };

                if let Some(_payload_val) = matched {
                    // Guard check (binding already done above)
                    if let Some(guard_expr) = &arm.guard {
                        let g = eval_expr(guard_expr, state)?;
                        if g == 0 {
                            continue;
                        }
                    }
                    // Execute the arm body; last ExprStmt value is the match result
                    let mut result: i64 = 0;
                    let n_stmts = arm.body.len();
                    for (i, stmt) in arm.body.iter().enumerate() {
                        match stmt {
                            ast::Stmt::ExprStmt(e) if i == n_stmts - 1 => {
                                result = eval_expr(e, state)?;
                            }
                            ast::Stmt::Return(Some(e)) => {
                                return eval_expr(e, state);
                            }
                            ast::Stmt::Return(None) => return Ok(0),
                            _ => {
                                exec_block(std::slice::from_ref(stmt), state)?;
                            }
                        }
                    }
                    return Ok(result);
                }
            }
            // No arm matched
            Err(anyhow::anyhow!("match expression: no arm matched"))
        }
        ast::Expr::Lambda { params, body, .. } => {
            let key = format!("__lambda_{}", state.lambda_counter);
            state.lambda_counter += 1;
            state.lambdas.insert(
                key,
                (
                    params.clone(),
                    body.clone(),
                    CapturedScope {
                        vars: state.vars.clone(),
                        str_vars: state.str_vars.clone(),
                        arr_vars: state.arr_vars.clone(),
                        struct_instances: state.struct_instances.clone(),
                        struct_type_of: state.struct_type_of.clone(),
                        enum_instances: state.enum_instances.clone(),
                    },
                ),
            );
            Ok(0)
        }
        ast::Expr::BinaryOp { op, left, right } => {
            // Snapshot auto keys before evaluating each side to detect intermediate strings
            let str_before_left: std::collections::HashSet<String> = state
                .str_vars
                .keys()
                .filter(|k| k.starts_with("__auto_"))
                .cloned()
                .collect();
            let l = eval_expr(left, state)?;
            let str_after_left: std::collections::HashSet<String> = state
                .str_vars
                .keys()
                .filter(|k| k.starts_with("__auto_"))
                .cloned()
                .collect();
            let left_auto_key: Option<String> = str_after_left
                .difference(&str_before_left)
                .cloned()
                .max_by(|a, b| a.cmp(b));

            let str_before_right: std::collections::HashSet<String> = state
                .str_vars
                .keys()
                .filter(|k| k.starts_with("__auto_"))
                .cloned()
                .collect();
            let r = eval_expr(right, state)?;
            let str_after_right: std::collections::HashSet<String> = state
                .str_vars
                .keys()
                .filter(|k| k.starts_with("__auto_"))
                .cloned()
                .collect();
            let right_auto_key: Option<String> = str_after_right
                .difference(&str_before_right)
                .cloned()
                .max_by(|a, b| a.cmp(b));

            // Resolve string values — prefer auto keys from intermediate eval, then AST-based resolution
            let left_str = left_auto_key
                .and_then(|k| state.str_vars.get(&k).cloned())
                .or_else(|| resolve_str(left, state));
            let right_str = right_auto_key
                .and_then(|k| state.str_vars.get(&k).cloned())
                .or_else(|| resolve_str(right, state));
            let left_arr = resolve_arr(left, state);
            let right_arr = resolve_arr(right, state);

            match op {
                ast::BinOp::Add => {
                    // String concatenation
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        let combined = format!("{}{}", ls, rs);
                        let key = format!("__auto_{}", state.str_vars.len());
                        state.str_vars.insert(key, combined);
                        return Ok(0);
                    }
                    // Array concat
                    if let (Some(la), Some(ra)) = (&left_arr, &right_arr) {
                        let mut combined = la.clone();
                        combined.extend(ra);
                        let key = format!("__auto_arr_{}", state.arr_vars.len());
                        state.arr_vars.insert(key, combined);
                        return Ok(0);
                    }
                    Ok(l + r)
                }
                ast::BinOp::Sub => Ok(l - r),
                ast::BinOp::Mul => Ok(l * r),
                ast::BinOp::Div => Ok(if r != 0 { l / r } else { 0 }),
                ast::BinOp::Mod => Ok(if r != 0 { l % r } else { 0 }),
                ast::BinOp::Eq => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls == rs { 1 } else { 0 })
                    } else {
                        Ok(if l == r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Neq => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls != rs { 1 } else { 0 })
                    } else {
                        Ok(if l != r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Lt => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls < rs { 1 } else { 0 })
                    } else {
                        Ok(if l < r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Gt => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls > rs { 1 } else { 0 })
                    } else {
                        Ok(if l > r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Le => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls <= rs { 1 } else { 0 })
                    } else {
                        Ok(if l <= r { 1 } else { 0 })
                    }
                }
                ast::BinOp::Ge => {
                    if let (Some(ls), Some(rs)) = (&left_str, &right_str) {
                        Ok(if ls >= rs { 1 } else { 0 })
                    } else {
                        Ok(if l >= r { 1 } else { 0 })
                    }
                }
                ast::BinOp::And => Ok(if l != 0 && r != 0 { 1 } else { 0 }),
                ast::BinOp::Or => Ok(if l != 0 || r != 0 { 1 } else { 0 }),
            }
        }
        ast::Expr::UnaryOp { op, expr } => {
            let val = eval_expr(expr, state)?;
            match op {
                ast::UnOp::Neg => Ok(-val),
                ast::UnOp::Not => Ok(if val == 0 { 1 } else { 0 }),
            }
        }
        ast::Expr::Index { target, index } => {
            let idx = eval_expr(index, state)? as usize;
            if let ast::Expr::Ident(n) = target.as_ref() {
                if let Some(arr) = state.arr_vars.get(n) {
                    return Ok(arr.get(idx).copied().unwrap_or(0));
                }
                if let Some(s) = state.str_vars.get(n) {
                    return Ok(s
                        .as_bytes()
                        .get(idx)
                        .copied()
                        .map(|b| b as i64)
                        .unwrap_or(0));
                }
            }
            Ok(0)
        }
        ast::Expr::Call {
            name,
            type_args: _,
            args,
        } => {
            if name == "print" {
                if !args.is_empty() {
                    // Inline print resolution for call context
                    match &args[0] {
                        ast::Expr::Ident(n) => {
                            if let Some(s) = state.str_vars.get(n) {
                                println!("{}", s);
                            } else if let Some(arr) = state.arr_vars.get(n) {
                                let elems: Vec<String> =
                                    arr.iter().map(|v| v.to_string()).collect();
                                println!("[{}]", elems.join(", "));
                            } else {
                                let val = state.vars.get(n.as_str()).unwrap_or(&0);
                                println!("{}", val);
                            }
                        }
                        ast::Expr::Str(s) => println!("{}", s),
                        // C backend prints bool literals as true/false
                        ast::Expr::Bool(b) => println!("{}", if *b { "true" } else { "false" }),
                        ast::Expr::ArrayLiteral(elems) => {
                            let vals: Vec<String> = elems
                                .iter()
                                .map(|e| match e {
                                    ast::Expr::Str(s) => s.to_string(),
                                    ast::Expr::Int(n) => n.to_string(),
                                    _ => "?".to_string(),
                                })
                                .collect();
                            println!("[{}]", vals.join(", "));
                        }
                        other => {
                            let str_before: std::collections::HashSet<String> =
                                state.str_vars.keys().cloned().collect();
                            let arr_before: std::collections::HashSet<String> =
                                state.arr_vars.keys().cloned().collect();
                            let val = eval_expr(other, state)?;
                            if val != 0 {
                                println!("{}", val);
                            } else {
                                let new_str = state
                                    .str_vars
                                    .keys()
                                    .filter(|k| !str_before.contains(*k))
                                    .max_by(|a, b| a.cmp(b))
                                    .cloned();
                                if let Some(key) = new_str {
                                    if let Some(s) = state.str_vars.get(&key) {
                                        println!("{}", s);
                                    } else {
                                        println!("{}", val);
                                    }
                                } else {
                                    let new_arr = state
                                        .arr_vars
                                        .keys()
                                        .filter(|k| !arr_before.contains(*k))
                                        .max_by(|a, b| a.cmp(b))
                                        .cloned();
                                    if let Some(key) = new_arr {
                                        if let Some(arr) = state.arr_vars.get(&key) {
                                            let elems: Vec<String> =
                                                arr.iter().map(|v| v.to_string()).collect();
                                            println!("[{}]", elems.join(", "));
                                        } else {
                                            println!("{}", val);
                                        }
                                    } else {
                                        println!("{}", val);
                                    }
                                }
                            }
                        }
                    }
                }
                return Ok(0);
            }
            if name == "len" && args.len() == 1 {
                // Snapshot auto-keys before evaluating arg (detect side-effectful calls)
                let str_keys_before: std::collections::HashSet<String> =
                    state.str_vars.keys().cloned().collect();
                let arr_keys_before: std::collections::HashSet<String> =
                    state.arr_vars.keys().cloned().collect();
                let _val = eval_expr(&args[0], state)?;
                // Check for new string auto-key
                let new_str_key = state
                    .str_vars
                    .keys()
                    .filter(|k| !str_keys_before.contains(*k))
                    .max_by(|a, b| a.cmp(b))
                    .cloned();
                if let Some(key) = new_str_key {
                    if let Some(s) = state.str_vars.get(&key) {
                        return Ok(s.len() as i64);
                    }
                }
                // Check for new array auto-key
                let new_arr_key = state
                    .arr_vars
                    .keys()
                    .filter(|k| !arr_keys_before.contains(*k))
                    .max_by(|a, b| a.cmp(b))
                    .cloned();
                if let Some(key) = new_arr_key {
                    if let Some(a) = state.arr_vars.get(&key) {
                        return Ok(a.len() as i64);
                    }
                }
                // Fallback: check named variables
                match &args[0] {
                    ast::Expr::Str(s) => return Ok(s.len() as i64),
                    ast::Expr::Ident(n) => {
                        if let Some(s) = state.str_vars.get(n) {
                            return Ok(s.len() as i64);
                        }
                        if let Some(a) = state.arr_vars.get(n) {
                            return Ok(a.len() as i64);
                        }
                    }
                    ast::Expr::ArrayLiteral(elems) => return Ok(elems.len() as i64),
                    _ => {}
                }
                return Ok(0);
            }
            // Array built-in: map(arr, lambda)
            if name == "map" && args.len() == 2 {
                // Get the array
                let arr = match &args[0] {
                    ast::Expr::Ident(n) => state.arr_vars.get(n).cloned().unwrap_or_default(),
                    _ => {
                        // Evaluate to get array - try to find auto key
                        eval_expr(&args[0], state)?;
                        let new_key = state
                            .arr_vars
                            .keys()
                            .find(|k| k.starts_with("__auto_arr_"))
                            .cloned();
                        if let Some(key) = new_key {
                            state.arr_vars.remove(&key).unwrap_or_default()
                        } else {
                            vec![]
                        }
                    }
                };
                // Get the lambda
                let lambda_info: Option<(Vec<ast::Param>, Vec<ast::Stmt>, CapturedScope)> =
                    match &args[1] {
                        ast::Expr::Lambda { params, body, .. } => Some((
                            params.clone(),
                            body.clone(),
                            CapturedScope {
                                vars: state.vars.clone(),
                                str_vars: state.str_vars.clone(),
                                arr_vars: state.arr_vars.clone(),
                                struct_instances: state.struct_instances.clone(),
                                struct_type_of: state.struct_type_of.clone(),
                                enum_instances: state.enum_instances.clone(),
                            },
                        )),
                        ast::Expr::Ident(n) => state.lambdas.get(n).cloned(),
                        _ => None,
                    };
                if let Some((params, body, cap)) = lambda_info {
                    let mut result = Vec::new();
                    for elem in &arr {
                        let mut local_state = InterpreterState::new();
                        local_state.functions = state.functions.clone();
                        local_state.struct_fields = state.struct_fields.clone();
                        local_state.inject_scope(&cap);
                        if let Some(param) = params.first() {
                            local_state.vars.insert(param.name.clone(), *elem);
                        }
                        match exec_block(&body, &mut local_state)? {
                            Some(v) => result.push(v),
                            None => result.push(0),
                        }
                    }
                    let key = format!("__auto_arr_{}", state.arr_vars.len());
                    state.arr_vars.insert(key, result);
                    return Ok(0);
                }
                return Ok(0);
            }
            // Array built-in: filter(arr, lambda)
            if name == "filter" && args.len() == 2 {
                let arr = match &args[0] {
                    ast::Expr::Ident(n) => state.arr_vars.get(n).cloned().unwrap_or_default(),
                    _ => {
                        eval_expr(&args[0], state)?;
                        let new_key = state
                            .arr_vars
                            .keys()
                            .find(|k| k.starts_with("__auto_arr_"))
                            .cloned();
                        if let Some(key) = new_key {
                            state.arr_vars.remove(&key).unwrap_or_default()
                        } else {
                            vec![]
                        }
                    }
                };
                let lambda_info: Option<(Vec<ast::Param>, Vec<ast::Stmt>, CapturedScope)> =
                    match &args[1] {
                        ast::Expr::Lambda { params, body, .. } => Some((
                            params.clone(),
                            body.clone(),
                            CapturedScope {
                                vars: state.vars.clone(),
                                str_vars: state.str_vars.clone(),
                                arr_vars: state.arr_vars.clone(),
                                struct_instances: state.struct_instances.clone(),
                                struct_type_of: state.struct_type_of.clone(),
                                enum_instances: state.enum_instances.clone(),
                            },
                        )),
                        ast::Expr::Ident(n) => state.lambdas.get(n).cloned(),
                        _ => None,
                    };
                if let Some((params, body, cap)) = lambda_info {
                    let mut result = Vec::new();
                    for elem in &arr {
                        let mut local_state = InterpreterState::new();
                        local_state.functions = state.functions.clone();
                        local_state.struct_fields = state.struct_fields.clone();
                        local_state.inject_scope(&cap);
                        if let Some(param) = params.first() {
                            local_state.vars.insert(param.name.clone(), *elem);
                        }
                        let cond = exec_block(&body, &mut local_state)?.unwrap_or_default();
                        if cond != 0 {
                            result.push(*elem);
                        }
                    }
                    let key = format!("__auto_arr_{}", state.arr_vars.len());
                    state.arr_vars.insert(key, result);
                    return Ok(0);
                }
                return Ok(0);
            }
            // Array built-in: reduce(arr, lambda, initial)
            if name == "reduce" && args.len() == 3 {
                let arr = match &args[0] {
                    ast::Expr::Ident(n) => state.arr_vars.get(n).cloned().unwrap_or_default(),
                    _ => {
                        eval_expr(&args[0], state)?;
                        let new_key = state
                            .arr_vars
                            .keys()
                            .find(|k| k.starts_with("__auto_arr_"))
                            .cloned();
                        if let Some(key) = new_key {
                            state.arr_vars.remove(&key).unwrap_or_default()
                        } else {
                            vec![]
                        }
                    }
                };
                let mut acc = eval_expr(&args[2], state)?;
                let lambda_info: Option<(Vec<ast::Param>, Vec<ast::Stmt>, CapturedScope)> =
                    match &args[1] {
                        ast::Expr::Lambda { params, body, .. } => Some((
                            params.clone(),
                            body.clone(),
                            CapturedScope {
                                vars: state.vars.clone(),
                                str_vars: state.str_vars.clone(),
                                arr_vars: state.arr_vars.clone(),
                                struct_instances: state.struct_instances.clone(),
                                struct_type_of: state.struct_type_of.clone(),
                                enum_instances: state.enum_instances.clone(),
                            },
                        )),
                        ast::Expr::Ident(n) => state.lambdas.get(n).cloned(),
                        _ => None,
                    };
                if let Some((params, body, cap)) = lambda_info {
                    for elem in &arr {
                        let mut local_state = InterpreterState::new();
                        local_state.functions = state.functions.clone();
                        local_state.struct_fields = state.struct_fields.clone();
                        local_state.inject_scope(&cap);
                        if params.len() >= 2 {
                            local_state.vars.insert(params[0].name.clone(), acc);
                            local_state.vars.insert(params[1].name.clone(), *elem);
                        } else if let Some(param) = params.first() {
                            local_state.vars.insert(param.name.clone(), *elem);
                        }
                        acc = exec_block(&body, &mut local_state)?.unwrap_or_default();
                    }
                    return Ok(acc);
                }
                return Ok(0);
            }
            // Lambda call
            if let Some((params, body, cap)) = state.lambdas.get(name).cloned() {
                let mut local_state = InterpreterState::new();
                local_state.functions = state.functions.clone();
                local_state.struct_fields = state.struct_fields.clone();
                local_state.inject_scope(&cap);
                // Inject explicit arguments (override captured if same name)
                for (param, arg) in params.iter().zip(args.iter()) {
                    let val = eval_expr(arg, state)?;
                    local_state.vars.insert(param.name.clone(), val);
                    // If arg is an Ident referring to a struct, copy the struct instance data
                    if let ast::Expr::Ident(arg_name) = arg {
                        if let Some(inst) = state.struct_instances.get(arg_name.as_str()) {
                            local_state
                                .struct_instances
                                .insert(param.name.clone(), inst.clone());
                        }
                        if let Some(ty) = state.struct_type_of.get(arg_name.as_str()) {
                            local_state
                                .struct_type_of
                                .insert(param.name.clone(), ty.clone());
                        }
                    }
                }
                match exec_block(&body, &mut local_state)? {
                    Some(v) => Ok(v),
                    None => Ok(0),
                }
            } else if let Some((params, _ret, body)) = state
                .functions
                .get(name)
                .cloned()
                .or_else(|| state.impl_methods.get(name).cloned())
            {
                let mut local_state = InterpreterState::new();
                local_state.functions = state.functions.clone();
                local_state.impl_methods = state.impl_methods.clone();
                local_state.struct_fields = state.struct_fields.clone();
                local_state.struct_instances = state.struct_instances.clone();
                local_state.struct_type_of = state.struct_type_of.clone();
                for (param, arg) in params.iter().zip(args.iter()) {
                    let val = eval_expr(arg, state)?;
                    local_state.vars.insert(param.name.clone(), val);
                    // If arg is an Ident referring to a struct, copy the struct instance data
                    if let ast::Expr::Ident(arg_name) = arg {
                        if let Some(inst) = state.struct_instances.get(arg_name.as_str()) {
                            local_state
                                .struct_instances
                                .insert(param.name.clone(), inst.clone());
                        }
                        if let Some(ty) = state.struct_type_of.get(arg_name.as_str()) {
                            local_state
                                .struct_type_of
                                .insert(param.name.clone(), ty.clone());
                        }
                    }
                }
                match exec_block(&body, &mut local_state)? {
                    Some(v) => Ok(v),
                    None => Ok(0),
                }
            } else {
                Ok(0)
            }
        }
        ast::Expr::Range {
            start,
            end,
            inclusive,
        } => {
            let s = eval_expr(start, state)?;
            let e = eval_expr(end, state)?;
            let count = if *inclusive { e - s + 1 } else { e - s };
            Ok(if count > 0 { count } else { 0 })
        }
        ast::Expr::FieldAccess { target, field } => {
            if let ast::Expr::Ident(name) = target.as_ref() {
                // Struct field access: my_struct.field
                if let Some(instance) = state.struct_instances.get(name) {
                    // Look up field defs using the type name, not the variable name
                    let type_name = state.struct_type_of.get(name).cloned();
                    if let Some(ref tn) = type_name {
                        if let Some(fields) = state.struct_fields.get(tn) {
                            if let Some(idx) = fields.iter().position(|f| f == field) {
                                return Ok(instance.get(idx).copied().unwrap_or(0));
                            }
                        }
                    }
                }
                // Fallback: string/array .len
                if let Some(s) = state.str_vars.get(name) {
                    if field == "len" {
                        return Ok(s.len() as i64);
                    }
                }
                if let Some(a) = state.arr_vars.get(name) {
                    if field == "len" {
                        return Ok(a.len() as i64);
                    }
                }
            }
            Ok(0)
        }
        ast::Expr::MethodCall {
            target,
            method,
            args,
        } => {
            // Built-in method sugar. Struct/trait methods resolve elsewhere;
            // here we handle string and array builtins natively.
            // Chained targets (s.trim().to_lower()) carry their value in a fresh
            // __auto_ string key: resolve the target first so chains work.
            let mut chain_target: Option<ast::Expr> = None;
            if !matches!(target.as_ref(), ast::Expr::Ident(_)) {
                let before: std::collections::HashSet<String> =
                    state.str_vars.keys().cloned().collect();
                eval_expr(target, state)?;
                if let Some(k) = state
                    .str_vars
                    .keys()
                    .filter(|k| k.starts_with("__auto_") && !before.contains(*k))
                    .max_by(|x, y| x.cmp(y))
                    .cloned()
                {
                    chain_target = Some(ast::Expr::Ident(k));
                }
            }
            let target = chain_target.map(Box::new).unwrap_or_else(|| target.clone());
            let is_array_target =
                matches!(target.as_ref(), ast::Expr::Ident(n) if state.arr_vars.contains_key(n));
            let is_string_target =
                matches!(target.as_ref(), ast::Expr::Ident(n) if state.str_vars.contains_key(n));
            if is_array_target {
                match method.as_str() {
                    "map" | "filter" if args.len() == 1 => {
                        let call = ast::Expr::Call {
                            name: method.clone(),
                            type_args: vec![],
                            args: vec![target.as_ref().clone(), args[0].clone()],
                        };
                        return eval_expr(&call, state);
                    }
                    "reduce" if args.len() == 2 => {
                        let call = ast::Expr::Call {
                            name: "reduce".to_string(),
                            type_args: vec![],
                            args: vec![target.as_ref().clone(), args[0].clone(), args[1].clone()],
                        };
                        return eval_expr(&call, state);
                    }
                    "push" if args.len() == 1 => {
                        let v = eval_expr(&args[0], state)?;
                        if let ast::Expr::Ident(n) = target.as_ref() {
                            if let Some(a) = state.arr_vars.get_mut(n) {
                                a.push(v);
                            }
                        }
                        return Ok(0);
                    }
                    "pop" if args.is_empty() => {
                        if let ast::Expr::Ident(n) = target.as_ref() {
                            if let Some(a) = state.arr_vars.get_mut(n) {
                                return Ok(a.pop().unwrap_or(0));
                            }
                        }
                        return Ok(0);
                    }
                    "sort" if args.is_empty() => {
                        if let ast::Expr::Ident(n) = target.as_ref() {
                            if let Some(a) = state.arr_vars.get_mut(n) {
                                a.sort();
                            }
                        }
                        return Ok(0);
                    }
                    "reverse" if args.is_empty() => {
                        if let ast::Expr::Ident(n) = target.as_ref() {
                            if let Some(a) = state.arr_vars.get_mut(n) {
                                a.reverse();
                            }
                        }
                        return Ok(0);
                    }
                    _ => return Ok(0),
                }
            }
            if is_string_target {
                let s = match target.as_ref() {
                    ast::Expr::Ident(n) => state.str_vars.get(n).cloned().unwrap_or_default(),
                    _ => String::new(),
                };
                // Evaluate string-method args (all are strings or ints)
                let mut arg_vals: Vec<i64> = Vec::new();
                let mut arg_strs: Vec<String> = Vec::new();
                for a in args {
                    // Strings come back via __auto_ keys; ints via value
                    let before: std::collections::HashSet<String> =
                        state.str_vars.keys().cloned().collect();
                    let v = eval_expr(a, state)?;
                    let new_key = state
                        .str_vars
                        .keys()
                        .filter(|k| k.starts_with("__auto_") && !before.contains(*k))
                        .max_by(|x, y| x.cmp(y))
                        .cloned();
                    if let Some(k) = new_key {
                        arg_strs.push(state.str_vars.get(&k).cloned().unwrap_or_default());
                        state.str_vars.remove(&k);
                    } else if let ast::Expr::Str(sv) = a {
                        arg_strs.push(sv.clone());
                    } else {
                        arg_strs.push(String::new());
                        arg_vals.push(v);
                    }
                }
                let result: String = match method.as_str() {
                    "to_upper" => s.to_uppercase(),
                    "to_lower" => s.to_lowercase(),
                    "trim" => s.trim().to_string(),
                    "replace" if arg_strs.len() == 2 => s.replace(&arg_strs[0], &arg_strs[1]),
                    "substring" if arg_vals.len() == 2 => {
                        let start = arg_vals[0].max(0) as usize;
                        let end = (start + arg_vals[1].max(0) as usize).min(s.len());
                        s.chars()
                            .skip(start)
                            .take(end.saturating_sub(start))
                            .collect()
                    }
                    "char_at" if arg_vals.len() == 1 => {
                        // C runtime returns the char code; keep backends in parity
                        s.chars()
                            .nth(arg_vals[0].max(0) as usize)
                            .map(|c| (c as i64).to_string())
                            .unwrap_or_else(|| "0".to_string())
                    }
                    "repeat" if arg_vals.len() == 1 => s.repeat(arg_vals[0].max(0) as usize),
                    _ => {
                        // Boolean/int-valued methods: compute and return as int
                        let b: i64 = match method.as_str() {
                            "contains" if arg_strs.len() == 1 => s.contains(&arg_strs[0]) as i64,
                            "starts_with" if arg_strs.len() == 1 => {
                                s.starts_with(&arg_strs[0]) as i64
                            }
                            "ends_with" if arg_strs.len() == 1 => s.ends_with(&arg_strs[0]) as i64,
                            "equals" if arg_strs.len() == 1 => (s == arg_strs[0]) as i64,
                            "is_empty" if args.is_empty() => s.is_empty() as i64,
                            "len" => s.chars().count() as i64,
                            "find" if arg_strs.len() == 1 => {
                                s.find(&arg_strs[0]).map(|i| i as i64).unwrap_or(-1)
                            }
                            _ => return Err(anyhow::anyhow!("Unknown string method '{}'", method)),
                        };
                        return Ok(b);
                    }
                };
                let key = format!("__auto_{}", state.str_vars.len());
                state.str_vars.insert(key, result);
                return Ok(0);
            }
            // Not a builtin target: fall back to struct/trait method call
            let method_name = if let ast::Expr::Ident(n) = target.as_ref() {
                // Try Type_method for struct instances
                if let Some(ty) = state.struct_type_of.get(n) {
                    format!("{}_{}", ty, method)
                } else {
                    method.clone()
                }
            } else {
                method.clone()
            };
            let mut call_args = vec![target.as_ref().clone()];
            call_args.extend(args.iter().cloned());
            let call = ast::Expr::Call {
                name: method_name,
                type_args: vec![],
                args: call_args,
            };
            eval_expr(&call, state)
        }
        ast::Expr::FString(parts) => {
            // Build the f-string by evaluating each part
            let mut result = String::new();
            for part in parts {
                match part {
                    crate::ast::FStringPart::Literal(s) => {
                        result.push_str(s);
                    }
                    crate::ast::FStringPart::Expr(expr) => {
                        // Evaluate the expression
                        let val = eval_expr(expr, state)?;
                        // Check if a new string was produced (from string ops)
                        let str_before: std::collections::HashSet<String> =
                            state.str_vars.keys().cloned().collect();
                        // The val is 0 for string expressions, so we need to
                        // check if a new auto-key was created
                        let new_str_key = state
                            .str_vars
                            .keys()
                            .filter(|k| k.starts_with("__auto_") && !str_before.contains(*k))
                            .max_by(|a, b| a.cmp(b))
                            .cloned();
                        if let Some(key) = new_str_key {
                            if let Some(s) = state.str_vars.get(&key) {
                                result.push_str(s);
                                // Clean up the consumed key
                                state.str_vars.remove(&key);
                            } else {
                                result.push_str(&val.to_string());
                            }
                        } else {
                            // Check if the expression is a string variable
                            if let ast::Expr::Ident(n) = expr.as_ref() {
                                if let Some(s) = state.str_vars.get(n) {
                                    result.push_str(s);
                                } else {
                                    result.push_str(&val.to_string());
                                }
                            } else {
                                result.push_str(&val.to_string());
                            }
                        }
                    }
                }
            }
            // Store the result in str_vars with an auto key
            let key = format!("__auto_{}", state.str_vars.len());
            state.str_vars.insert(key, result);
            Ok(0)
        }
        _ => Ok(0),
    }
}
