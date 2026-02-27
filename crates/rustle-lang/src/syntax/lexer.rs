use crate::error::{Error, ErrorCode};
use crate::syntax::token::{Token, TokenKind, keyword_or_ident};

pub struct Lexer<'a> {
    source: &'a [u8],
    pos: usize,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source: source.as_bytes(), pos: 0, line: 1, column: 1 }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, Vec<Error>> {
        let mut tokens = Vec::new();
        let mut errors = Vec::new();

        loop {
            self.skip_whitespace();

            if self.is_at_end() {
                tokens.push(Token::new(TokenKind::Eof, self.line, self.column));
                break;
            }

            match self.next_token() {
                Ok(Some(tok)) => tokens.push(tok),
                Ok(None) => {}
                Err(e) => errors.push(e),
            }
        }

        if errors.is_empty() { Ok(tokens) } else { Err(errors) }
    }

    fn next_token(&mut self) -> Result<Option<Token>, Error> {
        let line = self.line;
        let col = self.column;
        let ch = self.advance();

        let kind = match ch {
            b'+' => {
                if self.peek() == b'+' { self.advance(); TokenKind::PlusPlus }
                else if self.peek() == b'=' { self.advance(); TokenKind::PlusEq }
                else { TokenKind::Plus }
            }
            b'*' => {
                if self.peek() == b'=' { self.advance(); TokenKind::StarEq }
                else { TokenKind::Star }
            }
            b'%' => TokenKind::Percent,
            b'?' => TokenKind::Question,
            b':' => TokenKind::Colon,
            b',' => TokenKind::Comma,
            b';' => TokenKind::Semicolon,
            b'.' => TokenKind::Dot,
            b'(' => TokenKind::LParen,
            b')' => TokenKind::RParen,
            b'{' => TokenKind::LBrace,
            b'}' => TokenKind::RBrace,
            b'[' => TokenKind::LBracket,
            b']' => TokenKind::RBracket,
            b'@' => TokenKind::At,

            b'-' => {
                if self.peek() == b'-' { self.advance(); TokenKind::MinusMinus }
                else if self.peek() == b'>' { self.advance(); TokenKind::Arrow }
                else if self.peek() == b'=' { self.advance(); TokenKind::MinusEq }
                else { TokenKind::Minus }
            }
            b'/' => {
                if self.peek() == b'/' { self.skip_line(); return Ok(None); }
                else if self.peek() == b'*' { self.skip_block_comment(); return Ok(None); }
                else if self.peek() == b'=' { self.advance(); TokenKind::SlashEq }
                else { TokenKind::Slash }
            }
            b'=' => {
                if self.peek() == b'=' { self.advance(); TokenKind::EqEq }
                else { TokenKind::Eq }
            }
            b'!' => {
                if self.peek() == b'=' { self.advance(); TokenKind::BangEq }
                else {
                    return Err(Error::new(ErrorCode::L001, line, col,
                        "expected `!=`, bare `!` is not valid"));
                }
            }
            b'<' => {
                if self.peek() == b'<' { self.advance(); TokenKind::LtLt }
                else if self.peek() == b'=' { self.advance(); TokenKind::LtEq }
                else { TokenKind::Lt }
            }
            b'>' => {
                if self.peek() == b'=' { self.advance(); TokenKind::GtEq }
                else { TokenKind::Gt }
            }

            b'#' => {
                if self.is_hex_sequence() { TokenKind::HexColor(self.read_hex_color()) }
                else { self.skip_line(); return Ok(None); }
            }
            b'"' => TokenKind::StringLit(self.read_string(line, col)?),
            b'0'..=b'9' => TokenKind::Float(self.read_number(ch)),
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => keyword_or_ident(self.read_ident(ch)),

            other => {
                return Err(Error::new(ErrorCode::L001, line, col,
                    format!("unexpected character `{}`", other as char)));
            }
        };

        Ok(Some(Token::new(kind, line, col)))
    }

    // ─── Primitives ──────────────────────────────────────────────────────────

    fn advance(&mut self) -> u8 {
        let ch = self.source[self.pos];
        self.pos += 1;
        if ch == b'\n' { self.line += 1; self.column = 1; }
        else { self.column += 1; }
        ch
    }

    fn peek(&self) -> u8 {
        if self.is_at_end() { 0 } else { self.source[self.pos] }
    }

    fn peek_next(&self) -> u8 {
        if self.pos + 1 >= self.source.len() { 0 } else { self.source[self.pos + 1] }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.source.len()
    }

    fn skip_whitespace(&mut self) {
        while !self.is_at_end() {
            match self.peek() {
                b' ' | b'\t' | b'\r' | b'\n' => { self.advance(); }
                _ => break,
            }
        }
    }

    fn skip_line(&mut self) {
        while !self.is_at_end() && self.peek() != b'\n' { self.advance(); }
    }

    fn skip_block_comment(&mut self) {
        self.advance(); // consume *
        while !self.is_at_end() {
            if self.peek() == b'*' && self.peek_next() == b'/' {
                self.advance(); // *
                self.advance(); // /
                break;
            }
            self.advance();
        }
    }

    // ─── Readers ─────────────────────────────────────────────────────────────

    /// Returns true when the next 6 bytes are all hex digits.
    fn is_hex_sequence(&self) -> bool {
        self.pos + 6 <= self.source.len()
            && self.source[self.pos..self.pos + 6].iter().all(|b| b.is_ascii_hexdigit())
    }

    fn read_hex_color(&mut self) -> String {
        let mut s = String::with_capacity(8);
        for _ in 0..6 { s.push(self.advance() as char); }
        // optional 2-digit alpha channel
        if self.pos + 2 <= self.source.len()
            && self.source[self.pos..self.pos + 2].iter().all(|b| b.is_ascii_hexdigit())
        {
            s.push(self.advance() as char);
            s.push(self.advance() as char);
        }
        s
    }

    fn read_string(&mut self, start_line: usize, start_col: usize) -> Result<String, Error> {
        let mut s = String::new();
        let mut error: Option<Error> = None;
        loop {
            if self.is_at_end() || self.peek() == b'\n' {
                return Err(Error::new(ErrorCode::L002, start_line, start_col,
                    "unterminated string literal"));
            }
            let ch = self.advance();
            if ch == b'"' { break; }
            if ch == b'\\' {
                let esc_line = self.line;
                let esc_col  = self.column;
                match self.advance() {
                    b'n'  => s.push('\n'),
                    b't'  => s.push('\t'),
                    b'"'  => s.push('"'),
                    b'\\' => s.push('\\'),
                    other => {
                        // Record the first escape error but keep consuming so we
                        // don't produce cascading errors from the remainder of the string.
                        if error.is_none() {
                            error = Some(Error::new(ErrorCode::L003, esc_line, esc_col,
                                format!("unknown escape sequence `\\{}`", other as char)));
                        }
                    }
                }
            } else {
                s.push(ch as char);
            }
        }
        if let Some(e) = error { return Err(e); }
        Ok(s)
    }

    fn read_number(&mut self, first: u8) -> f64 {
        let mut s = String::new();
        s.push(first as char);
        while !self.is_at_end() && self.peek().is_ascii_digit() {
            s.push(self.advance() as char);
        }
        // consume decimal only if followed by at least one digit
        // (avoids treating `.` in `shape.field` as a decimal point)
        if !self.is_at_end() && self.peek() == b'.' && self.peek_next().is_ascii_digit() {
            s.push(self.advance() as char);
            while !self.is_at_end() && self.peek().is_ascii_digit() {
                s.push(self.advance() as char);
            }
        }
        s.parse().unwrap_or(0.0)
    }

    fn read_ident(&mut self, first: u8) -> String {
        let mut s = String::new();
        s.push(first as char);
        while !self.is_at_end() && (self.peek().is_ascii_alphanumeric() || self.peek() == b'_') {
            s.push(self.advance() as char);
        }
        s
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn lex(src: &str) -> Vec<TokenKind> {
        Lexer::new(src).tokenize().unwrap().into_iter().map(|t| t.kind).collect()
    }

    fn lex_err(src: &str) -> Vec<Error> {
        Lexer::new(src).tokenize().unwrap_err()
    }

    #[test]
    fn empty() {
        assert_eq!(lex(""), vec![TokenKind::Eof]);
    }

    #[test]
    fn integer_becomes_float() {
        assert_eq!(lex("42"), vec![TokenKind::Float(42.0), TokenKind::Eof]);
    }

    #[test]
    fn float_literal() {
        assert_eq!(lex("3.14"), vec![TokenKind::Float(3.14), TokenKind::Eof]);
    }

    #[test]
    fn dot_not_consumed_by_number() {
        assert_eq!(
            lex("s.x"),
            vec![TokenKind::Ident("s".into()), TokenKind::Dot, TokenKind::Ident("x".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn keywords() {
        assert_eq!(lex("fn"),      vec![TokenKind::Fn,      TokenKind::Eof]);
        assert_eq!(lex("let"),     vec![TokenKind::Let,     TokenKind::Eof]);
        assert_eq!(lex("const"),   vec![TokenKind::Const,   TokenKind::Eof]);
        assert_eq!(lex("foreach"), vec![TokenKind::Foreach,  TokenKind::Eof]);
        assert_eq!(lex("return"),  vec![TokenKind::Return,   TokenKind::Eof]);
        assert_eq!(lex("state"),   vec![TokenKind::State,    TokenKind::Eof]);
    }

    #[test]
    fn bool_literals() {
        assert_eq!(lex("true"),  vec![TokenKind::Bool(true),  TokenKind::Eof]);
        assert_eq!(lex("false"), vec![TokenKind::Bool(false), TokenKind::Eof]);
    }

    #[test]
    fn type_keywords() {
        // true primitives — remain grammar keywords
        assert_eq!(lex("float"), vec![TokenKind::TFloat, TokenKind::Eof]);
        assert_eq!(lex("bool"),  vec![TokenKind::TBool,  TokenKind::Eof]);
        // everything else lexes as a plain identifier
        assert_eq!(lex("vec2"),      vec![TokenKind::Ident("vec2".into()),      TokenKind::Eof]);
        assert_eq!(lex("color"),     vec![TokenKind::Ident("color".into()),     TokenKind::Eof]);
        assert_eq!(lex("shape"),     vec![TokenKind::Ident("shape".into()),     TokenKind::Eof]);
        assert_eq!(lex("transform"), vec![TokenKind::Ident("transform".into()), TokenKind::Eof]);
    }

    #[test]
    fn inc_dec_tokens() {
        assert_eq!(lex("++"), vec![TokenKind::PlusPlus,   TokenKind::Eof]);
        assert_eq!(lex("--"), vec![TokenKind::MinusMinus, TokenKind::Eof]);
    }

    #[test]
    fn two_char_operators() {
        assert_eq!(lex("=="), vec![TokenKind::EqEq,   TokenKind::Eof]);
        assert_eq!(lex("!="), vec![TokenKind::BangEq, TokenKind::Eof]);
        assert_eq!(lex("<="), vec![TokenKind::LtEq,   TokenKind::Eof]);
        assert_eq!(lex(">="), vec![TokenKind::GtEq,   TokenKind::Eof]);
        assert_eq!(lex("<<"), vec![TokenKind::LtLt,   TokenKind::Eof]);
        assert_eq!(lex("->"), vec![TokenKind::Arrow,  TokenKind::Eof]);
    }

    #[test]
    fn line_comment_skipped() {
        assert_eq!(lex("// comment\n42"), vec![TokenKind::Float(42.0), TokenKind::Eof]);
    }

    #[test]
    fn block_comment_skipped() {
        assert_eq!(lex("/* comment */42"), vec![TokenKind::Float(42.0), TokenKind::Eof]);
        assert_eq!(lex("/* a\nb */42"), vec![TokenKind::Float(42.0), TokenKind::Eof]);
    }

    #[test]
    fn metadata_comment_skipped() {
        assert_eq!(lex("# author: name\n42"), vec![TokenKind::Float(42.0), TokenKind::Eof]);
    }

    #[test]
    fn hex_color_6() {
        assert_eq!(lex("#ff0000"), vec![TokenKind::HexColor("ff0000".into()), TokenKind::Eof]);
    }

    #[test]
    fn hex_color_8() {
        assert_eq!(lex("#ff000080"), vec![TokenKind::HexColor("ff000080".into()), TokenKind::Eof]);
    }

    #[test]
    fn string_literal() {
        assert_eq!(lex(r#""hello""#), vec![TokenKind::StringLit("hello".into()), TokenKind::Eof]);
    }

    #[test]
    fn string_escape_newline() {
        assert_eq!(lex(r#""a\nb""#), vec![TokenKind::StringLit("a\nb".into()), TokenKind::Eof]);
    }

    #[test]
    fn unterminated_string_error() {
        let errs = lex_err(r#""oops"#);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code, ErrorCode::L002);
    }

    #[test]
    fn invalid_escape_error() {
        let errs = lex_err(r#""\q""#);
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code, ErrorCode::L003);
    }

    #[test]
    fn bare_bang_error() {
        let errs = lex_err("!");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code, ErrorCode::L001);
    }

    #[test]
    fn line_and_column_tracking() {
        let tokens = Lexer::new("a\nb").tokenize().unwrap();
        assert_eq!((tokens[0].line, tokens[0].column), (1, 1));
        assert_eq!((tokens[1].line, tokens[1].column), (2, 1));
    }

    #[test]
    fn variable_declaration() {
        assert_eq!(
            lex("let x: float = 3.14"),
            vec![TokenKind::Let, TokenKind::Ident("x".into()), TokenKind::Colon, TokenKind::TFloat, TokenKind::Eq, TokenKind::Float(3.14), TokenKind::Eof]
        );
    }

    #[test]
    fn function_signature() {
        assert_eq!(
            lex("fn add(a: float, b: float) -> float"),
            vec![
                TokenKind::Fn,
                TokenKind::Ident("add".into()),
                TokenKind::LParen,
                TokenKind::Ident("a".into()), TokenKind::Colon, TokenKind::TFloat,
                TokenKind::Comma,
                TokenKind::Ident("b".into()), TokenKind::Colon, TokenKind::TFloat,
                TokenKind::RParen,
                TokenKind::Arrow,
                TokenKind::TFloat,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn out_stream() {
        assert_eq!(
            lex("out << s"),
            vec![TokenKind::Out, TokenKind::LtLt, TokenKind::Ident("s".into()), TokenKind::Eof]
        );
    }

    #[test]
    fn token_kind_helpers() {
        assert!(TokenKind::Plus.is_arithmetic());
        assert!(TokenKind::EqEq.is_comparison());
        assert!(TokenKind::And.is_logical());
        assert!(TokenKind::TFloat.is_type_keyword());
        assert!(TokenKind::Float(1.0).is_literal());
        assert!(TokenKind::Fn.is_keyword());
        assert!(!TokenKind::Ident("x".into()).is_keyword());
    }
}
