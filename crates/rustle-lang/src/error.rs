/// Error codes prefixed by phase: L = lexer, P = parser, S = semantic, R = runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCode {
    // Lexer
    L001, // unexpected character
    L002, // unterminated string literal
    L003, // invalid escape sequence
    L004, // unterminated block comment
    L005, // invalid number literal

    // Parser
    P001, // unexpected token
    P002, // missing expected token
    P003, // unclosed delimiter
    P004, // invalid assignment target
    P005, // empty body
    P006, // duplicate else clause
    P007, // invalid type annotation

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
    S013, // missing return value
    S014, // unreachable code
    S015, // cannot infer type

    // Runtime
    R001, // type error
    R002, // undefined variable/function
    R003, // field not found
    R004, // method not found
    R005, // index out of bounds
    R006, // invalid index
    R007, // division by zero
    R008, // wrong argument count
    R009, // not callable
    R010, // cancelled
    R011, // invalid operation
    R012, // assertion error
}

impl ErrorCode {
    /// All current codes are hard errors (not warnings).
    /// Extend this when warning codes are added.
    #[must_use] 
    pub fn is_error(&self) -> bool { true }

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
        }
    }
}

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

#[derive(Debug, Clone)]
pub struct StackFrame {
    pub function: String,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub code: ErrorCode,
    pub line: usize,
    pub message: String,
    pub stack: Vec<StackFrame>,
}

impl RuntimeError {
    pub fn new(code: ErrorCode, line: usize, message: impl Into<String>) -> Self {
        Self { code, line, message: message.into(), stack: Vec::new() }
    }

    pub fn push_frame(&mut self, function: impl Into<String>, line: usize) {
        self.stack.push(StackFrame { function: function.into(), line });
    }
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {} — {}", self.code.as_str(), self.line, self.message)?;
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
