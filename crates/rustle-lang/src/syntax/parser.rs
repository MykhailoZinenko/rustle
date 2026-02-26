use crate::syntax::ast::*;
use crate::error::{Error, ErrorCode};
use crate::syntax::token::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(mut self) -> Result<Program, Vec<Error>> {
        let mut errors = Vec::new();
        let mut imports = Vec::new();
        let mut state = None;
        let mut items = Vec::new();

        while !self.is_at_end() {
            let pos_before = self.pos;

            match self.peek_kind() {
                TokenKind::Import => match self.parse_import() {
                    Ok(i) => imports.push(i),
                    Err(e) => { errors.push(e); self.recover(); }
                },
                TokenKind::State => match self.parse_state_block() {
                    Ok(s) => state = Some(s),
                    Err(e) => { errors.push(e); self.recover(); }
                },
                TokenKind::Fn => match self.parse_fn_item() {
                    Ok(item) => items.push(item),
                    Err(e) => { errors.push(e); self.recover(); }
                },
                TokenKind::Eof => break,
                _ => match self.parse_stmt() {
                    Ok(s) => items.push(Item::Stmt(s)),
                    Err(e) => { errors.push(e); self.recover(); }
                },
            }

            // guarantee progress — if nothing was consumed, force-advance
            // to prevent an infinite loop on unrecognised tokens
            if self.pos == pos_before {
                self.advance();
            }
        }

        if errors.is_empty() {
            Ok(Program { imports, state, items })
        } else {
            Err(errors)
        }
    }

    // ─── Imports ─────────────────────────────────────────────────────────────

    fn parse_import(&mut self) -> Result<ImportDecl, Error> {
        let span = self.span();
        self.expect(TokenKind::Import)?;
        let namespace = self.expect_ident()?;
        let members = if self.check(TokenKind::LBrace) {
            self.advance();
            let mut members = Vec::new();
            while !self.check(TokenKind::RBrace) && !self.is_at_end() {
                members.push(self.expect_ident()?);
                if !self.matches(TokenKind::Comma) { break; }
            }
            self.expect(TokenKind::RBrace)?;
            members
        } else {
            Vec::new()
        };
        Ok(ImportDecl { namespace, members, span })
    }

    // ─── State block ─────────────────────────────────────────────────────────

    fn parse_state_block(&mut self) -> Result<StateBlock, Error> {
        let span = self.span();
        self.expect(TokenKind::State)?;
        self.expect(TokenKind::LBrace)?;
        let mut fields = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            fields.push(self.parse_state_field()?);
        }
        self.expect(TokenKind::RBrace)?;
        Ok(StateBlock { fields, span })
    }

    fn parse_state_field(&mut self) -> Result<StateField, Error> {
        let span = self.span();
        self.expect(TokenKind::Let)?;
        let name = self.expect_ident()?;
        let ty = if self.matches(TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(TokenKind::Eq)?;
        let initializer = self.parse_expr()?;
        Ok(StateField { name, ty, initializer, span })
    }

    // ─── Function definition / variable ──────────────────────────────────────

    /// Handles both `fn name(params) -> T { }` and `fn name = expr`.
    fn parse_fn_item(&mut self) -> Result<Item, Error> {
        let span = self.span();
        self.expect(TokenKind::Fn)?;
        let name = self.expect_ident()?;
        if self.check(TokenKind::LParen) {
            // fn name(params) -> T { body }
            self.advance();
            let params = self.parse_param_list()?;
            self.expect(TokenKind::RParen)?;
            let return_ty = if self.matches(TokenKind::Arrow) { Some(self.parse_type()?) } else { None };
            let body = self.parse_block()?;
            Ok(Item::FnDef(FnDef { name, params, return_ty, body, span }))
        } else {
            // fn name = expr
            self.expect(TokenKind::Eq)?;
            let value = self.parse_expr()?;
            Ok(Item::Stmt(Stmt::FnVar { name, value, span }))
        }
    }

    /// `fn name = expr` inside a block.
    fn parse_fn_var_stmt(&mut self) -> Result<Stmt, Error> {
        let span = self.span();
        self.expect(TokenKind::Fn)?;
        let name = self.expect_ident()?;
        self.expect(TokenKind::Eq)?;
        let value = self.parse_expr()?;
        Ok(Stmt::FnVar { name, value, span })
    }

    fn parse_param_list(&mut self) -> Result<Vec<Param>, Error> {
        let mut params = Vec::new();
        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            let span = self.span();
            let name = self.expect_ident()?;
            self.expect(TokenKind::Colon)?;
            let ty = self.parse_type()?;
            params.push(Param { name, ty, span });
            if !self.matches(TokenKind::Comma) { break; }
        }
        Ok(params)
    }

    // ─── Statements ──────────────────────────────────────────────────────────

    fn parse_block(&mut self) -> Result<Vec<Stmt>, Error> {
        self.expect(TokenKind::LBrace)?;
        let mut stmts = Vec::new();
        while !self.check(TokenKind::RBrace) && !self.is_at_end() {
            stmts.push(self.parse_stmt()?);
        }
        self.expect(TokenKind::RBrace)?;
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, Error> {
        match self.peek_kind() {
            TokenKind::Let   => self.parse_var_decl(false),
            TokenKind::Const => self.parse_var_decl(true),
            TokenKind::Fn    => self.parse_fn_var_stmt(),
            TokenKind::If    => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::For   => self.parse_for(),
            TokenKind::Foreach => self.parse_foreach(),
            TokenKind::Return  => self.parse_return(),
            TokenKind::Out     => self.parse_out(),

            // ident (`.ident`)* `=` → assignment; anything else → expr stmt
            TokenKind::Ident(_) => {
                if self.is_path_assign() {
                    self.parse_assign()
                } else {
                    Ok(Stmt::Expr(self.parse_expr()?))
                }
            }

            _ => Ok(Stmt::Expr(self.parse_expr()?)),
        }
    }

    fn parse_var_decl(&mut self, is_const: bool) -> Result<Stmt, Error> {
        let span = self.span();
        self.advance(); // consume `let` or `const`
        let name = self.expect_ident()?;
        let ty = if self.matches(TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(TokenKind::Eq)?;
        let initializer = self.parse_expr()?;
        Ok(Stmt::VarDecl(VarDecl { name, ty, is_const, initializer, span }))
    }

    fn parse_assign(&mut self) -> Result<Stmt, Error> {
        let span = self.span();
        let mut target = vec![self.expect_ident()?];
        while self.matches(TokenKind::Dot) {
            target.push(self.expect_ident()?);
        }
        self.expect(TokenKind::Eq)?;
        let value = self.parse_expr()?;
        Ok(Stmt::Assign(Assign { target, value, span }))
    }

    fn parse_out(&mut self) -> Result<Stmt, Error> {
        let span = self.span();
        self.expect(TokenKind::Out)?;
        let mut shapes = Vec::new();
        self.expect(TokenKind::LtLt)?;
        shapes.push(self.parse_expr()?);
        while self.matches(TokenKind::LtLt) {
            shapes.push(self.parse_expr()?);
        }
        Ok(Stmt::Out(OutStmt { shapes, span }))
    }

    fn parse_if(&mut self) -> Result<Stmt, Error> {
        let span = self.span();
        self.expect(TokenKind::If)?;
        let condition = self.parse_expr()?;
        let then_block = self.parse_block()?;
        let else_block = if self.matches(TokenKind::Else) {
            Some(self.parse_block()?)
        } else {
            None
        };
        Ok(Stmt::If(IfStmt { condition, then_block, else_block, span }))
    }

    fn parse_while(&mut self) -> Result<Stmt, Error> {
        let span = self.span();
        self.expect(TokenKind::While)?;
        let condition = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::While(WhileStmt { condition, body, span }))
    }

    fn parse_for(&mut self) -> Result<Stmt, Error> {
        let span = self.span();
        self.expect(TokenKind::For)?;
        let init = Box::new(self.parse_var_decl(false)?);
        self.expect(TokenKind::Semicolon)?;
        let condition = self.parse_expr()?;
        self.expect(TokenKind::Semicolon)?;
        let step = Box::new(self.parse_assign()?);
        let body = self.parse_block()?;
        Ok(Stmt::For(ForStmt { init, condition, step, body, span }))
    }

    fn parse_foreach(&mut self) -> Result<Stmt, Error> {
        let span = self.span();
        self.expect(TokenKind::Foreach)?;
        let var_name = self.expect_ident()?;
        let var_ty = if self.matches(TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(TokenKind::In)?;
        let iterable = self.parse_expr()?;
        let body = self.parse_block()?;
        Ok(Stmt::Foreach(ForeachStmt { var_name, var_ty, iterable, body, span }))
    }

    fn parse_return(&mut self) -> Result<Stmt, Error> {
        let span = self.span();
        self.expect(TokenKind::Return)?;
        let value = if self.check(TokenKind::RBrace) || self.is_at_end() {
            None
        } else {
            Some(self.parse_expr()?)
        };
        Ok(Stmt::Return(value, span))
    }

    // ─── Expressions (precedence climbing) ───────────────────────────────────

    fn parse_expr(&mut self) -> Result<Expr, Error> {
        if self.check(TokenKind::Try) {
            let span = self.span();
            self.advance();
            let expr = self.parse_expr()?;
            return Ok(Expr::Try { expr: Box::new(expr), span });
        }
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> Result<Expr, Error> {
        let expr = self.parse_or()?;
        if self.matches(TokenKind::Question) {
            let span = expr.span().clone();
            let then_expr = self.parse_or()?;
            self.expect(TokenKind::Colon)?;
            let else_expr = self.parse_or()?;
            return Ok(Expr::Ternary {
                condition: Box::new(expr),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
                span,
            });
        }
        Ok(expr)
    }

    fn parse_or(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_and()?;
        while self.check(TokenKind::Or) {
            let span = left.span().clone();
            self.advance();
            let right = self.parse_and()?;
            left = Expr::BinOp { left: Box::new(left), op: BinOp::Or, right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_equality()?;
        while self.check(TokenKind::And) {
            let span = left.span().clone();
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::BinOp { left: Box::new(left), op: BinOp::And, right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_comparison()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::EqEq   => BinOp::Eq,
                TokenKind::BangEq => BinOp::NotEq,
                _ => break,
            };
            let span = left.span().clone();
            self.advance();
            let right = self.parse_comparison()?;
            left = Expr::BinOp { left: Box::new(left), op, right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_addition()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::Lt   => BinOp::Lt,
                TokenKind::LtEq => BinOp::LtEq,
                TokenKind::Gt   => BinOp::Gt,
                TokenKind::GtEq => BinOp::GtEq,
                _ => break,
            };
            let span = left.span().clone();
            self.advance();
            let right = self.parse_addition()?;
            left = Expr::BinOp { left: Box::new(left), op, right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_addition(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_multiplication()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::Plus  => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            let span = left.span().clone();
            self.advance();
            let right = self.parse_multiplication()?;
            left = Expr::BinOp { left: Box::new(left), op, right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_multiplication(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_unary()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::Star    => BinOp::Mul,
                TokenKind::Slash   => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            let span = left.span().clone();
            self.advance();
            let right = self.parse_unary()?;
            left = Expr::BinOp { left: Box::new(left), op, right: Box::new(right), span };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, Error> {
        let span = self.span();
        if self.matches(TokenKind::Minus) {
            let operand = self.parse_unary()?;
            return Ok(Expr::UnOp { op: UnOp::Neg, operand: Box::new(operand), span });
        }
        if self.matches(TokenKind::Not) {
            let operand = self.parse_unary()?;
            return Ok(Expr::UnOp { op: UnOp::Not, operand: Box::new(operand), span });
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, Error> {
        let mut expr = self.parse_primary()?;

        loop {
            match self.peek_kind() {
                // field access or method call: expr.name or expr.name(args)
                TokenKind::Dot => {
                    let span = expr.span().clone();
                    self.advance();
                    // Allow keyword `in` as a method name (used by shape.in()).
                    let field = if self.check(TokenKind::In) {
                        self.advance();
                        "in".to_string()
                    } else {
                        self.expect_ident()?
                    };
                    if self.check(TokenKind::LParen) {
                        self.advance();
                        let (args, named_args) = self.parse_mixed_arg_list()?;
                        self.expect(TokenKind::RParen)?;
                        expr = Expr::MethodCall { expr: Box::new(expr), method: field, args, named_args, span };
                    } else {
                        expr = Expr::Field { expr: Box::new(expr), field, span };
                    }
                }

                // index: expr[i]
                TokenKind::LBracket => {
                    let span = expr.span().clone();
                    self.advance();
                    let index = self.parse_expr()?;
                    self.expect(TokenKind::RBracket)?;
                    expr = Expr::Index { expr: Box::new(expr), index: Box::new(index), span };
                }

                // transform: expr@t or expr@(t1, t2, t3)
                TokenKind::At => {
                    let span = expr.span().clone();
                    self.advance();
                    let transforms = if self.check(TokenKind::LParen) {
                        self.advance(); // consume (
                        let mut ts = Vec::new();
                        while !self.check(TokenKind::RParen) && !self.is_at_end() {
                            ts.push(self.parse_expr()?);
                            if !self.matches(TokenKind::Comma) { break; }
                        }
                        self.expect(TokenKind::RParen)?;
                        ts
                    } else {
                        vec![self.parse_postfix()?]
                    };
                    expr = Expr::Transform { expr: Box::new(expr), transforms, span };
                }

                // cast: expr as Type
                TokenKind::As => {
                    let span = expr.span().clone();
                    self.advance();
                    let ty = self.parse_type()?;
                    expr = Expr::Cast { expr: Box::new(expr), ty, span };
                }

                _ => break,
            }
        }

        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Error> {
        let tok = self.peek().clone();
        let span = Span::new(tok.line, tok.column);

        match tok.kind {
            TokenKind::Float(v) => { self.advance(); Ok(Expr::Float(v, span)) }
            TokenKind::Bool(v)  => { self.advance(); Ok(Expr::Bool(v, span)) }
            TokenKind::StringLit(s) => { self.advance(); Ok(Expr::StringLit(s, span)) }
            TokenKind::HexColor(s)  => { self.advance(); Ok(Expr::HexColor(s, span)) }

            // lambda or grouped expression
            TokenKind::LParen => {
                if self.is_lambda_start() {
                    self.parse_lambda()
                } else {
                    self.advance();
                    let expr = self.parse_expr()?;
                    self.expect(TokenKind::RParen)?;
                    Ok(expr)
                }
            }

            // list literal
            TokenKind::LBracket => {
                self.advance();
                let mut items = Vec::new();
                while !self.check(TokenKind::RBracket) && !self.is_at_end() {
                    items.push(self.parse_expr()?);
                    if !self.matches(TokenKind::Comma) { break; }
                }
                self.expect(TokenKind::RBracket)?;
                Ok(Expr::List(items, span))
            }

            // lambda: (a: float) -> float { ... }
            // only if next token after `(` looks like a param (ident : type)
            // we let parse_call_or_ident handle ident(), so just handle ( here for lambdas
            // A lambda starts with ( ident : ...
            // We peek ahead: if we see `(` then ident then `:`, it's a lambda

            // identifier: either a call or a plain ident
            TokenKind::Ident(_) => self.parse_call_or_ident(),

            _ => Err(self.unexpected("expression")),
        }
    }

    fn parse_call_or_ident(&mut self) -> Result<Expr, Error> {
        let tok = self.advance();
        let span = Span::new(tok.line, tok.column);
        let name = match tok.kind {
            TokenKind::Ident(s) => s,
            _ => return Err(self.unexpected("identifier")),
        };

        if self.check(TokenKind::LParen) {
            self.advance();
            let (args, named_args) = self.parse_mixed_arg_list()?;
            self.expect(TokenKind::RParen)?;
            Ok(Expr::Call { callee: name, args, named_args, span })
        } else {
            Ok(Expr::Ident(name, span))
        }
    }

    // ─── Argument lists ──────────────────────────────────────────────────────

    /// Parse a plain positional arg list (no named args).
    fn parse_arg_list(&mut self) -> Result<Vec<Expr>, Error> {
        let mut args = Vec::new();
        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            args.push(self.parse_expr()?);
            if !self.matches(TokenKind::Comma) { break; }
        }
        Ok(args)
    }

    /// Parse positional + named args: `circle(x, y, radius, render: sdf)`.
    /// Named args must come after positional args.
    fn parse_mixed_arg_list(&mut self) -> Result<(Vec<Expr>, Vec<(String, Expr)>), Error> {
        let mut args = Vec::new();
        let mut named = Vec::new();

        while !self.check(TokenKind::RParen) && !self.is_at_end() {
            // named arg: ident `:` expr
            if let TokenKind::Ident(_) = self.peek_kind() {
                if self.peek_next_is(TokenKind::Colon) {
                    let name = self.expect_ident()?;
                    self.expect(TokenKind::Colon)?;
                    let val = self.parse_expr()?;
                    named.push((name, val));
                    if !self.matches(TokenKind::Comma) { break; }
                    continue;
                }
            }
            args.push(self.parse_expr()?);
            if !self.matches(TokenKind::Comma) { break; }
        }
        Ok((args, named))
    }

    // ─── Types ───────────────────────────────────────────────────────────────

    fn parse_type(&mut self) -> Result<Type, Error> {
        let tok = self.advance();
        match tok.kind {
            TokenKind::TFloat => Ok(Type::Float),
            TokenKind::TBool  => Ok(Type::Bool),

            // res<T>
            TokenKind::TRes => {
                self.expect(TokenKind::Lt)?;
                let inner = self.parse_type()?;
                self.expect(TokenKind::Gt)?;
                Ok(Type::Res(Box::new(inner)))
            }

            // list[T]
            TokenKind::TList => {
                self.expect(TokenKind::LBracket)?;
                let inner = self.parse_type()?;
                self.expect(TokenKind::RBracket)?;
                Ok(Type::List(Box::new(inner)))
            }

            // array[T, N]
            TokenKind::TArray => {
                self.expect(TokenKind::LBracket)?;
                let inner = self.parse_type()?;
                self.expect(TokenKind::Comma)?;
                let size = match self.advance().kind {
                    TokenKind::Float(n) => n as usize,
                    _ => return Err(self.error_at(&tok, "expected array size")),
                };
                self.expect(TokenKind::RBracket)?;
                Ok(Type::Array(Box::new(inner), size))
            }

            // fn(T, T) -> T
            TokenKind::Fn => {
                self.expect(TokenKind::LParen)?;
                let mut params = Vec::new();
                while !self.check(TokenKind::RParen) && !self.is_at_end() {
                    params.push(self.parse_type()?);
                    if !self.matches(TokenKind::Comma) { break; }
                }
                self.expect(TokenKind::RParen)?;
                let ret = if self.matches(TokenKind::Arrow) {
                    Some(Box::new(self.parse_type()?))
                } else {
                    None
                };
                Ok(Type::Fn(params, ret))
            }

            TokenKind::Ident(name) => Ok(Type::Named(name)),

            _ => Err(self.error_at(&tok, "expected type")),
        }
    }

    // ─── Token primitives ────────────────────────────────────────────────────

    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek_kind(&self) -> TokenKind {
        self.tokens[self.pos].kind.clone()
    }

    fn peek_next_is(&self, kind: TokenKind) -> bool {
        if self.pos + 1 < self.tokens.len() {
            self.tokens[self.pos + 1].kind == kind
        } else {
            false
        }
    }

    /// Returns true when the current position starts a dotted-path assignment:
    /// `ident (.ident)* =`
    fn is_path_assign(&self) -> bool {
        let mut i = self.pos;
        // must start with an ident
        if !matches!(self.tokens[i].kind, TokenKind::Ident(_)) { return false; }
        i += 1;
        // skip zero or more `.ident` segments
        while i + 1 < self.tokens.len()
            && self.tokens[i].kind == TokenKind::Dot
            && matches!(self.tokens[i + 1].kind, TokenKind::Ident(_))
        {
            i += 2;
        }
        // must be followed by `=` (not `==`)
        i < self.tokens.len() && self.tokens[i].kind == TokenKind::Eq
    }

    fn advance(&mut self) -> Token {
        let tok = self.tokens[self.pos].clone();
        if self.pos + 1 < self.tokens.len() { self.pos += 1; }
        tok
    }

    fn check(&self, kind: TokenKind) -> bool {
        self.peek_kind() == kind
    }

    fn matches(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) { self.advance(); true } else { false }
    }

    fn expect(&mut self, kind: TokenKind) -> Result<Token, Error> {
        if self.check(kind.clone()) {
            Ok(self.advance())
        } else {
            let tok = self.peek();
            Err(Error::new(
                ErrorCode::P002,
                tok.line,
                tok.column,
                format!("expected {:?}, found {:?}", kind, tok.kind),
            ))
        }
    }

    fn expect_ident(&mut self) -> Result<String, Error> {
        let tok = self.advance();
        match tok.kind {
            TokenKind::Ident(s) => Ok(s),
            _ => Err(self.error_at(&tok, "expected identifier")),
        }
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek_kind(), TokenKind::Eof)
    }

    fn span(&self) -> Span {
        let tok = self.peek();
        Span::new(tok.line, tok.column)
    }

    fn unexpected(&self, expected: &str) -> Error {
        let tok = self.peek();
        Error::new(
            ErrorCode::P001,
            tok.line,
            tok.column,
            format!("expected {}, found {:?}", expected, tok.kind),
        )
    }

    fn error_at(&self, tok: &Token, msg: &str) -> Error {
        Error::new(ErrorCode::P001, tok.line, tok.column, msg)
    }

    /// Returns true if the current `(` starts a lambda expression.
    /// Lambda patterns: `() ->` or `(ident :`
    fn is_lambda_start(&self) -> bool {
        let next = self.pos + 1;
        if next >= self.tokens.len() { return false; }
        match &self.tokens[next].kind {
            // () -> type { }
            TokenKind::RParen => {
                let after = next + 1;
                after < self.tokens.len() && matches!(self.tokens[after].kind, TokenKind::Arrow)
            }
            // (ident: type, ...)
            TokenKind::Ident(_) => {
                let after = next + 1;
                after < self.tokens.len() && matches!(self.tokens[after].kind, TokenKind::Colon)
            }
            _ => false,
        }
    }

    fn parse_lambda(&mut self) -> Result<Expr, Error> {
        let span = self.span();
        self.expect(TokenKind::LParen)?;
        let params = self.parse_param_list()?;
        self.expect(TokenKind::RParen)?;
        let return_ty = if self.matches(TokenKind::Arrow) { Some(self.parse_type()?) } else { None };
        let body = self.parse_block()?;
        Ok(Expr::Lambda { params, return_ty, body, span })
    }

    /// Skip tokens until we find something that looks like a new statement.
    /// Used after a parse error to attempt recovery.
    fn recover(&mut self) {
        loop {
            match self.peek_kind() {
                TokenKind::Eof
                | TokenKind::Let
                | TokenKind::Const
                | TokenKind::Fn
                | TokenKind::If
                | TokenKind::While
                | TokenKind::For
                | TokenKind::Foreach
                | TokenKind::Return
                | TokenKind::State
                | TokenKind::Import
                | TokenKind::RBrace => break,
                _ => { self.advance(); }
            }
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::lexer::Lexer;

    fn parse(src: &str) -> Program {
        let tokens = Lexer::new(src).tokenize().expect("lex failed");
        Parser::new(tokens).parse().expect("parse failed")
    }

    fn parse_expr_src(src: &str) -> Expr {
        let tokens = Lexer::new(src).tokenize().expect("lex failed");
        let mut p = Parser::new(tokens);
        p.parse_expr().expect("parse_expr failed")
    }

    fn parse_err(src: &str) -> Vec<Error> {
        let tokens = Lexer::new(src).tokenize().expect("lex failed");
        Parser::new(tokens).parse().expect_err("expected parse error")
    }

    // ── imports ──────────────────────────────────────────────────────────────

    #[test]
    fn import_whole_namespace() {
        let p = parse("import render");
        assert_eq!(p.imports.len(), 1);
        assert_eq!(p.imports[0].namespace, "render");
        assert!(p.imports[0].members.is_empty());
    }

    #[test]
    fn import_named_members() {
        let p = parse("import shapes { circle, rect }");
        assert_eq!(p.imports[0].members, vec!["circle", "rect"]);
    }

    // ── state block ──────────────────────────────────────────────────────────

    #[test]
    fn state_block() {
        let p = parse("state { let t: float = 0.0 let flag: bool = true }");
        let s = p.state.unwrap();
        assert_eq!(s.fields.len(), 2);
        assert_eq!(s.fields[0].name, "t");
        assert_eq!(s.fields[0].ty, Some(Type::Float));
        assert_eq!(s.fields[1].name, "flag");
        assert_eq!(s.fields[1].ty, Some(Type::Bool));
    }

    #[test]
    fn state_block_inferred_type() {
        let p = parse("state { let x = 0.0 }");
        let s = p.state.unwrap();
        assert_eq!(s.fields[0].name, "x");
        assert!(s.fields[0].ty.is_none());
    }

    // ── variable declarations ─────────────────────────────────────────────────

    #[test]
    fn var_decl_with_init() {
        let p = parse("let x: float = 3.14");
        match &p.items[0] {
            Item::Stmt(Stmt::VarDecl(v)) => {
                assert_eq!(v.name, "x");
                assert_eq!(v.ty, Some(Type::Float));
                assert!(!v.is_const);
            }
            _ => panic!("expected VarDecl"),
        }
    }

    #[test]
    fn var_decl_inferred_type() {
        let p = parse("let x = 3.14");
        match &p.items[0] {
            Item::Stmt(Stmt::VarDecl(v)) => {
                assert_eq!(v.name, "x");
                assert!(v.ty.is_none());
                assert!(matches!(v.initializer, Expr::Float(f, _) if f == 3.14));
            }
            _ => panic!("expected VarDecl"),
        }
    }

    #[test]
    fn const_var_decl() {
        let p = parse("const PI: float = 3.14159");
        match &p.items[0] {
            Item::Stmt(Stmt::VarDecl(v)) => {
                assert!(v.is_const);
                assert_eq!(v.ty, Some(Type::Float));
            }
            _ => panic!("expected VarDecl"),
        }
    }

    #[test]
    fn var_decl_no_init_is_error() {
        // all declarations must have an initializer
        let errs = parse_err("let y");
        assert!(!errs.is_empty());
    }

    // ── assignment ────────────────────────────────────────────────────────────

    #[test]
    fn assignment() {
        let p = parse("let x = 0.0\nx = 1.0");
        match &p.items[1] {
            Item::Stmt(Stmt::Assign(a)) => assert_eq!(a.target, vec!["x"]),
            _ => panic!("expected Assign"),
        }
    }

    // ── function definition ───────────────────────────────────────────────────

    #[test]
    fn fn_def_with_return() {
        let p = parse("fn add(a: float, b: float) -> float { return a + b }");
        match &p.items[0] {
            Item::FnDef(f) => {
                assert_eq!(f.name, "add");
                assert_eq!(f.params.len(), 2);
                assert_eq!(f.return_ty, Some(Type::Float));
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn fn_def_void() {
        let p = parse("fn draw() { }");
        match &p.items[0] {
            Item::FnDef(f) => assert!(f.return_ty.is_none()),
            _ => panic!("expected FnDef"),
        }
    }

    // ── out statement ─────────────────────────────────────────────────────────

    #[test]
    fn out_single() {
        let p = parse("out << s");
        match &p.items[0] {
            Item::Stmt(Stmt::Out(o)) => assert_eq!(o.shapes.len(), 1),
            _ => panic!("expected Out"),
        }
    }

    #[test]
    fn out_chained() {
        let p = parse("out << bg << s1 << s2");
        match &p.items[0] {
            Item::Stmt(Stmt::Out(o)) => assert_eq!(o.shapes.len(), 3),
            _ => panic!("expected Out"),
        }
    }

    // ── control flow ──────────────────────────────────────────────────────────

    #[test]
    fn if_else() {
        let p = parse("if x > 0.0 { out << a } else { out << b }");
        match &p.items[0] {
            Item::Stmt(Stmt::If(i)) => assert!(i.else_block.is_some()),
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn while_loop() {
        let p = parse("while i < 10.0 { i = i + 1.0 }");
        match &p.items[0] {
            Item::Stmt(Stmt::While(_)) => {}
            _ => panic!("expected While"),
        }
    }

    #[test]
    fn for_loop() {
        let p = parse("for let i = 0.0; i < 10.0; i = i + 1.0 { }");
        match &p.items[0] {
            Item::Stmt(Stmt::For(_)) => {}
            _ => panic!("expected For"),
        }
    }

    #[test]
    fn foreach_loop_inferred() {
        let p = parse("foreach v in values { out << v }");
        match &p.items[0] {
            Item::Stmt(Stmt::Foreach(f)) => {
                assert_eq!(f.var_name, "v");
                assert!(f.var_ty.is_none());
            }
            _ => panic!("expected Foreach"),
        }
    }

    #[test]
    fn foreach_loop_explicit_type() {
        let p = parse("foreach v: float in values { out << v }");
        match &p.items[0] {
            Item::Stmt(Stmt::Foreach(f)) => {
                assert_eq!(f.var_name, "v");
                assert_eq!(f.var_ty, Some(Type::Float));
            }
            _ => panic!("expected Foreach"),
        }
    }

    // ── expressions ───────────────────────────────────────────────────────────

    #[test]
    fn binary_precedence() {
        // 2.0 + 3.0 * 4.0 should parse as 2.0 + (3.0 * 4.0)
        let expr = parse_expr_src("2.0 + 3.0 * 4.0");
        match expr {
            Expr::BinOp { op: BinOp::Add, right, .. } => {
                matches!(*right, Expr::BinOp { op: BinOp::Mul, .. });
            }
            _ => panic!("expected Add at top level"),
        }
    }

    #[test]
    fn ternary_expr() {
        let expr = parse_expr_src("x > 0.0 ? 1.0 : 0.0");
        assert!(matches!(expr, Expr::Ternary { .. }));
    }

    #[test]
    fn call_with_named_arg() {
        let expr = parse_expr_src("circle(vec2(0.0, 0.0), 0.2, render: sdf)");
        match expr {
            Expr::Call { callee, named_args, .. } => {
                assert_eq!(callee, "circle");
                assert_eq!(named_args[0].0, "render");
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn field_access() {
        let expr = parse_expr_src("result.ok");
        assert!(matches!(expr, Expr::Field { .. }));
    }

    #[test]
    fn method_call() {
        let expr = parse_expr_src("t.move(0.5, 0.5)");
        assert!(matches!(expr, Expr::MethodCall { .. }));
    }

    #[test]
    fn transform_operator() {
        let expr = parse_expr_src("s@t");
        assert!(matches!(expr, Expr::Transform { .. }));
    }

    #[test]
    fn list_literal() {
        let expr = parse_expr_src("[1.0, 2.0, 3.0]");
        match expr {
            Expr::List(items, _) => assert_eq!(items.len(), 3),
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn type_res() {
        let p = parse("let x: res<float> = try 1.0");
        match &p.items[0] {
            Item::Stmt(Stmt::VarDecl(v)) => {
                assert_eq!(v.ty, Some(Type::Res(Box::new(Type::Float))));
            }
            _ => panic!("expected VarDecl"),
        }
    }

    #[test]
    fn type_list() {
        let p = parse("let xs: list[float] = [1.0]");
        match &p.items[0] {
            Item::Stmt(Stmt::VarDecl(v)) => {
                assert_eq!(v.ty, Some(Type::List(Box::new(Type::Float))));
            }
            _ => panic!("expected VarDecl"),
        }
    }

    #[test]
    fn named_type_in_fn() {
        let p = parse("fn update(s: State, i: Input) -> State { return s }");
        match &p.items[0] {
            Item::FnDef(f) => {
                assert_eq!(f.params[0].ty, Type::Named("State".to_string()));
                assert_eq!(f.params[1].ty, Type::Named("Input".to_string()));
                assert_eq!(f.return_ty, Some(Type::Named("State".to_string())));
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn builtin_types_are_named() {
        let p = parse("fn f(p: vec2, c: color) -> shape { return p }");
        match &p.items[0] {
            Item::FnDef(f) => {
                assert_eq!(f.params[0].ty, Type::Named("vec2".to_string()));
                assert_eq!(f.params[1].ty, Type::Named("color".to_string()));
                assert_eq!(f.return_ty, Some(Type::Named("shape".to_string())));
            }
            _ => panic!("expected FnDef"),
        }
    }

    // ── fn var and lambda ─────────────────────────────────────────────────────

    #[test]
    fn fn_var_assign() {
        let p = parse("fn f = add");
        match &p.items[0] {
            Item::Stmt(Stmt::FnVar { name, .. }) => assert_eq!(name, "f"),
            _ => panic!("expected FnVar"),
        }
    }

    #[test]
    fn fn_var_lambda() {
        let p = parse("fn g = (a: float, b: float) -> float { return a + b }");
        match &p.items[0] {
            Item::Stmt(Stmt::FnVar { name, value, .. }) => {
                assert_eq!(name, "g");
                assert!(matches!(value, Expr::Lambda { .. }));
            }
            _ => panic!("expected FnVar with lambda"),
        }
    }

    #[test]
    fn lambda_empty_params() {
        let expr = parse_expr_src("() -> float { return 1.0 }");
        assert!(matches!(expr, Expr::Lambda { .. }));
    }

    #[test]
    fn grouped_expr_not_confused_with_lambda() {
        // (x + 1.0) should NOT be parsed as a lambda
        let expr = parse_expr_src("(x + 1.0)");
        assert!(matches!(expr, Expr::BinOp { .. }));
    }

    #[test]
    fn transform_single() {
        let expr = parse_expr_src("s@t");
        match expr {
            Expr::Transform { transforms, .. } => assert_eq!(transforms.len(), 1),
            _ => panic!("expected Transform"),
        }
    }

    #[test]
    fn transform_multi() {
        // s@(t1, t2, t3) → one flat Transform node with three transforms
        let expr = parse_expr_src("s@(t1, t2, t3)");
        match expr {
            Expr::Transform { transforms, .. } => assert_eq!(transforms.len(), 3),
            _ => panic!("expected Transform"),
        }
    }

    #[test]
    fn transform_in_call_no_ambiguity() {
        // circle(s@t, 0.2) — the comma belongs to circle's args, not the transform
        let expr = parse_expr_src("circle(s@t, 0.2)");
        match expr {
            Expr::Call { args, .. } => assert_eq!(args.len(), 2),
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn try_wraps_full_expr() {
        // try 1.0 / 0.0 must parse as Try(BinOp(1.0 / 0.0)), not BinOp(Try(1.0), 0.0)
        let expr = parse_expr_src("try 1.0 / 0.0");
        match expr {
            Expr::Try { expr, .. } => assert!(matches!(*expr, Expr::BinOp { op: BinOp::Div, .. })),
            _ => panic!("expected Try at top level"),
        }
    }

    // ── error recovery ────────────────────────────────────────────────────────

    #[test]
    fn missing_brace_is_error() {
        let errs = parse_err("fn f() { ");
        assert!(!errs.is_empty());
    }

    #[test]
    fn partial_state_block_no_hang() {
        // was causing infinite loop before progress-guard fix
        let errs = parse_err("state { f }");
        assert!(!errs.is_empty());
    }

    // ── simple: literals ─────────────────────────────────────────────────────

    #[test]
    fn float_literal_expr() {
        let expr = parse_expr_src("42.0");
        assert!(matches!(expr, Expr::Float(42.0, _)));
    }

    #[test]
    fn bool_literal_true() {
        let expr = parse_expr_src("true");
        assert!(matches!(expr, Expr::Bool(true, _)));
    }

    #[test]
    fn bool_literal_false() {
        let expr = parse_expr_src("false");
        assert!(matches!(expr, Expr::Bool(false, _)));
    }

    #[test]
    fn string_literal_expr() {
        let expr = parse_expr_src("\"hello world\"");
        match expr {
            Expr::StringLit(s, _) => assert_eq!(s, "hello world"),
            _ => panic!("expected StringLit"),
        }
    }

    #[test]
    fn hex_color_expr() {
        let expr = parse_expr_src("#ff0000");
        match expr {
            Expr::HexColor(s, _) => assert_eq!(s, "ff0000"),
            _ => panic!("expected HexColor"),
        }
    }

    #[test]
    fn hex_color_with_alpha() {
        let expr = parse_expr_src("#ff000080");
        match expr {
            Expr::HexColor(s, _) => assert_eq!(s, "ff000080"),
            _ => panic!("expected HexColor with alpha"),
        }
    }

    // ── simple: unary operators ───────────────────────────────────────────────

    #[test]
    fn unary_neg() {
        let expr = parse_expr_src("-x");
        assert!(matches!(expr, Expr::UnOp { op: UnOp::Neg, .. }));
    }

    #[test]
    fn unary_not() {
        let expr = parse_expr_src("not x");
        assert!(matches!(expr, Expr::UnOp { op: UnOp::Not, .. }));
    }

    #[test]
    fn unary_double_neg() {
        // --x parses as Neg(Neg(x))
        let expr = parse_expr_src("- -x");
        match expr {
            Expr::UnOp { op: UnOp::Neg, operand, .. } => {
                assert!(matches!(*operand, Expr::UnOp { op: UnOp::Neg, .. }));
            }
            _ => panic!("expected nested Neg"),
        }
    }

    // ── simple: binary operators ──────────────────────────────────────────────

    #[test]
    fn all_arithmetic_ops() {
        assert!(matches!(parse_expr_src("a + b"), Expr::BinOp { op: BinOp::Add, .. }));
        assert!(matches!(parse_expr_src("a - b"), Expr::BinOp { op: BinOp::Sub, .. }));
        assert!(matches!(parse_expr_src("a * b"), Expr::BinOp { op: BinOp::Mul, .. }));
        assert!(matches!(parse_expr_src("a / b"), Expr::BinOp { op: BinOp::Div, .. }));
        assert!(matches!(parse_expr_src("a % b"), Expr::BinOp { op: BinOp::Mod, .. }));
    }

    #[test]
    fn all_comparison_ops() {
        assert!(matches!(parse_expr_src("a == b"),  Expr::BinOp { op: BinOp::Eq, .. }));
        assert!(matches!(parse_expr_src("a != b"),  Expr::BinOp { op: BinOp::NotEq, .. }));
        assert!(matches!(parse_expr_src("a < b"),   Expr::BinOp { op: BinOp::Lt, .. }));
        assert!(matches!(parse_expr_src("a <= b"),  Expr::BinOp { op: BinOp::LtEq, .. }));
        assert!(matches!(parse_expr_src("a > b"),   Expr::BinOp { op: BinOp::Gt, .. }));
        assert!(matches!(parse_expr_src("a >= b"),  Expr::BinOp { op: BinOp::GtEq, .. }));
    }

    #[test]
    fn logical_and_or() {
        assert!(matches!(parse_expr_src("a and b"), Expr::BinOp { op: BinOp::And, .. }));
        assert!(matches!(parse_expr_src("a or b"),  Expr::BinOp { op: BinOp::Or, .. }));
    }

    #[test]
    fn operator_precedence_mul_over_add() {
        // a + b * c → Add(a, Mul(b, c))
        let expr = parse_expr_src("a + b * c");
        match expr {
            Expr::BinOp { op: BinOp::Add, right, .. } => {
                assert!(matches!(*right, Expr::BinOp { op: BinOp::Mul, .. }));
            }
            _ => panic!("expected Add at top"),
        }
    }

    #[test]
    fn operator_precedence_compare_over_logical() {
        // a and b > c → And(a, Gt(b, c))
        let expr = parse_expr_src("a and b > c");
        match expr {
            Expr::BinOp { op: BinOp::And, right, .. } => {
                assert!(matches!(*right, Expr::BinOp { op: BinOp::Gt, .. }));
            }
            _ => panic!("expected And at top"),
        }
    }

    // ── simple: other expressions ─────────────────────────────────────────────

    #[test]
    fn index_expr() {
        let expr = parse_expr_src("xs[2]");
        match expr {
            Expr::Index { index, .. } => {
                assert!(matches!(*index, Expr::Float(2.0, _)));
            }
            _ => panic!("expected Index"),
        }
    }

    #[test]
    fn cast_expr() {
        let expr = parse_expr_src("x as float");
        match expr {
            Expr::Cast { ty, .. } => assert_eq!(ty, Type::Float),
            _ => panic!("expected Cast"),
        }
    }

    #[test]
    fn cast_to_named_type() {
        let expr = parse_expr_src("x as color");
        match expr {
            Expr::Cast { ty, .. } => assert_eq!(ty, Type::Named("color".into())),
            _ => panic!("expected Cast to Named"),
        }
    }

    #[test]
    fn empty_list() {
        let expr = parse_expr_src("[]");
        match expr {
            Expr::List(items, _) => assert!(items.is_empty()),
            _ => panic!("expected empty List"),
        }
    }

    #[test]
    fn nested_call() {
        let expr = parse_expr_src("f(g(x, y), z)");
        match expr {
            Expr::Call { callee, args, .. } => {
                assert_eq!(callee, "f");
                assert_eq!(args.len(), 2);
                assert!(matches!(args[0], Expr::Call { .. }));
            }
            _ => panic!("expected Call"),
        }
    }

    #[test]
    fn field_chain() {
        // a.b.c → Field(Field(a, b), c)
        let expr = parse_expr_src("a.b.c");
        match expr {
            Expr::Field { expr, field, .. } => {
                assert_eq!(field, "c");
                assert!(matches!(*expr, Expr::Field { .. }));
            }
            _ => panic!("expected nested Field"),
        }
    }

    #[test]
    fn method_chain() {
        // transform().move(0.5, 0.5).rotate(45.0)
        let expr = parse_expr_src("transform().move(0.5, 0.5).rotate(45.0)");
        assert!(matches!(expr, Expr::MethodCall { .. }));
    }

    // ── simple: statements ────────────────────────────────────────────────────

    #[test]
    fn return_bare() {
        let p = parse("fn f() { return }");
        match &p.items[0] {
            Item::FnDef(f) => {
                assert!(matches!(f.body[0], Stmt::Return(None, _)));
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn return_with_value() {
        let p = parse("fn f() -> float { return 1.0 }");
        match &p.items[0] {
            Item::FnDef(f) => {
                assert!(matches!(f.body[0], Stmt::Return(Some(_), _)));
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn if_no_else() {
        let p = parse("if x > 0.0 { out << s }");
        match &p.items[0] {
            Item::Stmt(Stmt::If(i)) => assert!(i.else_block.is_none()),
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn fn_def_no_params() {
        let p = parse("fn tick() -> float { return 1.0 }");
        match &p.items[0] {
            Item::FnDef(f) => {
                assert!(f.params.is_empty());
                assert_eq!(f.return_ty, Some(Type::Float));
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn const_inferred() {
        let p = parse("const PI = 3.14159");
        match &p.items[0] {
            Item::Stmt(Stmt::VarDecl(v)) => {
                assert!(v.is_const);
                assert!(v.ty.is_none());
                assert_eq!(v.name, "PI");
            }
            _ => panic!("expected const VarDecl"),
        }
    }

    #[test]
    fn var_decl_named_type() {
        let p = parse("let s: shape = circle(p, 0.2)");
        match &p.items[0] {
            Item::Stmt(Stmt::VarDecl(v)) => {
                assert_eq!(v.ty, Some(Type::Named("shape".into())));
            }
            _ => panic!("expected VarDecl"),
        }
    }

    // ── simple: types ─────────────────────────────────────────────────────────

    #[test]
    fn type_array() {
        let p = parse("let xs: array[float, 3] = [1.0, 2.0, 3.0]");
        match &p.items[0] {
            Item::Stmt(Stmt::VarDecl(v)) => {
                assert_eq!(v.ty, Some(Type::Array(Box::new(Type::Float), 3)));
            }
            _ => panic!("expected VarDecl"),
        }
    }

    #[test]
    fn type_fn_with_return() {
        let p = parse("fn apply(f: fn(float) -> float, x: float) -> float { return f(x) }");
        match &p.items[0] {
            Item::FnDef(f) => {
                assert_eq!(f.params[0].ty, Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))));
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn type_fn_no_return() {
        let p = parse("fn run(cb: fn(float)) { cb(1.0) }");
        match &p.items[0] {
            Item::FnDef(f) => {
                assert_eq!(f.params[0].ty, Type::Fn(vec![Type::Float], None));
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn type_res_named_inner() {
        let p = parse("let r: res<shape> = try make_shape()");
        match &p.items[0] {
            Item::Stmt(Stmt::VarDecl(v)) => {
                assert_eq!(v.ty, Some(Type::Res(Box::new(Type::Named("shape".into())))));
            }
            _ => panic!("expected VarDecl"),
        }
    }

    // ── simple: imports ───────────────────────────────────────────────────────

    #[test]
    fn multiple_imports() {
        let p = parse("import shapes { circle, rect }\nimport render { sdf, fill }");
        assert_eq!(p.imports.len(), 2);
        assert_eq!(p.imports[0].namespace, "shapes");
        assert_eq!(p.imports[1].namespace, "render");
        assert_eq!(p.imports[1].members, vec!["sdf", "fill"]);
    }

    #[test]
    fn import_single_member() {
        let p = parse("import coords { px }");
        assert_eq!(p.imports[0].members, vec!["px"]);
    }

    // ── complex: expressions ──────────────────────────────────────────────────

    #[test]
    fn deeply_nested_arithmetic() {
        // (a + b) * (c - d) / e
        let expr = parse_expr_src("(a + b) * (c - d) / e");
        assert!(matches!(expr, Expr::BinOp { op: BinOp::Div, .. }));
    }

    #[test]
    fn nested_ternary() {
        // a ? b : c ? d : e — right-associative ternary
        let expr = parse_expr_src("a ? b : c ? d : e");
        assert!(matches!(expr, Expr::Ternary { .. }));
    }

    #[test]
    fn try_with_ternary() {
        // try wraps the full expression including ternary
        let expr = parse_expr_src("try x > 0.0 ? x : 0.0");
        match expr {
            Expr::Try { expr, .. } => assert!(matches!(*expr, Expr::Ternary { .. })),
            _ => panic!("expected Try wrapping Ternary"),
        }
    }

    #[test]
    fn transform_multi_in_out() {
        let p = parse("out << s@(t1, t2, t3)");
        match &p.items[0] {
            Item::Stmt(Stmt::Out(o)) => {
                assert_eq!(o.shapes.len(), 1);
                match &o.shapes[0] {
                    Expr::Transform { transforms, .. } => assert_eq!(transforms.len(), 3),
                    _ => panic!("expected Transform in out"),
                }
            }
            _ => panic!("expected Out"),
        }
    }

    #[test]
    fn call_result_field_access() {
        // divide(a, b).ok — field access on call result
        let expr = parse_expr_src("divide(a, b).ok");
        match expr {
            Expr::Field { field, expr, .. } => {
                assert_eq!(field, "ok");
                assert!(matches!(*expr, Expr::Call { .. }));
            }
            _ => panic!("expected Field on Call"),
        }
    }

    #[test]
    fn index_on_call_result() {
        let expr = parse_expr_src("get_list()[0]");
        match expr {
            Expr::Index { expr, index, .. } => {
                assert!(matches!(*expr, Expr::Call { .. }));
                assert!(matches!(*index, Expr::Float(0.0, _)));
            }
            _ => panic!("expected Index on Call"),
        }
    }

    #[test]
    fn method_on_transform_result() {
        // s@t.x — . binds tighter than @
        // parsed as s@(t.x), i.e. Transform{ expr: s, transforms: [Field{t, x}] }
        let expr = parse_expr_src("s@t.x");
        match expr {
            Expr::Transform { expr, transforms, .. } => {
                assert!(matches!(*expr, Expr::Ident(ref n, _) if n == "s"));
                assert_eq!(transforms.len(), 1);
                assert!(matches!(transforms[0], Expr::Field { ref field, .. } if field == "x"));
            }
            _ => panic!("expected Transform with field RHS"),
        }
    }

    // ── complex: programs ─────────────────────────────────────────────────────

    #[test]
    fn full_static_program() {
        let src = "
import shapes { circle, rect }
import render { sdf, fill }

let bg = rect(vec2(0.0, 0.0), vec2(2.0, 2.0), render: fill)
let c = circle(vec2(0.0, 0.0), 0.5, render: sdf)

out << bg << c
        ";
        let p = parse(src);
        assert_eq!(p.imports.len(), 2);
        assert_eq!(p.items.len(), 3); // bg, c, out
    }

    #[test]
    fn full_animated_program() {
        let src = "
state {
    let t: float = 0.0
    let speed = 1.0
}

fn update(s: State, input: Input) -> State {
    s.t = s.t + input.dt * s.speed
    let c = circle(vec2(0.0, 0.0), 0.3)
    out << c
    return s
}
        ";
        let p = parse(src);
        assert!(p.state.is_some());
        let s = p.state.unwrap();
        assert_eq!(s.fields.len(), 2);
        assert_eq!(s.fields[0].name, "t");
        assert_eq!(s.fields[1].name, "speed");
        assert_eq!(p.items.len(), 1);
        match &p.items[0] {
            Item::FnDef(f) => {
                assert_eq!(f.name, "update");
                assert_eq!(f.params[0].ty, Type::Named("State".into()));
                assert_eq!(f.params[1].ty, Type::Named("Input".into()));
                assert_eq!(f.return_ty, Some(Type::Named("State".into())));
                assert_eq!(f.body.len(), 4);
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn multiple_fn_defs() {
        let src = "
fn add(a: float, b: float) -> float { return a + b }
fn sub(a: float, b: float) -> float { return a - b }
fn mul(a: float, b: float) -> float { return a * b }
        ";
        let p = parse(src);
        assert_eq!(p.items.len(), 3);
        let names: Vec<&str> = p.items.iter().map(|i| match i {
            Item::FnDef(f) => f.name.as_str(),
            _ => panic!("expected FnDef"),
        }).collect();
        assert_eq!(names, vec!["add", "sub", "mul"]);
    }

    #[test]
    fn higher_order_function() {
        let src = "fn apply(f: fn(float) -> float, x: float) -> float {
    return f(x)
}";
        let p = parse(src);
        match &p.items[0] {
            Item::FnDef(f) => {
                assert_eq!(f.params.len(), 2);
                assert_eq!(f.params[0].ty, Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))));
                assert_eq!(f.params[1].ty, Type::Float);
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn for_loop_with_body() {
        let src = "
for let i = 0.0; i < 5.0; i = i + 1.0 {
    let s = circle(vec2(i, 0.0), 0.1)
    out << s
}
        ";
        let p = parse(src);
        match &p.items[0] {
            Item::Stmt(Stmt::For(f)) => assert_eq!(f.body.len(), 2),
            _ => panic!("expected For"),
        }
    }

    #[test]
    fn foreach_with_transform() {
        let src = "
let t = transform().scale(2.0)
foreach s in shapes {
    out << s@t
}
        ";
        let p = parse(src);
        assert_eq!(p.items.len(), 2);
        match &p.items[1] {
            Item::Stmt(Stmt::Foreach(f)) => {
                assert_eq!(f.var_name, "s");
                assert_eq!(f.body.len(), 1);
            }
            _ => panic!("expected Foreach"),
        }
    }

    #[test]
    fn nested_if_else() {
        let src = "
if x > 0.0 {
    if y > 0.0 {
        out << circle(vec2(x, y), 0.1)
    } else {
        out << circle(vec2(x, 0.0), 0.1)
    }
} else {
    out << circle(vec2(0.0, 0.0), 0.1)
}
        ";
        let p = parse(src);
        match &p.items[0] {
            Item::Stmt(Stmt::If(outer)) => {
                assert!(outer.else_block.is_some());
                assert!(matches!(outer.then_block[0], Stmt::If(_)));
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn while_with_break_condition() {
        let src = "
let i = 0.0
while i < 100.0 and not done {
    i = i + 1.0
}
        ";
        let p = parse(src);
        match &p.items[1] {
            Item::Stmt(Stmt::While(w)) => {
                assert!(matches!(w.condition, Expr::BinOp { op: BinOp::And, .. }));
            }
            _ => panic!("expected While"),
        }
    }

    #[test]
    fn lambda_with_multiple_stmts() {
        let p = parse("fn f = (x: float) -> float { let y = x * 2.0 return y }");
        match &p.items[0] {
            Item::Stmt(Stmt::FnVar { value, .. }) => match value {
                Expr::Lambda { body, .. } => assert_eq!(body.len(), 2),
                _ => panic!("expected Lambda"),
            },
            _ => panic!("expected FnVar"),
        }
    }

    #[test]
    fn error_handling_pattern() {
        let src = "
fn divide(a: float, b: float) -> res<float> {
    if b == 0.0 {
        return error(\"division by zero\")
    }
    return ok(a / b)
}

let result = divide(10.0, 0.0)

if result.ok {
    out << circle(vec2(result.value, 0.0), 0.1)
} else {
    out << circle(vec2(0.0, 0.0), 0.1)
}
        ";
        let p = parse(src);
        assert_eq!(p.items.len(), 3); // fn, let result, if
        match &p.items[0] {
            Item::FnDef(f) => {
                assert_eq!(f.return_ty, Some(Type::Res(Box::new(Type::Float))));
            }
            _ => panic!("expected FnDef"),
        }
    }

    #[test]
    fn state_mixed_typed_inferred() {
        let src = "state {
    let x: float = 0.0
    let y = 0.0
    let active: bool = true
    let count = 0.0
}";
        let p = parse(src);
        let s = p.state.unwrap();
        assert_eq!(s.fields.len(), 4);
        assert_eq!(s.fields[0].ty, Some(Type::Float));
        assert!(s.fields[1].ty.is_none());
        assert_eq!(s.fields[2].ty, Some(Type::Bool));
        assert!(s.fields[3].ty.is_none());
    }

    // ── complex: error recovery ───────────────────────────────────────────────

    #[test]
    fn error_recovery_continues_after_bad_stmt() {
        // bad statement, then a valid one — parser should recover and parse both
        let src = "@@@@\nlet x = 1.0";
        let tokens = Lexer::new(src).tokenize().expect("lex failed");
        let result = Parser::new(tokens).parse();
        // should have errors but also recover to parse let x = 1.0
        match result {
            Err(errs) => assert!(!errs.is_empty()),
            Ok(p) => assert_eq!(p.items.len(), 1), // recovered, got the let
        }
    }

    #[test]
    fn error_missing_name_in_var_decl() {
        // `let = 3.14` — `=` is not a valid identifier
        let errs = parse_err("let = 3.14");
        assert!(!errs.is_empty());
    }

    #[test]
    fn error_missing_closing_paren() {
        let errs = parse_err("fn f(a: float { return a }");
        assert!(!errs.is_empty());
    }

    #[test]
    fn error_missing_return_type_after_arrow() {
        let errs = parse_err("fn f() -> { return 1.0 }");
        assert!(!errs.is_empty());
    }
}
