use crate::ast::*;
use std::collections::HashMap;
use std::fmt::Write;

/// LLVM IR text code generator.
/// Produces `.ll` files that can be compiled with `clang` or `llc`.
pub struct LlvmGen {
    output: String,
    strings: Vec<(String, String)>,
    var_counter: usize,
    fn_sigs: HashMap<String, (Vec<String>, String)>,
    enum_tags: HashMap<String, Vec<(String, i64)>>,
    struct_defs: HashMap<String, Vec<(String, String)>>,
    current_fn: String,
    variables: HashMap<String, (String, String)>,
    block_terminated: bool,
    lambda_counter: usize,
    #[allow(clippy::type_complexity)]
    pending_lambdas: Vec<(String, Vec<Param>, Option<Type>, Vec<Stmt>)>,
    /// Maps lambda name -> list of captured variable names (and their LLVM types)
    lambda_captures: HashMap<String, Vec<(String, String)>>,
    /// Maps variable name -> lambda function name (for calls to lambda-typed variables)
    var_lambdas: HashMap<String, String>,
    /// Generic struct definitions: name -> (type_params, fields)
    generic_structs: HashMap<String, (Vec<crate::ast::TypeParamDef>, Vec<Field>)>,
    /// Monomorphized struct names already generated
    mono_structs: std::collections::HashSet<String>,
    /// Pending struct monomorphizations: (mono_name, original_name, type_args)
    pending_mono_structs: Vec<(String, String, Vec<Type>)>,
}

/// Check if an LLVM type string represents a named struct (e.g., "%Point")
fn is_struct_type(ty: &str) -> bool {
    ty.starts_with('%') && ty.len() > 1 && ty[1..].chars().all(|c| c.is_alphanumeric() || c == '_')
}

impl LlvmGen {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            strings: Vec::new(),
            var_counter: 0,
            fn_sigs: HashMap::new(),
            enum_tags: HashMap::new(),
            struct_defs: HashMap::new(),
            current_fn: String::new(),
            variables: HashMap::new(),
            block_terminated: false,
            lambda_counter: 0,
            pending_lambdas: Vec::new(),
            lambda_captures: HashMap::new(),
            var_lambdas: HashMap::new(),
            generic_structs: HashMap::new(),
            mono_structs: std::collections::HashSet::new(),
            pending_mono_structs: Vec::new(),
        }
    }

    pub fn generate(&mut self, program: &Program) -> String {
        // Register enum tags
        for item in &program.items {
            if let TopLevel::EnumDef { name, variants, .. } = item {
                let tags: Vec<(String, i64)> = variants
                    .iter()
                    .enumerate()
                    .map(|(i, v)| (v.name.clone(), i as i64))
                    .collect();
                self.enum_tags.insert(name.clone(), tags);
            }
        }

        // Register struct definitions (separate generic from non-generic)
        for item in &program.items {
            if let TopLevel::StructDef { name, type_params, fields, .. } = item {
                if !type_params.is_empty() {
                    self.generic_structs.insert(name.clone(), (type_params.clone(), fields.clone()));
                } else {
                    let field_tys: Vec<(String, String)> = fields
                        .iter()
                        .map(|f| (f.name.clone(), self.llvm_type(&f.ty)))
                        .collect();
                    self.struct_defs.insert(name.clone(), field_tys);
                }
            }
        }

        // Pre-scan for generic struct usages and queue monomorphizations
        self.prescan_generic_structs(&program.items);
        let pending: Vec<_> = self.pending_mono_structs.clone();
        for (mono_name, original_name, type_args) in &pending {
            self.monomorphize_struct(mono_name, original_name, type_args);
        }

        // Register function signatures (including async fns and impl methods)
        for item in &program.items {
            match item {
                TopLevel::FnDef { name, params, ret, .. }
                | TopLevel::AsyncFnDef { name, params, ret, .. } => {
                    let param_tys: Vec<String> = params.iter().map(|p| self.llvm_type(&p.ty)).collect();
                    let ret_ty = ret
                        .as_ref()
                        .map_or("void".to_string(), |t| self.llvm_type(t));
                    self.fn_sigs.insert(name.clone(), (param_tys, ret_ty));
                }
                TopLevel::ImplDef { type_name, methods, .. } => {
                    for method in methods {
                        if let TopLevel::FnDef { name, params, ret, .. } = method {
                            let mangled = format!("{}_{}", type_name, name);
                            let param_tys: Vec<String> = params.iter().map(|p| self.llvm_type(&p.ty)).collect();
                            let ret_ty = ret
                                .as_ref()
                                .map_or("void".to_string(), |t| self.llvm_type(t));
                            self.fn_sigs.insert(mangled, (param_tys, ret_ty));
                        }
                    }
                }
                TopLevel::ModuleDef { items, .. } => {
                    for sub in items {
                        if let TopLevel::FnDef { name, params, ret, .. } = sub {
                            let param_tys: Vec<String> = params.iter().map(|p| self.llvm_type(&p.ty)).collect();
                            let ret_ty = ret
                                .as_ref()
                                .map_or("void".to_string(), |t| self.llvm_type(t));
                            self.fn_sigs.insert(name.clone(), (param_tys, ret_ty));
                        }
                    }
                }
                _ => {}
            }
        }

        // Preamble
        writeln!(self.output, "; Generated by Sandbox LLVM backend").unwrap();
        writeln!(self.output, "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"").unwrap();
        writeln!(self.output, "target triple = \"x86_64-unknown-linux-gnu\"").unwrap();
        writeln!(self.output).unwrap();

        // Declare external functions
        writeln!(self.output, "; External declarations").unwrap();
        writeln!(self.output, "declare i64 @printf(i8*, ...)").unwrap();
        writeln!(self.output, "declare i8* @malloc(i64)").unwrap();
        writeln!(self.output, "declare void @free(i8*)").unwrap();
        writeln!(self.output, "declare i32 @puts(i8*)").unwrap();
        writeln!(self.output, "declare i32 @fputs(i8*, i8*)").unwrap();
        writeln!(self.output, "declare i8* @strcat(i8*, i8*)").unwrap();
        writeln!(self.output, "declare i64 @strlen(i8*)").unwrap();
        writeln!(self.output, "declare i32 @strcmp(i8*, i8*)").unwrap();
        writeln!(self.output, "declare i32 @snprintf(i8*, i64, i8*, ...)").unwrap();
        writeln!(self.output, "declare i64 @strtol(i8*, i8**, i32)").unwrap();
        writeln!(self.output, "declare double @strtod(i8*, i8**)").unwrap();
        writeln!(self.output).unwrap();

        // Runtime helper declarations (defined in sbx_runtime.c, linked at build time)
        writeln!(self.output, "; Sandbox runtime helpers").unwrap();
        writeln!(self.output, "declare i8* @sbx_rc_alloc(i64)").unwrap();
        writeln!(self.output, "declare void @sbx_rc_retain(i8*)").unwrap();
        writeln!(self.output, "declare i8* @__sbx_to_string(i64)").unwrap();
        writeln!(self.output, "declare i8* @__sbx_to_string_f(double)").unwrap();
        writeln!(self.output, "declare i8* @__sbx_str_concat_multi(i32, ...)").unwrap();
        writeln!(self.output, "declare i64 @__sbx_future_spawn(i64)").unwrap();
        writeln!(self.output, "declare i64 @__sbx_future_wait(i64)").unwrap();
        writeln!(self.output, "declare i64 @__sbx_future_is_ready(i64)").unwrap();
        writeln!(self.output, "declare void @__sbx_sleep(i64)").unwrap();
        writeln!(self.output, "declare i64 @__sbx_time_ms()").unwrap();
        writeln!(self.output, "declare i64 @__sbx_result_unwrap(i64)").unwrap();
        writeln!(self.output, "declare i8* @__sbx_file_read(i8*)").unwrap();
        writeln!(self.output, "declare void @__sbx_file_write(i8*, i8*)").unwrap();
        writeln!(self.output, "declare i64 @__sbx_file_exists(i8*)").unwrap();
        writeln!(self.output, "declare i8* @__sbx_str_sub(i8*, i64, i64)").unwrap();
        writeln!(self.output, "declare i64 @__sbx_str_len(i8*)").unwrap();
        writeln!(self.output, "declare i64 @__sbx_str_eq(i8*, i8*)").unwrap();
        writeln!(self.output, "declare i8* @__sbx_str_concat(i8*, i8*)").unwrap();
        writeln!(self.output, "declare i8* @__sbx_str_trim(i8*)").unwrap();
        writeln!(self.output, "declare i64 @__sbx_str_starts_with(i8*, i8*)").unwrap();
        writeln!(self.output, "declare i64 @__sbx_str_contains(i8*, i8*)").unwrap();
        writeln!(self.output, "declare i64 @__sbx_str_find(i8*, i8*)").unwrap();
        writeln!(self.output, "declare i8* @__sbx_str_to_upper(i8*)").unwrap();
        writeln!(self.output, "declare i8* @__sbx_str_to_lower(i8*)").unwrap();
        writeln!(self.output, "declare i8* @__sbx_str_replace(i8*, i8*, i8*)").unwrap();
        writeln!(self.output, "declare void @__sbx_assert_eq(i64, i64)").unwrap();
        writeln!(self.output, "declare void @__sbx_exit(i32)").unwrap();
        writeln!(self.output).unwrap();

        // Generate struct type definitions
        for (name, fields) in &self.struct_defs {
            let field_tys: Vec<String> = fields.iter().map(|(_, ty)| ty.clone()).collect();
            writeln!(
                self.output,
                "%{} = type {{ {} }}",
                name,
                field_tys.join(", ")
            )
            .unwrap();
        }
        if !self.struct_defs.is_empty() {
            writeln!(self.output).unwrap();
        }

        // Generate functions
        for item in &program.items {
            match item {
                TopLevel::FnDef {
                    name,
                    params,
                    ret,
                    body,
                    ..
                }
                | TopLevel::AsyncFnDef {
                    name,
                    params,
                    ret,
                    body,
                    ..
                } => {
                    // For async fns, we generate a synchronous wrapper.
                    // The function runs the body and returns the result directly.
                    // `await` calls __sbx_future_wait which is a no-op pass-through for this case.
                    // TODO: true thread-based async like the C backend.
                    self.gen_fn(name, params, ret, body);
                }
                TopLevel::EnumDef { name, variants, .. } => {
                    self.gen_enum(name, variants);
                }
                TopLevel::ImplDef { type_name, methods, .. } => {
                    for method in methods {
                        if let TopLevel::FnDef { name, params, ret, body, .. } = method {
                            let mangled = format!("{}_{}", type_name, name);
                            self.gen_fn(&mangled, params, ret, body);
                        }
                    }
                }
                TopLevel::ModuleDef { items, .. } => {
                    for sub in items {
                        if let TopLevel::FnDef { name, params, ret, body, .. } = sub {
                            self.gen_fn(name, params, ret, body);
                        }
                    }
                }
                _ => {}
            }
        }

        // Emit pending lambda definitions
        let lambdas: Vec<_> = self.pending_lambdas.clone();
        for (name, params, ret, body) in &lambdas {
            self.gen_fn(name, params, ret, body);
        }

        // String constants
        let mut string_section = String::new();
        for (name, content) in &self.strings {
            let mut byte_str = String::new();
            for b in content.bytes() {
                if (32..127).contains(&b) && b != b'"' {
                    byte_str.push(b as char);
                } else {
                    write!(&mut byte_str, "\\{:02X}", b).unwrap();
                }
            }
            writeln!(
                string_section,
                "@{} = private constant [{} x i8] c\"{}\\00\"",
                name,
                content.len() + 1,
                byte_str
            )
            .unwrap();
        }

        // Combine: preamble first, then string constants, then functions
        let preamble_end = self
            .output
            .find("; enum ")
            .or_else(|| self.output.find("define "))
            .unwrap_or(self.output.len());
        let mut result = String::new();
        result.push_str(&self.output[..preamble_end]);
        result.push_str(&string_section);
        result.push('\n');
        result.push_str(&self.output[preamble_end..]);
        result
    }

    fn gen_fn(&mut self, name: &str, params: &[Param], ret: &Option<Type>, body: &[Stmt]) {
        self.current_fn = name.to_string();
        self.variables.clear();
        self.block_terminated = false;
        // If the signature isn't pre-registered (e.g. for lambdas), compute it from params.
        let (param_tys, ret_ty) = if let Some(sigs) = self.fn_sigs.get(name) {
            sigs.clone()
        } else {
            let ptys: Vec<String> = params.iter().map(|p| self.llvm_type(&p.ty)).collect();
            let rty = ret.as_ref().map_or("void".to_string(), |t| self.llvm_type(t));
            (ptys, rty)
        };

        // Check if this is a lambda with captures
        let captures = self.lambda_captures.get(name).cloned().unwrap_or_default();

        // Build the full parameter list: original params + capture params
        let mut params_str: Vec<String> = params
            .iter()
            .zip(&param_tys)
            .map(|(p, ty)| format!("{} %{}", ty, p.name))
            .collect();
        // Append capture params
        for (cname, cty) in &captures {
            let cap_param = format!("__cap_{}", cname);
            params_str.push(format!("{} %{}", cty, cap_param));
        }
        writeln!(
            self.output,
            "define {} @{}({}) {{",
            ret_ty,
            name,
            params_str.join(", ")
        )
        .unwrap();
        writeln!(self.output, "entry:").unwrap();

        // Allocate and store parameters
        for (i, p) in params.iter().enumerate() {
            let ty = &param_tys[i];
            let alloca = self.fresh_var();
            writeln!(self.output, "  {} = alloca {}", alloca, ty).unwrap();
            writeln!(
                self.output,
                "  store {} %{}, {}* {}",
                ty, p.name, ty, alloca
            )
            .unwrap();
            self.variables.insert(p.name.clone(), (alloca, ty.clone()));
        }
        // Allocate and store capture params, mapping __cap_<name> to <name> in variables
        for (cname, cty) in &captures {
            let cap_param = format!("__cap_{}", cname);
            let alloca = self.fresh_var();
            writeln!(self.output, "  {} = alloca {}", alloca, cty).unwrap();
            writeln!(
                self.output,
                "  store {} %{}, {}* {}",
                cty, cap_param, cty, alloca
            )
            .unwrap();
            // Map the original variable name to this alloca so the body can reference it
            self.variables.insert(cname.clone(), (alloca, cty.clone()));
        }

        // Generate body
        let mut last_val = "void".to_string();
        for stmt in body {
            last_val = self.gen_stmt(stmt);
        }

        // Return only if the current block isn't already terminated
        if !self.block_terminated {
            if ret_ty == "void" {
                writeln!(self.output, "  ret void").unwrap();
            } else if name == "main" {
                writeln!(self.output, "  ret {} 0", ret_ty).unwrap();
            } else {
                // If the last statement produced no value (e.g. an if/while), return 0.
                let ret_val = if last_val == "void" { "0" } else { last_val.as_str() };
                writeln!(self.output, "  ret {} {}", ret_ty, ret_val).unwrap();
            }
        }

        writeln!(self.output, "}}").unwrap();
        writeln!(self.output).unwrap();
    }

    fn gen_enum(&mut self, name: &str, variants: &[EnumVariantDef]) {
        for (i, v) in variants.iter().enumerate() {
            let tag_name = format!("{}.{}", name, v.name);
            writeln!(self.output, "; enum {}::{} = {}", name, v.name, i).unwrap();
            writeln!(self.output, "@.tag.{} = constant i64 {}", tag_name, i).unwrap();
        }
        writeln!(self.output).unwrap();
    }

    fn gen_stmt(&mut self, stmt: &Stmt) -> String {
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
                // If the value is a lambda, record the var→lambda mapping and skip the store
                // (lambda calls are resolved via var_lambdas, not via i64 variables).
                if let Expr::Lambda { .. } = value {
                    let lam_name = format!("__lambda_{}", self.lambda_counter);
                    self.var_lambdas.insert(name.clone(), lam_name);
                    // Still generate the lambda (registers it in pending_lambdas)
                    let _ = self.gen_expr(value);
                    // Don't emit a store — the lambda name is not an i64 value
                    return "void".to_string();
                }
                let llvm_ty = ty
                    .as_ref()
                    .map_or_else(|| self.infer_llvm_type(value), |t| self.llvm_type(t));
                if is_struct_type(&llvm_ty) {
                    // For struct types, StructLiteral already allocs + stores fields.
                    // Just alias the variable name to the same alloca.
                    let val = self.gen_expr(value);
                    self.variables.insert(name.clone(), (val.clone(), llvm_ty));
                    val
                } else {
                    let val = self.gen_expr(value);
                    let alloca = self.fresh_var();
                    writeln!(self.output, "  {} = alloca {}", alloca, llvm_ty).unwrap();
                    writeln!(
                        self.output,
                        "  store {} {}, {}* {}",
                        llvm_ty, val, llvm_ty, alloca
                    )
                    .unwrap();
                    self.variables.insert(name.clone(), (alloca, llvm_ty));
                    val
                }
            }
            Stmt::Assign { name, value } => {
                let val = self.gen_expr(value);
                if let Some((alloca, ty)) = self.variables.get(name) {
                    let alloca = alloca.clone();
                    let ty = ty.clone();
                    writeln!(self.output, "  store {} {}, {}* {}", ty, val, ty, alloca).unwrap();
                }
                val
            }
            Stmt::If {
                condition,
                then,
                else_,
            } => {
                let cond_val = self.gen_expr(condition);
                let then_label = self.fresh_label("if.then");
                let else_label = self.fresh_label("if.else");
                let end_label = self.fresh_label("if.end");

                writeln!(
                    self.output,
                    "  br i1 {}, label %{}, label %{}",
                    cond_val, then_label, else_label
                )
                .unwrap();
                self.block_terminated = true;

                writeln!(self.output, "{}:", then_label).unwrap();
                self.block_terminated = false;
                for s in then {
                    self.gen_stmt(s);
                }
                if !self.block_terminated {
                    writeln!(self.output, "  br label %{}", end_label).unwrap();
                    self.block_terminated = true;
                }

                writeln!(self.output, "{}:", else_label).unwrap();
                self.block_terminated = false;
                if let Some(else_body) = else_ {
                    for s in else_body {
                        self.gen_stmt(s);
                    }
                }
                if !self.block_terminated {
                    writeln!(self.output, "  br label %{}", end_label).unwrap();
                    self.block_terminated = true;
                }

                // Always emit the end label — the then-branch may reference it
                // even if the else-branch was fully terminated.
                writeln!(self.output, "{}:", end_label).unwrap();
                self.block_terminated = false;
                "void".to_string()
            }
            Stmt::While { condition, body } => {
                let cond_label = self.fresh_label("while.cond");
                let body_label = self.fresh_label("while.body");
                let end_label = self.fresh_label("while.end");

                writeln!(self.output, "  br label %{}", cond_label).unwrap();
                self.block_terminated = true;
                writeln!(self.output, "{}:", cond_label).unwrap();
                let cond_val = self.gen_expr(condition);
                writeln!(
                    self.output,
                    "  br i1 {}, label %{}, label %{}",
                    cond_val, body_label, end_label
                )
                .unwrap();
                self.block_terminated = true;

                writeln!(self.output, "{}:", body_label).unwrap();
                self.block_terminated = false;
                for s in body {
                    self.gen_stmt(s);
                }
                if !self.block_terminated {
                    writeln!(self.output, "  br label %{}", cond_label).unwrap();
                    self.block_terminated = true;
                }

                writeln!(self.output, "{}:", end_label).unwrap();
                self.block_terminated = false;
                "void".to_string()
            }
            Stmt::Return(Some(expr)) => {
                let val = self.gen_expr(expr);
                let ty = self.infer_llvm_type(expr);
                writeln!(self.output, "  ret {} {}", ty, val).unwrap();
                self.block_terminated = true;
                val
            }
            Stmt::Return(None) => {
                writeln!(self.output, "  ret void").unwrap();
                self.block_terminated = true;
                "void".to_string()
            }
            Stmt::Print(expr) => {
                let val = self.gen_expr(expr);
                let ty = self.infer_llvm_type(expr);
                self.gen_print_call(&val, &ty);
                "void".to_string()
            }
            Stmt::ExprStmt(expr) => self.gen_expr(expr),
            Stmt::IfLet { pattern, value, then, else_ } => {
                // For `if let x = value { ... }`: bind x, then branch on truthiness.
                // For Some/None patterns, compare tag (1 = Some, 0 = None).
                let val = self.gen_expr(value);
                let val_ty = self.infer_llvm_type(value);

                // Handle binding patterns: bind the variable to the value
                match pattern {
                    Pattern::Variable(name) => {
                        let alloca = self.fresh_var();
                        let ty = val_ty.clone();
                        writeln!(self.output, "  {} = alloca {}", alloca, ty).unwrap();
                        writeln!(self.output, "  store {} {}, {}* {}", ty, val, ty, alloca).unwrap();
                        self.variables.insert(name.clone(), (alloca, ty));
                    }
                    Pattern::SomePattern { binding: Some(b) } => {
                        let alloca = self.fresh_var();
                        writeln!(self.output, "  {} = alloca i64", alloca).unwrap();
                        writeln!(self.output, "  store i64 {}, i64* {}", val, alloca).unwrap();
                        self.variables.insert(b.clone(), (alloca, "i64".to_string()));
                    }
                    _ => {}
                }

                // Compute truthiness
                let cond = if val_ty == "i1" { val.clone() } else {
                    let c = self.fresh_var();
                    writeln!(self.output, "  {} = icmp ne {} {}, 0", c, val_ty, val).unwrap();
                    c
                };

                // For SomePattern, cond is val != 0 (Some)
                // For NonePattern, cond is val == 0 (None)
                let cond = match pattern {
                    Pattern::NonePattern => {
                        let c = self.fresh_var();
                        writeln!(self.output, "  {} = icmp eq {} {}, 0", c, val_ty, val).unwrap();
                        c
                    }
                    _ => cond,
                };

                let then_label = self.fresh_label("iflet.then");
                let else_label = self.fresh_label("iflet.else");
                let end_label = self.fresh_label("iflet.end");
                writeln!(self.output, "  br i1 {}, label %{}, label %{}", cond, then_label, else_label).unwrap();
                self.block_terminated = true;

                writeln!(self.output, "{}:", then_label).unwrap();
                self.block_terminated = false;
                for s in then {
                    self.gen_stmt(s);
                }
                if !self.block_terminated {
                    writeln!(self.output, "  br label %{}", end_label).unwrap();
                    self.block_terminated = true;
                }

                writeln!(self.output, "{}:", else_label).unwrap();
                self.block_terminated = false;
                if let Some(else_body) = else_ {
                    for s in else_body {
                        self.gen_stmt(s);
                    }
                }
                if !self.block_terminated {
                    writeln!(self.output, "  br label %{}", end_label).unwrap();
                    self.block_terminated = true;
                }

                writeln!(self.output, "{}:", end_label).unwrap();
                self.block_terminated = false;
                "void".to_string()
            }
            Stmt::For {
                variable,
                iterable,
                body,
            } => {
                if let Expr::Range {
                    start,
                    end,
                    inclusive,
                } = iterable
                {
                    let start_val = self.gen_expr(start);
                    let end_val = self.gen_expr(end);
                    let loop_var = variable.clone();
                    let cond_label = self.fresh_label("for.cond");
                    let body_label = self.fresh_label("for.body");
                    let end_label = self.fresh_label("for.end");
                    let incr_label = self.fresh_label("for.incr");

                    // Allocate loop variable
                    let alloca = self.fresh_var();
                    writeln!(self.output, "  {} = alloca i64", alloca).unwrap();
                    writeln!(self.output, "  store i64 {}, i64* {}", start_val, alloca).unwrap();
                    self.variables
                        .insert(loop_var.clone(), (alloca.clone(), "i64".to_string()));

                    // Branch to condition
                    writeln!(self.output, "  br label %{}", cond_label).unwrap();
                    self.block_terminated = true;

                    // Condition block
                    writeln!(self.output, "{}:", cond_label).unwrap();
                    self.block_terminated = false;
                    let loaded = self.fresh_var();
                    writeln!(self.output, "  {} = load i64, i64* {}", loaded, alloca).unwrap();
                    let cmp = self.fresh_var();
                    if *inclusive {
                        writeln!(
                            self.output,
                            "  {} = icmp sle i64 {}, {}",
                            cmp, loaded, end_val
                        )
                        .unwrap();
                    } else {
                        writeln!(
                            self.output,
                            "  {} = icmp slt i64 {}, {}",
                            cmp, loaded, end_val
                        )
                        .unwrap();
                    }
                    writeln!(
                        self.output,
                        "  br i1 {}, label %{}, label %{}",
                        cmp, body_label, end_label
                    )
                    .unwrap();
                    self.block_terminated = true;

                    // Body block
                    writeln!(self.output, "{}:", body_label).unwrap();
                    self.block_terminated = false;
                    for s in body {
                        self.gen_stmt(s);
                    }
                    if !self.block_terminated {
                        writeln!(self.output, "  br label %{}", incr_label).unwrap();
                        self.block_terminated = true;
                    }

                    // Increment block
                    writeln!(self.output, "{}:", incr_label).unwrap();
                    self.block_terminated = false;
                    let loaded2 = self.fresh_var();
                    writeln!(self.output, "  {} = load i64, i64* {}", loaded2, alloca).unwrap();
                    let incr = self.fresh_var();
                    writeln!(self.output, "  {} = add i64 {}, 1", incr, loaded2).unwrap();
                    writeln!(self.output, "  store i64 {}, i64* {}", incr, alloca).unwrap();
                    writeln!(self.output, "  br label %{}", cond_label).unwrap();
                    self.block_terminated = true;

                    // End block
                    writeln!(self.output, "{}:", end_label).unwrap();
                    self.block_terminated = false;
                    "void".to_string()
                } else {
                    for s in body {
                        self.gen_stmt(s);
                    }
                    "void".to_string()
                }
            }
        }
    }

    fn gen_expr(&mut self, expr: &Expr) -> String {
        match expr {
            Expr::Int(n) => format!("{}", n),
            Expr::Float(n) => format!("{}", n),
            Expr::Bool(b) => {
                if *b {
                    "1".to_string()
                } else {
                    "0".to_string()
                }
            }
            Expr::Str(s) => {
                let name = self.fresh_str(s);
                let len = s.len() + 1;
                format!("bitcast ([{} x i8]* @{} to i8*)", len, name)
            }
            Expr::Ident(name) => {
                if let Some((alloca, ty)) = self.variables.get(name) {
                    let alloca = alloca.clone();
                    let ty = ty.clone();
                    if is_struct_type(&ty) {
                        // For structs, return the alloca pointer (don't load the whole struct)
                        alloca
                    } else {
                        let loaded = self.fresh_var();
                        writeln!(
                            self.output,
                            "  {} = load {}, {}* {}",
                            loaded, ty, ty, alloca
                        )
                        .unwrap();
                        loaded
                    }
                } else {
                    format!("%{}", name)
                }
            }
            Expr::BinaryOp { op, left, right } => {
                let l = self.gen_expr(left);
                let r = self.gen_expr(right);
                let lt = self.infer_llvm_type(left);
                match op {
                    BinOp::Add => {
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = add {} {}, {}", result, lt, l, r).unwrap();
                        result
                    }
                    BinOp::Sub => {
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = sub {} {}, {}", result, lt, l, r).unwrap();
                        result
                    }
                    BinOp::Mul => {
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = mul {} {}, {}", result, lt, l, r).unwrap();
                        result
                    }
                    BinOp::Div => {
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = sdiv {} {}, {}", result, lt, l, r).unwrap();
                        result
                    }
                    BinOp::Eq => {
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = icmp eq {} {}, {}", result, lt, l, r)
                            .unwrap();
                        result
                    }
                    BinOp::Neq => {
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = icmp ne {} {}, {}", result, lt, l, r)
                            .unwrap();
                        result
                    }
                    BinOp::Lt => {
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = icmp slt {} {}, {}", result, lt, l, r)
                            .unwrap();
                        result
                    }
                    BinOp::Gt => {
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = icmp sgt {} {}, {}", result, lt, l, r)
                            .unwrap();
                        result
                    }
                    BinOp::Le => {
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = icmp sle {} {}, {}", result, lt, l, r)
                            .unwrap();
                        result
                    }
                    BinOp::Ge => {
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = icmp sge {} {}, {}", result, lt, l, r)
                            .unwrap();
                        result
                    }
                    BinOp::And => {
                        // non-short-circuit and (both operands evaluated)
                        let lb = self.infer_llvm_type(left);
                        let rb = self.infer_llvm_type(right);
                        let lc = if lb == "i1" { l.clone() } else {
                            let c = self.fresh_var();
                            writeln!(self.output, "  {} = icmp ne {} {}, 0", c, lb, l).unwrap();
                            c
                        };
                        let rc = if rb == "i1" { r.clone() } else {
                            let c = self.fresh_var();
                            writeln!(self.output, "  {} = icmp ne {} {}, 0", c, rb, r).unwrap();
                            c
                        };
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = and i1 {}, {}", result, lc, rc).unwrap();
                        result
                    }
                    BinOp::Or => {
                        // simple non-short-circuit or
                        let lb = self.infer_llvm_type(left);
                        let rb = self.infer_llvm_type(right);
                        let lc = if lb == "i1" { l.clone() } else {
                            let c = self.fresh_var();
                            writeln!(self.output, "  {} = icmp ne {} {}, 0", c, lb, l).unwrap();
                            c
                        };
                        let rc = if rb == "i1" { r.clone() } else {
                            let c = self.fresh_var();
                            writeln!(self.output, "  {} = icmp ne {} {}, 0", c, rb, r).unwrap();
                            c
                        };
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = or i1 {}, {}", result, lc, rc).unwrap();
                        result
                    }
                    BinOp::Mod => {
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = srem {} {}, {}", result, lt, l, r).unwrap();
                        result
                    }
                }
            }
            Expr::UnaryOp { op, expr } => {
                let val = self.gen_expr(expr);
                let ty = self.infer_llvm_type(expr);
                match op {
                    UnOp::Neg => {
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = sub {} 0, {}", result, ty, val).unwrap();
                        result
                    }
                    UnOp::Not => {
                        let result = self.fresh_var();
                        writeln!(self.output, "  {} = xor i1 {}, 1", result, val).unwrap();
                        result
                    }
                }
            }
            Expr::Call { name, type_args: _, args } => {
                // Map stdlib builtin names to their C runtime equivalents
                let mapped_name = if crate::stdlib::is_builtin(name) {
                    crate::stdlib::c_name(name).to_string()
                } else {
                    name.replace("::", "_")
                };
                // Check if this is a call to a lambda-typed variable
                let (call_name, captures): (String, Vec<(String, String)>) =
                    if let Some(lam) = self.var_lambdas.get(name) {
                        let caps = self.lambda_captures.get(lam).cloned().unwrap_or_default();
                        (lam.clone(), caps)
                    } else {
                        (mapped_name, Vec::new())
                    };
                let name = call_name.as_str();
                let (param_tys, ret_ty) = self.fn_sigs.get(name).cloned().unwrap_or_else(|| {
                    let arg_tys: Vec<String> =
                        args.iter().map(|a| self.infer_llvm_type(a)).collect();
                    // For lambdas with captures, add capture types to the signature
                    let mut all_tys = arg_tys;
                    for (_, cty) in &captures {
                        all_tys.push(cty.clone());
                    }
                    (all_tys, "i64".to_string())
                });
                // Generate arg values, loading struct types from their allocas
                let mut arg_vals = Vec::new();
                for (i, a) in args.iter().enumerate() {
                    let val = self.gen_expr(a);
                    let ty = if i < param_tys.len() {
                        &param_tys[i]
                    } else {
                        &"i64".to_string()
                    };
                    if is_struct_type(ty) && is_struct_type(&self.infer_llvm_type(a)) {
                        // val is an alloca pointer but we need the struct value — load it
                        let loaded = self.fresh_var();
                        writeln!(self.output, "  {} = load {}, {}* {}", loaded, ty, ty, val)
                            .unwrap();
                        arg_vals.push(loaded);
                    } else {
                        arg_vals.push(val);
                    }
                }
                // Append captured variable values for lambda calls
                for (cname, cty) in &captures {
                    if let Some((cap_alloca, _)) = self.variables.get(cname) {
                        let cap_alloca = cap_alloca.clone();
                        let loaded = self.fresh_var();
                        writeln!(self.output, "  {} = load {}, {}* {}", loaded, cty, cty, cap_alloca).unwrap();
                        arg_vals.push(loaded);
                    }
                }
                if ret_ty == "void" {
                    let args_str: Vec<String> = param_tys
                        .iter()
                        .zip(&arg_vals)
                        .map(|(ty, val)| format!("{} {}", ty, val))
                        .collect();
                    writeln!(
                        self.output,
                        "  call void @{}({})",
                        name,
                        args_str.join(", ")
                    )
                    .unwrap();
                    "void".to_string()
                } else {
                    let result = self.fresh_var();
                    let args_str: Vec<String> = param_tys
                        .iter()
                        .zip(&arg_vals)
                        .map(|(ty, val)| format!("{} {}", ty, val))
                        .collect();
                    writeln!(
                        self.output,
                        "  {} = call {} @{}({})",
                        result,
                        ret_ty,
                        name,
                        args_str.join(", ")
                    )
                    .unwrap();
                    result
                }
            }
            Expr::EnumVariant {
                enum_name, variant, ..
            } => {
                if let Some(tags) = self.enum_tags.get(enum_name) {
                    if let Some((_, tag)) = tags.iter().find(|(v, _)| v == variant) {
                        return format!("{}", tag);
                    }
                }
                "0".to_string()
            }
            Expr::Match { scrutinee, arms } => {
                let sc = self.gen_expr(scrutinee);
                let end_label = self.fresh_label("match.end");
                let result = self.fresh_var();
                writeln!(self.output, "  {} = alloca i64", result).unwrap();
                writeln!(self.output, "  store i64 0, i64* {}", result).unwrap();

                let num_arms = arms.len();
                let skip_labels: Vec<String> =
                    (0..num_arms).map(|_| self.fresh_label("skip")).collect();

                // Terminate the current basic block by branching to the first skip label
                writeln!(self.output, "  br label %{}", skip_labels[0]).unwrap();
                self.block_terminated = true;

                for (i, arm) in arms.iter().enumerate() {
                    writeln!(self.output, "{}:", skip_labels[i]).unwrap();
                    self.block_terminated = false;

                    let arm_label = self.fresh_label("match.arm");
                    let cond = match &arm.pattern {
                        Pattern::EnumVariant {
                            enum_name, variant, ..
                        } => {
                            let tag_val = self.enum_tags.get(enum_name).and_then(|tags| {
                                tags.iter().find(|(v, _)| v == variant).map(|(_, t)| *t)
                            });
                            if let Some(tag) = tag_val {
                                let cmp = self.fresh_var();
                                writeln!(self.output, "  {} = icmp eq i64 {}, {}", cmp, sc, tag)
                                    .unwrap();
                                cmp
                            } else {
                                "true".to_string()
                            }
                        }
                        Pattern::IntLiteral(n) => {
                            let cmp = self.fresh_var();
                            writeln!(self.output, "  {} = icmp eq i64 {}, {}", cmp, sc, n).unwrap();
                            cmp
                        }
                        Pattern::BoolLiteral(b) => {
                            let cmp = self.fresh_var();
                            writeln!(self.output, "  {} = icmp eq i64 {}, {}", cmp, sc, if *b { 1 } else { 0 }).unwrap();
                            cmp
                        }
                        Pattern::SomePattern { .. } => {
                            // Some = tag 1 (non-zero)
                            let cmp = self.fresh_var();
                            writeln!(self.output, "  {} = icmp ne i64 {}, 0", cmp, sc).unwrap();
                            cmp
                        }
                        Pattern::NonePattern => {
                            // None = tag 0
                            let cmp = self.fresh_var();
                            writeln!(self.output, "  {} = icmp eq i64 {}, 0", cmp, sc).unwrap();
                            cmp
                        }
                        Pattern::Variable(_) | Pattern::Wildcard => {
                            "true".to_string()
                        }
                        Pattern::StrLiteral(s) => {
                            // Use module-level string constant via fresh_str
                            let str_name = self.fresh_str(s);
                            let len = s.len() + 1; // include null terminator
                            let str_ptr = self.fresh_var();
                            writeln!(self.output, "  {} = getelementptr [{} x i8], [{} x i8]* @{}, i32 0, i32 0", str_ptr, len, len, str_name).unwrap();
                            let cmp = self.fresh_var();
                            writeln!(self.output, "  {} = call i32 @strcmp(i8* {}, i8* {})", cmp, sc, str_ptr).unwrap();
                            let eq = self.fresh_var();
                            writeln!(self.output, "  {} = icmp eq i32 {}, 0", eq, cmp).unwrap();
                            eq
                        }
                        _ => "true".to_string(),
                    };
                    // Add guard check if present
                    let final_cond = if let Some(ref guard_expr) = arm.guard {
                        let guard_val = self.gen_expr(guard_expr);
                        let guard_bool = self.fresh_var();
                        writeln!(
                            self.output,
                            "  {} = icmp ne i64 {}, 0",
                            guard_bool, guard_val
                        ).unwrap();
                        let and_var = self.fresh_var();
                        writeln!(
                            self.output,
                            "  {} = and i1 {}, {}",
                            and_var, cond, guard_bool
                        ).unwrap();
                        and_var
                    } else {
                        cond
                    };

                    let next_label = if i + 1 < num_arms {
                        &skip_labels[i + 1]
                    } else {
                        &end_label
                    };
                    writeln!(
                        self.output,
                        "  br i1 {}, label %{}, label %{}",
                        final_cond, arm_label, next_label
                    )
                    .unwrap();
                    self.block_terminated = true;
                    writeln!(self.output, "{}:", arm_label).unwrap();
                    self.block_terminated = false;
                    // Bind pattern variables
                    match &arm.pattern {
                        Pattern::EnumVariant { binding: Some(b), .. }
                        | Pattern::SomePattern { binding: Some(b) }
                        | Pattern::Variable(b) => {
                            let alloca = self.fresh_var();
                            writeln!(self.output, "  {} = alloca i64", alloca).unwrap();
                            writeln!(self.output, "  store i64 {}, i64* {}", sc, alloca).unwrap();
                            self.variables.insert(b.clone(), (alloca, "i64".to_string()));
                        }
                        _ => {}
                    }
                    // Run all statements in the arm body
                    let mut arm_val = "0".to_string();
                    for s in &arm.body {
                        match s {
                            Stmt::ExprStmt(e) => {
                                arm_val = self.gen_expr(e);
                            }
                            Stmt::Print(e) => {
                                let val = self.gen_expr(e);
                                let ty = self.infer_llvm_type(e);
                                self.gen_print_call(&val, &ty);
                                arm_val = "0".to_string();
                            }
                            _ => {
                                let v = self.gen_stmt(s);
                                if v != "void" {
                                    arm_val = v;
                                }
                            }
                        }
                    }
                    writeln!(self.output, "  store i64 {}, i64* {}", arm_val, result).unwrap();
                    writeln!(self.output, "  br label %{}", end_label).unwrap();
                    self.block_terminated = true;
                }

                writeln!(self.output, "{}:", end_label).unwrap();
                self.block_terminated = false;
                let loaded = self.fresh_var();
                writeln!(self.output, "  {} = load i64, i64* {}", loaded, result).unwrap();
                loaded
            }
            Expr::StructLiteral { name, type_args, fields } => {
                // Resolve to monomorphized name if generic
                let actual_name = if !type_args.is_empty() && self.generic_structs.contains_key(name) {
                    let type_suffix: Vec<String> = type_args.iter().map(|t| Self::type_id(t)).collect();
                    format!("{}_{}", name, type_suffix.join("_"))
                } else {
                    name.clone()
                };
                let struct_ty = format!("%{}", actual_name);
                let alloca = self.fresh_var();
                writeln!(self.output, "  {} = alloca {}", alloca, struct_ty).unwrap();
                let field_defs = self.struct_defs.get(&actual_name)
                    .or_else(|| self.struct_defs.get(name))
                    .cloned()
                    .unwrap_or_default();
                for (i, (_fname, field_val)) in fields.iter().enumerate() {
                    let val = self.gen_expr(field_val);
                    let field_ty = if i < field_defs.len() {
                        field_defs[i].1.clone()
                    } else {
                        "i64".to_string()
                    };
                    let gep = self.fresh_var();
                    writeln!(
                        self.output,
                        "  {} = getelementptr {}, {}* {}, i32 0, i32 {}",
                        gep, struct_ty, struct_ty, alloca, i
                    )
                    .unwrap();
                    writeln!(
                        self.output,
                        "  store {} {}, {}* {}",
                        field_ty, val, field_ty, gep
                    )
                    .unwrap();
                }
                alloca
            }
            Expr::FieldAccess { target, field } => {
                let target_val = self.gen_expr(target);
                // Get struct type from the variable directly if possible
                let struct_ty = if let Expr::Ident(name) = target.as_ref() {
                    if let Some((_, ty)) = self.variables.get(name) {
                        ty.clone()
                    } else {
                        self.infer_llvm_type(target)
                    }
                } else {
                    self.infer_llvm_type(target)
                };
                let fields_info = self.infer_struct_fields(target);
                if let Some(fields) = fields_info {
                    if let Some((idx, (_n, field_ty))) =
                        fields.iter().enumerate().find(|(_i, (n, _ty))| n == field)
                    {
                        let gep = self.fresh_var();
                        writeln!(
                            self.output,
                            "  {} = getelementptr {}, {}* {}, i32 0, i32 {}",
                            gep, struct_ty, struct_ty, target_val, idx
                        )
                        .unwrap();
                        let loaded = self.fresh_var();
                        writeln!(
                            self.output,
                            "  {} = load {}, {}* {}",
                            loaded, field_ty, field_ty, gep
                        )
                        .unwrap();
                        return loaded;
                    }
                }
                "0".to_string()
            }
            Expr::Lambda { params, ret, body } => {
                let lambda_name = format!("__lambda_{}", self.lambda_counter);
                self.lambda_counter += 1;

                // Detect captured variables: identifiers in body that aren't params
                let param_names: std::collections::HashSet<String> =
                    params.iter().map(|p| p.name.clone()).collect();
                let mut captures: Vec<String> = Vec::new();
                Self::find_captures_in_stmts(body, &param_names, &mut captures);
                captures.sort();
                captures.dedup();

                // Record capture types from the current variable table
                let capture_pairs: Vec<(String, String)> = captures
                    .iter()
                    .filter_map(|name| {
                        self.variables.get(name).map(|(_, ty)| (name.clone(), ty.clone()))
                    })
                    .collect();

                if !capture_pairs.is_empty() {
                    self.lambda_captures.insert(lambda_name.clone(), capture_pairs.clone());
                }

                // Store the lambda; captures are tracked separately in lambda_captures.
                // The lambda function signature will have extra __cap_<name> params appended.
                self.pending_lambdas.push((
                    lambda_name.clone(),
                    params.clone(),
                    ret.clone(),
                    body.clone(),
                ));

                // If there are captures, we need to return a value that encodes both
                // the function name and the captured values. Since LLVM doesn't have
                // closures natively, we use a simple approach: emit a wrapper struct
                // as an i64 pair (fn_index, capture_ptr). For simplicity with the
                // common test cases (immediate call), we just return the lambda name
                // and handle the call specially in Expr::Call.
                lambda_name
            }
            Expr::Range {
                start,
                end,
                inclusive,
            } => {
                let s = self.gen_expr(start);
                let e = self.gen_expr(end);
                let inc = if *inclusive { 1 } else { 0 };
                let result = self.fresh_var();
                writeln!(
                    self.output,
                    "  {} = call {{ i64, i64, i32 }} @sbx_range_create(i64 {}, i64 {}, i32 {})",
                    result, s, e, inc
                )
                .unwrap();
                result
            }
            Expr::FString(parts) => {
                // Build f-string via concatenated string literals in LLVM
                // For simplicity, fall back to a runtime call
                let mut concat_args = String::new();
                let mut count = 0i32;
                for part in parts {
                    match part {
                        crate::ast::FStringPart::Literal(s) => {
                            let name = self.fresh_str(s);
                            let len = s.len() + 1;
                            if count > 0 {
                                concat_args.push_str(", ");
                            }
                            concat_args.push_str(&format!(
                                "i8* bitcast ([{} x i8]* @{} to i8*)",
                                len, name
                            ));
                            count += 1;
                        }
                        crate::ast::FStringPart::Expr(expr) => {
                            let val = self.gen_expr(expr);
                            let ty = self.infer_llvm_type(expr);
                            // For i64 values, we need to convert to string at runtime
                            // For now, call __sbx_to_string
                            if ty == "i64" {
                                let str_val = self.fresh_var();
                                writeln!(
                                    self.output,
                                    "  {} = call i8* @__sbx_to_string(i64 {})",
                                    str_val, val
                                )
                                .unwrap();
                                if count > 0 {
                                    concat_args.push_str(", ");
                                }
                                concat_args.push_str(&format!("i8* {}", str_val));
                            } else {
                                if count > 0 {
                                    concat_args.push_str(", ");
                                }
                                concat_args.push_str(&format!("i8* {}", val));
                            }
                            count += 1;
                        }
                    }
                }
                let result = self.fresh_var();
                writeln!(
                    self.output,
                    "  {} = call i8* @__sbx_str_concat_multi(i32 {}, {})",
                    result, count, concat_args
                )
                .unwrap();
                result
            }
            Expr::MoneyLiteral { amount, currency: _ } => {
                // Money is stored as scaled i64 (×10000), same as the C backend
                let scaled = (*amount * 10000.0) as i64;
                format!("{}", scaled)
            }
            Expr::DecimalLiteral(s) => {
                // Decimal stored as i128 scaled ×10^18, same as C backend.
                // LLVM i128 is valid; emit as a constant.
                let total: i128 = if let Some((int_part, frac_part)) = s.split_once('.') {
                    let int_val: i128 = int_part.parse().unwrap_or(0);
                    let mut frac_str = frac_part.to_string();
                    while frac_str.len() < 18 {
                        frac_str.push('0');
                    }
                    frac_str.truncate(18);
                    let frac_val: i128 = frac_str.parse().unwrap_or(0);
                    int_val * 1_000_000_000_000_000_000 + frac_val
                } else {
                    let val: i128 = s.parse().unwrap_or(0);
                    val * 1_000_000_000_000_000_000
                };
                format!("i128 {}", total)
            }
            Expr::UnitLiteral { value, unit: _ } => self.gen_expr(value),
            Expr::OkExpr(value) => self.gen_expr(value),
            Expr::ErrExpr(error) => {
                // Print error to stderr and exit(1)
                let msg = self.gen_expr(error);
                let stderr_fmt = self.fresh_str("Error: %s\\n");
                let stderr_len = 10;
                let stderr_ptr = self.fresh_var();
                writeln!(
                    self.output,
                    "  {} = load i8*, i8** @stderr",
                    stderr_ptr
                ).unwrap();
                writeln!(
                    self.output,
                    "  call i32 (i8*, ...) @fprintf(i8* {}, i8* bitcast ([{} x i8]* @{} to i8*), i8* {})",
                    stderr_ptr, stderr_len, stderr_fmt, msg
                ).unwrap();
                writeln!(self.output, "  call void @exit(i32 1)").unwrap();
                writeln!(self.output, "  unreachable").unwrap();
                self.block_terminated = true;
                "0".to_string()
            }
            Expr::SomeExpr(value) => {
                // Option is represented as i64 tag: 1 = Some, 0 = None
                // For now, just emit the value (tag is implicit)
                self.gen_expr(value)
            }
            Expr::NoneExpr => "0".to_string(),
            Expr::PanicExpr(msg) => {
                let msg_val = self.gen_expr(msg);
                let panic_fmt = self.fresh_str("Panic: %s\n");
                let panic_len = 10;
                writeln!(
                    self.output,
                    "  call i32 (i8*, ...) @fprintf(i8* bitcast ([{} x i8]* @{} to i8*), i8* {})",
                    panic_len, panic_fmt, msg_val
                ).unwrap();
                writeln!(self.output, "  call void @exit(i32 1)").unwrap();
                writeln!(self.output, "  unreachable").unwrap();
                self.block_terminated = true;
                "0".to_string()
            }
            Expr::TryExpr(expr) => {
                let inner = self.gen_expr(expr);
                let result = self.fresh_var();
                writeln!(self.output, "  {} = call i64 @__sbx_result_unwrap(i64 {})", result, inner).unwrap();
                result
            }
            Expr::AssertExpr { condition, message } => {
                let cond = self.gen_expr(condition);
                let cond_ty = self.infer_llvm_type(condition);
                let cmp = if cond_ty == "i1" { cond.clone() } else {
                    let c = self.fresh_var();
                    writeln!(self.output, "  {} = icmp ne {} {}, 0", c, cond_ty, cond).unwrap();
                    c
                };
                let ok_bb = self.fresh_label("assert.ok");
                let fail_bb = self.fresh_label("assert.fail");
                writeln!(self.output, "  br i1 {}, label %{}, label %{}", cmp, ok_bb, fail_bb).unwrap();
                self.block_terminated = true;
                writeln!(self.output, "{}:", fail_bb).unwrap();
                self.block_terminated = false;
                let msg = match message {
                    Some(m) => self.gen_expr(m),
                    None => {
                        let s = self.fresh_str("assertion failed");
                        let len = 16;
                        format!("bitcast ([{} x i8]* @{} to i8*)", len, s)
                    }
                };
                let assert_fmt = self.fresh_str("Assert failed: %s\n");
                let assert_len = 20;
                writeln!(
                    self.output,
                    "  call i32 (i8*, ...) @fprintf(i8* bitcast ([{} x i8]* @{} to i8*), i8* {})",
                    assert_len, assert_fmt, msg
                ).unwrap();
                writeln!(self.output, "  call void @exit(i32 1)").unwrap();
                writeln!(self.output, "  unreachable").unwrap();
                self.block_terminated = true;
                writeln!(self.output, "{}:", ok_bb).unwrap();
                self.block_terminated = false;
                "0".to_string()
            }
            Expr::AssertEqExpr { left, right, message } => {
                let l = self.gen_expr(left);
                let r = self.gen_expr(right);
                let lt = self.infer_llvm_type(left);
                let cmp = self.fresh_var();
                writeln!(self.output, "  {} = icmp eq {} {}, {}", cmp, lt, l, r).unwrap();
                let ok_bb = self.fresh_label("asserteq.ok");
                let fail_bb = self.fresh_label("asserteq.fail");
                writeln!(self.output, "  br i1 {}, label %{}, label %{}", cmp, ok_bb, fail_bb).unwrap();
                self.block_terminated = true;
                writeln!(self.output, "{}:", fail_bb).unwrap();
                self.block_terminated = false;
                let msg = match message {
                    Some(m) => self.gen_expr(m),
                    None => {
                        let s = self.fresh_str("assert_eq failed");
                        let len = 18;
                        format!("bitcast ([{} x i8]* @{} to i8*)", len, s)
                    }
                };
                let fmt = self.fresh_str("%s: %ld != %ld\n");
                let flen = 16;
                writeln!(
                    self.output,
                    "  call i32 (i8*, ...) @fprintf(i8* bitcast ([{} x i8]* @{} to i8*), i8* {}, i64 {}, i64 {})",
                    flen, fmt, msg, l, r
                ).unwrap();
                writeln!(self.output, "  call void @exit(i32 1)").unwrap();
                writeln!(self.output, "  unreachable").unwrap();
                self.block_terminated = true;
                writeln!(self.output, "{}:", ok_bb).unwrap();
                self.block_terminated = false;
                "0".to_string()
            }
            Expr::Await(expr) => {
                // Sync-async model: the "future" already holds the result.
                // Just return the value directly.
                self.gen_expr(expr)
            }
            Expr::MethodCall { target, method, args } => {
                // String methods or struct methods
                let target_ty = self.infer_llvm_type(target);
                let target_val = self.gen_expr(target);
                // String methods
                let c_fn: String = if target_ty == "i8*" {
                    match method.as_str() {
                        "to_upper" => "__sbx_str_to_upper".to_string(),
                        "to_lower" => "__sbx_str_to_lower".to_string(),
                        "replace" => "__sbx_str_replace".to_string(),
                        "trim" => "__sbx_str_trim".to_string(),
                        "length" => "__sbx_str_len".to_string(),
                        _ => method.clone(),
                    }
                } else {
                    // Struct method: Type_method
                    let type_name = if let Expr::Ident(name) = target.as_ref() {
                        if let Some((_, ty)) = self.variables.get(name) {
                            ty.trim_start_matches('%').to_string()
                        } else {
                            name.clone()
                        }
                    } else if let Expr::StructLiteral { name, .. } = target.as_ref() {
                        name.clone()
                    } else {
                        target_ty.trim_start_matches('%').to_string()
                    };
                    format!("{}_{}", type_name, method)
                };
                let mut all_vals = vec![target_val];
                all_vals.extend(args.iter().map(|a| self.gen_expr(a)));
                let result = self.fresh_var();
                let args_str = all_vals.join(", ");
                writeln!(self.output, "  {} = call i64 @{}({})", result, c_fn, args_str).unwrap();
                result
            }
            Expr::ArrayLiteral(elems) => {
                // Allocate an array on the stack and store elements
                let n = elems.len();
                let arr = self.fresh_var();
                writeln!(self.output, "  {} = alloca i64, i64 {}", arr, n).unwrap();
                for (i, e) in elems.iter().enumerate() {
                    let val = self.gen_expr(e);
                    let gep = self.fresh_var();
                    writeln!(
                        self.output,
                        "  {} = getelementptr i64, i64* {}, i64 {}",
                        gep, arr, i
                    ).unwrap();
                    writeln!(self.output, "  store i64 {}, i64* {}", val, gep).unwrap();
                }
                arr
            }
            Expr::Index { target, index } => {
                let arr = self.gen_expr(target);
                let idx = self.gen_expr(index);
                let gep = self.fresh_var();
                writeln!(
                    self.output,
                    "  {} = getelementptr i64, i64* {}, i64 {}",
                    gep, arr, idx
                ).unwrap();
                let loaded = self.fresh_var();
                writeln!(self.output, "  {} = load i64, i64* {}", loaded, gep).unwrap();
                loaded
            }
            _ => "0".to_string(),
        }
    }

    /// Given an expression that yields a struct, return its field definitions
    fn infer_struct_fields(&self, expr: &Expr) -> Option<Vec<(String, String)>> {
        match expr {
            Expr::Ident(name) => {
                if let Some((_, ty)) = self.variables.get(name) {
                    if let Some(struct_name) = ty.strip_prefix('%') {
                        return self.struct_defs.get(struct_name).cloned();
                    }
                }
                None
            }
            Expr::FieldAccess { target, field, .. } => {
                if let Some(fields) = self.infer_struct_fields(target) {
                    if let Some((_n, field_ty)) = fields.iter().find(|(n, _)| n == field) {
                        if let Some(struct_name) = field_ty.strip_prefix('%') {
                            return self.struct_defs.get(struct_name).cloned();
                        }
                    }
                }
                None
            }
            Expr::StructLiteral { name, type_args, .. } => {
                if !type_args.is_empty() && self.generic_structs.contains_key(name) {
                    let type_suffix: Vec<String> = type_args.iter().map(|t| Self::type_id(t)).collect();
                    let mono_name = format!("{}_{}", name, type_suffix.join("_"));
                    self.struct_defs.get(&mono_name).cloned()
                } else {
                    self.struct_defs.get(name).cloned()
                }
            }
            _ => None,
        }
    }

    fn gen_print_call(&mut self, val: &str, ty: &str) {
        match ty {
            "i64" => {
                let fmt = self.fresh_str("%ld\n");
                let len = 5;
                writeln!(
                    self.output,
                    "  call i64 (i8*, ...) @printf(i8* bitcast ([{} x i8]* @{} to i8*), i64 {})",
                    len, fmt, val
                )
                .unwrap();
            }
            "double" => {
                let fmt = self.fresh_str("%f\n");
                let len = 4;
                writeln!(
                    self.output,
                    "  call i64 (i8*, ...) @printf(i8* bitcast ([{} x i8]* @{} to i8*), double {})",
                    len, fmt, val
                )
                .unwrap();
            }
            "i8*" => {
                let fmt = self.fresh_str("%s\n");
                let len = 4;
                writeln!(
                    self.output,
                    "  call i64 (i8*, ...) @printf(i8* bitcast ([{} x i8]* @{} to i8*), i8* {})",
                    len, fmt, val
                )
                .unwrap();
            }
            "i1" => {
                let fmt = self.fresh_str("%d\n");
                let len = 4;
                writeln!(
                    self.output,
                    "  call i64 (i8*, ...) @printf(i8* bitcast ([{} x i8]* @{} to i8*), i1 {})",
                    len, fmt, val
                )
                .unwrap();
            }
            _ => {
                let fmt = self.fresh_str("%ld\n");
                let len = 5;
                writeln!(
                    self.output,
                    "  call i64 (i8*, ...) @printf(i8* bitcast ([{} x i8]* @{} to i8*), i64 {})",
                    len, fmt, val
                )
                .unwrap();
            }
        }
    }


    /// Safe identifier for LLVM type names (replaces non-alphanumeric chars with _)
    fn type_id(ty: &Type) -> String {
        match ty {
            Type::I64 => "i64".to_string(),
            Type::F64 => "f64".to_string(),
            Type::Bool => "bool".to_string(),
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

    /// Pre-scan AST for generic struct usages and queue monomorphizations
    fn prescan_generic_structs(&mut self, items: &[TopLevel]) {
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

    fn prescan_struct_usage_in_stmts(&mut self, stmts: &[Stmt]) {
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

    fn prescan_struct_usage_in_expr(&mut self, expr: &Expr) {
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
    fn queue_struct_mono(&mut self, name: &str, type_args: &[Type]) {
        if let Some((type_params, _fields)) = self.generic_structs.get(name) {
            if type_params.len() == type_args.len() {
                let type_suffix: Vec<String> = type_args.iter().map(|t| Self::type_id(t)).collect();
                let mono_name = format!("{}_{}", name, type_suffix.join("_"));
                let key = format!("{}__{}", name, type_args.iter().map(|t| format!("{}", t)).collect::<Vec<_>>().join(","));
                if !self.mono_structs.contains(&key) {
                    self.pending_mono_structs.push((mono_name, name.to_string(), type_args.to_vec()));
                }
            }
        }
    }

    /// Generate a monomorphized struct typedef in LLVM IR
    fn monomorphize_struct(&mut self, mono_name: &str, original_name: &str, type_args: &[Type]) {
        if let Some((type_params, fields)) = self.generic_structs.get(original_name).cloned() {
            let key = format!("{}__{}", original_name, type_args.iter().map(|t| format!("{}", t)).collect::<Vec<_>>().join(","));
            if self.mono_structs.contains(&key) { return; }
            self.mono_structs.insert(key);

            let sub: std::collections::HashMap<String, Type> = type_params.iter().map(|tp| tp.name.clone())
                .zip(type_args.iter())
                .map(|(tp, concrete)| (tp, concrete.clone()))
                .collect();

            let field_tys: Vec<String> = fields.iter().map(|f| {
                let concrete_ty = Self::substitute_type(&f.ty, &sub);
                self.llvm_type(&concrete_ty)
            }).collect();

            writeln!(self.output, "%{} = type {{ {} }}", mono_name, field_tys.join(", ")).unwrap();
        }
    }

    /// Substitute type parameters with concrete types
    fn substitute_type(ty: &Type, sub: &std::collections::HashMap<String, Type>) -> Type {
        match ty {
            Type::TypeParam(name) => {
                sub.get(name).cloned().unwrap_or_else(|| ty.clone())
            }
            Type::Custom { name, type_args } => {
                if let Some(replacement) = sub.get(name) {
                    replacement.clone()
                } else {
                    let new_args: Vec<Type> = type_args.iter().map(|a| Self::substitute_type(a, sub)).collect();
                    Type::Custom { name: name.clone(), type_args: new_args }
                }
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

    fn llvm_type(&self, ty: &Type) -> String {
        match ty {
            Type::I64 => "i64".to_string(),
            Type::F64 => "double".to_string(),
            Type::Bool => "i1".to_string(),
            Type::String => "i8*".to_string(),
            Type::Void => "void".to_string(),
            Type::Money(_) | Type::Decimal | Type::Unit(_) => "i64".to_string(),
            Type::Array(_) => "i8*".to_string(),
            Type::Custom { name, type_args } => {
                if self.enum_tags.contains_key(name) {
                    "i64".to_string()
                } else if !type_args.is_empty() && self.generic_structs.contains_key(name) {
                    // Generic struct with type args — use monomorphized name
                    let type_suffix: Vec<String> = type_args.iter().map(|t| Self::type_id(t)).collect();
                    let mono_name = format!("{}_{}", name, type_suffix.join("_"));
                    format!("%{}", mono_name)
                } else if self.struct_defs.contains_key(name) {
                    format!("%{}", name)
                } else {
                    "%".to_string() + name
                }
            }
            Type::Result(_, _) => "i64".to_string(),
            Type::Option(_) => "i64".to_string(),
            Type::Fn(params, ret) => {
                // Function pointer type: i64 (i64)*
                let params_str: Vec<String> = params.iter().map(|p| self.llvm_type(p)).collect();
                format!("{} ({})*", self.llvm_type(ret), params_str.join(", "))
            }
            Type::Future(_) => "i64".to_string(),
            Type::TypeParam(name) => name.clone(), // Should be monomorphized
        }
    }

    fn infer_llvm_type(&self, expr: &Expr) -> String {
        match expr {
            Expr::Int(_) => "i64".to_string(),
            Expr::Float(_) => "double".to_string(),
            Expr::Bool(_) => "i1".to_string(),
            Expr::Str(_) => "i8*".to_string(),
            Expr::EnumVariant { .. } => "i64".to_string(),
            Expr::Match { .. } => "i64".to_string(),
            Expr::Call { name, .. } => {
                if let Some((_, ret)) = self.fn_sigs.get(name) {
                    ret.clone()
                } else {
                    "i64".to_string()
                }
            }
            Expr::Ident(name) => {
                if let Some((_, ty)) = self.variables.get(name) {
                    ty.clone()
                } else {
                    "i64".to_string()
                }
            }
            Expr::BinaryOp {
                op:
                    BinOp::Eq
                    | BinOp::Neq
                    | BinOp::Lt
                    | BinOp::Gt
                    | BinOp::Le
                    | BinOp::Ge
                    | BinOp::And
                    | BinOp::Or,
                ..
            } => "i1".to_string(),
            Expr::BinaryOp { .. } => "i64".to_string(),
            Expr::StructLiteral { name, type_args, .. } => {
                if !type_args.is_empty() && self.generic_structs.contains_key(name) {
                    let type_suffix: Vec<String> = type_args.iter().map(|t| Self::type_id(t)).collect();
                    format!("%{}_{}", name, type_suffix.join("_"))
                } else {
                    format!("%{}", name)
                }
            }
            Expr::FieldAccess { target, field, .. } => {
                if let Some(fields) = self.infer_struct_fields(target) {
                    if let Some((_, field_ty)) = fields.iter().find(|(n, _)| n == field) {
                        return field_ty.clone();
                    }
                }
                "i64".to_string()
            }
            Expr::Range { .. } => "i64".to_string(),
            Expr::FString(_) => "i8*".to_string(),
            _ => "i64".to_string(),
        }
    }

    /// Find free variables (captures) in a lambda body — identifiers not in scope.
    fn find_captures_in_stmts(
        stmts: &[Stmt],
        local_scope: &std::collections::HashSet<String>,
        captures: &mut Vec<String>,
    ) {
        for stmt in stmts {
            match stmt {
                Stmt::Let { name, value, .. } => {
                    Self::find_captures_in_expr(value, local_scope, captures);
                    // name is now in scope, not a capture
                    let mut scope = local_scope.clone();
                    scope.insert(name.clone());
                    // don't recurse for let with nested stmts — Let has no nested body
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
                    if let Some(e) = else_ {
                        Self::find_captures_in_stmts(e, local_scope, captures);
                    }
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
                    if let Some(e) = else_ {
                        Self::find_captures_in_stmts(e, local_scope, captures);
                    }
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
            Expr::UnaryOp { expr, .. } => {
                Self::find_captures_in_expr(expr, local_scope, captures);
            }
            Expr::Call { args, .. } => {
                for a in args {
                    Self::find_captures_in_expr(a, local_scope, captures);
                }
            }
            Expr::MethodCall { target, args, .. } => {
                Self::find_captures_in_expr(target, local_scope, captures);
                for a in args {
                    Self::find_captures_in_expr(a, local_scope, captures);
                }
            }
            Expr::FieldAccess { target, .. } => {
                Self::find_captures_in_expr(target, local_scope, captures);
            }
            Expr::Index { target, index } => {
                Self::find_captures_in_expr(target, local_scope, captures);
                Self::find_captures_in_expr(index, local_scope, captures);
            }
            Expr::ArrayLiteral(elems) => {
                for e in elems {
                    Self::find_captures_in_expr(e, local_scope, captures);
                }
            }
            Expr::StructLiteral { fields, .. } => {
                for (_, v) in fields {
                    Self::find_captures_in_expr(v, local_scope, captures);
                }
            }
            Expr::Lambda { .. } => {
                // Nested lambda — its captures are separate
            }
            Expr::Await(e) | Expr::OkExpr(e) | Expr::ErrExpr(e) | Expr::SomeExpr(e)
            | Expr::PanicExpr(e) | Expr::TryExpr(e) => {
                Self::find_captures_in_expr(e, local_scope, captures);
            }
            Expr::AssertExpr { condition, message, .. } => {
                Self::find_captures_in_expr(condition, local_scope, captures);
                if let Some(m) = message {
                    Self::find_captures_in_expr(m, local_scope, captures);
                }
            }
            Expr::AssertEqExpr { left, right, message, .. } => {
                Self::find_captures_in_expr(left, local_scope, captures);
                Self::find_captures_in_expr(right, local_scope, captures);
                if let Some(m) = message {
                    Self::find_captures_in_expr(m, local_scope, captures);
                }
            }
            Expr::Match { scrutinee, arms } => {
                Self::find_captures_in_expr(scrutinee, local_scope, captures);
                for arm in arms {
                    for s in &arm.body {
                        Self::find_captures_in_stmts(std::slice::from_ref(s), local_scope, captures);
                    }
                }
            }
            Expr::Range { start, end, .. } => {
                Self::find_captures_in_expr(start, local_scope, captures);
                Self::find_captures_in_expr(end, local_scope, captures);
            }
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

    fn fresh_var(&mut self) -> String {
        let v = format!("%v{}", self.var_counter);
        self.var_counter += 1;
        v
    }

    fn fresh_label(&mut self, prefix: &str) -> String {
        let l = format!("{}_{}", prefix, self.var_counter);
        self.var_counter += 1;
        l
    }

    fn fresh_str(&mut self, content: &str) -> String {
        let name = format!(".str.{}", self.strings.len());
        self.strings.push((name.clone(), content.to_string()));
        name
    }
}
