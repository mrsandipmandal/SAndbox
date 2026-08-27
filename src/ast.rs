#![allow(dead_code, clippy::enum_variant_names)]
use std::fmt;

// ── Types ──

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    I64,
    F64,
    Bool,
    String,
    Money(String),
    Decimal,
    Unit(String),
    Array(Box<Type>),
    Void,
    Custom(String),
    Result(Box<Type>, Box<Type>),
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::I64 => write!(f, "i64"),
            Type::F64 => write!(f, "f64"),
            Type::Bool => write!(f, "bool"),
            Type::String => write!(f, "string"),
            Type::Money(c) => write!(f, "Money<{}>", c),
            Type::Decimal => write!(f, "Decimal"),
            Type::Unit(u) => write!(f, "{}", u),
            Type::Array(t) => write!(f, "[{}]", t),
            Type::Void => write!(f, "void"),
            Type::Custom(n) => write!(f, "{}", n),
            Type::Result(ok, err) => write!(f, "Result<{}, {}>", ok, err),
        }
    }
}

// ── Expressions ──

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
    Ident(String),
    BinaryOp {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnOp,
        expr: Box<Expr>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
    StructLiteral {
        name: String,
        fields: Vec<(String, Expr)>,
    },
    ArrayLiteral(Vec<Expr>),
    Index {
        target: Box<Expr>,
        index: Box<Expr>,
    },
    FieldAccess {
        target: Box<Expr>,
        field: String,
    },
    MoneyLiteral {
        amount: f64,
        currency: String,
    },
    DecimalLiteral(String),
    UnitLiteral {
        value: Box<Expr>,
        unit: String,
    },
    OkExpr(Box<Expr>),
    ErrExpr(Box<Expr>),
    PanicExpr(Box<Expr>),
    TryExpr(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone)]
pub enum UnOp {
    Neg,
    Not,
}

// ── Statements ──

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: String,
        ty: Option<Type>,
        value: Expr,
        mutable: bool,
    },
    Assign {
        name: String,
        value: Expr,
    },
    If {
        condition: Expr,
        then: Vec<Stmt>,
        else_: Option<Vec<Stmt>>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    For {
        variable: String,
        iterable: Expr,
        body: Vec<Stmt>,
    },
    Return(Option<Expr>),
    ExprStmt(Expr),
    Print(Expr),
}

// ── Top-level ──

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

// ── v1.0: Ledger DSL ──

#[derive(Debug, Clone)]
pub struct LedgerEntry {
    pub side: LedgerSide,
    pub account: Expr,
    pub amount: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LedgerSide {
    Debit,
    Credit,
}

#[derive(Debug, Clone)]
pub struct LedgerDef {
    pub name: String,
    pub entries: Vec<LedgerEntry>,
}

// ── v1.0: Database DSL ──

#[derive(Debug, Clone)]
pub struct ColumnDef {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone)]
pub struct TableDef {
    pub name: String,
    pub columns: Vec<ColumnDef>,
}

#[derive(Debug, Clone)]
pub enum SqlExpr {
    Column(String),
    Literal(Expr),
    Star,
}

#[derive(Debug, Clone)]
pub enum QueryKind {
    Select {
        columns: Vec<SqlExpr>,
        from_table: String,
        where_clause: Option<Expr>,
    },
    Insert {
        table: String,
        columns: Vec<String>,
        values: Vec<Expr>,
    },
    Update {
        table: String,
        set_clauses: Vec<(String, Expr)>,
        where_clause: Option<Expr>,
    },
    Delete {
        table: String,
        where_clause: Option<Expr>,
    },
}

#[derive(Debug, Clone)]
pub struct QueryDef {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Option<Type>,
    pub kind: QueryKind,
}

#[derive(Debug, Clone)]
pub struct DatabaseDef {
    pub name: String,
    pub tables: Vec<TableDef>,
    pub queries: Vec<QueryDef>,
}

// ── Top-level items ──

#[derive(Debug, Clone)]
pub enum TopLevel {
    FnDef {
        name: String,
        params: Vec<Param>,
        ret: Option<Type>,
        body: Vec<Stmt>,
    },
    StructDef {
        name: String,
        fields: Vec<Field>,
    },
    ModuleDef {
        name: String,
        items: Vec<TopLevel>,
    },
    // v1.0: Ledger
    LedgerDef(LedgerDef),
    // v1.0: Database
    DatabaseDef(DatabaseDef),
}

// ── Program ──

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<TopLevel>,
}
