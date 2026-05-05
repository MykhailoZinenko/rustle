use std::sync::Arc;

/// Source location attached to every node for error reporting.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

impl Span {
    #[must_use]
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column, end_line: line, end_column: column }
    }

    /// Create a span covering from start to end position.
    #[must_use]
    pub fn range(line: usize, column: usize, end_line: usize, end_column: usize) -> Self {
        Self { line, column, end_line, end_column }
    }
}

// ─── Top level ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Program {
    pub imports: Vec<ImportDecl>,
    pub state: Option<StateBlock>,
    pub items: Vec<Item>,
}

/// `import shapes { circle, rect }` or `import render`
#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub namespace: String,
    pub members: Vec<String>, // empty = import whole namespace
    pub span: Span,
}

/// The `state { ... }` block — fields persist between frames.
#[derive(Debug, Clone)]
pub struct StateBlock {
    pub fields: Vec<StateField>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StateField {
    pub name: String,
    pub ty: Option<Type>,
    pub initializer: Expr,
    pub span: Span,
}

/// A top-level item is either a function definition or a statement.
#[derive(Debug, Clone)]
pub enum Item {
    FnDef(FnDef),
    Stmt(Stmt),
}

// ─── Functions ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: String,
    pub params: Arc<[Param]>,
    pub return_ty: Option<Type>,
    pub body: Arc<[Stmt]>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub span: Span,
}

// ─── Statements ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Stmt {
    /// `let x = 3.14` or `let x: float = 3.14` or `const PI = 3.14`
    VarDecl(VarDecl),
    /// `x = 3.14`
    Assign(Assign),
    /// `out << s1 << s2`
    Out(OutStmt),
    /// `console << x`, `console.warn << x`, `console.error << x`
    Print(PrintStmt),
    /// `if ... { } else { }`
    If(IfStmt),
    /// `if let v = expr { } else { }`
    IfLet {
        binding: String,
        expr: Expr,
        then_block: Vec<Stmt>,
        else_block: Option<Vec<Stmt>>,
        span: Span,
    },
    /// `while cond { }`
    While(WhileStmt),
    /// `for let i = 0.0; i < 10.0; i = i + 1.0 { }`
    For(ForStmt),
    /// `foreach v in list { }` or `foreach v: float in list { }`
    Foreach(ForeachStmt),
    /// `match expr { val1, val2 => { } else => { } }`
    Match(MatchStmt),
    /// `return expr` or bare `return`
    Return(Option<Expr>, Span),
    /// `break`
    Break(Span),
    /// `continue`
    Continue(Span),
    /// `fn f = add` or `fn g = (a: float) -> float { ... }`
    FnVar {
        name: String,
        value: Expr,
        span: Span,
    },
    /// A standalone expression used as a statement (e.g. a function call).
    Expr(Expr),
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub name: String,
    pub ty: Option<Type>,
    pub is_const: bool,
    pub initializer: Expr,
    pub span: Span,
}

/// Assignable target: dotted path or indexed (arr[i], s.arr[i]).
#[derive(Debug, Clone)]
pub enum AssignTarget {
    /// `x` or `s.x` — dotted path only
    Path(Vec<String>),
    /// `arr[i]` or `s.arr[i]` — path plus index expressions
    Indexed {
        path: Vec<String>,
        indices: Vec<Expr>,
    },
}

impl AssignTarget {
    #[must_use] 
    pub fn path(&self) -> &[String] {
        match self {
            AssignTarget::Path(p) => p,
            AssignTarget::Indexed { path, .. } => path,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Assign {
    pub target: AssignTarget,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct OutStmt {
    pub shapes: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrintLevel { Log, Warn, Error }

#[derive(Debug, Clone)]
pub struct PrintStmt {
    pub level: PrintLevel,
    pub values: Vec<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchStmt {
    pub expr: Expr,
    pub arms: Vec<MatchArm>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    /// Patterns to match (empty for `else`).
    pub values: Vec<Expr>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub condition: Expr,
    pub then_block: Vec<Stmt>,
    pub else_block: Option<Vec<Stmt>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub condition: Expr,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ForStmt {
    pub init: Box<Stmt>,      // VarDecl
    pub condition: Expr,
    pub step: Box<Stmt>,      // Assign
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct ForeachStmt {
    pub var_name: String,
    pub var_ty: Option<Type>,
    pub iterable: Expr,
    pub body: Vec<Stmt>,
    pub span: Span,
}

// ─── Expressions ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Expr {
    Float(f64, Span),
    Bool(bool, Span),
    None(Span),
    StringLit(String, Span),
    HexColor(String, Span),
    Ident(String, Span),

    /// `a + b`, `a == b`, etc.
    BinOp {
        left: Box<Expr>,
        op: BinOp,
        right: Box<Expr>,
        span: Span,
    },

    /// `not x`, `-x`
    UnOp {
        op: UnOp,
        operand: Box<Expr>,
        span: Span,
    },

    /// `cond ? then : else`
    Ternary {
        condition: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
        span: Span,
    },

    /// `expr as float`
    Cast {
        expr: Box<Expr>,
        ty: Type,
        span: Span,
    },

    /// `try expr`
    Try {
        expr: Box<Expr>,
        span: Span,
    },

    /// `name(args, named: val)`
    Call {
        callee: String,
        args: Vec<Expr>,
        named_args: Vec<(String, Expr)>,
        span: Span,
    },

    /// `expr[index]`
    Index {
        expr: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },

    /// `expr.field`
    Field {
        expr: Box<Expr>,
        field: String,
        span: Span,
    },

    /// `expr?.field` — optional chaining
    OptionalChain {
        expr: Box<Expr>,
        field: String,
        span: Span,
    },

    /// `expr.method(args, name: val)`
    MethodCall {
        expr: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        named_args: Vec<(String, Expr)>,
        span: Span,
    },

    /// `shape@t` or `shape@(t1, t2, t3)`
    Transform {
        expr: Box<Expr>,
        transforms: Vec<Expr>,
        span: Span,
    },

    /// `[1.0, 2.0, 3.0]`
    List(Vec<Expr>, Span),

    /// `(a: float, b: float) -> float { return a + b }`
    Lambda {
        params: Arc<[Param]>,
        return_ty: Option<Type>,
        body: Arc<[Stmt]>,
        span: Span,
    },
}

impl Expr {
    #[must_use] 
    pub fn span(&self) -> &Span {
        match self {
            Expr::Float(_, s) | Expr::Bool(_, s) | Expr::StringLit(_, s)
            | Expr::HexColor(_, s) | Expr::Ident(_, s)
            | Expr::None(s) | Expr::List(_, s) => s,
            Expr::BinOp { span, .. }
            | Expr::UnOp { span, .. }
            | Expr::Ternary { span, .. }
            | Expr::Cast { span, .. }
            | Expr::Try { span, .. }
            | Expr::Call { span, .. }
            | Expr::Index { span, .. }
            | Expr::Field { span, .. }
            | Expr::OptionalChain { span, .. }
            | Expr::MethodCall { span, .. }
            | Expr::Transform { span, .. }
            | Expr::Lambda { span, .. } => span,
        }
    }
}

// ─── Operators ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add, Sub, Mul, Div, Mod,
    Eq, NotEq,
    Lt, LtEq, Gt, GtEq,
    And, Or,
    Coalesce,
}

impl std::fmt::Display for BinOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            BinOp::Add   => "+",
            BinOp::Sub   => "-",
            BinOp::Mul   => "*",
            BinOp::Div   => "/",
            BinOp::Mod   => "%",
            BinOp::Eq    => "==",
            BinOp::NotEq => "!=",
            BinOp::Lt    => "<",
            BinOp::LtEq  => "<=",
            BinOp::Gt    => ">",
            BinOp::GtEq  => ">=",
            BinOp::And      => "and",
            BinOp::Or       => "or",
            BinOp::Coalesce => "??",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
    PrefixInc,   // ++x
    PrefixDec,   // --x
    PostfixInc,  // x++
    PostfixDec,  // x--
}

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Float,
    Bool,
    String,
    Unit,
    Vec2,
    Vec3,
    Vec4,
    Color,
    Mat3,
    Mat4,
    Transform,
    Shape,
    Circle,
    Rect,
    Line,
    Polygon,
    State,
    Input,
    Array(Box<Type>, usize),
    List(Box<Type>),
    Res(Box<Type>),
    Optional(Box<Type>),
    Fn(Vec<Type>, Option<Box<Type>>),
    /// User-defined types (future structs/enums). Built-in types have their own variants above.
    Named(String),
}

impl Type {
    /// Convert a string name to the corresponding built-in Type variant.
    /// Falls back to `Type::Named(name)` for unknown names.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "string" => Self::String,
            "vec2" => Self::Vec2, "vec3" => Self::Vec3, "vec4" => Self::Vec4,
            "color" => Self::Color,
            "mat3" => Self::Mat3, "mat4" => Self::Mat4,
            "transform" => Self::Transform,
            "shape" => Self::Shape,
            "circle" => Self::Circle, "rect" => Self::Rect,
            "line" => Self::Line, "polygon" => Self::Polygon,
            "State" => Self::State, "Input" => Self::Input,
            other => Self::Named(other.into()),
        }
    }
}
