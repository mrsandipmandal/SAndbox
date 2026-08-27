use crate::ast::*;
use crate::stdlib;
use std::collections::HashMap;
use std::fmt::Write;

const MONEY_SCALE: i64 = 10000;
const DECIMAL_SCALE: i64 = 1_000_000_000_000_000_000; // 10^18

pub struct CodeGen {
    output: String,
    indent: usize,
    var_types: HashMap<String, String>,
}

impl CodeGen {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
            var_types: HashMap::new(),
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
        self.output.clone()
    }

    fn gen_program_items(&mut self, items: &[TopLevel]) {
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
        self.gen_fn_decls(items);
        writeln!(self.output).unwrap();
        self.gen_fn_bodies(items);
    }

    fn gen_fn_decls(&mut self, items: &[TopLevel]) {
        for item in items {
            match item {
                TopLevel::FnDef {
                    name, params, ret, ..
                } => {
                    self.gen_fn_decl(name, params, ret);
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
                    self.gen_fn(name, params, ret, body);
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
            .map(|p| format!("{} {}", self.c_type(&p.ty), p.name))
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
            .map(|p| format!("{} {}", self.c_type(&p.ty), p.name))
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

    fn gen_fn(&mut self, name: &str, params: &[Param], ret: &Option<Type>, body: &[Stmt]) {
        let ret_str = if name == "main" {
            "int".into()
        } else {
            ret.as_ref().map_or("void".into(), |t| self.c_type(t))
        };
        let params_str = params
            .iter()
            .map(|p| format!("{} {}", self.c_type(&p.ty), p.name))
            .collect::<Vec<_>>()
            .join(", ");

        self.var_types.clear();
        writeln!(self.output, "{} {}({}) {{", ret_str, name, params_str).unwrap();
        self.indent += 1;

        for stmt in body {
            self.gen_stmt(stmt);
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
            _ => {
                writeln!(
                    self.output,
                    "printf(\"%ld\\n\", (long){});",
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
                } else {
                    "long".into()
                }
            }
            Expr::OkExpr(val) => self.infer_c_type(val),
            Expr::ErrExpr(_) => "const char*".into(),
            _ => "long".into(),
        }
    }

    fn gen_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::Int(n) => format!("{}", n),
            Expr::Float(n) => format!("{}", n),
            Expr::Str(s) => format!("\"{}\"", s.replace('"', "\\\"")),
            Expr::Bool(b) => {
                if *b {
                    "1".into()
                } else {
                    "0".into()
                }
            }
            Expr::Ident(name) => name.clone(),
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
                    format!("{}({})", c_fn, args_str.join(", "))
                } else {
                    let c_name = name.rfind("::").map_or(name.as_str(), |i| &name[i + 2..]);
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
            Type::Custom(name) => name.clone(),
            Type::Result(ok, _) => self.c_type(ok),
        }
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            write!(self.output, "    ").unwrap();
        }
    }
}
