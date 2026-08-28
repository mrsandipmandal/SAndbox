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
        match self.current_token() {
            Token::Fn => self.parse_fn_def(),
            Token::Struct => self.parse_struct_def(),
            Token::Enum => self.parse_enum_def(),
            Token::Mod => self.parse_module_def(),
            Token::Use => self.parse_use_statement(),
            Token::Ledger => self.parse_ledger_def(),
            Token::Database => self.parse_database_def(),
            t => Err(self.error(format!(
                "Expected 'fn', 'struct', 'mod', 'ledger', or 'database', got {:?}",
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
                self.expect_token(&Token::Semicolon)?;
                return Ok(TopLevel::Use { path, wildcard });
            }
            path.push(self.expect_ident()?);
        }
        self.expect_token(&Token::Semicolon)?;
        Ok(TopLevel::Use { path, wildcard: false })
    }

    // ── Function / Struct / Module (unchanged) ──

    fn parse_fn_def(&mut self) -> Result<TopLevel> {
        self.expect_token(&Token::Fn)?;
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
        let body = self.parse_block()?;
        Ok(TopLevel::FnDef {
            name,
            params,
            ret,
            body,
        })
    }

    fn parse_struct_def(&mut self) -> Result<TopLevel> {
        self.expect_token(&Token::Struct)?;
        let name = self.expect_ident()?;
        self.expect_token(&Token::LBrace)?;
        let mut fields = Vec::new();
        while !self.peek_token(&Token::RBrace) {
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
        Ok(TopLevel::StructDef { name, fields })
    }

    fn parse_enum_def(&mut self) -> Result<TopLevel> {
        self.expect_token(&Token::Enum)?;
        let name = self.expect_ident()?;
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
        Ok(TopLevel::EnumDef { name, variants })
    }

    fn parse_module_def(&mut self) -> Result<TopLevel> {
        self.expect_token(&Token::Mod)?;
        let name = self.expect_ident()?;
        self.expect_token(&Token::LBrace)?;
        let mut items = Vec::new();
        while !self.peek_token(&Token::RBrace) {
            items.push(self.parse_top_level()?);
        }
        self.expect_token(&Token::RBrace)?;
        Ok(TopLevel::ModuleDef { name, items })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>> {
        let mut params = Vec::new();
        while !self.peek_token(&Token::RParen) {
            let name = self.expect_ident()?;
            self.expect_token(&Token::Colon)?;
            let ty = self.parse_type()?;
            params.push(Param { name, ty });
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
                Ok(Type::Custom(name))
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
        let mut left = self.parse_addition()?;
        if self.peek_token(&Token::DotDot) || self.peek_token(&Token::DotDotEq) {
            let inclusive = self.peek_token(&Token::DotDotEq);
            self.advance();
            let right = self.parse_addition()?;
            left = Expr::Range {
                start: Box::new(left),
                end: Box::new(right),
                inclusive,
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
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.current_token() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::BinaryOp {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut left = self.parse_unary()?;
        loop {
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
            let right = self.parse_unary()?;
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
                    expr = Expr::FieldAccess {
                        target: Box::new(expr),
                        field,
                    };
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
                Token::LParen => {
                    if let Expr::Ident(name) = expr {
                        self.advance();
                        let args = self.parse_args()?;
                        self.expect_token(&Token::RParen)?;
                        expr = Expr::Call { name, args };
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
            Token::Select | Token::Insert | Token::Update | Token::Set
            | Token::Where | Token::From | Token::Into | Token::Values => {
                let s = format!("{:?}", self.current_token());
                self.advance();
                Ok(s.to_lowercase())
            }
            ref t => Err(self.error(format!(
                "Expected function name after '::', got {:?}",
                t
            ))),
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
            Token::Ident(name) => {
                self.advance();
                if self.peek_token(&Token::Colon) && self.peek_token_at(1, &Token::Colon) {
                    self.advance();
                    self.advance();
                    // Could be enum variant or module::function — handle keyword names
                    let variant_name = self.take_module_fn_name()?;
                    if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                        // Enum variant — check for payload
                        if self.peek_token(&Token::LParen) {
                            self.advance();
                            let payload = self.parse_expr()?;
                            self.expect_token(&Token::RParen)?;
                            Ok(Expr::EnumVariant {
                                enum_name: name,
                                variant: variant_name,
                                payload: Some(Box::new(payload)),
                            })
                        } else {
                            Ok(Expr::EnumVariant {
                                enum_name: name,
                                variant: variant_name,
                                payload: None,
                            })
                        }
                    } else {
                        // Module function call
                        let full_name = format!("{}::{}", name, variant_name);
                        Ok(Expr::Ident(full_name))
                    }
                } else if self.peek_token(&Token::LBrace)
                    && name.chars().next().is_some_and(|c| c.is_uppercase())
                {
                    self.advance();
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
                    Ok(Expr::StructLiteral {
                        name,
                        fields,
                    })
                } else {
                    Ok(Expr::Ident(name))
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
            self.expect_token(&Token::FatArrow)?;
            // Arm body: single expression or block
            let body = if self.peek_token(&Token::LBrace) {
                self.parse_block()?
            } else {
                let expr = self.parse_expr()?;
                vec![Stmt::ExprStmt(expr)]
            };
            arms.push(MatchArm { pattern, body });
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
}
