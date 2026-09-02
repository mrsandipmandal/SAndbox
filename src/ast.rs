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
    Custom { name: String, type_args: Vec<Type> },
    Result(Box<Type>, Box<Type>),
    Option(Box<Type>),
    Fn(Vec<Type>, Box<Type>),
    Future(Box<Type>),
    TypeParam(String),  // Generic type parameter like T, U
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
            Type::Custom { name, type_args } => {
                if type_args.is_empty() {
                    write!(f, "{}", name)
                } else {
                    let args: Vec<String> = type_args.iter().map(|a| format!("{}", a)).collect();
                    write!(f, "{}<{}>", name, args.join(", "))
                }
            }
            Type::Result(ok, err) => write!(f, "Result<{}, {}>", ok, err),
            Type::Option(inner) => write!(f, "Option<{}>", inner),
            Type::Fn(params, ret) => {
                let params_str: Vec<String> = params.iter().map(|p| format!("{}", p)).collect();
                write!(f, "Fn({}) -> {}", params_str.join(", "), ret)
            }
            Type::Future(inner) => write!(f, "Future<{}>", inner),
            Type::TypeParam(name) => write!(f, "{}", name),
        }
    }
}


impl Type {
    /// Create a Custom type with no type arguments (backwards-compatible constructor)
    pub fn custom(name: &str) -> Self {
        Type::Custom { name: name.to_string(), type_args: Vec::new() }
    }

    /// Get the name of a Custom type (panics if not Custom)
    pub fn custom_name(&self) -> &str {
        match self {
            Type::Custom { name, .. } => name,
            _ => panic!("expected Custom type, got {:?}", self),
        }
    }

    /// Check if this is a Custom type with no type arguments
    pub fn is_custom_simple(&self) -> bool {
        matches!(self, Type::Custom { type_args, .. } if type_args.is_empty())
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
        type_args: Vec<Type>,
        args: Vec<Expr>,
    },
    StructLiteral {
        name: String,
        type_args: Vec<Type>,
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
    SomeExpr(Box<Expr>),
    NoneExpr,
    PanicExpr(Box<Expr>),
    TryExpr(Box<Expr>),
    AssertExpr {
        condition: Box<Expr>,
        message: Option<Box<Expr>>,
    },
    AssertEqExpr {
        left: Box<Expr>,
        right: Box<Expr>,
        message: Option<Box<Expr>>,
    },
    EnumVariant {
        enum_name: String,
        type_args: Vec<Type>,
        variant: String,
        payload: Option<Box<Expr>>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    MethodCall {
        target: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },
    Lambda {
        params: Vec<Param>,
        ret: Option<Type>,
        body: Vec<Stmt>,
    },
    Await(Box<Expr>),
    Range {
        start: Box<Expr>,
        end: Box<Expr>,
        inclusive: bool,
    },
    FString(Vec<FStringPart>),
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
    IfLet {
        pattern: Pattern,
        value: Box<Expr>,
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
    pub default: Option<Expr>,  // default parameter value
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

// ── Enums + pattern matching ──

#[derive(Debug, Clone)]
pub struct EnumVariantDef {
    pub name: String,
    pub payload: Option<Type>,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    EnumVariant {
        enum_name: String,
        variant: String,
        binding: Option<String>,
    },
    SomePattern {
        binding: Option<String>,
    },
    NonePattern,
    IntLiteral(i64),
    BoolLiteral(bool),
    StrLiteral(String),
    Wildcard,
    Variable(String),
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Vec<Stmt>,
}

// ── FString parts ──

#[derive(Debug, Clone)]
pub enum FStringPart {
    Literal(String),
    Expr(Box<Expr>),
}

// ── Top-level items ──

#[derive(Debug, Clone)]
pub enum TopLevel {
    FnDef {
        name: String,
        type_params: Vec<String>,
        params: Vec<Param>,
        ret: Option<Type>,
        body: Vec<Stmt>,
        doc: Option<String>,
    },
    AsyncFnDef {
        name: String,
        type_params: Vec<String>,
        params: Vec<Param>,
        ret: Option<Type>,
        body: Vec<Stmt>,
        doc: Option<String>,
    },
    StructDef {
        name: String,
        type_params: Vec<String>,
        fields: Vec<Field>,
        doc: Option<String>,
    },
    ModuleDef {
        name: String,
        items: Vec<TopLevel>,
        doc: Option<String>,
    },
    // Traits
    TraitDef {
        name: String,
        methods: Vec<TopLevel>,  // FnDefs without bodies (signatures only)
        doc: Option<String>,
    },
    // v1.0: Ledger
    LedgerDef(LedgerDef),
    // v1.0: Database
    DatabaseDef(DatabaseDef),
    // v2.0: Use imports
    Use {
        path: Vec<String>,
        wildcard: bool,
    },
    // v1.1: Enums
    EnumDef {
        name: String,
        type_params: Vec<String>,
        variants: Vec<EnumVariantDef>,
        doc: Option<String>,
    },
    // v2.1: Impl blocks
    ImplDef {
        type_name: String,
        trait_name: Option<String>,  // if `impl Trait for Type`, this is Some("Trait")
        methods: Vec<TopLevel>,
        doc: Option<String>,
    },
    // v2.1: Test functions
    TestDef {
        name: String,
        body: Vec<Stmt>,
        doc: Option<String>,
    },
}

// ── Program ──

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<TopLevel>,
}
