//! Pass 2 — Type Resolver
//!
//! Walks every statement and expression, infers types, and checks type
//! compatibility. Updates the symbol table where types were left as `None`
//! by the collector.

use crate::syntax::ast::*;
use crate::error::{Error, ErrorCode};
use crate::namespaces::NamespaceRegistry;
use crate::types::binop_registry::{BinopRegistry, type_to_key, key_to_type};
use super::lookup::LookupContext;
use super::symbols::{ScopeKind, Symbol, SymbolKind, SymbolTable};

pub struct TypeResolver<'a> {
    pub table: SymbolTable,
    pub errors: Vec<Error>,
    /// The declaration_order of the function currently being checked.
    /// `None` at the top level.
    current_fn_order: Option<usize>,
    /// Expected return type of the current function (`None` = void).
    current_fn_return: Option<Type>,
    /// Program being resolved.
    program: Option<&'a Program>,
    /// Lookup context for field/method resolution (State, namespaces).
    lookup: LookupContext<'a>,
    /// Operator type table — same registry used at runtime, queried for return types.
    binops: BinopRegistry,
}

impl<'a> TypeResolver<'a> {
    pub fn new(table: SymbolTable, registry: &'a NamespaceRegistry) -> Self {
        Self {
            table,
            errors: Vec::new(),
            current_fn_order: None,
            current_fn_return: None,
            program: None,
            lookup: LookupContext::new(None, registry),
            binops: BinopRegistry::default(),
        }
    }

    pub fn run(mut self, program: &'a Program) -> (SymbolTable, Vec<Error>) {
        self.program = Some(program);
        self.lookup = LookupContext::new(Some(program), self.lookup.registry);
        if let Some(state) = &program.state {
            self.check_state(state);
        }
        for item in &program.items {
            match item {
                Item::FnDef(f)  => self.check_fn(f),
                Item::Stmt(s)   => { self.check_stmt(s); }
            }
        }
        (self.table, self.errors)
    }

    // ── State block ───────────────────────────────────────────────────────────

    fn check_state(&mut self, state: &StateBlock) {
        for field in &state.fields {
            let init_ty = self.infer_expr(&field.initializer);
            let resolved_ty = match (&field.ty, init_ty) {
                (Some(ann), Ok(inferred)) => {
                    self.expect_type(&ann.clone(), &inferred, &field.span);
                    ann.clone()
                }
                (Some(ann), Err(_)) => ann.clone(),
                (None, Ok(inferred)) => {
                    // Update the state field symbol
                    self.table.update_type(&format!("__state__{}", field.name), inferred.clone());
                    inferred
                }
                (None, Err(_)) => return,
            };
            self.table.update_type(&format!("__state__{}", field.name), resolved_ty);
        }
    }

    // ── Functions ─────────────────────────────────────────────────────────────

    fn check_fn(&mut self, f: &FnDef) {
        // Find this function's declaration order for strict scoping
        let fn_order = self.table.lookup(&f.name)
            .map(|s| s.declaration_order)
            .unwrap_or(0);

        let prev_order  = self.current_fn_order.replace(fn_order);
        let prev_return = std::mem::replace(&mut self.current_fn_return, f.return_ty.clone());

        self.table.push_scope(ScopeKind::Function);

        // Declare params in function scope
        for param in &f.params {
            let sym = Symbol::new(param.name.clone(), Some(param.ty.clone()), SymbolKind::Param, param.span.clone());
            self.table.declare(sym);
        }

        for stmt in &f.body {
            self.check_stmt(stmt);
        }

        self.table.pop_scope();
        self.current_fn_order  = prev_order;
        self.current_fn_return = prev_return;
    }

    // ── Statements ────────────────────────────────────────────────────────────

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl(v) => self.check_var_decl(v),
            Stmt::Assign(a)  => self.check_assign(a),
            Stmt::Out(o)     => self.check_out(o),
            Stmt::If(i)      => self.check_if(i),
            Stmt::While(w)   => self.check_while(w),
            Stmt::For(f)     => self.check_for(f),
            Stmt::Foreach(f) => self.check_foreach(f),
            Stmt::Return(expr, span) => self.check_return(expr.as_ref(), span),
            Stmt::FnVar { name, value, span } => self.check_fn_var(name, value, span),
            Stmt::Expr(e)    => {
                if let Err(e) = self.infer_expr(e) {
                    self.errors.extend(e);
                }
            }
        }
    }

    fn check_var_decl(&mut self, v: &VarDecl) {
        let init_ty = match self.infer_expr(&v.initializer) {
            Ok(t)  => t,
            Err(e) => { self.errors.extend(e); return; }
        };

        let final_ty = if let Some(ann) = &v.ty {
            self.expect_type(ann, &init_ty, &v.span);
            ann.clone()
        } else {
            init_ty
        };

        let kind = if v.is_const { SymbolKind::Const } else { SymbolKind::Variable };
        let sym = Symbol::new(v.name.clone(), Some(final_ty), kind, v.span.clone());

        if self.table.current_scope_kind() == &ScopeKind::Global {
            // Update the already-declared top-level symbol's type
            self.table.update_type(&v.name, sym.ty.clone().unwrap());
        } else {
            if !self.table.declare(sym) {
                self.errors.push(Error::new(
                    ErrorCode::S003, v.span.line, v.span.column,
                    format!("`{}` already declared in this scope", v.name),
                ));
            }
        }
    }

    fn check_assign(&mut self, a: &Assign) {
        let root = &a.target[0];
        let sym = self.lookup_symbol(root, &a.span);

        if let Some(sym) = sym {
            if sym.kind == SymbolKind::Const {
                self.errors.push(Error::new(
                    ErrorCode::S004, a.span.line, a.span.column,
                    format!("cannot reassign const `{root}`"),
                ));
                return;
            }
            let mut ty = sym.ty.clone();
            for segment in &a.target[1..] {
                ty = ty.and_then(|t| self.lookup.resolve_field(&t, segment));
            }
            match self.infer_expr(&a.value) {
                Ok(val_ty) => {
                    if let Some(target_ty) = &ty {
                        self.expect_type(target_ty, &val_ty, &a.span);
                    }
                }
                Err(e) => self.errors.extend(e),
            }
        }
    }

    fn check_out(&mut self, o: &OutStmt) {
        for expr in &o.shapes {
            match self.infer_expr(expr) {
                Ok(ty) if ty != Type::Named("shape".into()) => {
                    self.errors.push(Error::new(
                        ErrorCode::S002,
                        expr.span().line, expr.span().column,
                        format!("out << expects `shape`, found `{}`", type_name(&ty)),
                    ));
                }
                Err(e) => self.errors.extend(e),
                _ => {}
            }
        }
    }

    fn check_if(&mut self, i: &IfStmt) {
        match self.infer_expr(&i.condition) {
            Ok(cond_ty) if cond_ty != Type::Bool => {
                self.errors.push(Error::new(
                    ErrorCode::S002,
                    i.condition.span().line, i.condition.span().column,
                    format!("condition must be `bool`, found `{}`", type_name(&cond_ty)),
                ));
            }
            Err(e) => self.errors.extend(e),
            _ => {}
        }
        self.check_block(&i.then_block);
        if let Some(else_block) = &i.else_block {
            self.check_block(else_block);
        }
    }

    fn check_while(&mut self, w: &WhileStmt) {
        match self.infer_expr(&w.condition) {
            Ok(cond_ty) if cond_ty != Type::Bool => {
                self.errors.push(Error::new(
                    ErrorCode::S002,
                    w.condition.span().line, w.condition.span().column,
                    format!("condition must be `bool`, found `{}`", type_name(&cond_ty)),
                ));
            }
            Err(e) => self.errors.extend(e),
            _ => {}
        }
        self.check_block(&w.body);
    }

    fn check_for(&mut self, f: &ForStmt) {
        self.table.push_scope(ScopeKind::Block);
        self.check_stmt(&f.init);
        match self.infer_expr(&f.condition) {
            Ok(cond_ty) if cond_ty != Type::Bool => {
                self.errors.push(Error::new(
                    ErrorCode::S002,
                    f.condition.span().line, f.condition.span().column,
                    format!("for condition must be `bool`, found `{}`", type_name(&cond_ty)),
                ));
            }
            Err(e) => self.errors.extend(e),
            _ => {}
        }
        self.check_stmt(&f.step);
        for stmt in &f.body { self.check_stmt(stmt); }
        self.table.pop_scope();
    }

    fn check_foreach(&mut self, f: &ForeachStmt) {
        let elem_ty = match self.infer_expr(&f.iterable) {
            Ok(Type::List(elem))        => Some(*elem),
            Ok(Type::Array(elem, _))    => Some(*elem),
            Ok(other) => {
                self.errors.push(Error::new(
                    ErrorCode::S002,
                    f.iterable.span().line, f.iterable.span().column,
                    format!("foreach expects a list or array, found `{}`", type_name(&other)),
                ));
                None
            }
            Err(e) => { self.errors.extend(e); None }
        };

        self.table.push_scope(ScopeKind::Block);

        if let Some(elem_ty) = elem_ty {
            let var_ty = if let Some(ann) = &f.var_ty {
                self.expect_type(ann, &elem_ty, &f.span);
                ann.clone()
            } else {
                elem_ty
            };
            let sym = Symbol::new(f.var_name.clone(), Some(var_ty), SymbolKind::Variable, f.span.clone());
            self.table.declare(sym);
        }

        for stmt in &f.body { self.check_stmt(stmt); }
        self.table.pop_scope();
    }

    fn check_return(&mut self, expr: Option<&Expr>, span: &Span) {
        match (expr, &self.current_fn_return.clone()) {
            (Some(e), Some(expected)) => {
                match self.infer_expr(e) {
                    Ok(actual) => self.expect_type(expected, &actual, span),
                    Err(e) => self.errors.extend(e),
                }
            }
            (Some(e), None) => {
                if let Err(e) = self.infer_expr(e) {
                    self.errors.extend(e);
                }
                self.errors.push(Error::new(
                    ErrorCode::S002, span.line, span.column,
                    "returning a value from a void function",
                ));
            }
            (None, Some(expected)) => {
                self.errors.push(Error::new(
                    ErrorCode::S002, span.line, span.column,
                    format!("expected return value of type `{}`", type_name(expected)),
                ));
            }
            (None, None) => {} // bare return in void function — OK
        }
    }

    fn check_fn_var(&mut self, name: &str, value: &Expr, span: &Span) {
        match self.infer_expr(value) {
            Ok(ty) => {
                if !matches!(ty, Type::Fn(..)) {
                    self.errors.push(Error::new(
                        ErrorCode::S002, span.line, span.column,
                        format!("`fn {name} = …` requires a function value, found `{}`", type_name(&ty)),
                    ));
                } else if self.table.current_scope_kind() == &ScopeKind::Global {
                    self.table.update_type(name, ty);
                } else {
                    // Local fn-var (inside a function body) — declare in the current scope.
                    let sym = Symbol::new(name.to_string(), Some(ty), SymbolKind::Function, span.clone());
                    if !self.table.declare(sym) {
                        self.errors.push(Error::new(
                            ErrorCode::S003, span.line, span.column,
                            format!("`{name}` already declared"),
                        ));
                    }
                }
            }
            Err(e) => self.errors.extend(e),
        }
    }

    fn check_block(&mut self, stmts: &[Stmt]) {
        self.table.push_scope(ScopeKind::Block);
        for s in stmts { self.check_stmt(s); }
        self.table.pop_scope();
    }

    // ── Expression type inference ─────────────────────────────────────────────

    pub fn infer_expr(&mut self, expr: &Expr) -> Result<Type, Vec<Error>> {
        match expr {
            Expr::Float(_, _)     => Ok(Type::Float),
            Expr::Bool(_, _)      => Ok(Type::Bool),
            Expr::StringLit(_, _) => Ok(Type::Named("string".into())),
            Expr::HexColor(_, _)  => Ok(Type::Named("color".into())),

            Expr::Ident(name, span) => self.lookup_type(name, span),

            Expr::BinOp { left, op, right, span } => {
                let l = self.infer_expr(left)?;
                let r = self.infer_expr(right)?;
                self.check_binop(op, &l, &r, span)
            }

            Expr::UnOp { op, operand, span } => {
                let ty = self.infer_expr(operand)?;
                self.check_unop(op, &ty, span)
            }

            Expr::Ternary { condition, then_expr, else_expr, span } => {
                let cond_ty = self.infer_expr(condition)?;
                if cond_ty != Type::Bool {
                    return Err(vec![Error::new(
                        ErrorCode::S002, span.line, span.column,
                        format!("ternary condition must be `bool`, found `{}`", type_name(&cond_ty)),
                    )]);
                }
                let then_ty = self.infer_expr(then_expr)?;
                let else_ty = self.infer_expr(else_expr)?;
                if then_ty != else_ty {
                    return Err(vec![Error::new(
                        ErrorCode::S002, span.line, span.column,
                        format!(
                            "ternary branches have different types: `{}` and `{}`",
                            type_name(&then_ty), type_name(&else_ty)
                        ),
                    )]);
                }
                Ok(then_ty)
            }

            Expr::Cast { ty, .. } => Ok(ty.clone()),

            Expr::Try { expr, .. } => {
                let inner = self.infer_expr(expr)?;
                Ok(Type::Res(Box::new(inner)))
            }

            Expr::Call { callee, args, named_args, span } => {
                self.check_call(callee, args, named_args, span)
            }

            Expr::Index { expr, index, span } => {
                let coll_ty = self.infer_expr(expr)?;
                let idx_ty  = self.infer_expr(index)?;
                if idx_ty != Type::Float {
                    self.errors.push(Error::new(
                        ErrorCode::S002, span.line, span.column,
                        format!("index must be `float`, found `{}`", type_name(&idx_ty)),
                    ));
                }
                match coll_ty {
                    Type::List(elem)     => Ok(*elem),
                    Type::Array(elem, _) => Ok(*elem),
                    other => Err(vec![Error::new(
                        ErrorCode::S008, span.line, span.column,
                        format!("cannot index into `{}`", type_name(&other)),
                    )]),
                }
            }

            Expr::Field { expr, field, span } => {
                let obj_ty = self.infer_expr(expr)?;
                let ty = self.lookup.resolve_field(&obj_ty, field);
                ty.ok_or_else(|| vec![Error::new(
                    ErrorCode::S009, span.line, span.column,
                    format!("type `{}` has no field `{field}`", type_name(&obj_ty)),
                )])
            }

            Expr::MethodCall { expr, method, args, named_args: _, span } => {
                let obj_ty = self.infer_expr(expr)?;
                let ty = self.resolve_method_call(&obj_ty, method, args, span);
                ty.ok_or_else(|| vec![Error::new(
                    ErrorCode::S009, span.line, span.column,
                    format!("type `{}` has no method `{method}`", type_name(&obj_ty)),
                )])
            }

            Expr::Transform { expr, transforms, span } => {
                let shape_ty = self.infer_expr(expr)?;
                if shape_ty != Type::Named("shape".into()) {
                    self.errors.push(Error::new(
                        ErrorCode::S002, span.line, span.column,
                        format!("`@` expects `shape` on the left, found `{}`", type_name(&shape_ty)),
                    ));
                }
                for t in transforms {
                    if let Ok(t_ty) = self.infer_expr(t) {
                        if t_ty != Type::Named("transform".into()) {
                            self.errors.push(Error::new(
                                ErrorCode::S002, t.span().line, t.span().column,
                                format!("`@` expects `transform`, found `{}`", type_name(&t_ty)),
                            ));
                        }
                    }
                }
                Ok(Type::Named("shape".into()))
            }

            Expr::List(items, span) => {
                if items.is_empty() {
                    // Empty list — type cannot be inferred here; return a placeholder.
                    // The surrounding context (var decl annotation) should provide the type.
                    return Ok(Type::List(Box::new(Type::Float))); // lenient for now
                }
                let first_ty = self.infer_expr(&items[0])?;
                for item in items.iter().skip(1) {
                    if let Ok(ty) = self.infer_expr(item) {
                        if ty != first_ty {
                            self.errors.push(Error::new(
                                ErrorCode::S002, span.line, span.column,
                                format!(
                                    "list elements must all have the same type, found `{}` and `{}`",
                                    type_name(&first_ty), type_name(&ty)
                                ),
                            ));
                        }
                    }
                }
                Ok(Type::List(Box::new(first_ty)))
            }

            Expr::Lambda { params, return_ty, body, .. } => {
                let param_types: Vec<Type> = params.iter().map(|p| p.ty.clone()).collect();
                let ret = return_ty.clone().map(Box::new);

                // Check the lambda body in its own scope
                self.table.push_scope(ScopeKind::Function);
                let prev_return = std::mem::replace(&mut self.current_fn_return, return_ty.clone());
                for param in params {
                    let sym = Symbol::new(param.name.clone(), Some(param.ty.clone()), SymbolKind::Param, param.span.clone());
                    self.table.declare(sym);
                }
                for stmt in body { self.check_stmt(stmt); }
                self.table.pop_scope();
                self.current_fn_return = prev_return;

                Ok(Type::Fn(param_types, ret))
            }
        }
    }

    // ── Call checking ─────────────────────────────────────────────────────────

    fn check_call(
        &mut self,
        callee: &str,
        args: &[Expr],
        _named_args: &[(String, Expr)],
        span: &Span,
    ) -> Result<Type, Vec<Error>> {
        // Special-case generic built-ins before general lookup
        match callee {
            "ok" => {
                let inner = if let Some(arg) = args.first() {
                    self.infer_expr(arg).unwrap_or(Type::Float)
                } else {
                    Type::Float
                };
                return Ok(Type::Res(Box::new(inner)));
            }
            "error" => {
                return Ok(Type::Res(Box::new(Type::Float))); // placeholder
            }
            "len" => {
                if let Some(arg) = args.first() {
                    self.infer_expr(arg).ok();
                }
                return Ok(Type::Float);
            }
            // color is overloaded (3 or 4 float args)
            "color" => {
                if args.len() != 3 && args.len() != 4 {
                    return Err(vec![Error::new(
                        ErrorCode::S007, span.line, span.column,
                        "color() takes 3 or 4 arguments",
                    )]);
                }
                for arg in args { self.infer_expr(arg).ok(); }
                return Ok(Type::Named("color".into()));
            }
            _ => {}
        }

        let fn_ty = self.lookup_type(callee, span)?;

        match fn_ty {
            Type::Fn(param_types, ret_ty) => {
                // Check positional arg count
                if args.len() != param_types.len() {
                    return Err(vec![Error::new(
                        ErrorCode::S007, span.line, span.column,
                        format!(
                            "`{callee}` expects {} argument(s), got {}",
                            param_types.len(), args.len()
                        ),
                    )]);
                }
                // Check each arg type
                let mut has_arg_error = false;
                for (arg, expected) in args.iter().zip(param_types.iter()) {
                    match self.infer_expr(arg) {
                        Ok(actual) => self.expect_type(expected, &actual, span),
                        Err(e) => {
                            self.errors.extend(e);
                            has_arg_error = true;
                        }
                    }
                }
                if has_arg_error {
                    return Err(vec![]); // Errors already extended; signal failure
                }
                Ok(ret_ty.map(|t| *t).unwrap_or(Type::Named("void".into())))
            }
            other => Err(vec![Error::new(
                ErrorCode::S010, span.line, span.column,
                format!("`{callee}` is not callable (type: `{}`)", type_name(&other)),
            )]),
        }
    }

    // ── Operator checking ─────────────────────────────────────────────────────

    fn check_binop(&mut self, op: &BinOp, l: &Type, r: &Type, span: &Span) -> Result<Type, Vec<Error>> {
        if let (Some(lk), Some(rk)) = (type_to_key(l), type_to_key(r)) {
            if let Some(ret_key) = self.binops.result_type(op, lk, rk) {
                return Ok(key_to_type(ret_key));
            }
        }
        Err(vec![Error::new(
            ErrorCode::S008, span.line, span.column,
            format!("operator `{op}` not applicable to `{}` and `{}`", type_name(l), type_name(r)),
        )])
    }

    fn check_unop(&mut self, op: &UnOp, ty: &Type, span: &Span) -> Result<Type, Vec<Error>> {
        match op {
            UnOp::Neg => {
                if *ty != Type::Float {
                    Err(vec![Error::new(
                        ErrorCode::S008, span.line, span.column,
                        format!("unary `-` requires `float`, found `{}`", type_name(ty)),
                    )])
                } else {
                    Ok(Type::Float)
                }
            }
            UnOp::Not => {
                if *ty != Type::Bool {
                    Err(vec![Error::new(
                        ErrorCode::S008, span.line, span.column,
                        format!("`not` requires `bool`, found `{}`", type_name(ty)),
                    )])
                } else {
                    Ok(Type::Bool)
                }
            }
        }
    }

    // ── Lookup helpers ────────────────────────────────────────────────────────

    /// Resolve the return type of `obj.method(args)`.
    fn resolve_method_call(
        &mut self,
        obj_ty: &Type,
        method: &str,
        args: &[Expr],
        span: &Span,
    ) -> Option<Type> {
        let member_ty = self.lookup.get_method_type(&obj_ty, method)?;
        if let Type::Fn(param_types, ret_ty) = &member_ty {
            if args.len() == param_types.len() {
                for (arg, expected) in args.iter().zip(param_types.iter()) {
                    match self.infer_expr(arg) {
                        Ok(actual) => self.expect_type(expected, &actual, span),
                        Err(e) => self.errors.extend(e),
                    }
                }
                // Void methods return Unit so the caller can distinguish
                // "method found, void return" from "method not found".
                return Some(ret_ty.clone().map(|t| *t).unwrap_or(Type::Unit));
            }
        }
        for arg in args {
            self.infer_expr(arg).ok();
        }
        Some(member_ty)
    }

    fn lookup_symbol(&self, name: &str, _span: &Span) -> Option<&super::symbols::Symbol> {
        if let Some(order) = self.current_fn_order {
            self.table.lookup_strict(name, order)
        } else {
            self.table.lookup(name)
        }
    }

    fn lookup_type(&mut self, name: &str, span: &Span) -> Result<Type, Vec<Error>> {
        let sym = self.lookup_symbol(name, span);
        match sym {
            Some(s) => match &s.ty {
                Some(t) => Ok(t.clone()),
                None => Err(vec![Error::new(
                    ErrorCode::S001, span.line, span.column,
                    format!("`{name}` used before its type could be resolved"),
                )]),
            },
            None => Err(vec![Error::new(
                ErrorCode::S001, span.line, span.column,
                format!("undefined: `{name}`"),
            )]),
        }
    }

    fn expect_type(&mut self, expected: &Type, actual: &Type, span: &Span) {
        if expected != actual {
            self.errors.push(Error::new(
                ErrorCode::S002, span.line, span.column,
                format!("expected `{}`, found `{}`", type_name(expected), type_name(actual)),
            ));
        }
    }
}

// ─── Type display ─────────────────────────────────────────────────────────────

pub fn type_name(ty: &Type) -> String {
    match ty {
        Type::Float           => "float".into(),
        Type::Bool            => "bool".into(),
        Type::Unit            => "()".into(),
        Type::Array(t, n)     => format!("array[{}, {n}]", type_name(t)),
        Type::List(t)         => format!("list[{}]", type_name(t)),
        Type::Res(t)          => format!("res<{}>", type_name(t)),
        Type::Fn(ps, Some(r)) => format!("fn({}) -> {}", ps.iter().map(type_name).collect::<Vec<_>>().join(", "), type_name(r)),
        Type::Fn(ps, None)    => format!("fn({})", ps.iter().map(type_name).collect::<Vec<_>>().join(", ")),
        Type::Named(n)        => n.clone(),
    }
}
