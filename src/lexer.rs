use crate::token::{Spanned, Token};
use anyhow::{anyhow, Result};

pub struct Lexer {
    input: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Spanned>> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token()?;
            let is_eof = tok.token == Token::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        Ok(tokens)
    }

    fn next_token(&mut self) -> Result<Spanned> {
        self.skip_whitespace_and_comments();
        if self.pos >= self.input.len() {
            return Ok(Spanned::new(Token::Eof, self.line, self.col));
        }
        let line = self.line;
        let col = self.col;
        let ch = self.input[self.pos];

        match ch {
            '"' => self.read_string().map(|s| Spanned::new(s, line, col)),
            '0'..='9' => self.read_number().map(|t| Spanned::new(t, line, col)),
            'a'..='z' | 'A'..='Z' | '_' => self
                .read_ident_or_keyword()
                .map(|t| Spanned::new(t, line, col)),
            '+' => {
                self.advance();
                Ok(Spanned::new(Token::Plus, line, col))
            }
            '-' => {
                self.advance();
                if self.peek() == Some('>') {
                    self.advance();
                    Ok(Spanned::new(Token::Arrow, line, col))
                } else {
                    Ok(Spanned::new(Token::Minus, line, col))
                }
            }
            '*' => {
                self.advance();
                Ok(Spanned::new(Token::Star, line, col))
            }
            '/' => {
                self.advance();
                Ok(Spanned::new(Token::Slash, line, col))
            }
            '%' => {
                self.advance();
                Ok(Spanned::new(Token::Percent, line, col))
            }
            '=' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Spanned::new(Token::Eq, line, col))
                } else {
                    Ok(Spanned::new(Token::Assign, line, col))
                }
            }
            '!' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Spanned::new(Token::Neq, line, col))
                } else {
                    Err(anyhow!("Unexpected character '!' at {}:{}", line, col))
                }
            }
            '<' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Spanned::new(Token::Le, line, col))
                } else {
                    Ok(Spanned::new(Token::Lt, line, col))
                }
            }
            '>' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    Ok(Spanned::new(Token::Ge, line, col))
                } else {
                    Ok(Spanned::new(Token::Gt, line, col))
                }
            }
            '?' => {
                self.advance();
                Ok(Spanned::new(Token::QuestionMark, line, col))
            }
            '.' => {
                self.advance();
                Ok(Spanned::new(Token::Dot, line, col))
            }
            ',' => {
                self.advance();
                Ok(Spanned::new(Token::Comma, line, col))
            }
            ':' => {
                self.advance();
                Ok(Spanned::new(Token::Colon, line, col))
            }
            ';' => {
                self.advance();
                Ok(Spanned::new(Token::Semicolon, line, col))
            }
            '(' => {
                self.advance();
                Ok(Spanned::new(Token::LParen, line, col))
            }
            ')' => {
                self.advance();
                Ok(Spanned::new(Token::RParen, line, col))
            }
            '{' => {
                self.advance();
                Ok(Spanned::new(Token::LBrace, line, col))
            }
            '}' => {
                self.advance();
                Ok(Spanned::new(Token::RBrace, line, col))
            }
            '[' => {
                self.advance();
                Ok(Spanned::new(Token::LBracket, line, col))
            }
            ']' => {
                self.advance();
                Ok(Spanned::new(Token::RBracket, line, col))
            }
            _ => Err(anyhow!("Unexpected character '{}' at {}:{}", ch, line, col)),
        }
    }

    fn read_string(&mut self) -> Result<Token> {
        self.advance(); // skip opening "
        let mut s = String::new();
        loop {
            if self.pos >= self.input.len() {
                return Err(anyhow!("Unterminated string at {}:{}", self.line, self.col));
            }
            let ch = self.input[self.pos];
            if ch == '"' {
                self.advance();
                return Ok(Token::Str(s));
            }
            if ch == '\\' {
                self.advance();
                if self.pos >= self.input.len() {
                    return Err(anyhow!("Unterminated escape at {}:{}", self.line, self.col));
                }
                let esc = self.input[self.pos];
                match esc {
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    '\\' => s.push('\\'),
                    '"' => s.push('"'),
                    _ => {
                        s.push('\\');
                        s.push(esc);
                    }
                }
            } else {
                if ch == '\n' {
                    self.line += 1;
                    self.col = 0;
                }
                s.push(ch);
            }
            self.advance();
        }
    }

    fn read_number(&mut self) -> Result<Token> {
        let start = self.pos;
        let mut is_float = false;
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            if ch.is_ascii_digit() {
                self.advance();
            } else if ch == '.' && !is_float {
                if self.pos + 1 < self.input.len() && self.input[self.pos + 1].is_ascii_digit() {
                    is_float = true;
                    self.advance();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        let text: String = self.input[start..self.pos].iter().collect();
        if is_float {
            Ok(Token::Float(text.parse()?))
        } else {
            Ok(Token::Int(text.parse()?))
        }
    }

    fn read_ident_or_keyword(&mut self) -> Result<Token> {
        let start = self.pos;
        while self.pos < self.input.len()
            && (self.input[self.pos].is_alphanumeric() || self.input[self.pos] == '_')
        {
            self.advance();
        }
        let text: String = self.input[start..self.pos].iter().collect();
        Ok(match text.as_str() {
            "let" => Token::Let,
            "mut" => Token::Mut,
            "fn" => Token::Fn,
            "struct" => Token::Struct,
            "if" => Token::If,
            "else" => Token::Else,
            "while" => Token::While,
            "for" => Token::For,
            "in" => Token::In,
            "return" => Token::Return,
            "print" => Token::Print,
            // v0.2: Error handling
            "Result" => Token::Result,
            "Ok" => Token::Ok,
            "Err" => Token::Err,
            "panic" => Token::Panic,
            // v0.2: Modules
            "mod" => Token::Mod,
            "use" => Token::Use,
            // Types
            "i64" => Token::TypeI64,
            "f64" => Token::TypeF64,
            "bool" => Token::TypeBool,
            "string" => Token::TypeString,
            "Money" => Token::TypeMoney,
            "Decimal" => Token::TypeDecimal,
            "true" => Token::Bool(true),
            "false" => Token::Bool(false),
            // Known currencies
            "INR" | "USD" | "EUR" | "GBP" | "JPY" | "CNY" | "BDT" => Token::Currency(text),
            _ => Token::Ident(text),
        })
    }

    fn skip_whitespace_and_comments(&mut self) {
        while self.pos < self.input.len() {
            let ch = self.input[self.pos];
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.advance();
            } else if ch == '\n' {
                self.line += 1;
                self.col = 0;
                self.advance();
            } else if ch == '/'
                && self.pos + 1 < self.input.len()
                && self.input[self.pos + 1] == '/'
            {
                while self.pos < self.input.len() && self.input[self.pos] != '\n' {
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    fn advance(&mut self) {
        if self.pos < self.input.len() {
            self.pos += 1;
            self.col += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        if self.pos < self.input.len() {
            Some(self.input[self.pos])
        } else {
            None
        }
    }
}
