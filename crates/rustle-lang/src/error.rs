/// Error classification code, prefixed by phase: L=lexer, P=parser, S=semantic, R=runtime.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCode {
    // Lexer
    /// Unexpected character in source.
    L001,
    /// Unterminated string literal.
    L002,
    /// Invalid escape sequence.
    L003,
    /// Unterminated block comment.
    L004,
    /// Invalid number literal.
    L005,

    // Parser
    /// Unexpected token.
    P001,
    /// Missing expected token.
    P002,
    /// Unclosed delimiter.
    P003,
    /// Invalid assignment target.
    P004,
    /// Empty body.
    P005,
    /// Duplicate else clause.
    P006,
    /// Invalid type annotation.
    P007,

    // Semantic / resolver
    /// Undefined symbol.
    S001,
    /// Type mismatch.
    S002,
    /// Redeclaration in same scope.
    S003,
    /// Reassignment of const.
    S004,
    /// Unknown namespace.
    S005,
    /// Member not exported by namespace.
    S006,
    /// Wrong argument count.
    S007,
    /// Operator not applicable to type.
    S008,
    /// Field or method not found on type.
    S009,
    /// Expression is not callable.
    S010,
    /// Duplicate state block.
    S011,
    /// Invalid lifecycle function signature.
    S012,
    /// Missing return value.
    S013,
    /// Unreachable code or invalid control flow.
    S014,
    /// Cannot infer type.
    S015,
    /// Private method accessed from outside struct.
    S016,
    /// Missing required field in struct construction.
    S017,
    /// Unknown field in struct construction.
    S018,
    /// Duplicate field in struct definition.
    S019,
    /// Duplicate method in struct definition.
    S020,
    /// `this` used outside struct method.
    S021,

    // Runtime
    /// Runtime type error.
    R001,
    /// Undefined variable or function.
    R002,
    /// Field not found.
    R003,
    /// Method not found.
    R004,
    /// Index out of bounds.
    R005,
    /// Invalid index value.
    R006,
    /// Division by zero.
    R007,
    /// Wrong argument count.
    R008,
    /// Expression is not callable.
    R009,
    /// Script cancelled.
    R010,
    /// Invalid operation.
    R011,
    /// Assertion failed.
    R012,
    /// Field not found on struct (runtime safety net).
    R013,
}

impl ErrorCode {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::L001 => "L001",
            Self::L002 => "L002",
            Self::L003 => "L003",
            Self::L004 => "L004",
            Self::L005 => "L005",
            Self::P001 => "P001",
            Self::P002 => "P002",
            Self::P003 => "P003",
            Self::P004 => "P004",
            Self::P005 => "P005",
            Self::P006 => "P006",
            Self::P007 => "P007",
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
            Self::S013 => "S013",
            Self::S014 => "S014",
            Self::S015 => "S015",
            Self::S016 => "S016",
            Self::S017 => "S017",
            Self::S018 => "S018",
            Self::S019 => "S019",
            Self::S020 => "S020",
            Self::S021 => "S021",
            Self::R001 => "R001",
            Self::R002 => "R002",
            Self::R003 => "R003",
            Self::R004 => "R004",
            Self::R005 => "R005",
            Self::R006 => "R006",
            Self::R007 => "R007",
            Self::R008 => "R008",
            Self::R009 => "R009",
            Self::R010 => "R010",
            Self::R011 => "R011",
            Self::R012 => "R012",
            Self::R013 => "R013",
        }
    }
}

/// Compile-time error with source location, code, message, and optional hint.
#[derive(Debug, Clone)]
pub struct Error {
    pub code: ErrorCode,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub hint: Option<String>,
}

impl Error {
    pub fn new(code: ErrorCode, line: usize, column: usize, message: impl Into<String>) -> Self {
        Self { code, line, column, message: message.into(), hint: None }
    }

    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}:{} — {}", self.code.as_str(), self.line, self.column, self.message)?;
        if let Some(hint) = &self.hint {
            write!(f, "\n  hint: {hint}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Error {}

// ─────────────────────────────────────────────────────────────────────────────

/// A single frame in the runtime call stack.
#[derive(Debug, Clone)]
pub struct StackFrame {
    pub function: String,
    pub line: usize,
}

/// Runtime error with source location, code, message, and call stack trace.
#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub code: ErrorCode,
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub stack: Vec<StackFrame>,
}

impl RuntimeError {
    pub fn new(code: ErrorCode, line: usize, column: usize, message: impl Into<String>) -> Self {
        Self { code, line, column, message: message.into(), stack: Vec::new() }
    }

    pub fn push_frame(&mut self, function: impl Into<String>, line: usize) {
        self.stack.push(StackFrame { function: function.into(), line });
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}:{} — {}", self.code.as_str(), self.line, self.column, self.message)?;
        for frame in &self.stack {
            write!(f, "\n  at {} (line {})", frame.function, frame.line)?;
        }
        Ok(())
    }
}

impl std::error::Error for RuntimeError {}

// ─────────────────────────────────────────────────────────────────────────────
// Suggestion helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the Levenshtein edit distance between two strings.
#[must_use]
pub fn levenshtein(a: &str, b: &str) -> usize {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (m, n) = (a.len(), b.len());
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Find the closest match to `name` from `candidates` within `max_dist` edits.
#[must_use]
pub fn suggest_similar<'a>(name: &str, candidates: &[&'a str], max_dist: usize) -> Option<&'a str> {
    candidates
        .iter()
        .filter(|c| **c != name)
        .map(|c| (*c, levenshtein(name, c)))
        .filter(|(_, d)| *d <= max_dist)
        .min_by_key(|(_, d)| *d)
        .map(|(c, _)| c)
}
