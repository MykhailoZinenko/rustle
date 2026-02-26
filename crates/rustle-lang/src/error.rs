/// Error codes prefixed by phase: L = lexer, P = parser, S = semantic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCode {
    // Lexer
    L001, // unexpected character
    L002, // unterminated string literal
    L003, // invalid escape sequence

    // Parser
    P001, // unexpected token
    P002, // missing expected token

    // Semantic / resolver
    S001, // undefined symbol
    S002, // type mismatch
    S003, // redeclaration in same scope
    S004, // reassignment of const
    S005, // unknown namespace
    S006, // member not exported by namespace
    S007, // wrong argument count
    S008, // operator not applicable to type
    S009, // field or method not found on type
    S010, // not callable
    S011, // duplicate state block
    S012, // invalid update function signature
}

impl ErrorCode {
    /// All current codes are hard errors (not warnings).
    /// Extend this when warning codes are added.
    pub fn is_error(&self) -> bool { true }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::L001 => "L001",
            Self::L002 => "L002",
            Self::L003 => "L003",
            Self::P001 => "P001",
            Self::P002 => "P002",
            Self::S001 => "S001",
            Self::S002 => "S002",
            Self::S003 => "S003",
            Self::S004 => "S004",
            Self::S005 => "S005",
            Self::S006 => "S006",
            Self::S007 => "S007",
            Self::S008 => "S008",
            Self::S009 => "S009",
            Self::S010 => "S010",
            Self::S011 => "S011",
            Self::S012 => "S012",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Error {
    pub code: ErrorCode,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl Error {
    pub fn new(code: ErrorCode, line: usize, column: usize, message: impl Into<String>) -> Self {
        Self { code, line, column, message: message.into() }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}:{} — {}", self.code.as_str(), self.line, self.column, self.message)
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub line: usize,
    pub message: String,
}

impl RuntimeError {
    pub fn new(line: usize, message: impl Into<String>) -> Self {
        Self { line, message: message.into() }
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[runtime] {} — {}", self.line, self.message)
    }
}
