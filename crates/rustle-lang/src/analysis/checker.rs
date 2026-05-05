//! Pass 2 — Type Resolver
//!
//! Walks every statement and expression, infers types, and checks type
//! compatibility. Updates the symbol table where types were left as `None`
//! by the collector.

use crate::syntax::ast::{Type, Program, Item, StateBlock, FnDef, Stmt, VarDecl, Assign, AssignTarget, OutStmt, PrintStmt, MatchStmt, IfStmt, Expr, Span, WhileStmt, ForStmt, ForeachStmt, BinOp, UnOp};
use crate::error::{Error, ErrorCode, suggest_similar};
use crate::namespaces::NamespaceRegistry;
use crate::types::binop_registry::{BinopRegistry, type_to_key, key_to_type};
use crate::types::registry::TypeRegistry;
use super::lookup::LookupContext;
use super::symbols::{ScopeKind, Symbol, SymbolKind, SymbolTable};

pub struct TypeResolver<'a> {
    pub table: SymbolTable,
    pub errors: Vec<Error>,
    /// The `declaration_order` of the function currently being checked.
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
    /// When inside a struct method, holds the struct name for `this` resolution.
    current_struct: Option<String>,
}

impl<'a> TypeResolver<'a> {
    #[must_use]
    pub fn new(table: SymbolTable, registry: &'a NamespaceRegistry, type_registry: &'a TypeRegistry) -> Self {
        Self {
            table,
            errors: Vec::new(),
            current_fn_order: None,
            current_fn_return: None,
            program: None,
            lookup: LookupContext::new(None, registry, type_registry),
            binops: BinopRegistry::default(),
            current_struct: None,
        }
    }

    #[must_use]
    pub fn run(mut self, program: &'a Program) -> (SymbolTable, Vec<Error>) {
        self.program = Some(program);
        self.lookup = LookupContext::new(Some(program), self.lookup.registry, self.lookup.type_registry);
        if let Some(state) = &program.state {
            self.check_state(state);
        }
        for item in &program.items {
            match item {
                Item::FnDef(f)  => self.check_fn(f),
                Item::Stmt(s)   => { self.check_stmt(s); }
                Item::Struct(def) => self.check_struct(def),
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
            .map_or(0, |s| s.declaration_order);

        let prev_order  = self.current_fn_order.replace(fn_order);
        let prev_return = std::mem::replace(&mut self.current_fn_return, f.return_ty.clone());

        self.table.push_scope(ScopeKind::Function);

        // Declare params in function scope
        for param in f.params.iter() {
            let sym = Symbol::new(param.name.clone(), Some(param.ty.clone()), SymbolKind::Param, param.span);
            self.table.declare(sym);
        }

        for stmt in f.body.iter() {
            self.check_stmt(stmt);
        }

        self.table.pop_scope();
        self.current_fn_order  = prev_order;
        self.current_fn_return = prev_return;
    }

    // ── Structs ───────────────────────────────────────────────────────────────

    fn check_struct(&mut self, def: &crate::syntax::ast::StructDef) {
        // Check field default expressions
        for field in &def.fields {
            if let Some(ref default_expr) = field.default {
                let expr_ty = self.infer_expr(default_expr);
                if let (Some(expected_ty), Ok(actual_ty)) = (&field.ty, &expr_ty) {
                    self.expect_type(expected_ty, actual_ty, &field.span);
                }
            }
        }

        // Check method bodies with `this` and struct fields in scope
        for method in &def.methods {
            self.current_struct = Some(def.name.clone());
            self.check_struct_method(&method.def, &def.fields);
            self.current_struct = None;
        }
    }

    fn check_struct_method(&mut self, f: &FnDef, fields: &[crate::syntax::ast::StructField]) {
        let fn_order = self.table.lookup(&f.name)
            .map_or(0, |s| s.declaration_order);

        let prev_order  = self.current_fn_order.replace(fn_order);
        let prev_return = std::mem::replace(&mut self.current_fn_return, f.return_ty.clone());

        self.table.push_scope(ScopeKind::Function);

        // Declare struct fields as local variables in the method scope
        for field in fields {
            if let Some(ref ty) = field.ty {
                let sym = Symbol::new(field.name.clone(), Some(ty.clone()), SymbolKind::Variable, field.span);
                self.table.declare(sym);
            }
        }

        // Declare params in function scope
        for param in f.params.iter() {
            let sym = Symbol::new(param.name.clone(), Some(param.ty.clone()), SymbolKind::Param, param.span);
            self.table.declare(sym);
        }

        for stmt in f.body.iter() {
            self.check_stmt(stmt);
        }

        self.table.pop_scope();
        self.current_fn_order  = prev_order;
        self.current_fn_return = prev_return;
    }

    fn find_struct_def(&self, name: &str) -> Option<&crate::syntax::ast::StructDef> {
        let program = self.program?;
        program.items.iter().find_map(|item| {
            if let Item::Struct(def) = item {
                if def.name == name { return Some(def); }
            }
            None
        })
    }

    // ── Statements ────────────────────────────────────────────────────────────

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl(v) => self.check_var_decl(v),
            Stmt::Assign(a)  => self.check_assign(a),
            Stmt::Out(o)     => self.check_out(o),
            Stmt::Print(p)   => self.check_print(p),
            Stmt::If(i)      => self.check_if(i),
            Stmt::IfLet { binding, expr, then_block, else_block, span } => {
                self.check_if_let(binding, expr, then_block, else_block.as_deref(), span);
            }
            Stmt::Match(m)   => self.check_match(m),
            Stmt::While(w)   => self.check_while(w),
            Stmt::For(f)     => self.check_for(f),
            Stmt::Foreach(f) => self.check_foreach(f),
            Stmt::Return(expr, span) => self.check_return(expr.as_ref(), span),
            Stmt::FnVar { name, value, span } => self.check_fn_var(name, value, span),
            Stmt::Break(_) | Stmt::Continue(_) => {}
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
            // Bare `let x = none` — can't infer the inner type
            if init_ty == Type::Optional(Box::new(Type::Unit)) {
                self.errors.push(Error::new(
                    ErrorCode::S002, v.span.line, v.span.column,
                    "cannot infer type of `none` without a type annotation",
                ));
                return;
            }
            init_ty
        };

        let kind = if v.is_const { SymbolKind::Const } else { SymbolKind::Variable };
        let sym = Symbol::new(v.name.clone(), Some(final_ty), kind, v.span);

        if self.table.current_scope_kind() == &ScopeKind::Global {
            // Update the already-declared top-level symbol's type
            self.table.update_type(&v.name, sym.ty.expect("type was just resolved"));
        } else if !self.table.declare(sym) {
            self.errors.push(Error::new(
                ErrorCode::S003, v.span.line, v.span.column,
                format!("`{}` already declared in this scope", v.name),
            ));
        }
    }

    fn check_assign(&mut self, a: &Assign) {
        let path = a.target.path();
        let root = &path[0];
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
            for segment in &path[1..] {
                ty = ty.and_then(|t| self.lookup.resolve_field(&t, segment));
            }
            // For indexed target, drill down to element type and check index is float
            if let AssignTarget::Indexed { indices, .. } = &a.target {
                for idx in indices {
                    match self.infer_expr(idx) {
                        Ok(idx_ty) if idx_ty != Type::Float => {
                            self.errors.push(Error::new(
                                ErrorCode::S002, idx.span().line, idx.span().column,
                                format!("index must be `float`, found `{}`", type_name(&idx_ty)),
                            ));
                        }
                        Err(e) => self.errors.extend(e),
                        _ => {}
                    }
                    ty = ty.and_then(|t| self.indexed_type(&t));
                }
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

    #[expect(clippy::unused_self, reason = "kept as method for consistent call site syntax")]
    fn indexed_type(&self, ty: &Type) -> Option<Type> {
        match ty {
            Type::List(elem) | Type::Array(elem, _) => Some(*elem.clone()),
            _ => None,
        }
    }

    fn check_out(&mut self, o: &OutStmt) {
        for expr in &o.shapes {
            match self.infer_expr(expr) {
                Ok(ty) if !is_drawable(&ty) => {
                    self.errors.push(Error::new(
                        ErrorCode::S002,
                        expr.span().line, expr.span().column,
                        format!("out << expects a shape type, found `{}`", type_name(&ty)),
                    ));
                }
                Err(e) => self.errors.extend(e),
                _ => {}
            }
        }
    }

    fn check_print(&mut self, p: &PrintStmt) {
        for expr in &p.values {
            if let Err(e) = self.infer_expr(expr) {
                self.errors.extend(e);
            }
        }
    }

    fn check_match(&mut self, m: &MatchStmt) {
        let scrut_ty = match self.infer_expr(&m.expr) {
            Ok(t) => t,
            Err(e) => { self.errors.extend(e); return; }
        };
        if !is_matchable(&scrut_ty) {
            self.errors.push(Error::new(
                ErrorCode::S008, m.expr.span().line, m.expr.span().column,
                format!("match scrutinee must be a comparable type (float, bool, string, vec2, vec3, vec4, color), found `{}`", type_name(&scrut_ty)),
            ));
        }
        for arm in &m.arms {
            for val in &arm.values {
                match self.infer_expr(val) {
                    Ok(val_ty) if !types_compatible(&scrut_ty, &val_ty) => {
                        self.errors.push(Error::new(
                            ErrorCode::S002, val.span().line, val.span().column,
                            format!("match arm value must match scrutinee type `{}`, found `{}`", type_name(&scrut_ty), type_name(&val_ty)),
                        ));
                    }
                    Err(e) => self.errors.extend(e),
                    _ => {}
                }
            }
            self.check_block(&arm.body);
        }
    }

    fn check_if(&mut self, i: &IfStmt) {
        match self.infer_expr(&i.condition) {
            Ok(ref cond_ty) if !is_truthy_type(cond_ty) => {
                self.errors.push(Error::new(
                    ErrorCode::S002,
                    i.condition.span().line, i.condition.span().column,
                    format!("condition must be a truthy type (bool, float, string, list, or optional), found `{}`", type_name(cond_ty)),
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

    fn check_if_let(&mut self, binding: &str, expr: &Expr, then_block: &[Stmt], else_block: Option<&[Stmt]>, span: &Span) {
        let expr_ty = match self.infer_expr(expr) {
            Ok(t) => t,
            Err(e) => { self.errors.extend(e); return; }
        };
        let inner = match expr_ty {
            Type::Optional(inner) => *inner,
            other => {
                self.errors.push(Error::new(
                    ErrorCode::S002, span.line, span.column,
                    format!("`if let` requires an optional type, found `{}`", type_name(&other)),
                ));
                return;
            }
        };
        // Then block: binding has the unwrapped type
        self.table.push_scope(ScopeKind::Block);
        let sym = Symbol::new(binding.to_string(), Some(inner), SymbolKind::Variable, *span);
        self.table.declare(sym);
        for s in then_block { self.check_stmt(s); }
        self.table.pop_scope();
        // Else block: no binding
        if let Some(els) = else_block {
            self.check_block(els);
        }
    }

    fn check_while(&mut self, w: &WhileStmt) {
        match self.infer_expr(&w.condition) {
            Ok(ref cond_ty) if !is_truthy_type(cond_ty) => {
                self.errors.push(Error::new(
                    ErrorCode::S002,
                    w.condition.span().line, w.condition.span().column,
                    format!("condition must be a truthy type (bool, float, string, list, or optional), found `{}`", type_name(cond_ty)),
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
            Ok(ref cond_ty) if !is_truthy_type(cond_ty) => {
                self.errors.push(Error::new(
                    ErrorCode::S002,
                    f.condition.span().line, f.condition.span().column,
                    format!("for condition must be a truthy type (bool, float, string, list, or optional), found `{}`", type_name(cond_ty)),
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
            Ok(Type::List(elem) | Type::Array(elem, _)) => Some(*elem),
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
            let sym = Symbol::new(f.var_name.clone(), Some(var_ty), SymbolKind::Variable, f.span);
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
                    let sym = Symbol::new(name.to_string(), Some(ty), SymbolKind::Function, *span);
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

    /// # Errors
    /// Returns errors if the expression contains type mismatches or undefined symbols.
    #[expect(clippy::too_many_lines, reason = "large match on Expr variants; splitting would add indirection")]
    pub fn infer_expr(&mut self, expr: &Expr) -> Result<Type, Vec<Error>> {
        match expr {
            Expr::Float(_, _)     => Ok(Type::Float),
            Expr::Bool(_, _)      => Ok(Type::Bool),
            Expr::None(_)         => Ok(Type::Optional(Box::new(Type::Unit))),
            Expr::StringLit(_, _) => Ok(Type::String),
            Expr::HexColor(_, _)  => Ok(Type::Color),

            Expr::Interpolated(parts, _) => {
                for part in parts {
                    if let crate::syntax::ast::InterpolPart::Expr(e) = part {
                        self.infer_expr(e)?;
                    }
                }
                Ok(Type::String)
            }

            Expr::Ident(name, span) => self.lookup_type(name, span),

            Expr::BinOp { left, op, right, span } => {
                let l = self.infer_expr(left)?;
                let r = self.infer_expr(right)?;
                self.check_binop(op, &l, &r, span)
            }

            Expr::UnOp { op, operand, span } => {
                let ty = self.infer_expr(operand)?;
                self.check_unop(op, operand, &ty, span)
            }

            Expr::Ternary { condition, then_expr, else_expr, span } => {
                let cond_ty = self.infer_expr(condition)?;
                if !is_truthy_type(&cond_ty) {
                    return Err(vec![Error::new(
                        ErrorCode::S002, span.line, span.column,
                        format!("ternary condition must be a truthy type (bool, float, string, list, or optional), found `{}`", type_name(&cond_ty)),
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

            Expr::Cast { expr, ty, span } => {
                let from_ty = self.infer_expr(expr)?;
                if !is_castable(&from_ty, ty) {
                    return Err(vec![Error::new(
                        ErrorCode::S002,
                        span.line,
                        span.column,
                        format!(
                            "cannot cast `{}` to `{}`",
                            type_name(&from_ty),
                            type_name(ty)
                        ),
                    )]);
                }
                Ok(ty.clone())
            }

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
                    Type::List(elem) | Type::Array(elem, _) => Ok(*elem),
                    other => Err(vec![Error::new(
                        ErrorCode::S008, span.line, span.column,
                        format!("cannot index into `{}`", type_name(&other)),
                    )]),
                }
            }

            Expr::Field { expr, field, span } => {
                let obj_ty = self.infer_expr(expr)?;
                let ty = self.lookup.resolve_field(&obj_ty, field);
                ty.ok_or_else(|| {
                    let mut err = Error::new(
                        ErrorCode::S009, span.line, span.column,
                        format!("type `{}` has no field `{field}`", type_name(&obj_ty)),
                    );
                    let candidates = self.lookup.field_names(&obj_ty);
                    if !candidates.is_empty() {
                        if let Some(suggestion) = suggest_similar(field, &candidates, 2) {
                            err = err.with_hint(format!("did you mean '{suggestion}'?"));
                        } else {
                            err = err.with_hint(format!("available fields: {}", candidates.join(", ")));
                        }
                    }
                    vec![err]
                })
            }

            Expr::OptionalChain { expr, field, span } => {
                let obj_ty = self.infer_expr(expr)?;
                let inner = match &obj_ty {
                    Type::Optional(inner) => inner,
                    other => return Err(vec![Error::new(
                        ErrorCode::S002, span.line, span.column,
                        format!("`?.` requires an optional type, found `{}`", type_name(other)),
                    )]),
                };
                let field_ty = self.lookup.resolve_field(inner, field);
                if let Some(t) = field_ty { Ok(Type::Optional(Box::new(t))) } else {
                    let mut err = Error::new(
                        ErrorCode::S009, span.line, span.column,
                        format!("type `{}` has no field `{field}`", type_name(inner)),
                    );
                    let candidates = self.lookup.field_names(inner);
                    if !candidates.is_empty() {
                        if let Some(suggestion) = suggest_similar(field, &candidates, 2) {
                            err = err.with_hint(format!("did you mean '{suggestion}'?"));
                        } else {
                            err = err.with_hint(format!("available fields: {}", candidates.join(", ")));
                        }
                    }
                    Err(vec![err])
                }
            }

            Expr::MethodCall { expr, method, args, named_args: _, span } => {
                let obj_ty = self.infer_expr(expr)?;

                // Private method access check for struct types
                if let Type::Named(ref struct_name) = obj_ty {
                    if let Some(def) = self.find_struct_def(struct_name) {
                        if let Some(m) = def.methods.iter().find(|m| m.def.name == *method) {
                            if m.visibility == crate::syntax::ast::Visibility::Private
                                && self.current_struct.as_deref() != Some(struct_name.as_str())
                            {
                                return Err(vec![Error::new(
                                    ErrorCode::S016,
                                    span.line,
                                    span.column,
                                    format!("method '{}' is private in '{}'", method, struct_name),
                                )]);
                            }
                        }
                    }
                }

                let ty = self.resolve_method_call(&obj_ty, method, args, span);
                ty.ok_or_else(|| {
                    let mut err = Error::new(
                        ErrorCode::S009, span.line, span.column,
                        format!("type `{}` has no method `{method}`", type_name(&obj_ty)),
                    );
                    let candidates = self.lookup.method_names(&obj_ty);
                    if !candidates.is_empty() {
                        if let Some(suggestion) = suggest_similar(method, &candidates, 2) {
                            err = err.with_hint(format!("did you mean '{suggestion}'?"));
                        } else {
                            err = err.with_hint(format!("available methods: {}", candidates.join(", ")));
                        }
                    }
                    vec![err]
                })
            }

            Expr::Transform { expr, transforms, span } => {
                let shape_ty = self.infer_expr(expr)?;
                if !is_drawable(&shape_ty) {
                    self.errors.push(Error::new(
                        ErrorCode::S002, span.line, span.column,
                        format!("`@` expects a shape type on the left, found `{}`", type_name(&shape_ty)),
                    ));
                }
                for t in transforms {
                    if let Ok(t_ty) = self.infer_expr(t)
                        && t_ty != Type::Transform {
                            self.errors.push(Error::new(
                                ErrorCode::S002, t.span().line, t.span().column,
                                format!("`@` expects `transform`, found `{}`", type_name(&t_ty)),
                            ));
                        }
                }
                // Preserve the specific shape type through a transform.
                Ok(shape_ty)
            }

            Expr::List(items, span) => {
                if items.is_empty() {
                    // Empty list — Unit placeholder, compatible with any list[T] via types_compatible.
                    return Ok(Type::List(Box::new(Type::Unit)));
                }
                let first_ty = self.infer_expr(&items[0])?;
                for item in items.iter().skip(1) {
                    if let Ok(ty) = self.infer_expr(item)
                        && ty != first_ty {
                            self.errors.push(Error::new(
                                ErrorCode::S002, span.line, span.column,
                                format!(
                                    "list elements must all have the same type, found `{}` and `{}`",
                                    type_name(&first_ty), type_name(&ty)
                                ),
                            ));
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
                for param in params.iter() {
                    let sym = Symbol::new(param.name.clone(), Some(param.ty.clone()), SymbolKind::Param, param.span);
                    self.table.declare(sym);
                }
                for stmt in body.iter() { self.check_stmt(stmt); }
                self.table.pop_scope();
                self.current_fn_return = prev_return;

                Ok(Type::Fn(param_types, ret))
            }

            Expr::StructConstruction { name, fields, span } => {
                let struct_def = self.find_struct_def(name);
                let Some(struct_def) = struct_def else {
                    return Err(vec![Error::new(
                        ErrorCode::S001, span.line, span.column,
                        format!("undefined struct '{name}'"),
                    )]);
                };
                let struct_def = struct_def.clone();

                // Check each provided field
                for (field_name, field_expr) in fields {
                    let field_def = struct_def.fields.iter().find(|f| f.name == *field_name);
                    let Some(field_def) = field_def else {
                        let mut err = Error::new(
                            ErrorCode::S018, span.line, span.column,
                            format!("'{field_name}' is not a field of '{name}'"),
                        );
                        let candidates: Vec<&str> = struct_def.fields.iter().map(|f| f.name.as_str()).collect();
                        if let Some(suggestion) = suggest_similar(field_name, &candidates, 2) {
                            err = err.with_hint(format!("did you mean '{suggestion}'?"));
                        }
                        return Err(vec![err]);
                    };
                    let field_def = field_def.clone();

                    let expr_ty = self.infer_expr(field_expr)?;
                    if let Some(ref expected_ty) = field_def.ty {
                        if !types_compatible(expected_ty, &expr_ty) {
                            return Err(vec![Error::new(
                                ErrorCode::S002, span.line, span.column,
                                format!("field '{field_name}' expects `{}`, got `{}`",
                                    type_name(expected_ty), type_name(&expr_ty)),
                            )]);
                        }
                    }
                }

                // Check that all required fields (no default) are provided
                let provided: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
                for field_def in &struct_def.fields {
                    if field_def.default.is_none() && !provided.contains(&field_def.name.as_str()) {
                        return Err(vec![Error::new(
                            ErrorCode::S017, span.line, span.column,
                            format!("missing required field '{}' in '{name}'", field_def.name),
                        )]);
                    }
                }

                Ok(Type::Named(name.clone()))
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
                if let Some(arg) = args.first()
                    && let Ok(ty) = self.infer_expr(arg)
                        && ty != Type::String {
                            self.errors.push(Error::new(
                                ErrorCode::S002, span.line, span.column,
                                format!("`error` expects a string message, found `{}`", type_name(&ty)),
                            ));
                        }
                let inner = self.current_fn_return.as_ref()
                    .and_then(|ret| match ret {
                        Type::Res(inner) => Some(*inner.clone()),
                        _ => None,
                    })
                    .unwrap_or(Type::Unit);
                return Ok(Type::Res(Box::new(inner)));
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
                return Ok(Type::Color);
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
                Ok(ret_ty.map_or(Type::Unit, |t| *t))
            }
            other => Err(vec![Error::new(
                ErrorCode::S010, span.line, span.column,
                format!("`{callee}` is not callable (type: `{}`)", type_name(&other)),
            )]),
        }
    }

    // ── Operator checking ─────────────────────────────────────────────────────

    fn check_binop(&mut self, op: &BinOp, l: &Type, r: &Type, span: &Span) -> Result<Type, Vec<Error>> {
        // ?? (coalesce): T? ?? T → T
        if *op == BinOp::Coalesce {
            if let Type::Optional(inner) = l {
                if types_compatible(inner, r) {
                    return Ok(*inner.clone());
                }
                return Err(vec![Error::new(
                    ErrorCode::S002, span.line, span.column,
                    format!("`??` default must be `{}`, found `{}`", type_name(inner), type_name(r)),
                )]);
            }
            return Err(vec![Error::new(
                ErrorCode::S008, span.line, span.column,
                format!("`??` requires an optional type on the left, found `{}`", type_name(l)),
            )]);
        }

        // == / != with none or optional types
        if matches!(op, BinOp::Eq | BinOp::NotEq) {
            let is_opt_or_none = |t: &Type| matches!(t, Type::Optional(_));
            if is_opt_or_none(l) || is_opt_or_none(r) {
                // At least one side is optional — allow the comparison
                return Ok(Type::Bool);
            }
        }

        // and / or with truthy types → bool
        if matches!(op, BinOp::And | BinOp::Or) {
            if !is_truthy_type(l) {
                return Err(vec![Error::new(
                    ErrorCode::S008, span.line, span.column,
                    format!("operator `{op}` requires a truthy type, found `{}`", type_name(l)),
                )]);
            }
            if !is_truthy_type(r) {
                return Err(vec![Error::new(
                    ErrorCode::S008, span.line, span.column,
                    format!("operator `{op}` requires a truthy type, found `{}`", type_name(r)),
                )]);
            }
            return Ok(Type::Bool);
        }

        if let (Some(lk), Some(rk)) = (type_to_key(l), type_to_key(r))
            && let Some(ret_key) = self.binops.result_type(op, lk, rk) {
                return Ok(key_to_type(ret_key));
            }
        Err(vec![Error::new(
            ErrorCode::S008, span.line, span.column,
            format!("operator `{op}` not applicable to `{}` and `{}`", type_name(l), type_name(r)),
        )])
    }

    fn check_unop(&mut self, op: &UnOp, operand: &Expr, ty: &Type, span: &Span) -> Result<Type, Vec<Error>> {
        match op {
            UnOp::Neg => {
                if *ty == Type::Float {
                    Ok(Type::Float)
                } else {
                    Err(vec![Error::new(
                        ErrorCode::S008, span.line, span.column,
                        format!("unary `-` requires `float`, found `{}`", type_name(ty)),
                    )])
                }
            }
            UnOp::Not => {
                if is_truthy_type(ty) {
                    Ok(Type::Bool)
                } else {
                    Err(vec![Error::new(
                        ErrorCode::S008, span.line, span.column,
                        format!("`not` requires a truthy type, found `{}`", type_name(ty)),
                    )])
                }
            }
            UnOp::PrefixInc | UnOp::PrefixDec | UnOp::PostfixInc | UnOp::PostfixDec => {
                if *ty != Type::Float {
                    return Err(vec![Error::new(
                        ErrorCode::S008, span.line, span.column,
                        format!("`++`/`--` require `float`, found `{}`", type_name(ty)),
                    )]);
                }
                self.check_assignable_lvalue(operand, span)?;
                Ok(Type::Float)
            }
        }
    }

    fn check_assignable_lvalue(&mut self, expr: &Expr, span: &Span) -> Result<(), Vec<Error>> {
        match expr {
            Expr::Ident(name, _) => {
                let sym = self.lookup_symbol(name, span);
                match sym {
                    Some(s) if s.kind == SymbolKind::Const => Err(vec![Error::new(
                        ErrorCode::S004, span.line, span.column,
                        format!("cannot modify const `{name}`"),
                    )]),
                    Some(_) => Ok(()),
                    None => {
                        let mut err = Error::new(
                            ErrorCode::S001, span.line, span.column,
                            format!("undefined: `{name}`"),
                        );
                        let visible = self.table.all_visible_names();
                        let candidates: Vec<&str> = visible.iter().map(std::string::String::as_str).collect();
                        if let Some(suggestion) = suggest_similar(name, &candidates, 2) {
                            err = err.with_hint(format!("did you mean '{suggestion}'?"));
                        }
                        Err(vec![err])
                    }
                }
            }
            Expr::Field { expr: base, .. } | Expr::Index { expr: base, .. } => {
                self.check_assignable_lvalue(base, span)
            }
            _ => Err(vec![Error::new(
                ErrorCode::S008, span.line, span.column,
                "`++`/`--` require an assignable expression (variable, field, or index)",
            )]),
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
        let member_ty = self.lookup.get_method_type(obj_ty, method)?;
        if let Type::Fn(param_types, ret_ty) = &member_ty {
            // sort() accepts 0 or 1 args (optional comparator) — skip strict count check
            if method == "sort" && matches!(obj_ty, Type::List(_)) && args.len() <= 1 {
                for arg in args {
                    self.infer_expr(arg).ok();
                }
                return Some(ret_ty.clone().map_or(Type::Unit, |t| *t));
            }
            // paste() accepts a single value OR a list — skip strict type check on second arg
            if method == "paste" && matches!(obj_ty, Type::List(_)) && args.len() == 2 {
                // Just validate types without strict matching (list[T] is also acceptable)
                for arg in args {
                    self.infer_expr(arg).ok();
                }
                return Some(ret_ty.clone().map_or(Type::Unit, |t| *t));
            }
            if args.len() == param_types.len() {
                for (arg, expected) in args.iter().zip(param_types.iter()) {
                    match self.infer_expr(arg) {
                        Ok(actual) => self.expect_type(expected, &actual, span),
                        Err(e) => self.errors.extend(e),
                    }
                }
                // Void methods return Unit so the caller can distinguish
                // "method found, void return" from "method not found".
                return Some(ret_ty.clone().map_or(Type::Unit, |t| *t));
            }
            // Wrong arg count
            for arg in args {
                self.infer_expr(arg).ok();
            }
            self.errors.push(Error::new(
                ErrorCode::S007, span.line, span.column,
                format!(
                    "`{method}` expects {} argument{}, got {}",
                    param_types.len(),
                    if param_types.len() == 1 { "" } else { "s" },
                    args.len(),
                ),
            ));
            return Some(ret_ty.clone().map_or(Type::Unit, |t| *t));
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
        if name == "this" {
            if let Some(ref struct_name) = self.current_struct {
                return Ok(Type::Named(struct_name.clone()));
            }
            return Err(vec![Error::new(
                ErrorCode::S021, span.line, span.column,
                "'this' is only valid inside struct methods".to_string(),
            )]);
        }
        let sym = self.lookup_symbol(name, span);
        if let Some(s) = sym { match &s.ty {
            Some(t) => Ok(t.clone()),
            None => Err(vec![Error::new(
                ErrorCode::S001, span.line, span.column,
                format!("`{name}` used before its type could be resolved"),
            )]),
        } } else {
            let mut err = Error::new(
                ErrorCode::S001, span.line, span.column,
                format!("undefined: `{name}`"),
            );
            let visible = self.table.all_visible_names();
            let candidates: Vec<&str> = visible.iter().map(std::string::String::as_str).collect();
            if let Some(suggestion) = suggest_similar(name, &candidates, 2) {
                err = err.with_hint(format!("did you mean '{suggestion}'?"));
            }
            Err(vec![err])
        }
    }

    fn expect_type(&mut self, expected: &Type, actual: &Type, span: &Span) {
        if !types_compatible(expected, actual) {
            self.errors.push(Error::new(
                ErrorCode::S002, span.line, span.column,
                format!("expected `{}`, found `{}`", type_name(expected), type_name(actual)),
            ));
        }
    }
}

// ─── Shape helpers ────────────────────────────────────────────────────────────

/// True for types that support equality (usable in match).
#[must_use] 
pub fn is_matchable(ty: &Type) -> bool {
    matches!(ty, Type::Float | Type::Bool | Type::String | Type::Vec2 | Type::Vec3 | Type::Vec4 | Type::Color)
}

/// True for any type that can be pushed to `out <<` or used with `@`.
#[must_use] 
pub fn is_drawable(ty: &Type) -> bool {
    matches!(ty, Type::Shape | Type::Circle | Type::Rect | Type::Line | Type::Polygon)
}

/// True if `actual` is compatible where `expected` is required.
/// Adds coercion: any concrete shape kind is assignable to `shape`.
#[must_use] 
pub fn types_compatible(expected: &Type, actual: &Type) -> bool {
    if expected == actual { return true; }
    // Concrete shape kind → erased shape
    if *expected == Type::Shape && is_drawable(actual) { return true; }
    // Empty list (list[()]) is compatible with any list[T]
    if let (Type::List(_), Type::List(inner)) = (expected, actual)
        && **inner == Type::Unit { return true; }
    // res<()> is compatible with any res<T> (from error() without context)
    if let (Type::Res(_), Type::Res(inner)) = (expected, actual)
        && **inner == Type::Unit { return true; }
    // Optional compatibility rules
    if let Type::Optional(inner) = expected {
        // none literal (Optional(Unit)) fits any Optional(T)
        if *actual == Type::Optional(Box::new(Type::Unit)) { return true; }
        // T is assignable to T?
        if types_compatible(inner, actual) { return true; }
        // Optional(T) is assignable to Optional(T) — already handled by == above
        // Also handle Optional(T) where inner types are compatible
        if let Type::Optional(actual_inner) = actual
            && types_compatible(inner, actual_inner) { return true; }
    }
    false
}

// ─── Truthiness ──────────────────────────────────────────────────────────────

/// Returns whether a type can be used in a boolean context (conditions, logical ops).
fn is_truthy_type(ty: &Type) -> bool {
    matches!(ty, Type::Bool | Type::Float | Type::String | Type::List(_) | Type::Optional(_))
}

// ─── Cast validation ──────────────────────────────────────────────────────────

fn is_castable(from: &Type, to: &Type) -> bool {
    if from == to {
        return true;
    }
    matches!(
        (from, to),
        (Type::Float | Type::String, Type::Bool)
            | (Type::Bool | Type::String, Type::Float)
            | (Type::Float | Type::Bool, Type::String)
    )
}

// ─── Type display ─────────────────────────────────────────────────────────────

pub fn type_name(ty: &Type) -> String {
    match ty {
        Type::Float           => "float".into(),
        Type::Bool            => "bool".into(),
        Type::String          => "string".into(),
        Type::Unit            => "()".into(),
        Type::Vec2            => "vec2".into(),
        Type::Vec3            => "vec3".into(),
        Type::Vec4            => "vec4".into(),
        Type::Color           => "color".into(),
        Type::Mat3            => "mat3".into(),
        Type::Mat4            => "mat4".into(),
        Type::Transform       => "transform".into(),
        Type::Shape           => "shape".into(),
        Type::Circle          => "circle".into(),
        Type::Rect            => "rect".into(),
        Type::Line            => "line".into(),
        Type::Polygon         => "polygon".into(),
        Type::State           => "State".into(),
        Type::Input           => "Input".into(),
        Type::Array(t, n)     => format!("array[{}, {n}]", type_name(t)),
        Type::List(t)         => format!("list[{}]", type_name(t)),
        Type::Res(t)          => format!("res<{}>", type_name(t)),
        Type::Optional(t)    => format!("{}?", type_name(t)),
        Type::Fn(ps, Some(r)) => format!("fn({}) -> {}", ps.iter().map(type_name).collect::<Vec<_>>().join(", "), type_name(r)),
        Type::Fn(ps, None)    => format!("fn({})", ps.iter().map(type_name).collect::<Vec<_>>().join(", ")),
        Type::Named(n)        => n.clone(),
    }
}
