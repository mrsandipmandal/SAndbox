use crate::lexer::Lexer;
use crate::token::{Spanned, Token};

/// Format Sandbox source code with consistent style.
pub fn format_source(source: &str) -> String {
    let mut lexer = Lexer::new(source);
    let tokens = match lexer.tokenize() {
        Ok(t) => t,
        Err(_) => return source.to_string(),
    };

    let mut f = Formatter::new(&tokens);
    f.run()
}

/// Convert a token back to its source text.
fn tok_str(tok: &Token) -> String {
    match tok {
        Token::Int(n) => n.to_string(),
        Token::Float(n) => format!("{}", n),
        Token::Str(s) => format!("\"{}\"", s),
        Token::Bool(b) => b.to_string(),
        Token::Ident(s) | Token::Currency(s) | Token::FString(s) => s.clone(),
        Token::Let => "let".into(),
        Token::Mut => "mut".into(),
        Token::Fn => "fn".into(),
        Token::Struct => "struct".into(),
        Token::Enum => "enum".into(),
        Token::If => "if".into(),
        Token::Else => "else".into(),
        Token::While => "while".into(),
        Token::For => "for".into(),
        Token::In => "in".into(),
        Token::Return => "return".into(),
        Token::Print => "print".into(),
        Token::Mod => "mod".into(),
        Token::Use => "use".into(),
        Token::Async => "async".into(),
        Token::Await => "await".into(),
        Token::Impl => "impl".into(),
        Token::Trait => "trait".into(),
        Token::Self_ => "Self".into(),
        Token::Test => "test".into(),
        Token::Assert => "assert".into(),
        Token::Match => "match".into(),
        Token::Const => "const".into(),
        Token::Panic => "panic".into(),
        Token::TypeI64 => "i64".into(),
        Token::TypeF64 => "f64".into(),
        Token::TypeBool => "bool".into(),
        Token::TypeString => "string".into(),            Token::TypeMoney => "Money".into(),
            Token::TypeDecimal => "Decimal".into(),
            Token::Result => "Result".into(),
            Token::Option => "Option".into(),
            Token::Some_ => "Some".into(),
            Token::None_ => "None".into(),
            Token::Ok => "Ok".into(),
            Token::Err => "Err".into(),
        Token::Database => "database".into(),
        Token::Table => "table".into(),
        Token::Query => "query".into(),
        Token::Ledger => "ledger".into(),
        Token::Debit => "debit".into(),
        Token::Credit => "credit".into(),
        Token::Select => "select".into(),
        Token::Insert => "insert".into(),
        Token::Update => "update".into(),
        Token::Delete => "delete".into(),
        Token::Where => "where".into(),
        Token::From => "from".into(),
        Token::Into => "into".into(),
        Token::Values => "values".into(),
        Token::Set => "set".into(),
        Token::Plus => "+".into(),
        Token::Minus => "-".into(),
        Token::Star => "*".into(),
        Token::Slash => "/".into(),
        Token::Percent => "%".into(),
        Token::Eq => "==".into(),
        Token::Neq => "!=".into(),
        Token::Lt => "<".into(),
        Token::Gt => ">".into(),
        Token::Le => "<=".into(),
        Token::Ge => ">=".into(),
        Token::And => "&&".into(),
        Token::Or => "||".into(),
        Token::Assign => "=".into(),
        Token::Arrow => "->".into(),
        Token::FatArrow => "=>".into(),
        Token::Bang => "!".into(),
        Token::Dot => ".".into(),
        Token::Comma => ",".into(),
        Token::Colon => ":".into(),
        Token::Semicolon => ";".into(),
        Token::LParen => "(".into(),
        Token::RParen => ")".into(),
        Token::LBrace => "{".into(),
        Token::RBrace => "}".into(),
        Token::LBracket => "[".into(),
        Token::RBracket => "]".into(),
        Token::DotDot => "..".into(),
        Token::DotDotEq => "..=".into(),
        Token::Pipe => "|".into(),
        Token::QuestionMark => "?".into(),
        Token::Eof | Token::DocComment(_) => String::new(),
    }
}

/// Is this token a keyword that should be followed by a space?
fn is_keyword(tok: &Token) -> bool {
    matches!(
        tok,
        Token::Fn | Token::Let | Token::Mut | Token::If | Token::Else | Token::While
            | Token::For | Token::In | Token::Return | Token::Print | Token::Struct
            | Token::Enum | Token::Mod | Token::Use | Token::Async | Token::Await
            | Token::Impl | Token::Trait | Token::Test | Token::Assert | Token::Match
            | Token::Const | Token::Panic | Token::Database | Token::Table
            | Token::Query | Token::Ledger | Token::Select | Token::Insert
            | Token::Update | Token::Delete | Token::Where | Token::From
            | Token::Into | Token::Values | Token::Set | Token::Debit | Token::Credit
    )
}

/// Is this token an operator that needs spaces around it?
fn is_binary_op(tok: &Token) -> bool {
    matches!(
        tok,
        Token::Plus | Token::Minus | Token::Star | Token::Slash | Token::Percent
            | Token::Eq | Token::Neq | Token::Lt | Token::Gt | Token::Le | Token::Ge
            | Token::And | Token::Or
    )
}

/// Does the previous token need no space before the current token?
fn no_space_before(tok: &Token) -> bool {
    matches!(
        tok,
        Token::Comma | Token::Semicolon | Token::Colon | Token::RParen
            | Token::RBracket | Token::RBrace
    )
}

/// Does the next token need no space after the current token?
fn no_space_after(tok: &Token) -> bool {
    matches!(
        tok,
        Token::Comma | Token::Semicolon | Token::Colon | Token::LParen
            | Token::LBracket | Token::Dot
    )
}

struct Formatter<'a> {
    tokens: &'a [Spanned],
    pos: usize,
    output: String,
    indent: usize,
}

impl<'a> Formatter<'a> {
    fn new(tokens: &'a [Spanned]) -> Self {
        Self { tokens, pos: 0, output: String::new(), indent: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).map(|s| &s.token).unwrap_or(&Token::Eof)
    }

    fn peek_next(&self) -> &Token {
        self.tokens.get(self.pos + 1).map(|s| &s.token).unwrap_or(&Token::Eof)
    }

    fn prev(&self) -> &Token {
        if self.pos == 0 { return &Token::Eof; }
        self.tokens.get(self.pos - 1).map(|s| &s.token).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens.get(self.pos).map(|s| s.token.clone()).unwrap_or(Token::Eof);
        self.pos += 1;
        tok
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }

    /// Emit a token with appropriate spacing.
    fn emit(&mut self, text: &str) {
        let prev = self.prev();
        let needs_space = !self.output.is_empty()
            && !self.output.ends_with('\n')
            && !self.output.ends_with(' ')
            && !self.output.ends_with('{')
            && !no_space_before(&self.peek())
            && !no_space_after(prev)
            && !matches!(prev, Token::LBrace | Token::LBracket | Token::LParen | Token::Dot);

        if needs_space {
            self.output.push(' ');
        }
        self.output.push_str(text);
    }

    fn run(&mut self) -> String {
        // Emit newlines based on line number gaps between tokens.
        let mut last_line = 0;
        let mut just_wrote_newline = true; // start true so first token doesn't get extra newline

        while self.pos < self.tokens.len() {
            let tok = self.peek().clone();

            // Detect newlines: if current token is on a later line than last emitted,
            // insert a newline + indent (unless we just wrote one).
            if self.pos < self.tokens.len() {
                let cur_line = self.tokens[self.pos].line;
                if cur_line > last_line
                    && last_line > 0
                    && !just_wrote_newline
                    && !matches!(&tok, Token::RBrace | Token::Eof)
                {
                    self.output.push('\n');
                    self.write_indent();
                    just_wrote_newline = true;
                }
                last_line = cur_line;
            }

            // Reset just_wrote_newline if we haven't written a newline recently
            if !self.output.ends_with('\n') && !self.output.ends_with("\n\n") {
                just_wrote_newline = false;
            }

            match &tok {
                Token::Eof => break,

                Token::DocComment(text) => {
                    self.write_indent();
                    self.output.push_str(&format!("/// {}\n", text));
                    self.advance();
                }

                // Opening brace — space before, newline after
                Token::LBrace => {
                    let prev = self.prev();
                    // Space before { unless after another { or ( or keyword that already has space
                    if !self.output.ends_with('\n')
                        && !self.output.ends_with('{')
                        && !self.output.ends_with('(')
                        && !self.output.ends_with(' ')
                        && !matches!(prev, Token::FatArrow | Token::Arrow | Token::Else | Token::Semicolon)
                    {
                        self.output.push(' ');
                    }
                    // Remove trailing double spaces
                    while self.output.ends_with("  ") {
                        self.output.pop();
                    }
                    self.output.push_str("{\n");
                    self.indent += 1;
                    just_wrote_newline = true;
                    self.advance();

                    // If next is }, keep on same line
                    if matches!(self.peek(), Token::RBrace) {
                        // Don't indent — will be handled by RBrace
                    } else {
                        self.write_indent();
                    }
                }

                // Closing brace — dedent, then }
                Token::RBrace => {
                    self.indent = self.indent.saturating_sub(1);
                    // Remove trailing spaces and content on current line
                    while self.output.ends_with(' ') {
                        self.output.pop();
                    }
                    // Ensure } starts on a fresh line
                    if !self.output.ends_with('\n') {
                        self.output.push('\n');
                    }
                    self.write_indent();
                    self.output.push('}');
                    // We wrote } but no newline yet — the post-advance block may add one
                    self.advance();

                    // After }, add newline unless followed by else/comma/;/}
                    let next = self.peek().clone();
                    match next {
                        Token::Else | Token::Comma | Token::Semicolon | Token::RBrace | Token::Eof => {}
                        _ => {
                            self.output.push('\n');
                            self.write_indent();
                            just_wrote_newline = true;
                        }
                    }
                }

                // Semicolon — emit ; then newline
                Token::Semicolon => {
                    self.output.push(';');
                    self.advance();
                    let next = self.peek().clone();
                    match next {
                        Token::RBrace | Token::Eof => {}
                        _ => {
                            self.output.push('\n');
                            self.write_indent();
                            just_wrote_newline = true;
                        }
                    }
                }

                // Comma — emit , then space (unless next is } or ) or ])
                Token::Comma => {
                    self.output.push(',');
                    self.advance();
                    match self.peek() {
                        Token::RBrace | Token::RBracket | Token::RParen | Token::Eof => {}
                        _ => self.output.push(' '),
                    }
                }

                // Colon — no space before, space after (unless next is : or Eof)
                Token::Colon => {
                    // Remove trailing space before colon
                    while self.output.ends_with(' ') {
                        self.output.pop();
                    }
                    self.output.push(':');
                    self.advance();
                    // :: — no space between colons
                    if matches!(self.peek(), Token::Colon) {
                        // Don't add space — next colon will handle it
                    } else if matches!(self.peek(), Token::Eof) {
                        // No space after trailing colon
                    } else if matches!(self.prev(), Token::Colon) {
                        // After ::, no space (e.g. Color::Red)
                    } else {
                        self.output.push(' ');
                    }
                }

                // Binary operators — space around
                t if is_binary_op(t) => {
                    // Remove trailing space before operator
                    while self.output.ends_with(' ') {
                        self.output.pop();
                    }
                    self.output.push(' ');
                    self.output.push_str(&tok_str(&tok));
                    self.output.push(' ');
                    self.advance();
                }

                // Assignment = — space around
                Token::Assign => {
                    while self.output.ends_with(' ') {
                        self.output.pop();
                    }
                    self.output.push_str(" = ");
                    self.advance();
                }

                // Arrow -> — space around
                Token::Arrow => {
                    self.output.push_str(" -> ");
                    self.advance();
                }

                // FatArrow => — space around
                Token::FatArrow => {
                    self.output.push_str(" => ");
                    self.advance();
                }

                // Range .. — space around
                Token::DotDot => {
                    let prev = self.prev();
                    if !matches!(prev, Token::LParen | Token::LBracket | Token::Comma) {
                        while self.output.ends_with(' ') { self.output.pop(); }
                        self.output.push(' ');
                    }
                    self.output.push_str("..");
                    self.advance();
                    match self.peek() {
                        Token::RParen | Token::RBracket | Token::Comma | Token::LBrace | Token::Eof => {}
                        _ => self.output.push(' '),
                    }
                }

                // Range ..= — same as above
                Token::DotDotEq => {
                    while self.output.ends_with(' ') { self.output.pop(); }
                    self.output.push(' ');
                    self.output.push_str("..=");
                    self.advance();
                    match self.peek() {
                        Token::RParen | Token::RBracket | Token::Comma | Token::LBrace | Token::Eof => {}
                        _ => self.output.push(' '),
                    }
                }

                // Dot — no space
                Token::Dot => {
                    self.output.push('.');
                    self.advance();
                }

                // Bang — no space before
                Token::Bang => {
                    self.output.push('!');
                    self.advance();
                }

                // Question mark — no space
                Token::QuestionMark => {
                    self.output.push('?');
                    self.advance();
                }

                // Pipe — context dependent (lambda vs operator)
                Token::Pipe => {
                    let next = self.peek_next();
                    if matches!(next, Token::Ident(_) | Token::Pipe) {
                        // Lambda: |x| or ||
                        self.output.push('|');
                    } else {
                        // Operator
                        while self.output.ends_with(' ') { self.output.pop(); }
                        self.output.push_str(" | ");
                    }
                    self.advance();
                }

                // LParen — check spacing
                Token::LParen => {
                    let prev = self.prev();
                    if matches!(prev, Token::RParen | Token::RBracket) {
                        self.output.push('(');
                    } else if is_keyword(prev) || matches!(prev, Token::Ident(_)) {
                        // Space after keyword/fn name before (
                        // But not after: "fn main (" -> "fn main("
                        // Actually: no space before ( in function calls/definitions
                        // Remove trailing space
                        while self.output.ends_with(' ') { self.output.pop(); }
                        self.output.push('(');
                    } else {
                        while self.output.ends_with(' ') { self.output.pop(); }
                        self.output.push('(');
                    }
                    self.advance();
                }

                // RParen — no space before
                Token::RParen => {
                    while self.output.ends_with(' ') { self.output.pop(); }
                    self.output.push(')');
                    self.advance();
                }

                // LBracket — no space
                Token::LBracket => {
                    while self.output.ends_with(' ') { self.output.pop(); }
                    self.output.push('[');
                    self.advance();
                }

                // RBracket — no space
                Token::RBracket => {
                    while self.output.ends_with(' ') { self.output.pop(); }
                    self.output.push(']');
                    self.advance();
                }

                // Keywords — emit with trailing space
                t if is_keyword(t) => {
                    self.emit(&tok_str(&tok));
                    // Ensure space after keyword
                    if !self.output.ends_with(' ') && !matches!(self.peek(), Token::LBrace) {
                        self.output.push(' ');
                    }
                    self.advance();
                }

                // Type tokens — emit like keywords
                Token::TypeI64 | Token::TypeF64 | Token::TypeBool | Token::TypeString
                | Token::TypeMoney | Token::TypeDecimal => {
                    self.emit(&tok_str(&tok));
                    self.advance();
                }

                // Literals and identifiers
                Token::Ident(_) | Token::Int(_) | Token::Float(_)
                | Token::Str(_) | Token::Bool(_) | Token::Currency(_)
                | Token::FString(_) | Token::Some_ | Token::None_
                | Token::Ok | Token::Err | Token::Self_ => {
                    let prev = self.prev();
                    let needs_space = !self.output.is_empty()
                        && !self.output.ends_with('\n')
                        && !self.output.ends_with(' ')
                        && !self.output.ends_with('{')
                        && !matches!(
                            prev,
                            Token::LBrace | Token::LParen | Token::LBracket
                                | Token::Comma | Token::Dot | Token::Colon
                                | Token::Arrow | Token::FatArrow
                        );
                    if needs_space {
                        self.output.push(' ');
                    }
                    self.output.push_str(&tok_str(&tok));
                    self.advance();
                }

                // Doc comments (handled above, but just in case)
                Token::DocComment(text) => {
                    self.write_indent();
                    self.output.push_str(&format!("/// {}\n", text));
                    self.advance();
                }

                // Anything else
                _ => {
                    self.output.push_str(&tok_str(&tok));
                    self.advance();
                }
            }
        }

        // Ensure single trailing newline
        let result = self.output.trim_end_matches('\n').to_string();
        format!("{}\n", result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_simple() {
        let input = "fn main(){print(42)}\n";
        let output = format_source(input);
        assert!(output.contains("fn main() {"), "Got: {}", output);
        assert!(output.contains("print(42)"), "Got: {}", output);
    }

    #[test]
    fn test_format_trailing_whitespace() {
        let input = "let x = 5   \n";
        let output = format_source(input);
        assert!(!output.ends_with("   \n"), "Got: {:?}", output);
    }

    #[test]
    fn test_format_blank_lines() {
        let input = "fn a() {}\n\n\n\nfn b() {}\n";
        let output = format_source(input);
        assert!(!output.contains("\n\n\n"), "Got: {:?}", output);
    }

    #[test]
    fn test_format_operators() {
        let input = "let x = 1+2\n";
        let output = format_source(input);
        assert!(output.contains("1 + 2"), "Got: {:?}", output);
    }
}
