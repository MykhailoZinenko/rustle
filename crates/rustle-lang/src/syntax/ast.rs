/// Source location attached to every node for error reporting.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

impl Span {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
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
    pub params: Vec<Param>,
    pub return_ty: Option<Type>,
    pub body: Vec<Stmt>,
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
    /// `if ... { } else { }`
    If(IfStmt),
    /// `while cond { }`
    While(WhileStmt),
    /// `for let i = 0.0; i < 10.0; i = i + 1.0 { }`
    For(ForStmt),
    /// `foreach v in list { }` or `foreach v: float in list { }`
    Foreach(ForeachStmt),
    /// `return expr` or bare `return`
    Return(Option<Expr>, Span),
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

#[derive(Debug, Clone)]
pub struct Assign {
    /// Dotted path: `["x"]` for `x = …`, `["s", "t"]` for `s.t = …`
    pub target: Vec<String>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct OutStmt {
    pub shapes: Vec<Expr>,
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
        params: Vec<Param>,
        return_ty: Option<Type>,
        body: Vec<Stmt>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> &Span {
        match self {
            Expr::Float(_, s)       => s,
            Expr::Bool(_, s)        => s,
            Expr::StringLit(_, s)   => s,
            Expr::HexColor(_, s)    => s,
            Expr::Ident(_, s)       => s,
            Expr::BinOp { span, .. }    => span,
            Expr::UnOp { span, .. }     => span,
            Expr::Ternary { span, .. }  => span,
            Expr::Cast { span, .. }     => span,
            Expr::Try { span, .. }      => span,
            Expr::Call { span, .. }     => span,
            Expr::Index { span, .. }    => span,
            Expr::Field { span, .. }    => span,
            Expr::MethodCall { span, .. } => span,
            Expr::Transform { span, .. } => span,
            Expr::List(_, s)        => s,
            Expr::Lambda { span, .. } => span,
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnOp {
    Neg,
    Not,
}

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Float,
    Bool,
    Unit,
    Array(Box<Type>, usize),
    List(Box<Type>),
    Res(Box<Type>),
    Fn(Vec<Type>, Option<Box<Type>>),
    /// Any named type — built-in (`vec2`, `color`, `shape`) or user-defined (`State`, `Input`, future structs).
    Named(String),
}
