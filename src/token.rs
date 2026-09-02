use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Literals
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),

    // Identifier
    Ident(String),

    // Keywords
    Let,
    Mut,
    Fn,
    Struct,
    If,
    Else,
    While,
    For,
    In,
    Return,
    Print,

    // v0.2: Error handling
    Result,
    Ok,
    Err,
    Some_,
    None_,
    Option,
    Panic,
    QuestionMark,

    // v0.2: Modules
    Mod,
    Use,

    // v1.1: Enums + pattern matching
    Enum,
    Match,

    // Traits
    Trait,

    // v2.1: Impl blocks + tests
    Impl,
    Self_,
    Test,
    Assert,

    // v2.0: Async/await + Future
    Async,
    Await,

    // v0.3: Types
    TypeI64,
    TypeF64,
    TypeBool,
    TypeString,
    TypeMoney,
    TypeDecimal,

    // v1.0: Ledger DSL
    Ledger,
    Debit,
    Credit,

    // v1.0: Database DSL
    Database,
    Table,
    Query,
    Select,
    Insert,
    Update,
    Delete,
    Where,
    From,
    Into,
    Values,
    Set,

    // v3.0: Compile-time constants
    Const,

    // Lambda
    Pipe,

    // Ranges
    DotDot,
    DotDotEq,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
    Bang,
    And,
    Or,
    Assign,
    Arrow,
    FatArrow,
    Dot,
    Comma,
    Colon,
    Semicolon,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,

    // FString
    FString(String),

    // Money
    Currency(String),

    // Special
    Eof,

    // Doc comments
    DocComment(String),
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Int(n) => write!(f, "{}", n),
            Token::Float(n) => write!(f, "{}", n),
            Token::Str(s) => write!(f, "\"{}\"", s),
            Token::Bool(b) => write!(f, "{}", b),
            Token::Ident(s) => write!(f, "{}", s),
            Token::Currency(s) => write!(f, "{}", s),
            Token::FString(s) => write!(f, "f\"{}\"", s),
            _ => write!(f, "{:?}", self),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Spanned {
    pub token: Token,
    pub line: usize,
    pub col: usize,
}

impl Spanned {
    pub fn new(token: Token, line: usize, col: usize) -> Self {
        Self { token, line, col }
    }
}
