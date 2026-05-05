// NOTE: TokenKind stores owned Strings for Ident/StringLit/HexColor.
// This causes cloning in parser's peek_kind(). A string-interning approach
// would eliminate this, but isn't worth the complexity until parsing
// becomes a measured bottleneck. See PRACTICES_REVIEW.md #31.
//
// Note: Float(f64) uses IEEE equality (NaN != NaN). This is acceptable because
// the lexer never produces NaN literals, and token comparison is only used for parsing.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    Float(f64),
    Bool(bool),
    Ident(String),
    StringLit(String),
    /// Template string with interpolation: `hello ${name}`
    TemplateLit(Vec<TemplatePart>),
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
    Console,
    Try,
    And,
    Or,
    Not,
    As,
    Break,
    Continue,
    None,
    Struct,

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
    Question,         // ?
    QuestionQuestion, // ??
    QuestionDot,      // ?.

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
    #[must_use] 
    pub fn is_literal(&self) -> bool {
        matches!(self, Self::Float(_) | Self::Bool(_) | Self::StringLit(_) | Self::TemplateLit(_) | Self::HexColor(_))
    }

    #[must_use] 
    pub fn is_arithmetic(&self) -> bool {
        matches!(self, Self::Plus | Self::Minus | Self::Star | Self::Slash | Self::Percent)
    }

    #[must_use] 
    pub fn is_comparison(&self) -> bool {
        matches!(self, Self::EqEq | Self::BangEq | Self::Lt | Self::LtEq | Self::Gt | Self::GtEq)
    }

    #[must_use] 
    pub fn is_logical(&self) -> bool {
        matches!(self, Self::And | Self::Or | Self::Not)
    }

    #[must_use] 
    pub fn is_type_keyword(&self) -> bool {
        matches!(self, Self::TFloat | Self::TBool | Self::TArray | Self::TList | Self::TRes)
    }

    #[must_use] 
    pub fn display_name(&self) -> &str {
        match self {
            Self::Float(_)     => "number",
            Self::Bool(_)      => "bool",
            Self::Ident(_)     => "identifier",
            Self::StringLit(_) | Self::TemplateLit(_) => "string",
            Self::HexColor(_)  => "color literal",
            Self::Fn           => "'fn'",
            Self::Let          => "'let'",
            Self::If           => "'if'",
            Self::Else         => "'else'",
            Self::Match        => "'match'",
            Self::While        => "'while'",
            Self::For          => "'for'",
            Self::Foreach      => "'foreach'",
            Self::In           => "'in'",
            Self::Return       => "'return'",
            Self::Const        => "'const'",
            Self::State        => "'state'",
            Self::Import       => "'import'",
            Self::Out          => "'out'",
            Self::Console      => "'console'",
            Self::Try          => "'try'",
            Self::And          => "'and'",
            Self::Or           => "'or'",
            Self::Not          => "'not'",
            Self::As           => "'as'",
            Self::Break        => "'break'",
            Self::Continue     => "'continue'",
            Self::None         => "'none'",
            Self::Struct       => "'struct'",
            Self::TFloat       => "'float'",
            Self::TBool        => "'bool'",
            Self::TArray       => "'array'",
            Self::TList        => "'list'",
            Self::TRes         => "'res'",
            Self::Plus         => "'+'",
            Self::Minus        => "'-'",
            Self::Star         => "'*'",
            Self::Slash        => "'/'",
            Self::Percent      => "'%'",
            Self::PlusEq       => "'+='",
            Self::MinusEq      => "'-='",
            Self::PlusPlus     => "'++'",
            Self::MinusMinus   => "'--'",
            Self::StarEq       => "'*='",
            Self::SlashEq      => "'/='",
            Self::Eq           => "'='",
            Self::EqEq         => "'=='",
            Self::BangEq       => "'!='",
            Self::Lt           => "'<'",
            Self::LtEq         => "'<='",
            Self::Gt           => "'>'",
            Self::GtEq         => "'>='",
            Self::LtLt         => "'<<'",
            Self::Arrow        => "'->'",
            Self::FatArrow     => "'=>'",
            Self::At           => "'@'",
            Self::Question     => "'?'",
            Self::QuestionQuestion => "'??'",
            Self::QuestionDot  => "'?.'",
            Self::Colon        => "':'",
            Self::Comma        => "','",
            Self::Semicolon    => "';'",
            Self::Dot          => "'.'",
            Self::LParen       => "'('",
            Self::RParen       => "')'",
            Self::LBrace       => "'{'",
            Self::RBrace       => "'}'",
            Self::LBracket     => "'['",
            Self::RBracket     => "']'",
            Self::Eof          => "end of file",
        }
    }

    #[must_use] 
    pub fn is_keyword(&self) -> bool {
        matches!(
            self,
            Self::Fn | Self::Let | Self::If | Self::Else | Self::Match | Self::While | Self::For | Self::Foreach
            | Self::In | Self::Return | Self::Const | Self::State | Self::Import
            | Self::Out | Self::Console | Self::Try | Self::And | Self::Or | Self::Not | Self::As
            | Self::Break | Self::Continue | Self::None | Self::Struct
        )
    }
}

/// Maps an identifier string to its keyword token, or returns `Ident`.
#[must_use] 
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
        "console"   => TokenKind::Console,
        "try"       => TokenKind::Try,
        "and"       => TokenKind::And,
        "or"        => TokenKind::Or,
        "not"       => TokenKind::Not,
        "as"        => TokenKind::As,
        "break"     => TokenKind::Break,
        "continue"  => TokenKind::Continue,
        "match"     => TokenKind::Match,
        "none"      => TokenKind::None,
        "struct"    => TokenKind::Struct,
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

// ─── Template string parts ───────────────────────────────────────────────────

/// A segment of an interpolated template string.
#[derive(Debug, Clone, PartialEq)]
pub enum TemplatePart {
    /// Literal text between interpolations.
    Lit(String),
    /// Raw source text of an expression inside `${}`.
    Expr(String),
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub column: usize,
}

impl Token {
    #[must_use] 
    pub fn new(kind: TokenKind, line: usize, column: usize) -> Self {
        Self { kind, line, column }
    }
}
