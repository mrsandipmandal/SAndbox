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
    fn_sigs: HashMap<String, (Vec<String>, String)>, // (param C types, return C type)
    lambda_counter: Cell<usize>,
    pending_lambdas: RefCell<Vec<(String, Vec<Param>, Option<Type>, Vec<Stmt>)>>,
    lambda_return_idx: Cell<usize>,
}

impl CodeGen {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
            var_counter: Cell::new(0),
            enums: HashMap::new(),
            fn_returns: HashMap::new(),
            fn_sigs: HashMap::new(),
            var_types: HashMap::new(),
            lambda_counter: Cell::new(0),
            pending_lambdas: RefCell::new(Vec::new()),
            lambda_return_idx: Cell::new(0),
        }
    }

    pub fn generate(&mut self, program: &Program) -> String {
        writeln!(self.output, "#include <stdio.h>").unwrap();
        writeln!(self.output, "#include <stdlib.h>").unwrap();
        writeln!(self.output, "#include <string.h>").unwrap();
        writeln!(self.output, "#include <math.h>").unwrap();
        writeln!(self.output).unwrap();

        self.output.push_str(&stdlib::c_preamble());
        writeln!(self.output).unwrap();

        self.gen_program_items(&program.items);
        // Emit pending lambda forward declarations + definitions
        if !self.pending_lambdas.borrow().is_empty() {
            let lambdas: Vec<_> = self.pending_lambdas.borrow().clone();
            // Forward declarations
            for (name, params, ret, _) in &lambdas {
                self.gen_fn_decl(name, params, ret);
            }
            self.output.push_str("\n// Lambda definitions\n");
            // Definitions
            for (name, params, ret, body) in lambdas {
                self.gen_fn(&name, &params, &ret, &body);
            }
        }
        self.output.clone()
    }

    fn gen_program_items(&mut self, items: &[TopLevel]) {
        // Enums — generate tag constants and register for type mapping
        for item in items {
            if let TopLevel::EnumDef { name, variants } = item {
                self.enums.insert(name.clone(), variants.clone());
                self.gen_enum(name, variants);
            }
        }
        // Structs
        for item in items {
            if let TopLevel::StructDef { name, fields } = item {
                self.gen_struct(name, fields);
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
        // Register function signatures for type inference and function references
        for item in items {
            if let TopLevel::FnDef { name, params, ret, .. } = item {
                let c_ret = ret.as_ref().map_or("void".to_string(), |t| self.c_type(t));
                let c_params: Vec<String> = params.iter().map(|p| self.c_type(&p.ty)).collect();
                self.fn_returns.insert(name.clone(), c_ret.clone());
                self.fn_sigs.insert(name.clone(), (c_params.clone(), c_ret.clone()));
                // Also register with mangled name
                let c_name = Self::c_mangle(name);
                if c_name != *name {
                    self.fn_returns.insert(c_name.clone(), c_ret.clone());
                    self.fn_sigs.insert(c_name, (c_params, c_ret));
                }
            }
        }
        // Pre-scan: discover all lambda expressions and register them
        self.prescan_lambdas(items);
        // Emit lambda forward declarations before function declarations
        if !self.pending_lambdas.borrow().is_empty() {
            let lambdas: Vec<_> = self.pending_lambdas.borrow().clone();
            for (name, params, ret, _) in &lambdas {
                self.gen_fn_decl(name, params, ret);
            }
        }
        self.gen_fn_decls(items);
        writeln!(self.output).unwrap();
        self.gen_fn_bodies(items);
    }

    fn prescan_lambdas(&mut self, items: &[TopLevel]) {
        for item in items {
            if let TopLevel::FnDef { body, .. } = item {
                self.prescan_lambdas_in_stmts(body);
            }
        }
    }

    fn prescan_lambdas_in_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt {
                Stmt::Let { value, .. } => self.prescan_lambdas_in_expr(value),
                Stmt::Assign { value, .. } => self.prescan_lambdas_in_expr(value),
                Stmt::If { condition, then, else_ } => {
                    self.prescan_lambdas_in_expr(condition);
                    self.prescan_lambdas_in_stmts(then);
                    if let Some(e) = else_ { self.prescan_lambdas_in_stmts(e); }
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
                self.pending_lambdas.borrow_mut().push((
                    lambda_name, params.clone(), ret.clone(), body.clone(),
                ));
            }
            Expr::BinaryOp { left, right, .. } => {
                self.prescan_lambdas_in_expr(left);
                self.prescan_lambdas_in_expr(right);
            }
            Expr::UnaryOp { expr, .. } => self.prescan_lambdas_in_expr(expr),
            Expr::Call { args, .. } => {
                for a in args { self.prescan_lambdas_in_expr(a); }
            }
            Expr::Match { scrutinee, arms } => {
                self.prescan_lambdas_in_expr(scrutinee);
                for arm in arms { self.prescan_lambdas_in_stmts(&arm.body); }
            }
            _ => {}
        }
    }

    fn gen_fn_decls(&mut self, items: &[TopLevel]) {
        for item in items {
            match item {
                TopLevel::FnDef {
                    name, params, ret, ..
                } => {
                    let c_name = Self::c_mangle(name);
                    self.gen_fn_decl(&c_name, params, ret);
                }
                TopLevel::ModuleDef { items, .. } => {
                    self.gen_fn_decls(items);
                }
                _ => {}
            }
        }
    }

    fn gen_fn_bodies(&mut self, items: &[TopLevel]) {
        for item in items {
            match item {
                TopLevel::FnDef {
                    name,
                    params,
                    ret,
                    body,
                } => {
                    let c_name = Self::c_mangle(name);
                    self.gen_fn(&c_name, params, ret, body);
                }
                TopLevel::ModuleDef { items, .. } => {
                    self.gen_fn_bodies(items);
                }
                _ => {}
            }
        }
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
                Type::Custom(name) => {
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

        self.var_types.clear();
        writeln!(self.output, "{} {}({}) {{", ret_str, name, params_str).unwrap();
        self.indent += 1;

        for (i, stmt) in body.iter().enumerate() {
            let is_last = i == body.len() - 1;
            if is_last && name != "main" {
                // Implicit return of last expression
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
    }

    fn gen_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
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
                } else if let Expr::Range { start, end, inclusive } = iterable {
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
                if var_type == Some("const char*") {
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
                // Use %g for general numeric output (handles both int and float)
                writeln!(
                    self.output,
                    "printf(\"%g\\n\", (double){});",
                    self.gen_expr(expr)
                )
                .unwrap();
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
            Expr::StructLiteral { name, .. } => name.clone(),
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
                // Check if this is a function reference (check both original and mangled names)
                if let Some((params, ret)) = self.fn_sigs.get(name.as_str())
                    .or_else(|| self.fn_sigs.get(&Self::c_mangle(name)))
                {
                    let params_str = params.join(", ");
                    format!("{} (*)({})", ret, params_str)
                } else {
                    self.var_types.get(name.as_str()).cloned().unwrap_or_else(|| "long".to_string())
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
            },
            Expr::Bool(b) => {
                if *b {
                    "1".into()
                } else {
                    "0".into()
                }
            }
            Expr::Ident(name) => {
                // If this is a function reference, mangle the name
                if self.fn_sigs.contains_key(name.as_str()) || self.fn_returns.contains_key(name.as_str()) {
                    Self::c_mangle(name)
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
            Expr::Call { name, args } => {
                let args_str: Vec<String> = args.iter().map(|a| self.gen_expr(a)).collect();
                if stdlib::is_builtin(name) {
                    let c_fn = stdlib::c_name(name);
                    // v2.0: spawn / serve_once take a *function name* — emit it as a
                    // C function pointer (dropping the string quotes) instead of a literal.
                    let args_joined =                    if matches!(name.as_str(), "spawn" | "http::serve_once" | "http::serve") {
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
                            format!(
                                "{}, (const char* (*)(const char*)){}",
                                first, handler
                            )
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
                    let c_name = name.rfind("::").map_or(name.as_str(), |i| &name[i + 2..]);
                    let c_name = Self::c_mangle(c_name);
                    format!("{}({})", c_name, args_str.join(", "))
                }
            }
            Expr::StructLiteral { name, fields } => {
                let fields_str: Vec<String> = fields
                    .iter()
                    .map(|(k, v)| format!(".{} = {}", k, self.gen_expr(v)))
                    .collect();
                format!("({}){{ {} }}", name, fields_str.join(", "))
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
                variant,
                payload,
            } => {
                let tag = format!("{}_{}", enum_name, variant);
                match payload {
                    Some(expr) => {
                        let val = self.gen_expr(expr);
                        // Store as double — works for both int and float payloads
                        format!("((sbx_enum){{.tag = {}, .payload = {{.d = {}}}}})", tag, val)
                    }
                    None => format!("((sbx_enum){{.tag = {}, .payload = {{.d = 0}}}})", tag),
                }
            }
            Expr::Match { scrutinee, arms } => {
                let sc = self.gen_expr(scrutinee);
                let idx = self.var_counter.get();
                self.var_counter.set(idx + 1);
                let tmp = format!("__m{}", idx);
                let res = format!("__r{}", idx);
                // All enum scrutinees are sbx_enum, always use .tag
                let tag_expr = format!("{}.tag", sc);
                // GNU statement expression: ({ long __r=0; long __m=tag; if(...) __r=val; ... __r; })
                let mut c = format!("({{ double {res} = 0; long {tmp} = {tag_expr}; ");
                let mut first = true;
                for arm in arms {
                    let cond = match &arm.pattern {
                        Pattern::EnumVariant { enum_name, variant, .. } => {
                            format!("{tmp} == {enum_name}_{variant}")
                        }
                        Pattern::IntLiteral(n) => format!("{tmp} == {n}"),
                        Pattern::BoolLiteral(b) => format!("{tmp} == {}", if *b { 1 } else { 0 }),
                        _ => "1".to_string(),
                    };
                    let kw = if first { "if" } else { "else if" };
                    first = false;
                    // Check if pattern has a binding variable
                    let binding_decl = match &arm.pattern {
                        Pattern::EnumVariant { binding: Some(b), .. } => {
                            // Extract payload from the matched enum — cast double to long
                            format!("long {b} = (long){sc}.payload.d; ")
                        }
                        Pattern::Variable(name) => {
                            format!("long {name} = (long){sc}.payload.d; ")
                        }
                        _ => String::new(),
                    };
                    // Extract the last expression value from the arm body
                    let body_val = match arm.body.last() {
                        Some(Stmt::ExprStmt(e)) => self.gen_expr(e),
                        Some(Stmt::Return(Some(e))) => self.gen_expr(e),
                        _ => "0".to_string(),
                    };
                    c.push_str(&format!("{kw} ({cond}) {{ {binding_decl}{res} = {body_val}; }} "));
                }
                c.push_str(&format!("{res}; }})"));
                c
            }
            Expr::Lambda { .. } => {
                // Lambda was already registered by prescan_lambdas
                let idx = self.lambda_return_idx.get();
                self.lambda_return_idx.set(idx + 1);
                format!("__lambda_{}", idx)
            }
            Expr::Range { start, end, inclusive } => {
                let s = self.gen_expr(start);
                let e = self.gen_expr(end);
                if *inclusive {
                    format!("sbx_range_inclusive({}, {})", s, e)
                } else {
                    format!("sbx_range({}, {})", s, e)
                }
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
                            result.push_str(&format!("\"{}\"", s.replace('"', "\\\"").replace('\n', "\\n")));
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
                result = result.replacen("__sbx_str_concat_multi(", &format!("__sbx_str_concat_multi({}, ", count), 1);
                // The result already has the parts with commas between them, close the call
                result.push_str(")");
                result
            }
        }
    }

    fn is_money_binop(&self, left: &Expr, right: &Expr) -> bool {
        matches!(left, Expr::MoneyLiteral { .. }) || matches!(right, Expr::MoneyLiteral { .. })
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
            Type::Custom(name) => {
                if self.enums.contains_key(name) {
                    "sbx_enum".into()
                } else {
                    name.clone()
                }
            },
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
            format!("{} (*{})({})", self.c_type(ret), name, params_str.join(", "))
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
            "double" | "float" | "int" | "long" | "short" | "char" | "void"
            | "if" | "else" | "for" | "while" | "do" | "switch" | "case"
            | "return" | "break" | "continue" | "struct" | "enum" | "union"
            | "typedef" | "sizeof" | "static" | "extern" | "const" | "volatile"
            | "auto" | "register" | "signed" | "unsigned" | "inline"
            | "asm" | "goto" | "default" | "NULL"
            | "stdin" | "stdout" | "stderr" => {
                format!("sbx_{}", name)
            }
            _ => name.to_string(),
        }
    }
}
