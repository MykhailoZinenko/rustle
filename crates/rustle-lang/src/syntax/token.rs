#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Float(f64),
    Bool(bool),
    Ident(String),
    StringLit(String),
    HexColor(String), // digits only — "ff0000" or "ff0000ff"

    // Keywords
    Fn,
    Let,
    If,
    Else,
    Match,
    While,
    For,
    Foreach,
    In,
    Return,
    Const,
    State,
    Import,
    Out,
    Try,
    And,
    Or,
    Not,
    As,

    // Type keywords — only true primitives and parameterised collection types
    TFloat,
    TBool,
    TArray,
    TList,
    TRes,

    // Operators
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Percent,    // %
    PlusEq,     // +=
    MinusEq,    // -=
    PlusPlus,   // ++
    MinusMinus, // --
    StarEq,     // *=
    SlashEq,    // /=
    Eq,         // =
    EqEq,       // ==
    BangEq,     // !=
    Lt,         // <
    LtEq,       // <=
    Gt,         // >
    GtEq,       // >=
    LtLt,       // <<
    Arrow,      // ->
    FatArrow,   // =>
    At,         // @
    Question,   // ?

    // Punctuation
    Colon,      // :
    Comma,      // ,
    Semicolon,  // ;
    Dot,        // .
    LParen,     // (
    RParen,     // )
    LBrace,     // {
    RBrace,     // }
    LBracket,   // [
    RBracket,   // ]

    Eof,
}

impl TokenKind {
    pub fn is_literal(&self) -> bool {
        matches!(self, Self::Float(_) | Self::Bool(_) | Self::StringLit(_) | Self::HexColor(_))
    }

    pub fn is_arithmetic(&self) -> bool {
        matches!(self, Self::Plus | Self::Minus | Self::Star | Self::Slash | Self::Percent)
    }

    pub fn is_comparison(&self) -> bool {
        matches!(self, Self::EqEq | Self::BangEq | Self::Lt | Self::LtEq | Self::Gt | Self::GtEq)
    }

    pub fn is_logical(&self) -> bool {
        matches!(self, Self::And | Self::Or | Self::Not)
    }

    pub fn is_type_keyword(&self) -> bool {
        matches!(self, Self::TFloat | Self::TBool | Self::TArray | Self::TList | Self::TRes)
    }

    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            Self::Fn | Self::Let | Self::If | Self::Else | Self::Match | Self::While | Self::For | Self::Foreach
            | Self::In | Self::Return | Self::Const | Self::State | Self::Import
            | Self::Out | Self::Try | Self::And | Self::Or | Self::Not | Self::As
        )
    }
}

/// Maps an identifier string to its keyword token, or returns `Ident`.
pub fn keyword_or_ident(s: String) -> TokenKind {
    match s.as_str() {
        "fn"        => TokenKind::Fn,
        "let"       => TokenKind::Let,
        "if"        => TokenKind::If,
        "else"      => TokenKind::Else,
        "while"     => TokenKind::While,
        "for"       => TokenKind::For,
        "foreach"   => TokenKind::Foreach,
        "in"        => TokenKind::In,
        "return"    => TokenKind::Return,
        "const"     => TokenKind::Const,
        "state"     => TokenKind::State,
        "import"    => TokenKind::Import,
        "out"       => TokenKind::Out,
        "try"       => TokenKind::Try,
        "and"       => TokenKind::And,
        "or"        => TokenKind::Or,
        "not"       => TokenKind::Not,
        "as"        => TokenKind::As,
        "match"     => TokenKind::Match,
        "true"      => TokenKind::Bool(true),
        "false"     => TokenKind::Bool(false),
        "float"     => TokenKind::TFloat,
        "bool"      => TokenKind::TBool,
        "array"     => TokenKind::TArray,
        "list"      => TokenKind::TList,
        "res"       => TokenKind::TRes,
        _           => TokenKind::Ident(s),
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
}

impl Token {
    pub fn new(kind: TokenKind, line: usize, column: usize) -> Self {
        Self { kind, line, column }
    }
}
