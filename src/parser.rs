use crate::ast::*;
use crate::token::{Spanned, Token};
use anyhow::{anyhow, Result};

const UNITS: &[&str] = &[
    "kg",
    "g",
    "mg",
    "ton",
    "meter",
    "m",
    "km",
    "cm",
    "mm",
    "second",
    "s",
    "ms",
    "minute",
    "min",
    "hour",
    "h",
    "watt",
    "kW",
    "MW",
    "joule",
    "J",
    "kJ",
    "newton",
    "N",
    "pascal",
    "Pa",
    "celsius",
    "fahrenheit",
    "kelvin",
    "liter",
    "L",
    "mL",
    "byte",
    "KB",
    "MB",
    "GB",
    "TB",
    "hertz",
    "Hz",
    "kHz",
    "MHz",
    "GHz",
    "percent",
    "bps",
    "kbps",
    "Mbps",
];

fn is_unit_name(name: &str) -> bool {
    UNITS.contains(&name)
}

pub struct Parser {
    tokens: Vec<Spanned>,
    pos: usize,
    eof: Spanned,
}

impl Parser {
    pub fn new(tokens: Vec<Spanned>) -> Self {
        Self {
            eof: Spanned::new(Token::Eof, 0, 0),
            tokens,
            pos: 0,
        }
    }

    pub fn parse(&mut self) -> Result<Program> {
        let mut items = Vec::new();
        while !self.is_at_end() {
            items.push(self.parse_top_level()?);
        }
        Ok(Program { items })
    }

    fn parse_top_level(&mut self) -> Result<TopLevel> {
        // Collect doc comments preceding the item
        let doc = self.collect_doc_comments();
        match self.current_token() {
            Token::Fn => self.parse_fn_def_with_doc(doc),
            Token::Async => self.parse_async_fn_def_with_doc(doc),
            Token::Struct => self.parse_struct_def_with_doc(doc),
            Token::Enum => self.parse_enum_def_with_doc(doc),
            Token::Mod => self.parse_module_def_with_doc(doc),
            Token::Use => self.parse_use_statement(),
            Token::Ledger => self.parse_ledger_def(),
            Token::Database => self.parse_database_def(),
            Token::Impl => self.parse_impl_def_with_doc(doc),
            Token::Trait => self.parse_trait_def_with_doc(doc),
            Token::Test => self.parse_test_def_with_doc(doc),
            Token::Const => self.parse_const_def(),
            t => Err(self.error(format!(
                "Expected 'fn', 'async', 'struct', 'mod', 'impl', 'trait', 'test', 'ledger', or 'database', got {:?}",
                t
            ))),
        }
    }

    fn parse_use_statement(&mut self) -> Result<TopLevel> {
        self.expect_token(&Token::Use)?;
        let mut path = Vec::new();
        path.push(self.expect_ident()?);
        while self.peek_token(&Token::Colon) && self.peek_token_at(1, &Token::Colon) {
            self.advance(); // skip first :
            self.advance(); // skip second :
            if self.peek_token(&Token::Star) {
                self.advance();
                let wildcard = true;
                if self.peek_token(&Token::Semicolon) { self.advance(); }
                return Ok(TopLevel::Use { path, wildcard });
            }
            path.push(self.expect_ident()?);
        }
        if self.peek_token(&Token::Semicolon) { self.advance(); }
        Ok(TopLevel::Use {
            path,
            wildcard: false,
        })
    }

    // ── Function / Struct / Module ──

    /// Parse optional <T> or <T: Ord, U> after a name
    fn parse_type_params(&mut self) -> Result<Vec<TypeParamDef>> {
        if self.peek_token(&Token::Lt) {
            self.advance();
            let mut params = Vec::new();
            let name = self.expect_ident()?;
            let bounds = self.parse_trait_bounds()?;
            params.push(TypeParamDef { name, bounds });
            while self.peek_token(&Token::Comma) {
                self.advance();
                let name = self.expect_ident()?;
                let bounds = self.parse_trait_bounds()?;
                params.push(TypeParamDef { name, bounds });
            }
            self.expect_token(&Token::Gt)?;
            Ok(params)
        } else {
            Ok(Vec::new())
        }
    }

    /// Parse optional : Trait1, Trait2 bounds
    fn parse_trait_bounds(&mut self) -> Result<Vec<String>> {
        if self.peek_token(&Token::Colon) && !self.peek_token_at(1, &Token::Colon) {
            self.advance(); // skip :
            let mut bounds = vec![self.expect_ident()?];
            while self.peek_token(&Token::Comma) {
                self.advance();
                bounds.push(self.expect_ident()?);
            }
            Ok(bounds)
        } else {
            Ok(Vec::new())
        }
    }

    fn parse_fn_def_with_doc(&mut self, doc: Option<String>) -> Result<TopLevel> {
        self.expect_token(&Token::Fn)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect_token(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect_token(&Token::RParen)?;
        let ret = if self.peek_token(&Token::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(TopLevel::FnDef {
            name,
            type_params,
            params,
            ret,
            body,
            doc,
        })
    }

    fn parse_async_fn_def_with_doc(&mut self, doc: Option<String>) -> Result<TopLevel> {
        self.expect_token(&Token::Async)?;
        self.expect_token(&Token::Fn)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect_token(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect_token(&Token::RParen)?;
        let ret = if self.peek_token(&Token::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(TopLevel::AsyncFnDef {
            name,
            type_params,
            params,
            ret,
            body,
            doc,
        })
    }

    fn parse_struct_def_with_doc(&mut self, doc: Option<String>) -> Result<TopLevel> {
        self.expect_token(&Token::Struct)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect_token(&Token::LBrace)?;
        let mut fields = Vec::new();
        while !self.peek_token(&Token::RBrace) {
            self.skip_doc_comments();
            if self.peek_token(&Token::RBrace) { break; }
            let field_name = self.expect_ident()?;
            self.expect_token(&Token::Colon)?;
            let ty = self.parse_type()?;
            fields.push(Field {
                name: field_name,
                ty,
            });
            if !self.peek_token(&Token::RBrace) {
                self.expect_token(&Token::Comma)?;
            }
        }
        self.expect_token(&Token::RBrace)?;
        Ok(TopLevel::StructDef { name, type_params, fields, doc })
    }

    fn parse_enum_def_with_doc(&mut self, doc: Option<String>) -> Result<TopLevel> {
        self.expect_token(&Token::Enum)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect_token(&Token::LBrace)?;
        let mut variants = Vec::new();
        while !self.peek_token(&Token::RBrace) {
            let vname = self.expect_ident()?;
            let payload = if self.peek_token(&Token::LParen) {
                self.advance();
                let ty = self.parse_type()?;
                self.expect_token(&Token::RParen)?;
                Some(ty)
            } else {
                None
            };
            variants.push(EnumVariantDef {
                name: vname,
                payload,
            });
            if !self.peek_token(&Token::RBrace) {
                self.expect_token(&Token::Comma)?;
            }
        }
        self.expect_token(&Token::RBrace)?;
        Ok(TopLevel::EnumDef { name, type_params, variants, doc })
    }

    fn parse_module_def_with_doc(&mut self, doc: Option<String>) -> Result<TopLevel> {
        self.expect_token(&Token::Mod)?;
        let name = self.expect_ident()?;
        // `mod name;` — file-based module (loaded by compiler)
        if self.peek_token(&Token::Semicolon) {
            self.advance();
            return Ok(TopLevel::ModuleDef { name, items: Vec::new(), doc });
        }
        // `mod name { ... }` — inline module
        self.expect_token(&Token::LBrace)?;
        let mut items = Vec::new();
        while !self.peek_token(&Token::RBrace) {
            items.push(self.parse_top_level()?);
        }
        self.expect_token(&Token::RBrace)?;
        Ok(TopLevel::ModuleDef { name, items, doc })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>> {
        let mut params = Vec::new();
        while !self.peek_token(&Token::RParen) {
            let name = self.expect_ident()?;
            self.expect_token(&Token::Colon)?;
            let ty = self.parse_type()?;
            let default = if self.peek_token(&Token::Assign) {
                self.advance();
                Some(self.parse_primary()?)
            } else {
                None
            };
            params.push(Param { name, ty, default });
            if !self.peek_token(&Token::RParen) {
                self.expect_token(&Token::Comma)?;
            }
        }
        Ok(params)
    }

    // ── v1.0: Ledger DSL ──

    fn parse_ledger_def(&mut self) -> Result<TopLevel> {
        self.expect_token(&Token::Ledger)?;
        let name = self.expect_ident()?;
        self.expect_token(&Token::LBrace)?;
        let mut entries = Vec::new();
        while !self.peek_token(&Token::RBrace) {
            let side = match self.current_token() {
                Token::Debit => {
                    self.advance();
                    LedgerSide::Debit
                }
                Token::Credit => {
                    self.advance();
                    LedgerSide::Credit
                }
                t => return Err(self.error(format!("Expected 'debit' or 'credit', got {:?}", t))),
            };
            let account = self.parse_expr()?;
            let amount = self.parse_expr()?;
            entries.push(LedgerEntry {
                side,
                account,
                amount,
            });
            if self.peek_token(&Token::Comma) {
                self.advance();
            }
        }
        self.expect_token(&Token::RBrace)?;
        Ok(TopLevel::LedgerDef(LedgerDef { name, entries }))
    }

    // ── v1.0: Database DSL ──

    fn parse_database_def(&mut self) -> Result<TopLevel> {
        self.expect_token(&Token::Database)?;
        let name = self.expect_ident()?;
        self.expect_token(&Token::LBrace)?;
        let mut tables = Vec::new();
        let mut queries = Vec::new();
        while !self.peek_token(&Token::RBrace) {
            match self.current_token() {
                Token::Table => {
                    tables.push(self.parse_table_def()?);
                }
                Token::Query => {
                    queries.push(self.parse_query_def()?);
                }
                t => return Err(self.error(format!("Expected 'table' or 'query', got {:?}", t))),
            }
        }
        self.expect_token(&Token::RBrace)?;
        Ok(TopLevel::DatabaseDef(DatabaseDef {
            name,
            tables,
            queries,
        }))
    }

    fn parse_table_def(&mut self) -> Result<TableDef> {
        self.expect_token(&Token::Table)?;
        let name = self.expect_ident()?;
        self.expect_token(&Token::LBrace)?;
        let mut columns = Vec::new();
        while !self.peek_token(&Token::RBrace) {
            let col_name = self.expect_ident()?;
            self.expect_token(&Token::Colon)?;
            let ty = self.parse_type()?;
            columns.push(ColumnDef { name: col_name, ty });
            if !self.peek_token(&Token::RBrace) {
                self.expect_token(&Token::Comma)?;
            }
        }
        self.expect_token(&Token::RBrace)?;
        Ok(TableDef { name, columns })
    }

    fn parse_query_def(&mut self) -> Result<QueryDef> {
        self.expect_token(&Token::Query)?;
        let name = self.expect_ident()?;
        self.expect_token(&Token::LParen)?;
        let params = self.parse_params()?;
        self.expect_token(&Token::RParen)?;
        let ret = if self.peek_token(&Token::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect_token(&Token::LBrace)?;

        // Parse SQL-like query body
        let kind = match self.current_token() {
            Token::Select => self.parse_select_query()?,
            Token::Insert => self.parse_insert_query()?,
            Token::Update => self.parse_update_query()?,
            Token::Delete => self.parse_delete_query()?,
            t => return Err(self.error(format!("Expected SQL keyword, got {:?}", t))),
        };

        self.expect_token(&Token::RBrace)?;
        Ok(QueryDef {
            name,
            params,
            ret,
            kind,
        })
    }

    fn parse_select_query(&mut self) -> Result<QueryKind> {
        self.expect_token(&Token::Select)?;
        let mut columns = Vec::new();
        loop {
            match self.current_token() {
                Token::Star => {
                    self.advance();
                    columns.push(SqlExpr::Star);
                    break;
                }
                Token::Ident(_) => {
                    let name = self.expect_ident()?;
                    columns.push(SqlExpr::Column(name));
                }
                _ => break,
            }
            if !self.peek_token(&Token::Comma) {
                break;
            }
            self.advance();
        }
        self.expect_token(&Token::From)?;
        let from_table = self.expect_ident()?;
        let where_clause = if self.peek_token(&Token::Where) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(QueryKind::Select {
            columns,
            from_table,
            where_clause,
        })
    }

    fn parse_insert_query(&mut self) -> Result<QueryKind> {
        self.expect_token(&Token::Insert)?;
        self.expect_token(&Token::Into)?;
        let table = self.expect_ident()?;
        self.expect_token(&Token::LParen)?;
        let mut columns = Vec::new();
        while !self.peek_token(&Token::RParen) {
            columns.push(self.expect_ident()?);
            if !self.peek_token(&Token::RParen) {
                self.expect_token(&Token::Comma)?;
            }
        }
        self.expect_token(&Token::RParen)?;
        self.expect_token(&Token::Values)?;
        self.expect_token(&Token::LParen)?;
        let mut values = Vec::new();
        while !self.peek_token(&Token::RParen) {
            values.push(self.parse_expr()?);
            if !self.peek_token(&Token::RParen) {
                self.expect_token(&Token::Comma)?;
            }
        }
        self.expect_token(&Token::RParen)?;
        Ok(QueryKind::Insert {
            table,
            columns,
            values,
        })
    }

    fn parse_update_query(&mut self) -> Result<QueryKind> {
        self.expect_token(&Token::Update)?;
        let table = self.expect_ident()?;
        self.expect_token(&Token::Set)?;
        let mut set_clauses = Vec::new();
        loop {
            if self.peek_token(&Token::Where) {
                break;
            }
            let col = self.expect_ident()?;
            self.expect_token(&Token::Assign)?;
            let val = self.parse_expr()?;
            set_clauses.push((col, val));
            if !self.peek_token(&Token::Comma) {
                break;
            }
            self.advance();
        }
        let where_clause = if self.peek_token(&Token::Where) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(QueryKind::Update {
            table,
            set_clauses,
            where_clause,
        })
    }

    fn parse_delete_query(&mut self) -> Result<QueryKind> {
        self.expect_token(&Token::Delete)?;
        self.expect_token(&Token::From)?;
        let table = self.expect_ident()?;
        let where_clause = if self.peek_token(&Token::Where) {
            self.advance();
            Some(self.parse_expr()?)
        } else {
            None
        };
        Ok(QueryKind::Delete {
            table,
            where_clause,
        })
    }

    // ── Types ──

    /// Try to parse optional <T, U> type arguments after a name.
    /// Uses lookahead to distinguish from comparison operator.
    /// Returns empty vec if not a type argument list.
    fn try_parse_type_args(&mut self) -> Vec<Type> {
        if !self.peek_token(&Token::Lt) {
            return Vec::new();
        }
        // Lookahead: after <, check if next looks like a type token (not a numeric literal)
        // This distinguishes from comparison: a < b, a < 5, etc.
        let saved_pos = self.pos;
        self.advance(); // skip <
        let is_type_start = matches!(
            self.current_token(),
            Token::Ident(_) | Token::TypeI64 | Token::TypeF64 | Token::TypeBool | Token::TypeString | Token::Some_ | Token::None_
        );
        if is_type_start {
            // After a type token inside <...>, check what follows
            // Parse the first type, then check if followed by , or >
            let mut args = vec![self.parse_type().unwrap_or(Type::Void)];
            while self.peek_token(&Token::Comma) {
                self.advance();
                if let Ok(t) = self.parse_type() {
                    args.push(t);
                } else {
                    break;
                }
            }
            if self.peek_token(&Token::Gt) {
                self.advance(); // consume >
                return args;
            }
        }
        // Not type args — reset position
        self.pos = saved_pos;
        Vec::new()
    }

    fn parse_type(&mut self) -> Result<Type> {
        match self.current_token().clone() {
            Token::TypeI64 => {
                self.advance();
                Ok(Type::I64)
            }
            Token::TypeF64 => {
                self.advance();
                Ok(Type::F64)
            }
            Token::TypeBool => {
                self.advance();
                Ok(Type::Bool)
            }
            Token::TypeString => {
                self.advance();
                Ok(Type::String)
            }
            Token::TypeDecimal => {
                self.advance();
                Ok(Type::Decimal)
            }
            Token::TypeMoney => {
                self.advance();
                self.expect_token(&Token::Lt)?;
                let currency = match self.current_token().clone() {
                    Token::Currency(c) => {
                        self.advance();
                        c
                    }
                    _ => self.expect_ident()?,
                };
                self.expect_token(&Token::Gt)?;
                Ok(Type::Money(currency))
            }
            Token::LBracket => {
                self.advance();
                let inner = self.parse_type()?;
                self.expect_token(&Token::RBracket)?;
                Ok(Type::Array(Box::new(inner)))
            }
            Token::Result => {
                self.advance();
                self.expect_token(&Token::Lt)?;
                let ok_ty = self.parse_type()?;
                self.expect_token(&Token::Comma)?;
                let err_ty = self.parse_type()?;
                self.expect_token(&Token::Gt)?;
                Ok(Type::Result(Box::new(ok_ty), Box::new(err_ty)))
            }
            Token::Option => {
                self.advance();
                self.expect_token(&Token::Lt)?;
                let inner = self.parse_type()?;
                self.expect_token(&Token::Gt)?;
                Ok(Type::Option(Box::new(inner)))
            }
            Token::Ident(ref name) if name == "Future" => {
                self.advance();
                self.expect_token(&Token::Lt)?;
                let inner = self.parse_type()?;
                self.expect_token(&Token::Gt)?;
                Ok(Type::Future(Box::new(inner)))
            }
            ref tok if matches!(tok, Token::Fn) || matches!(tok, Token::Ident(n) if n == "Fn") => {
                self.advance();
                self.expect_token(&Token::LParen)?;
                let mut param_tys = Vec::new();
                if !self.peek_token(&Token::RParen) {
                    param_tys.push(self.parse_type()?);
                    while self.peek_token(&Token::Comma) {
                        self.advance();
                        param_tys.push(self.parse_type()?);
                    }
                }
                self.expect_token(&Token::RParen)?;
                let ret = if self.peek_token(&Token::Arrow) {
                    self.advance();
                    self.parse_type()?
                } else {
                    Type::Void
                };
                Ok(Type::Fn(param_tys, Box::new(ret)))
            }
            Token::Ident(ref name) if is_unit_name(name) => {
                let unit = name.clone();
                self.advance();
                Ok(Type::Unit(unit))
            }
            Token::Ident(name) => {
                self.advance();
                // Check for generic type arguments: Pair<T>
                if self.peek_token(&Token::Lt) {
                    let saved = self.pos;
                    self.advance(); // skip <
                    let mut type_args = Vec::new();
                    if let Ok(ty) = self.parse_type() {
                        type_args.push(ty);
                        while self.peek_token(&Token::Comma) {
                            self.advance();
                            if let Ok(ty) = self.parse_type() {
                                type_args.push(ty);
                            }
                        }
                        if self.peek_token(&Token::Gt) {
                            self.advance(); // consume >
                            // For now, return the generic type as Custom with the first type arg
                            // The typechecker/codegen will handle the full generic name
                            return Ok(Type::Custom { name, type_args });
                        }
                    }
                    // Not type args — reset position
                    self.pos = saved;
                }
                Ok(Type::Custom { name, type_args: vec![] })
            }
            ref t => Err(self.error(format!("Expected type, got {:?}", t))),
        }
    }

    // ── Block / Statements ──

    fn parse_block(&mut self) -> Result<Vec<Stmt>> {
        self.expect_token(&Token::LBrace)?;
        let mut stmts = Vec::new();
        while !self.peek_token(&Token::RBrace) {
            stmts.push(self.parse_stmt()?);
        }
        self.expect_token(&Token::RBrace)?;
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt> {
        match self.current_token().clone() {
            Token::Let => self.parse_let(),
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::For => self.parse_for(),
            Token::Return => self.parse_return(),
            Token::Print => self.parse_print(),
            Token::Break => {
                self.advance();
                Ok(Stmt::Break)
            }
            Token::Continue => {
                self.advance();
                Ok(Stmt::Continue)
            }
            Token::Panic => self.parse_panic(),
            Token::Ident(_) if self.peek_token_at(1, &Token::Assign) => {
                let name = self.expect_ident()?;
                self.expect_token(&Token::Assign)?;
                let value = self.parse_expr()?;
                Ok(Stmt::Assign { name, value })
            }
            _ => {
                let expr = self.parse_expr()?;
                Ok(Stmt::ExprStmt(expr))
            }
        }
    }

    fn parse_let(&mut self) -> Result<Stmt> {
        self.expect_token(&Token::Let)?;
        let mutable = self.peek_token(&Token::Mut);
        if mutable {
            self.advance();
        }
        let name = self.expect_ident()?;
        let ty = if self.peek_token(&Token::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect_token(&Token::Assign)?;
        let value = self.parse_expr()?;
        Ok(Stmt::Let {
            name,
            ty,
            value,
            mutable,
        })
    }

    fn parse_if(&mut self) -> Result<Stmt> {
        self.expect_token(&Token::If)?;
        // Check for `if let pattern = expr`
        if self.peek_token(&Token::Let) {
            self.advance(); // skip 'let'
            let pattern = self.parse_pattern()?;
            self.expect_token(&Token::Assign)?;
            let value = self.parse_expr()?;
            let then = self.parse_block()?;
            let else_ = if self.peek_token(&Token::Else) {
                self.advance();
                if self.peek_token(&Token::If) {
                    Some(vec![self.parse_if()?])
                } else {
                    Some(self.parse_block()?)
                }
            } else {
                None
            };
            return Ok(Stmt::IfLet {
                pattern,
                value: Box::new(value),
                then,
                else_,
            });
        }
        let condition = self.parse_expr()?;
        let then = self.parse_block()?;
        let else_ = if self.peek_token(&Token::Else) {
            self.advance();
            if self.peek_token(&Token::If) {
                Some(vec![self.parse_if()?])
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };
        Ok(Stmt::If {
            condition,
            then,
            else_,
        })
    }

    fn parse_while(&mut self) -> Result<Stmt> {
        self.expect_token(&Token::While)?;
        let condition = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::While { condition, body })
    }

    fn parse_for(&mut self) -> Result<Stmt> {
        self.expect_token(&Token::For)?;
        let variable = self.expect_ident()?;
        self.expect_token(&Token::In)?;
        let iterable = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::For {
            variable,
            iterable,
            body,
        })
    }

    fn parse_return(&mut self) -> Result<Stmt> {
        self.expect_token(&Token::Return)?;
        if self.peek_token(&Token::Semicolon) || self.peek_token(&Token::RBrace) {
            Ok(Stmt::Return(None))
        } else {
            let expr = self.parse_expr()?;
            Ok(Stmt::Return(Some(expr)))
        }
    }

    fn parse_print(&mut self) -> Result<Stmt> {
        self.expect_token(&Token::Print)?;
        self.expect_token(&Token::LParen)?;
        let expr = self.parse_expr()?;
        self.expect_token(&Token::RParen)?;
        Ok(Stmt::Print(expr))
    }

    fn parse_panic(&mut self) -> Result<Stmt> {
        self.expect_token(&Token::Panic)?;
        self.expect_token(&Token::LParen)?;
        let msg = self.parse_expr()?;
        self.expect_token(&Token::RParen)?;
        Ok(Stmt::ExprStmt(Expr::PanicExpr(Box::new(msg))))
    }

    // ── Expressions ──

    fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_range()
    }

    fn parse_range(&mut self) -> Result<Expr> {
        let mut left = self.parse_or()?;
        if self.peek_token(&Token::DotDot) || self.peek_token(&Token::DotDotEq) {
            let inclusive = self.peek_token(&Token::DotDotEq);
            self.advance();
            let right = self.parse_or()?;
            left = Expr::Range {
                start: Box::new(left),
                end: Box::new(right),
                inclusive,
            };
        }
        Ok(left)
    }


    fn parse_or(&mut self) -> Result<Expr> {
        let mut left = self.parse_and()?;
        loop {
            let op = match self.current_token() {
                Token::Or => BinOp::Or,
                _ => break,
            };
            self.advance();
            let right = self.parse_and()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr> {
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.current_token() {
                Token::And => BinOp::And,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_addition(&mut self) -> Result<Expr> {
        let mut left = self.parse_multiplication()?;
        loop {
            let op = match self.current_token() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplication()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_multiplication(&mut self) -> Result<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.current_token() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut left = self.parse_addition()?;
        loop {
            // Check if < is actually generic type args: func<T>(...)
            if matches!(self.current_token(), Token::Lt) {
                if let Expr::Ident(_) = &left {
                    let type_args = self.try_parse_type_args();
                    if !type_args.is_empty() {
                        // It was a generic call, not a comparison
                        if let Expr::Ident(name) = &left {
                            let name = name.clone();
                            if self.peek_token(&Token::LParen) {
                                self.advance();
                                let args = self.parse_args()?;
                                self.expect_token(&Token::RParen)?;
                                left = Expr::Call { name, type_args, args };
                                continue;
                            }
                        }
                    }
                }
            }
            let op = match self.current_token() {
                Token::Eq => BinOp::Eq,
                Token::Neq => BinOp::Neq,
                Token::Lt => BinOp::Lt,
                Token::Gt => BinOp::Gt,
                Token::Le => BinOp::Le,
                Token::Ge => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_addition()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr> {
        if self.peek_token(&Token::Minus) {
            self.advance();
            let expr = self.parse_postfix()?;
            Ok(Expr::UnaryOp {
                op: UnOp::Neg,
                expr: Box::new(expr),
            })
        } else if self.peek_token(&Token::Bang) {
            self.advance();
            let expr = self.parse_postfix()?;
            Ok(Expr::UnaryOp {
                op: UnOp::Not,
                expr: Box::new(expr),
            })
        } else {
            self.parse_postfix()
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.current_token().clone() {
                Token::Dot => {
                    self.advance();
                    let field = self.expect_ident()?;
                    // Check if this is a method call: .method(args)
                    if self.peek_token(&Token::LParen) {
                        self.advance();
                        let args = self.parse_args()?;
                        self.expect_token(&Token::RParen)?;
                        expr = Expr::MethodCall {
                            target: Box::new(expr),
                            method: field,
                            args,
                        };
                    } else {
                        expr = Expr::FieldAccess {
                            target: Box::new(expr),
                            field,
                        };
                    }
                }
                Token::LBracket => {
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect_token(&Token::RBracket)?;
                    expr = Expr::Index {
                        target: Box::new(expr),
                        index: Box::new(index),
                    };
                }
                Token::Lt if matches!(&expr, Expr::Ident(_)) => {
                    // Generic type arguments: func<T>(...)
                    let type_args = self.try_parse_type_args();
                    if !type_args.is_empty() {
                        if let Expr::Ident(ref name) = expr {
                            let name = name.clone();
                            if self.peek_token(&Token::LParen) {
                                self.advance();
                                let args = self.parse_args()?;
                                self.expect_token(&Token::RParen)?;
                                expr = Expr::Call { name, type_args, args };
                            }
                        }
                    } else {
                        break;
                    }
                }
                Token::LParen => {
                    if let Expr::Ident(name) = expr {
                        self.advance();
                        let args = self.parse_args()?;
                        self.expect_token(&Token::RParen)?;
                        expr = Expr::Call { name, type_args: Vec::new(), args };
                    } else {
                        break;
                    }
                }
                Token::QuestionMark => {
                    self.advance();
                    expr = Expr::TryExpr(Box::new(expr));
                }
                _ => break,
            }
        }
        Ok(expr)
    }

    /// Returns an identifier that names a module function; some keyword tokens
    /// (e.g. `delete` in db::delete, `get` in http::get) are lexed as keywords.
    fn take_module_fn_name(&mut self) -> Result<String> {
        match self.current_token().clone() {
            Token::Ident(name) => {
                self.advance();
                Ok(name)
            }
            Token::Delete => {
                self.advance();
                Ok("delete".to_string())
            }
            Token::Select
            | Token::Insert
            | Token::Update
            | Token::Set
            | Token::Where
            | Token::From
            | Token::Into
            | Token::Values => {
                let s = format!("{:?}", self.current_token());
                self.advance();
                Ok(s.to_lowercase())
            }
            ref t => Err(self.error(format!("Expected function name after '::', got {:?}", t))),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        match self.current_token().clone() {
            Token::Int(n) => {
                self.advance();
                if let Token::Ident(ref unit) = self.current_token() {
                    if is_unit_name(unit) {
                        let unit_name = unit.clone();
                        self.advance();
                        return Ok(Expr::UnitLiteral {
                            value: Box::new(Expr::Int(n)),
                            unit: unit_name,
                        });
                    }
                }
                if let Token::Currency(ref c) = self.current_token().clone() {
                    let currency = c.clone();
                    self.advance();
                    Ok(Expr::MoneyLiteral {
                        amount: n as f64,
                        currency,
                    })
                } else {
                    Ok(Expr::Int(n))
                }
            }
            Token::Float(n) => {
                self.advance();
                if let Token::Ident(ref unit) = self.current_token() {
                    if is_unit_name(unit) {
                        let unit_name = unit.clone();
                        self.advance();
                        return Ok(Expr::UnitLiteral {
                            value: Box::new(Expr::Float(n)),
                            unit: unit_name,
                        });
                    }
                }
                if let Token::Currency(ref c) = self.current_token().clone() {
                    let currency = c.clone();
                    self.advance();
                    Ok(Expr::MoneyLiteral {
                        amount: n,
                        currency,
                    })
                } else {
                    Ok(Expr::Float(n))
                }
            }
            Token::Str(s) => {
                self.advance();
                Ok(Expr::Str(s))
            }
            Token::Bool(b) => {
                self.advance();
                Ok(Expr::Bool(b))
            }
            Token::Currency(c) => {
                self.advance();
                Err(self.error(format!("Unexpected currency '{}' without amount", c)))
            }
            Token::Ok => {
                self.advance();
                self.expect_token(&Token::LParen)?;
                let value = self.parse_expr()?;
                self.expect_token(&Token::RParen)?;
                Ok(Expr::OkExpr(Box::new(value)))
            }
            Token::Err => {
                self.advance();
                self.expect_token(&Token::LParen)?;
                let error = self.parse_expr()?;
                self.expect_token(&Token::RParen)?;
                Ok(Expr::ErrExpr(Box::new(error)))
            }
            Token::Some_ => {
                self.advance();
                self.expect_token(&Token::LParen)?;
                let value = self.parse_expr()?;
                self.expect_token(&Token::RParen)?;
                Ok(Expr::SomeExpr(Box::new(value)))
            }
            Token::None_ => {
                self.advance();
                Ok(Expr::NoneExpr)
            }
            Token::Ident(name) => {
                self.advance();
                // Check for type args before :: : EnumName<T>::Variant
                let type_args = self.try_parse_type_args();
                if self.peek_token(&Token::Colon) && self.peek_token_at(1, &Token::Colon) {
                    self.advance();
                    self.advance();
                    // Could be enum variant or module::function — handle keyword names
                    let variant_name = self.take_module_fn_name()?;
                    // Check if the SECOND name starts with uppercase → enum variant
                    // If it starts with lowercase → module/impl function call
                    let is_enum = variant_name.chars().next().is_some_and(|c| c.is_uppercase());
                    if is_enum {
                        // Enum variant — check for payload
                        if self.peek_token(&Token::LParen) {
                            self.advance();
                            let payload = self.parse_expr()?;
                            self.expect_token(&Token::RParen)?;
                            Ok(Expr::EnumVariant {
                                enum_name: name,
                                type_args: type_args.clone(),
                                variant: variant_name,
                                payload: Some(Box::new(payload)),
                            })
                        } else {
                            Ok(Expr::EnumVariant {
                                enum_name: name,
                                type_args,
                                variant: variant_name,
                                payload: None,
                            })
                        }
                    } else {
                        // Module function call — keep :: separator for typechecker
                        let full_name = format!("{}::{}", name, variant_name);
                        Ok(Expr::Ident(full_name))
                    }
                } else if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                    // type_args already parsed above; check for struct literal
                    // Lookahead: only consume { if it looks like `Ident : expr` (struct literal)
                    // not `Ident { ... }` (identifier followed by block/statement)
                    let is_struct_literal = self.peek_token(&Token::LBrace) && {
                        // Peek inside: after {, next should be ident followed by :
                        if self.pos + 2 < self.tokens.len() {
                            matches!(&self.tokens[self.pos + 1].token, Token::Ident(_))
                            && matches!(&self.tokens[self.pos + 2].token, Token::Colon)
                        } else {
                            false
                        }
                    };
                    if is_struct_literal {
                    self.expect_token(&Token::LBrace)?;
                    let mut fields = Vec::new();
                    while !self.peek_token(&Token::RBrace) {
                        let field_name = self.expect_ident()?;
                        self.expect_token(&Token::Colon)?;
                        let value = self.parse_expr()?;
                        fields.push((field_name, value));
                        if !self.peek_token(&Token::RBrace) {
                            self.expect_token(&Token::Comma)?;
                        }
                    }
                    self.expect_token(&Token::RBrace)?;
                    Ok(Expr::StructLiteral { name, type_args, fields })
                    } else {
                        // No LBrace after type args or name — treat as identifier
                        Ok(Expr::Ident(name))
                    }
                } else {
                    // Lowercase name with type args and ( → generic function call
                    if !type_args.is_empty() && self.peek_token(&Token::LParen) {
                        self.advance(); // skip (
                        let args = self.parse_args()?;
                        self.expect_token(&Token::RParen)?;
                        Ok(Expr::Call { name, type_args, args })
                    } else if !type_args.is_empty() {
                        // type args consumed but no ( — shouldn't happen, but handle gracefully
                        Ok(Expr::Ident(name))
                    } else {
                        Ok(Expr::Ident(name))
                    }
                }
            }
            Token::TypeString
            | Token::TypeI64
            | Token::TypeF64
            | Token::TypeBool
            | Token::TypeMoney
            | Token::TypeDecimal
                if self.peek_token_at(1, &Token::Colon) && self.peek_token_at(2, &Token::Colon) =>
            {
                let module_name = match self.current_token() {
                    Token::TypeString => "string".to_string(),
                    Token::TypeI64 => "int".to_string(),
                    Token::TypeF64 => "float".to_string(),
                    Token::TypeBool => "bool".to_string(),
                    Token::TypeMoney => "money".to_string(),
                    Token::TypeDecimal => "decimal".to_string(),
                    _ => unreachable!(),
                };
                self.advance();
                self.advance();
                self.advance();
                let fn_name = self.expect_ident()?;
                let full_name = format!("{}::{}", module_name, fn_name);
                Ok(Expr::Ident(full_name))
            }
            Token::LBracket => {
                self.advance();
                let mut elems = Vec::new();
                while !self.peek_token(&Token::RBracket) {
                    elems.push(self.parse_expr()?);
                    if !self.peek_token(&Token::RBracket) {
                        self.expect_token(&Token::Comma)?;
                    }
                }
                self.expect_token(&Token::RBracket)?;
                Ok(Expr::ArrayLiteral(elems))
            }
            Token::FString(raw) => {
                self.advance();
                let parts = self.parse_fstring_parts(&raw)?;
                Ok(Expr::FString(parts))
            }
            Token::Pipe => self.parse_lambda(),
            Token::Match => self.parse_match_expr(),
            Token::Await => {
                self.advance();
                let expr = self.parse_primary()?;
                Ok(Expr::Await(Box::new(expr)))
            }
            Token::Assert => {
                self.advance();
                self.expect_token(&Token::LParen)?;
                let condition = self.parse_expr()?;
                let message = if self.peek_token(&Token::Comma) {
                    self.advance();
                    Some(Box::new(self.parse_expr()?))
                } else {
                    None
                };
                self.expect_token(&Token::RParen)?;
                Ok(Expr::AssertExpr {
                    condition: Box::new(condition),
                    message,
                })
            }
            Token::Self_ => {
                self.advance();
                // Self::method or Self::Variant
                if self.peek_token(&Token::Colon) && self.peek_token_at(1, &Token::Colon) {
                    self.advance();
                    self.advance();
                    let name = self.expect_ident()?;
                    if self.peek_token(&Token::LParen) {
                        self.advance();
                        let args = self.parse_args()?;
                        self.expect_token(&Token::RParen)?;
                        let full_name = format!("Self::{}", name);
                        Ok(Expr::Call { name: full_name, type_args: Vec::new(), args })
                    } else {
                        Ok(Expr::Ident(format!("Self::{}", name)))
                    }
                } else {
                    Ok(Expr::Ident("Self".to_string()))
                }
            }
            Token::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect_token(&Token::RParen)?;
                Ok(expr)
            }
            ref t => Err(self.error(format!("Unexpected token {:?}", t))),
        }
    }

    fn parse_lambda(&mut self) -> Result<Expr> {
        self.expect_token(&Token::Pipe)?;
        let mut params = Vec::new();
        while !self.peek_token(&Token::Pipe) {
            let name = self.expect_ident()?;
            let ty = if self.peek_token(&Token::Colon) {
                self.advance();
                Some(self.parse_type()?)
            } else {
                None
            };
            params.push(Param {
                name,
                ty: ty.unwrap_or(Type::I64),
                default: None,
            });
            if !self.peek_token(&Token::Pipe) {
                self.expect_token(&Token::Comma)?;
            }
        }
        self.expect_token(&Token::Pipe)?;

        // Optional return type: -> Type
        let ret = if self.peek_token(&Token::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        // Body
        self.expect_token(&Token::LBrace)?;
        let mut body = Vec::new();
        while !self.peek_token(&Token::RBrace) {
            body.push(self.parse_stmt()?);
        }
        self.expect_token(&Token::RBrace)?;

        Ok(Expr::Lambda { params, ret, body })
    }

    fn parse_fstring_parts(&self, raw: &str) -> Result<Vec<crate::ast::FStringPart>> {
        let mut parts = Vec::new();
        let chars: Vec<char> = raw.chars().collect();
        let mut i = 0;
        let mut current_literal = String::new();

        while i < chars.len() {
            if chars[i] == '{' && i + 1 < chars.len() && chars[i + 1] != '{' {
                // Flush literal
                if !current_literal.is_empty() {
                    parts.push(crate::ast::FStringPart::Literal(current_literal.clone()));
                    current_literal.clear();
                }
                // Find matching closing brace
                let start = i + 1;
                let mut depth = 1;
                i += 1;
                while i < chars.len() && depth > 0 {
                    if chars[i] == '{' {
                        depth += 1;
                    } else if chars[i] == '}' {
                        depth -= 1;
                    }
                    if depth > 0 {
                        i += 1;
                    }
                }
                if depth != 0 {
                    return Err(anyhow::anyhow!("Unclosed {{ in f-string expression"));
                }
                let expr_str: String = chars[start..i].iter().collect();
                i += 1; // skip closing }

                // Tokenize and parse the expression
                let mut lexer = crate::lexer::Lexer::new(&expr_str);
                let tokens = lexer.tokenize()?;
                let mut parser = Parser::new(tokens);
                let expr = parser.parse_expr()?;
                parts.push(crate::ast::FStringPart::Expr(Box::new(expr)));
            } else if chars[i] == '{' && i + 1 < chars.len() && chars[i + 1] == '{' {
                current_literal.push('{');
                i += 2;
            } else if chars[i] == '}' && i + 1 < chars.len() && chars[i + 1] == '}' {
                current_literal.push('}');
                i += 2;
            } else {
                current_literal.push(chars[i]);
                i += 1;
            }
        }

        if !current_literal.is_empty() {
            parts.push(crate::ast::FStringPart::Literal(current_literal));
        }

        Ok(parts)
    }

    fn parse_match_expr(&mut self) -> Result<Expr> {
        self.expect_token(&Token::Match)?;
        let scrutinee = self.parse_expr()?;
        self.expect_token(&Token::LBrace)?;
        let mut arms = Vec::new();
        while !self.peek_token(&Token::RBrace) {
            let pattern = self.parse_pattern()?;
            // Optional guard: Pattern if expr =>
            let guard = if self.peek_token(&Token::If) {
                self.advance();
                Some(Box::new(self.parse_expr()?))
            } else {
                None
            };
            self.expect_token(&Token::FatArrow)?;
            // Arm body: single expression or block
            let body = if self.peek_token(&Token::LBrace) {
                self.parse_block()?
            } else {
                let expr = self.parse_expr()?;
                vec![Stmt::ExprStmt(expr)]
            };
            arms.push(MatchArm { pattern, guard, body });
            if !self.peek_token(&Token::RBrace) {
                self.expect_token(&Token::Comma)?;
            }
        }
        self.expect_token(&Token::RBrace)?;
        Ok(Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms,
        })
    }

    fn parse_pattern(&mut self) -> Result<Pattern> {
        match self.current_token().clone() {
            Token::Int(n) => {
                self.advance();
                Ok(Pattern::IntLiteral(n))
            }
            Token::Bool(b) => {
                self.advance();
                Ok(Pattern::BoolLiteral(b))
            }
            Token::Str(s) => {
                self.advance();
                Ok(Pattern::StrLiteral(s))
            }
            Token::Ident(name) => {
                self.advance();
                if self.peek_token(&Token::Colon) && self.peek_token_at(1, &Token::Colon) {
                    // EnumVariant pattern: EnumName::Variant or EnumName::Variant(x)
                    self.advance();
                    self.advance();
                    let variant = self.expect_ident()?;
                    let binding = if self.peek_token(&Token::LParen) {
                        self.advance();
                        let b = self.expect_ident()?;
                        self.expect_token(&Token::RParen)?;
                        Some(b)
                    } else {
                        None
                    };
                    Ok(Pattern::EnumVariant {
                        enum_name: name,
                        variant,
                        binding,
                    })
                } else if name == "_" {
                    Ok(Pattern::Wildcard)
                } else {
                    Ok(Pattern::Variable(name))
                }
            }
            Token::Some_ => {
                self.advance();
                let binding = if self.peek_token(&Token::LParen) {
                    self.advance();
                    let b = self.expect_ident()?;
                    self.expect_token(&Token::RParen)?;
                    Some(b)
                } else {
                    None
                };
                Ok(Pattern::SomePattern { binding })
            }
            Token::None_ => {
                self.advance();
                Ok(Pattern::NonePattern)
            }
            _ => Err(self.error(format!("Expected pattern, got {:?}", self.current_token()))),
        }
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>> {
        let mut args = Vec::new();
        while !self.peek_token(&Token::RParen) {
            args.push(self.parse_expr()?);
            if !self.peek_token(&Token::RParen) {
                self.expect_token(&Token::Comma)?;
            }
        }
        Ok(args)
    }

    // ── Doc comments ──

    /// Collect consecutive doc comments (/// ...) and join them into one string.
    fn collect_doc_comments(&mut self) -> Option<String> {
        let mut lines = Vec::new();
        while let Token::DocComment(text) = self.current_token() {
            lines.push(text.clone());
            self.advance();
        }
        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    }

    /// Skip any doc comment tokens (used inside struct/enum bodies)
    fn skip_doc_comments(&mut self) {
        while let Token::DocComment(_) = self.current_token() {
            self.advance();
        }
    }

    // ── Helpers ──

    fn current_token(&self) -> &Token {
        &self.current().token
    }

    fn current(&self) -> &Spanned {
        self.tokens.get(self.pos).unwrap_or(&self.eof)
    }

    fn advance(&mut self) -> &Spanned {
        if self.pos < self.tokens.len() {
            self.pos += 1;
        }
        self.current()
    }

    fn expect_token(&mut self, expected: &Token) -> Result<()> {
        if self.peek_token(expected) {
            self.advance();
            Ok(())
        } else {
            Err(self.error(format!(
                "Expected {:?}, got {:?}",
                expected,
                self.current_token()
            )))
        }
    }

    fn expect_ident(&mut self) -> Result<String> {
        match self.current_token().clone() {
            Token::Ident(s) => {
                self.advance();
                Ok(s)
            }
            ref t => Err(self.error(format!("Expected identifier, got {:?}", t))),
        }
    }

    fn peek_token(&self, expected: &Token) -> bool {
        std::mem::discriminant(self.current_token()) == std::mem::discriminant(expected)
            && self.current_token() == expected
    }

    fn peek_token_at(&self, offset: usize, expected: &Token) -> bool {
        self.tokens
            .get(self.pos + offset)
            .is_some_and(|t| t.token == *expected)
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.tokens.len() || *self.current_token() == Token::Eof
    }

    fn error(&self, msg: String) -> anyhow::Error {
        let t = self.current();
        anyhow!("{} at {}:{}", msg, t.line, t.col)
    }

    // ── v2.1: Impl blocks ──

    fn parse_trait_def_with_doc(&mut self, doc: Option<String>) -> Result<TopLevel> {
        self.expect_token(&Token::Trait)?;
        let name = self.expect_ident()?;
        self.expect_token(&Token::LBrace)?;
        let mut methods = Vec::new();
        while !self.peek_token(&Token::RBrace) {
            methods.push(self.parse_trait_method()?);
        }
        self.expect_token(&Token::RBrace)?;
        Ok(TopLevel::TraitDef { name, methods, doc })
    }

    fn parse_trait_method(&mut self) -> Result<TopLevel> {
        self.expect_token(&Token::Fn)?;
        let name = self.expect_ident()?;
        let type_params = self.parse_type_params()?;
        self.expect_token(&Token::LParen)?;
        // Handle bare `self` parameter
        let mut params = Vec::new();
        if matches!(self.current_token(), Token::Ident(s) if s == "self") {
            self.advance();
            if self.peek_token(&Token::Colon) {
                self.advance();
                let ty = self.parse_type()?;
                params.push(Param { name: "self".to_string(), ty, default: None });
            } else {
                params.push(Param { name: "self".to_string(), ty: Type::custom("Self"), default: None });
            }
            if !self.peek_token(&Token::RParen) {
                self.expect_token(&Token::Comma)?;
            }
        }
        while !self.peek_token(&Token::RParen) {
            let pname = self.expect_ident()?;
            self.expect_token(&Token::Colon)?;
            let ty = self.parse_type()?;
            params.push(Param { name: pname, ty, default: None });
            if !self.peek_token(&Token::RParen) {
                self.expect_token(&Token::Comma)?;
            }
        }
        self.expect_token(&Token::RParen)?;
        let ret = if self.peek_token(&Token::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        // Trait methods have no body
        Ok(TopLevel::FnDef {
            name,
            type_params,
            params,
            ret,
            body: Vec::new(),
            doc: None,
        })
    }

    fn parse_impl_def_with_doc(&mut self, doc: Option<String>) -> Result<TopLevel> {
        self.expect_token(&Token::Impl)?;
        let first_name = self.expect_ident()?;
        // Check for `impl Trait for Type` syntax
        let (type_name, trait_name) = if self.peek_token(&Token::For) {
            self.advance(); // skip 'for'
            let type_name = self.expect_ident()?;
            (type_name, Some(first_name))
        } else {
            (first_name, None)
        };
        self.expect_token(&Token::LBrace)?;
        let mut methods = Vec::new();
        while !self.peek_token(&Token::RBrace) {
            methods.push(self.parse_impl_method()?);
        }
        self.expect_token(&Token::RBrace)?;
        Ok(TopLevel::ImplDef { type_name, trait_name, methods, doc })
    }

    fn parse_impl_method(&mut self) -> Result<TopLevel> {
        self.expect_token(&Token::Fn)?;
        let name = self.expect_ident()?;
        self.expect_token(&Token::LParen)?;
        let mut params = Vec::new();
        // Check for 'self' parameter
        let has_self = if matches!(self.current_token(), Token::Ident(s) if s == "self") {
            self.advance();
            // Handle self: Type syntax
            if self.peek_token(&Token::Colon) {
                self.advance(); // skip colon
                let ty = self.parse_type()?;
                params.push(Param { name: "self".to_string(), ty, default: None });
            } else {
                // bare 'self' — infer type later
                params.push(Param { name: "self".to_string(), ty: Type::custom("Self"), default: None });
            }
            if !self.peek_token(&Token::RParen) {
                self.expect_token(&Token::Comma)?;
            }
            true
        } else {
            false
        };
        while !self.peek_token(&Token::RParen) {
            let pname = self.expect_ident()?;
            self.expect_token(&Token::Colon)?;
            let ty = self.parse_type()?;
            params.push(Param { name: pname, ty, default: None });
            if !self.peek_token(&Token::RParen) {
                self.expect_token(&Token::Comma)?;
            }
        }
        self.expect_token(&Token::RParen)?;
        let ret = if self.peek_token(&Token::Arrow) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(TopLevel::FnDef {
            name,
            type_params: Vec::new(),
            params,
            ret,
            body,
            doc: None,
        })
    }

    // ── v2.1: Test functions ──

    fn parse_test_def_with_doc(&mut self, _doc: Option<String>) -> Result<TopLevel> {
        self.expect_token(&Token::Test)?;
        // Optional 'fn' keyword for readability: `test fn name { }` or `test name { }`
        if self.peek_token(&Token::Fn) {
            self.advance();
        }
        let name = self.expect_ident()?;
        let body = self.parse_block()?;
        Ok(TopLevel::TestDef { name, body, doc: _doc })
    }

    fn parse_const_def(&mut self) -> Result<TopLevel> {
        self.expect_token(&Token::Const)?;
        let name = self.expect_ident()?;
        let ty = if self.peek_token(&Token::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect_token(&Token::Assign)?;
        let value = self.parse_expr()?;
        Ok(TopLevel::ConstDef { name, ty, value })
    }
}
