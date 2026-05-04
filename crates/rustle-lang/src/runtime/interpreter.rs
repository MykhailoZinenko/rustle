//! Tree-walking interpreter. Evaluates AST → Vec<DrawCommand>.
//! All domain-specific calls are dispatched through the NamespaceRegistry.
//! The interpreter itself contains no hardcoded function implementations.

use crate::syntax::ast::{self, AssignTarget, BinOp, Expr, Item, Param, Span, Stmt, UnOp};
use crate::types::draw::DrawCommand;
use crate::types::binop_registry::BinopRegistry;
use crate::types::registry::TypeRegistry;
use crate::error::{ErrorCode, RuntimeError};
use crate::namespaces::{value_type_name, NamespaceRegistry, RuntimeState};
use crate::{Input, State, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

// ─── Environment ──────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Env {
    scopes: Vec<HashMap<String, Value>>,
    output: Rc<RefCell<Vec<DrawCommand>>>,
}

impl Env {
    fn new() -> Self {
        Self { scopes: vec![HashMap::new()], output: Rc::new(RefCell::new(Vec::new())) }
    }

    fn push_scope(&mut self) { self.scopes.push(HashMap::new()); }
    fn pop_scope(&mut self)  { if self.scopes.len() > 1 { self.scopes.pop(); } }

    fn declare(&mut self, name: &str, val: Value) {
        self.scopes.last_mut().unwrap().insert(name.to_string(), val);
    }

    fn set(&mut self, name: &str, val: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains_key(name) {
                scope.insert(name.to_string(), val);
                return true;
            }
        }
        false
    }

    fn get(&self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) { return Some(v.clone()); }
        }
        None
    }

    fn emit(&self, cmd: DrawCommand) {
        self.output.borrow_mut().push(cmd);
    }
}

const MAX_CALL_DEPTH: usize = 256;

// ─── Interpreter ──────────────────────────────────────────────────────────────

pub struct Interpreter<'a> {
    program: &'a ast::Program,
    fn_table: HashMap<&'a str, &'a ast::FnDef>,
    registry: &'a NamespaceRegistry,
    binops: &'a BinopRegistry,
    types: &'a TypeRegistry,
    env: Env,
    return_value: Option<Value>,
    break_flag: bool,
    continue_flag: bool,
    call_depth: usize,
    runtime_state: RuntimeState,
    cancel: Option<Arc<AtomicBool>>,
}

impl<'a> Interpreter<'a> {
    pub fn new(
        program: &'a ast::Program,
        registry: &'a NamespaceRegistry,
        types: &'a TypeRegistry,
        binops: &'a BinopRegistry,
    ) -> Self {
        let fn_table = program.items.iter().filter_map(|item| match item {
            ast::Item::FnDef(f) => Some((f.name.as_str(), f)),
            _ => None,
        }).collect();
        Self {
            program,
            fn_table,
            registry,
            binops,
            types,
            env: Env::new(),
            return_value: None,
            break_flag: false,
            continue_flag: false,
            call_depth: 0,
            runtime_state: RuntimeState::default(),
            cancel: None,
        }
    }

    /// Seed the interpreter with persisted runtime state (coord_meta, etc.) from
    /// a prior init or tick so that resolution/origin survive across frames.
    pub fn with_runtime_state(mut self, rs: RuntimeState) -> Self {
        self.runtime_state = rs;
        self
    }

    /// Set a cancellation token. When the flag is set to `true`, the interpreter
    /// will abort at the next loop iteration or function call boundary.
    pub fn with_cancel(mut self, cancel: Arc<AtomicBool>) -> Self {
        self.cancel = Some(cancel);
        self
    }

    fn check_cancel(&self, line: usize) -> Result<(), RuntimeError> {
        if let Some(ref c) = self.cancel {
            if c.load(Ordering::Relaxed) {
                return Err(self.err(ErrorCode::R010, line, "script cancelled"));
            }
        }
        Ok(())
    }

    /// Check whether a named function is defined in the program.
    pub fn has_fn(&self, name: &str) -> bool {
        self.fn_table.contains_key(name)
    }

    /// Extract the final runtime state after running (captures resolution/origin calls).
    pub fn take_runtime_state(&self) -> RuntimeState {
        self.runtime_state.clone()
    }

    fn should_stop_block(&self) -> bool {
        self.return_value.is_some() || self.break_flag || self.continue_flag
    }

    fn err(&self, code: ErrorCode, line: usize, msg: impl Into<String>) -> RuntimeError {
        RuntimeError::new(code, line, msg)
    }

    // ─── Imports ──────────────────────────────────────────────────────────────

    /// Bind all import declarations into the current environment.
    /// - `import shapes`           → `shapes = Namespace("shapes")`
    /// - `import shapes { circle }` → `circle = NativeFn("circle")`
    /// - `import render { sdf }`   → `sdf = RenderMode(Sdf)`  (constant)
    pub fn setup_imports(&mut self) {
        let program = self.program;
        for import in &program.imports {
            if import.members.is_empty() {
                self.env.declare(&import.namespace, Value::Namespace(import.namespace.clone()));
            } else {
                let exports: Vec<_> = {
                    let Some(ns) = self.registry.get(&import.namespace) else { continue };
                    import.members.iter()
                        .filter_map(|m| ns.get_export(m).map(|e| (m.clone(), e)))
                        .collect()
                };
                for (member, export) in exports {
                    use crate::namespaces::ExportKind;
                    let val = match export.kind {
                        ExportKind::Function => Value::NativeFn(member.clone()),
                        ExportKind::Constant => self.registry.get_constant(&member)
                            .unwrap_or(Value::NativeFn(member.clone())),
                    };
                    self.env.declare(&member, val);
                }
            }
        }
    }

    // ─── Entry points ─────────────────────────────────────────────────────────

    pub fn run_top_level(&mut self) -> Result<(), RuntimeError> {
        self.setup_imports();
        let program = self.program;
        for item in &program.items {
            if let Item::Stmt(s) = item { self.exec_stmt(s)?; }
        }
        Ok(())
    }

    pub fn run_update(&mut self, state: State, input: &Input) -> Result<State, RuntimeError> {
        let Some(f) = self.fn_table.get("on_update").copied() else { return Ok(state); };

        self.run_top_level()?;

        let state_rc = Rc::new(RefCell::new(state.0));
        let state_val = Value::State(state_rc.clone());
        let input_val = Value::Input { dt: input.dt };

        self.env.push_scope();
        if let Some(p) = f.params.first()  { self.env.declare(&p.name, state_val); }
        if let Some(p) = f.params.get(1)   { self.env.declare(&p.name, input_val); }

        self.return_value = None;
        for stmt in f.body.iter() {
            match self.exec_stmt(stmt) {
                Ok(()) => {}
                Err(mut e) => {
                    e.push_frame("on_update", 0);
                    self.env.pop_scope();
                    return Err(e);
                }
            }
            if self.should_stop_block() { break; }
        }
        self.env.pop_scope();

        let new_map = match self.return_value.take() {
            Some(Value::State(rc)) => rc.borrow().clone(),
            _ => state_rc.borrow().clone(),
        };
        Ok(State(new_map))
    }

    pub fn run_init(&mut self, state: State) -> Result<State, RuntimeError> {
        let Some(f) = self.fn_table.get("on_init").copied() else { return Ok(state); };

        let state_rc = Rc::new(RefCell::new(state.0));
        let state_val = Value::State(state_rc.clone());

        self.env.push_scope();
        if let Some(p) = f.params.first() { self.env.declare(&p.name, state_val); }

        self.return_value = None;
        for stmt in f.body.iter() {
            match self.exec_stmt(stmt) {
                Ok(()) => {}
                Err(mut e) => {
                    e.push_frame("on_init", 0);
                    self.env.pop_scope();
                    return Err(e);
                }
            }
            if self.should_stop_block() { break; }
        }
        self.env.pop_scope();

        let new_map = match self.return_value.take() {
            Some(Value::State(rc)) => rc.borrow().clone(),
            _ => state_rc.borrow().clone(),
        };
        Ok(State(new_map))
    }

    pub fn run_on_exit(&mut self, state: State) -> Result<State, RuntimeError> {
        let Some(f) = self.fn_table.get("on_exit").copied() else { return Ok(state); };

        let state_rc = Rc::new(RefCell::new(state.0));
        let state_val = Value::State(state_rc.clone());

        self.env.push_scope();
        if let Some(p) = f.params.first() { self.env.declare(&p.name, state_val); }

        self.return_value = None;
        for stmt in f.body.iter() {
            match self.exec_stmt(stmt) {
                Ok(()) => {}
                Err(mut e) => {
                    e.push_frame("on_exit", 0);
                    self.env.pop_scope();
                    return Err(e);
                }
            }
            if self.should_stop_block() { break; }
        }
        self.env.pop_scope();

        let new_map = match self.return_value.take() {
            Some(Value::State(rc)) => rc.borrow().clone(),
            _ => state_rc.borrow().clone(),
        };
        Ok(State(new_map))
    }

    pub fn take_output(&self) -> Vec<DrawCommand> {
        self.env.output.borrow_mut().drain(..).collect()
    }

    // ─── Expression evaluator ─────────────────────────────────────────────────

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Float(v, _)     => Ok(Value::Float(*v)),
            Expr::Bool(v, _)      => Ok(Value::Bool(*v)),
            Expr::None(_)         => Ok(Value::None),
            Expr::StringLit(s, _) => Ok(Value::Str(s.clone())),
            Expr::HexColor(s, _)  => parse_hex_color(s),

            Expr::Ident(name, span) => {
                self.env.get(name)
                    .or_else(|| self.registry.get_constant(name))
                    .or_else(|| {
                        self.fn_table.get(name.as_str()).map(|f| Value::Closure {
                            params: f.params.clone(),
                            body: f.body.clone(),
                            captured: HashMap::new(),
                        })
                    })
                    .ok_or_else(|| self.err(ErrorCode::R002, span.line, format!("undefined: `{name}`")))
            }

            Expr::BinOp { left, op, right, span } => {
                // Short-circuit operators: ??, and, or
                if *op == BinOp::Coalesce {
                    let l = self.eval_expr(left)?;
                    return match l {
                        Value::None => self.eval_expr(right),
                        other => Ok(other),
                    };
                }
                if *op == BinOp::And {
                    let l = self.eval_expr(left)?;
                    if !l.is_truthy() { return Ok(Value::Bool(false)); }
                    let r = self.eval_expr(right)?;
                    return Ok(Value::Bool(r.is_truthy()));
                }
                if *op == BinOp::Or {
                    let l = self.eval_expr(left)?;
                    if l.is_truthy() { return Ok(Value::Bool(true)); }
                    let r = self.eval_expr(right)?;
                    return Ok(Value::Bool(r.is_truthy()));
                }
                let l = self.eval_expr(left)?;
                let r = self.eval_expr(right)?;
                // == / != with none values
                if matches!(op, BinOp::Eq | BinOp::NotEq) {
                    if matches!(&l, Value::None) || matches!(&r, Value::None) {
                        let both_none = matches!((&l, &r), (Value::None, Value::None));
                        return Ok(Value::Bool(if *op == BinOp::Eq { both_none } else { !both_none }));
                    }
                }
                eval_binop(op, l, r, span.line, &self.binops)
            }

            Expr::UnOp { op, operand, span } => {
                match op {
                    UnOp::PrefixInc | UnOp::PrefixDec | UnOp::PostfixInc | UnOp::PostfixDec => {
                        self.eval_inc_dec(op, operand, span)
                    }
                    _ => {
                        let v = self.eval_expr(operand)?;
                        eval_unop(op, v, span.line)
                    }
                }
            }

            Expr::Ternary { condition, then_expr, else_expr, .. } => {
                if self.eval_expr(condition)?.is_truthy() {
                    self.eval_expr(then_expr)
                } else {
                    self.eval_expr(else_expr)
                }
            }

            Expr::Cast { expr, .. } => self.eval_expr(expr),

            Expr::Try { expr, .. } => {
                // `try expr` wraps the result into res<T>.
                // Success → ResOk(value), runtime error → ResErr(message).
                match self.eval_expr(expr) {
                    Ok(v)  => Ok(Value::ResOk(Box::new(v))),
                    Err(e) => Ok(Value::ResErr(e.message)),
                }
            }

            Expr::Call { callee, args, named_args, span } => {
                self.eval_call(callee, args, named_args, span)
            }

            Expr::Index { expr, index, span } => {
                let coll = self.eval_expr(expr)?;
                let idx  = self.eval_expr(index)?;
                let i = safe_index(&idx, span.line)?;
                match coll {
                    Value::List(items) => {
                        let guard = items.borrow();
                        guard.get(i).cloned()
                            .ok_or_else(|| self.err(ErrorCode::R005, span.line,
                                format!("index {} out of bounds (list has {} elements)", i, guard.len())))
                    }
                    _ => Err(self.err(ErrorCode::R001, span.line, format!(
                        "cannot index `{}`", value_type_name(&coll)
                    ))),
                }
            }

            Expr::Field { expr, field, span } => {
                let obj = self.eval_expr(expr)?;
                eval_field(&self.types, &obj, field, span.line)
            }

            Expr::OptionalChain { expr, field, span } => {
                let obj = self.eval_expr(expr)?;
                match obj {
                    Value::None => Ok(Value::None),
                    other => eval_field(&self.types, &other, field, span.line),
                }
            }

            Expr::MethodCall { expr, method, args, named_args, span } => {
                let obj = self.eval_expr(expr)?;
                self.eval_method(obj, method, args, named_args, span)
            }

            Expr::Transform { expr, transforms, span } => {
                let mut shape = self.eval_expr(expr)?;
                for t in transforms {
                    let tf = self.eval_expr(t)?;
                    shape = apply_transform(shape, tf, span.line)?;
                }
                Ok(shape)
            }

            Expr::List(items, _) => {
                let vals: Result<Vec<_>, _> = items.iter().map(|e| self.eval_expr(e)).collect();
                Ok(Value::List(Rc::new(RefCell::new(vals?))))
            }

            Expr::Lambda { params, body, .. } => {
                let needed = free_vars_in_body(params, body);
                let captured = self.env.scopes.iter()
                    .flat_map(|s| s.iter())
                    .filter(|(k, _)| needed.contains(k.as_str()))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                Ok(Value::Closure { params: params.clone(), body: body.clone(), captured })
            }
        }
    }

    // ─── Call dispatch ────────────────────────────────────────────────────────

    fn eval_call(
        &mut self,
        callee: &str,
        args: &[Expr],
        named_args: &[(String, Expr)],
        span: &Span,
    ) -> Result<Value, RuntimeError> {
        let arg_vals: Vec<Value> = args.iter()
            .map(|a| self.eval_expr(a))
            .collect::<Result<_, _>>()?;
        let named: HashMap<String, Value> = named_args.iter()
            .map(|(k, v)| self.eval_expr(v).map(|val| (k.clone(), val)))
            .collect::<Result<_, _>>()?;

        // 1. Env — NativeFn, Closure, or fn-var
        if let Some(val) = self.env.get(callee) {
            match val {
                Value::NativeFn(ref name) => {
                    let n = name.clone();
                    return self.registry.call_any(&n, &arg_vals, &named, &mut self.runtime_state, span.line)?
                        .ok_or_else(|| self.err(ErrorCode::R004, span.line, format!("unknown native fn: `{n}`")));
                }
                Value::Closure { params, body, captured } => {
                    return self.call_closure(callee, &params, &body, &captured, &arg_vals, span.line);
                }
                _ => {} // fall through — may be a user fn with same name
            }
        }

        // 2. Registry (all namespace providers: core, shapes, render, coords)
        if let Some(v) = self.registry.call_any(callee, &arg_vals, &named, &mut self.runtime_state, span.line)? {
            return Ok(v);
        }

        // 3. User-defined functions (FnDef items)
        if let Some(f) = self.fn_table.get(callee).copied() {
            if f.params.len() != arg_vals.len() {
                return Err(self.err(ErrorCode::R008, span.line, format!(
                    "`{}` expects {} args, got {}", f.name, f.params.len(), arg_vals.len()
                )));
            }
            return self.call_fn(&f.name, &f.params, &f.body, &arg_vals, span.line);
        }

        Err(self.err(ErrorCode::R002, span.line, format!("undefined function: `{callee}`")))
    }

    fn call_fn(
        &mut self,
        name: &str,
        params: &[Param],
        body: &[Stmt],
        arg_vals: &[Value],
        call_line: usize,
    ) -> Result<Value, RuntimeError> {
        self.check_cancel(call_line)?;
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(self.err(ErrorCode::R011, call_line,
                format!("maximum call depth ({MAX_CALL_DEPTH}) exceeded — possible infinite recursion")));
        }
        self.call_depth += 1;
        self.env.push_scope();
        for (p, v) in params.iter().zip(arg_vals) {
            self.env.declare(&p.name, v.clone());
        }
        let saved = self.return_value.take();
        let mut err_result = None;
        for stmt in body {
            match self.exec_stmt(stmt) {
                Ok(()) => {}
                Err(mut e) => {
                    e.push_frame(name, call_line);
                    err_result = Some(e);
                    break;
                }
            }
            if self.should_stop_block() { break; }
        }
        self.call_depth -= 1;
        if let Some(e) = err_result {
            self.return_value = saved;
            self.env.pop_scope();
            return Err(e);
        }
        let result = self.return_value.take().unwrap_or(Value::Float(0.0));
        self.return_value = saved;
        self.env.pop_scope();
        Ok(result)
    }

    fn call_closure(
        &mut self,
        name: &str,
        params: &[Param],
        body: &[Stmt],
        captured: &HashMap<String, Value>,
        arg_vals: &[Value],
        call_line: usize,
    ) -> Result<Value, RuntimeError> {
        if params.len() != arg_vals.len() {
            return Err(self.err(ErrorCode::R008, call_line, format!(
                "closure expects {} args, got {}", params.len(), arg_vals.len()
            )));
        }
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(self.err(ErrorCode::R011, call_line,
                format!("maximum call depth ({MAX_CALL_DEPTH}) exceeded — possible infinite recursion")));
        }
        self.call_depth += 1;
        self.env.push_scope();
        for (k, v) in captured { self.env.declare(k, v.clone()); }
        for (p, v) in params.iter().zip(arg_vals) { self.env.declare(&p.name, v.clone()); }
        let saved = self.return_value.take();
        let mut err_result = None;
        for stmt in body {
            match self.exec_stmt(stmt) {
                Ok(()) => {}
                Err(mut e) => {
                    e.push_frame(name, call_line);
                    err_result = Some(e);
                    break;
                }
            }
            if self.should_stop_block() { break; }
        }
        self.call_depth -= 1;
        if let Some(e) = err_result {
            self.return_value = saved;
            self.env.pop_scope();
            return Err(e);
        }
        let result = self.return_value.take().unwrap_or(Value::Float(0.0));
        self.return_value = saved;
        self.env.pop_scope();
        Ok(result)
    }

    // ─── Method dispatch ──────────────────────────────────────────────────────

    fn eval_method(
        &mut self,
        obj: Value,
        method: &str,
        args: &[Expr],
        named_args: &[(String, Expr)],
        span: &Span,
    ) -> Result<Value, RuntimeError> {
        // Namespace dot-calls go through NamespaceRegistry (handles constants,
        // named args, and per-namespace dispatch).
        if let Value::Namespace(ns_name) = &obj {
            let ns_name = ns_name.clone();
            if let Some(ns) = self.registry.get(&ns_name) {
                if let Some(export) = ns.get_export(method) {
                    use crate::namespaces::ExportKind;
                    if export.kind == ExportKind::Constant {
                        return ns.get_constant(method)
                            .or_else(|| self.registry.get_constant(method))
                            .ok_or_else(|| self.err(ErrorCode::R004, span.line, format!(
                                "`{ns_name}.{method}` has no runtime value"
                            )));
                    } else {
                        let arg_vals: Vec<Value> = args.iter()
                            .map(|a| self.eval_expr(a))
                            .collect::<Result<_, _>>()?;
                        let named_vals: HashMap<String, Value> = named_args.iter()
                            .map(|(k, v)| self.eval_expr(v).map(|val| (k.clone(), val)))
                            .collect::<Result<_, _>>()?;
                        return ns.call(method, &arg_vals, &named_vals, &mut self.runtime_state, span.line)?
                            .ok_or_else(|| self.err(ErrorCode::R004, span.line, format!(
                                "`{ns_name}` does not implement `{method}`"
                            )));
                    }
                }
            }
            return Err(self.err(ErrorCode::R004, span.line, format!("`{ns_name}` has no member `{method}`")));
        }

        // All other types: evaluate args, delegate to TypeRegistry.
        let arg_vals: Vec<Value> = args.iter()
            .map(|a| self.eval_expr(a))
            .collect::<Result<_, _>>()?;

        self.types.call_method(&obj, method, &arg_vals, span.line)
            .unwrap_or_else(|| Err(self.err(ErrorCode::R004, span.line, format!(
                "`{}` has no method `{method}`", value_type_name(&obj)
            ))))
    }

    // ─── Statement executor ───────────────────────────────────────────────────

    pub fn exec_stmt(&mut self, stmt: &Stmt) -> Result<(), RuntimeError> {
        match stmt {
            Stmt::VarDecl(v) => {
                let val = self.eval_expr(&v.initializer)?;
                self.env.declare(&v.name, val);
            }

            Stmt::Assign(a) => {
                let val = self.eval_expr(&a.value)?;
                match &a.target {
                    AssignTarget::Path(p) if p.len() == 1 => {
                        let name = &p[0];
                        if !self.env.set(name, val) {
                            return Err(self.err(ErrorCode::R002, a.span.line, format!("undefined: `{name}`")));
                        }
                    }
                    AssignTarget::Path(p) => {
                        let root = &p[0];
                        let obj = self.env.get(root)
                            .ok_or_else(|| self.err(ErrorCode::R002, a.span.line, format!("undefined: `{root}`")))?;
                        if let Value::State(rc) = &obj {
                            assign_state_path(rc, &p[1..], val, a.span.line, &self.types)?;
                        } else {
                            let updated = set_field_path(&self.types, obj, &p[1..], val, a.span.line)?;
                            self.env.set(root, updated);
                        }
                    }
                    AssignTarget::Indexed { path: p, indices } => {
                        self.exec_indexed_assign(p, indices, val, a.span.line)?;
                    }
                }
            }

            Stmt::Out(o) => {
                for expr in &o.shapes {
                    match self.eval_expr(expr)? {
                        Value::Shape(data) => self.env.emit(DrawCommand::DrawShape(data)),
                        other => return Err(self.err(ErrorCode::R001, o.span.line, format!(
                            "out << expects shape, got `{}`", value_type_name(&other)
                        ))),
                    }
                }
            }

            Stmt::Print(p) => {
                let parts: Result<Vec<String>, _> = p.values.iter()
                    .map(|e| self.eval_expr(e).map(|v| v.to_string()))
                    .collect();
                let msg = parts?.join(" ");
                match p.level {
                    ast::PrintLevel::Log   => {
                        println!("{msg}");
                        self.env.emit(DrawCommand::Print(msg));
                    }
                    ast::PrintLevel::Warn  => {
                        eprintln!("[warn] {msg}");
                        self.env.emit(DrawCommand::Warn(msg));
                    }
                    ast::PrintLevel::Error => {
                        eprintln!("[error] {msg}");
                        self.env.emit(DrawCommand::Error(msg));
                    }
                }
            }

            Stmt::If(i) => {
                let branch = if self.eval_expr(&i.condition)?.is_truthy() {
                    Some(&i.then_block)
                } else {
                    i.else_block.as_ref()
                };
                if let Some(block) = branch {
                    self.env.push_scope();
                    for s in block {
                        self.exec_stmt(s)?;
                        if self.should_stop_block() { break; }
                    }
                    self.env.pop_scope();
                }
            }

            Stmt::IfLet { binding, expr, then_block, else_block, .. } => {
                let val = self.eval_expr(expr)?;
                match val {
                    Value::None => {
                        if let Some(els) = else_block {
                            self.env.push_scope();
                            for s in els {
                                self.exec_stmt(s)?;
                                if self.should_stop_block() { break; }
                            }
                            self.env.pop_scope();
                        }
                    }
                    other => {
                        self.env.push_scope();
                        self.env.declare(binding, other);
                        for s in then_block {
                            self.exec_stmt(s)?;
                            if self.should_stop_block() { break; }
                        }
                        self.env.pop_scope();
                    }
                }
            }

            Stmt::Match(m) => {
                let scrut = self.eval_expr(&m.expr)?;
                let mut matched = false;
                for arm in &m.arms {
                    if arm.values.is_empty() {
                        // else arm
                        matched = true;
                    } else {
                        for val_expr in &arm.values {
                            let val = self.eval_expr(val_expr)?;
                            if values_equal(&scrut, &val) {
                                matched = true;
                                break;
                            }
                        }
                    }
                    if matched {
                        self.env.push_scope();
                        for s in &arm.body {
                            self.exec_stmt(s)?;
                            if self.should_stop_block() { break; }
                        }
                        self.env.pop_scope();
                        break;
                    }
                }
                // no match and no else: nothing happens
            }

            Stmt::While(w) => {
                loop {
                    self.check_cancel(w.span.line)?;
                    if !self.eval_expr(&w.condition)?.is_truthy() { break; }
                    self.env.push_scope();
                    for s in &w.body {
                        self.exec_stmt(s)?;
                        if self.should_stop_block() { break; }
                    }
                    self.env.pop_scope();
                    if self.continue_flag { self.continue_flag = false; continue; }
                    if self.break_flag { self.break_flag = false; break; }
                    if self.return_value.is_some() { break; }
                }
            }

            Stmt::For(f) => {
                self.env.push_scope();
                self.exec_stmt(&f.init)?;
                loop {
                    self.check_cancel(f.span.line)?;
                    if !self.eval_expr(&f.condition)?.is_truthy() { break; }
                    self.env.push_scope();
                    for s in &f.body {
                        self.exec_stmt(s)?;
                        if self.should_stop_block() { break; }
                    }
                    self.env.pop_scope();
                    if self.continue_flag { self.continue_flag = false; }
                    else if self.break_flag { self.break_flag = false; break; }
                    else if self.return_value.is_some() { break; }
                    self.exec_stmt(&f.step)?;
                }
                self.env.pop_scope();
            }

            Stmt::Foreach(f) => {
                let list = match self.eval_expr(&f.iterable)? {
                    Value::List(items) => items.borrow().clone(),
                    other => return Err(self.err(ErrorCode::R001, f.span.line, format!(
                        "foreach expects list, got `{}`", value_type_name(&other)
                    ))),
                };
                for item in list {
                    self.check_cancel(f.span.line)?;
                    self.env.push_scope();
                    self.env.declare(&f.var_name, item);
                    for s in &f.body {
                        self.exec_stmt(s)?;
                        if self.should_stop_block() { break; }
                    }
                    self.env.pop_scope();
                    if self.continue_flag { self.continue_flag = false; continue; }
                    if self.break_flag { self.break_flag = false; break; }
                    if self.return_value.is_some() { break; }
                }
            }

            Stmt::Return(expr, _) => {
                let val = match expr {
                    Some(e) => self.eval_expr(e)?,
                    None    => Value::Float(0.0),
                };
                self.return_value = Some(val);
            }

            Stmt::Break(_) => { self.break_flag = true; }
            Stmt::Continue(_) => { self.continue_flag = true; }

            Stmt::FnVar { name, value, .. } => {
                let val = self.eval_expr(value)?;
                self.env.declare(name, val);
            }

            Stmt::Expr(e) => { self.eval_expr(e)?; }
        }
        Ok(())
    }

    fn eval_inc_dec(&mut self, op: &UnOp, operand: &Expr, span: &Span) -> Result<Value, RuntimeError> {
        let target = expr_to_assign_target(operand)
            .ok_or_else(|| self.err(ErrorCode::R011, span.line, "`++`/`--` require an assignable expression"))?;
        let old = self.read_assign_target(&target, span.line)?;
        let x = as_float(&old, span.line)?;
        let new_val = Value::Float(match op {
            UnOp::PrefixInc | UnOp::PostfixInc => x + 1.0,
            UnOp::PrefixDec | UnOp::PostfixDec => x - 1.0,
            _ => unreachable!(),
        });
        self.write_assign_target(&target, new_val.clone(), span.line)?;
        Ok(match op {
            UnOp::PrefixInc | UnOp::PrefixDec => new_val,
            UnOp::PostfixInc | UnOp::PostfixDec => old,
            _ => unreachable!(),
        })
    }

    fn read_assign_target(&mut self, target: &AssignTarget, line: usize) -> Result<Value, RuntimeError> {
        match target {
            AssignTarget::Path(p) if p.len() == 1 => self.env.get(&p[0])
                .ok_or_else(|| self.err(ErrorCode::R002, line, format!("undefined: `{}`", p[0]))),
            AssignTarget::Path(p) => {
                let root = self.env.get(&p[0])
                    .ok_or_else(|| self.err(ErrorCode::R002, line, format!("undefined: `{}`", p[0])))?;
                let mut v = root;
                for seg in &p[1..] {
                    v = eval_field(&self.types, &v, seg, line)?;
                }
                Ok(v)
            }
            AssignTarget::Indexed { path: p, indices } => {
                let mut coll = self.env.get(&p[0])
                    .ok_or_else(|| self.err(ErrorCode::R002, line, format!("undefined: `{}`", p[0])))?
                    .clone();
                for seg in &p[1..] {
                    coll = eval_field(&self.types, &coll, seg, line)?;
                }
                for idx_expr in indices {
                    let idx = self.eval_expr(idx_expr)?;
                    let i = safe_index(&idx, line)?;
                    coll = match &coll {
                        Value::List(items) => {
                            let guard = items.borrow();
                            guard.get(i).cloned()
                                .ok_or_else(|| self.err(ErrorCode::R005, line,
                                    format!("index {} out of bounds (list has {} elements)", i, guard.len())))
                        }
                        _ => Err(self.err(ErrorCode::R001, line, format!(
                            "cannot index `{}`", value_type_name(&coll)
                        ))),
                    }?;
                }
                Ok(coll)
            }
        }
    }

    fn write_assign_target(&mut self, target: &AssignTarget, val: Value, line: usize) -> Result<(), RuntimeError> {
        match target {
            AssignTarget::Path(p) if p.len() == 1 => {
                if !self.env.set(&p[0], val) {
                    return Err(self.err(ErrorCode::R002, line, format!("undefined: `{}`", p[0])));
                }
            }
            AssignTarget::Path(p) => {
                let root = &p[0];
                let obj = self.env.get(root)
                    .ok_or_else(|| self.err(ErrorCode::R002, line, format!("undefined: `{root}`")))?;
                if let Value::State(rc) = &obj {
                    assign_state_path(rc, &p[1..], val, line, &self.types)?;
                } else {
                    let updated = set_field_path(&self.types, obj, &p[1..], val, line)?;
                    self.env.set(root, updated);
                }
            }
            AssignTarget::Indexed { path: p, indices } => {
                self.exec_indexed_assign(p, indices, val, line)?;
            }
        }
        Ok(())
    }

    fn exec_indexed_assign(
        &mut self,
        path: &[String],
        indices: &[Expr],
        val: Value,
        line: usize,
    ) -> Result<(), RuntimeError> {
        let root = &path[0];
        let mut coll = self.env.get(root)
            .ok_or_else(|| self.err(ErrorCode::R002, line, format!("undefined: `{root}`")))?
            .clone();
        for p in path.iter().skip(1) {
            coll = eval_field(&self.types, &coll, p, line)?;
        }
        for idx_expr in &indices[..indices.len().saturating_sub(1)] {
            let idx = self.eval_expr(idx_expr)?;
            let i = safe_index(&idx, line)?;
            coll = match &coll {
                Value::List(items) => {
                    let guard = items.borrow();
                    guard.get(i).cloned()
                        .ok_or_else(|| self.err(ErrorCode::R005, line,
                            format!("index {} out of bounds (list has {} elements)", i, guard.len())))
                }
                _ => Err(self.err(ErrorCode::R001, line, format!(
                    "cannot index `{}`", value_type_name(&coll)
                ))),
            }?;
        }
        let last_idx = indices.last().unwrap();
        let idx = self.eval_expr(last_idx)?;
        let i = safe_index(&idx, line)?;
        match &coll {
            Value::List(items) => {
                let mut guard = items.borrow_mut();
                if i >= guard.len() {
                    return Err(self.err(ErrorCode::R005, line,
                        format!("index {} out of bounds (list has {} elements)", i, guard.len())));
                }
                guard[i] = val;
            }
            _ => return Err(self.err(ErrorCode::R001, line, format!(
                "cannot assign to index of `{}`", value_type_name(&coll)
            ))),
        }
        Ok(())
    }
}

/// Convert an assignable Expr (Ident, Field, Index) to AssignTarget.
fn expr_to_assign_target(expr: &Expr) -> Option<AssignTarget> {
    match expr {
        Expr::Ident(name, _) => Some(AssignTarget::Path(vec![name.clone()])),
        Expr::Field { expr: base, field, .. } => {
            let mut p = expr_to_assign_target(base)?.path().to_vec();
            p.push(field.clone());
            Some(AssignTarget::Path(p))
        }
        Expr::Index { expr: base, index, .. } => {
            let base_target = expr_to_assign_target(base)?;
            match base_target {
                AssignTarget::Path(p) => Some(AssignTarget::Indexed {
                    path: p,
                    indices: vec![(**index).clone()],
                }),
                AssignTarget::Indexed { path, mut indices } => {
                    indices.push((**index).clone());
                    Some(AssignTarget::Indexed { path, indices })
                }
            }
        }
        _ => None,
    }
}

// ─── Field access ─────────────────────────────────────────────────────────────

fn eval_field(types: &TypeRegistry, obj: &Value, field: &str, line: usize) -> Result<Value, RuntimeError> {
    // State fields are dynamic (per-script) — not in the static registry.
    if let Value::State(rc) = obj {
        let guard = rc.borrow();
        return guard.get(field).cloned()
            .ok_or_else(|| {
                let mut keys: Vec<&str> = guard.keys().map(|k| k.as_str()).collect();
                keys.sort();
                RuntimeError::new(ErrorCode::R003, line,
                    format!("state has no field `{field}` (available: {})", keys.join(", ")))
            });
    }
    types.get_field(obj, field)
        .ok_or_else(|| RuntimeError::new(ErrorCode::R003, line, format!(
            "`{}` has no field `{field}`", value_type_name(obj)
        )))
}

// ─── Dotted-path assignment ───────────────────────────────────────────────────

/// Assign into a State value at `path`, mutating through the Rc in-place.
fn assign_state_path(
    rc: &std::cell::RefCell<std::collections::HashMap<String, Value>>,
    path: &[String],
    val: Value,
    line: usize,
    types: &TypeRegistry,
) -> Result<(), RuntimeError> {
    let field = &path[0];
    if path.len() == 1 {
        rc.borrow_mut().insert(field.clone(), val);
    } else {
        let guard = rc.borrow();
        let intermediate = guard.get(field.as_str()).cloned()
            .ok_or_else(|| {
                let mut keys: Vec<&str> = guard.keys().map(|k| k.as_str()).collect();
                keys.sort();
                RuntimeError::new(ErrorCode::R003, line,
                    format!("state has no field `{field}` (available: {})", keys.join(", ")))
            })?;
        drop(guard);
        let updated = set_field_path(types, intermediate, &path[1..], val, line)?;
        rc.borrow_mut().insert(field.clone(), updated);
    }
    Ok(())
}

/// Produce a new Value with the nested field at `path` replaced by `val`.
/// Used for local-variable dotted assignment: `v.x = 1.0`, `c.r = 0.5`, etc.
fn set_field_path(types: &TypeRegistry, obj: Value, path: &[String], val: Value, line: usize) -> Result<Value, RuntimeError> {
    if path.is_empty() { return Ok(val); }
    let field = path[0].as_str();
    let new_val = if path.len() > 1 {
        // Nested: get the sub-value, recurse, then write it back.
        let sub = types.get_field(&obj, field)
            .ok_or_else(|| RuntimeError::new(ErrorCode::R003, line, format!(
                "`{}` has no field `{field}`", value_type_name(&obj)
            )))?;
        set_field_path(types, sub, &path[1..], val, line)?
    } else {
        val
    };
    types.set_field(obj, field, new_val)
        .ok_or_else(|| RuntimeError::new(ErrorCode::R003, line, format!(
            "cannot assign field `{field}` (read-only or unknown)"
        )))
}

// ─── Transform application ────────────────────────────────────────────────────

fn apply_transform(shape: Value, tf: Value, line: usize) -> Result<Value, RuntimeError> {
    let Value::Transform(td) = tf else {
        return Err(RuntimeError::new(ErrorCode::R001, line, format!(
            "@  requires transform, got `{}`", value_type_name(&tf)
        )));
    };
    let Value::Shape(mut data) = shape else {
        return Err(RuntimeError::new(ErrorCode::R001, line, format!(
            "@ can only be applied to shape, got `{}`", value_type_name(&shape)
        )));
    };
    data.transforms.push(td);
    Ok(Value::Shape(data))
}

// ─── Binary / unary operators ─────────────────────────────────────────────────

fn eval_binop(op: &BinOp, l: Value, r: Value, line: usize, binops: &BinopRegistry) -> Result<Value, RuntimeError> {
    // Eq / NotEq — generic structural equality, no registry needed
    if let BinOp::Eq   = op { return Ok(Value::Bool(values_equal(&l, &r))); }
    if let BinOp::NotEq = op { return Ok(Value::Bool(!values_equal(&l, &r))); }

    // All other operators go through the registry
    binops.eval(op, l, r, line).unwrap_or_else(|| {
        Err(RuntimeError::new(ErrorCode::R011, line, format!(
            "operator `{}` not supported for these types",
            match op {
                BinOp::Add  => "+",  BinOp::Sub  => "-",
                BinOp::Mul  => "*",  BinOp::Div  => "/",  BinOp::Mod  => "%",
                BinOp::Lt   => "<",  BinOp::LtEq => "<=",
                BinOp::Gt   => ">",  BinOp::GtEq => ">=",
                BinOp::And | BinOp::Or | BinOp::Eq | BinOp::NotEq | BinOp::Coalesce => unreachable!(),
            }
        )))
    })
}

fn eval_unop(op: &UnOp, v: Value, line: usize) -> Result<Value, RuntimeError> {
    match op {
        UnOp::PrefixInc | UnOp::PrefixDec | UnOp::PostfixInc | UnOp::PostfixDec => {
            unreachable!("inc/dec handled in eval_expr")
        }
        UnOp::Neg => match v {
            Value::Float(x)   => Ok(Value::Float(-x)),
            Value::Vec2(x, y) => Ok(Value::Vec2(-x, -y)),
            other => Err(RuntimeError::new(ErrorCode::R011, line, format!(
                "unary `-` not supported on `{}`", value_type_name(&other)
            ))),
        },
        UnOp::Not => Ok(Value::Bool(!v.is_truthy())),
    }
}

// ─── Utilities ────────────────────────────────────────────────────────────────

fn as_float(v: &Value, line: usize) -> Result<f64, RuntimeError> {
    match v {
        Value::Float(x) => Ok(*x),
        _ => Err(RuntimeError::new(ErrorCode::R001, line, format!(
            "expected float, got `{}`", value_type_name(v)
        ))),
    }
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Float(x),  Value::Float(y))  => x == y,
        (Value::Bool(x),   Value::Bool(y))   => x == y,
        (Value::Str(x),    Value::Str(y))    => x == y,
        (Value::Vec2(ax, ay), Value::Vec2(bx, by)) => ax == bx && ay == by,
        (Value::Vec3(ax,ay,az), Value::Vec3(bx,by,bz)) => ax==bx && ay==by && az==bz,
        (Value::Vec4(ax,ay,az,aw), Value::Vec4(bx,by,bz,bw)) => ax==bx && ay==by && az==bz && aw==bw,
        (Value::Color { r: ar, g: ag, b: ab, a: aa }, Value::Color { r: br, g: bg, b: bb, a: ba }) => {
            ar == br && ag == bg && ab == bb && aa == ba
        }
        _ => false,
    }
}


// ─── Free variable analysis (for selective lambda capture) ───────────────────

use std::collections::HashSet;

fn free_vars_in_body(params: &[Param], body: &[Stmt]) -> HashSet<String> {
    let mut free = HashSet::new();
    let mut bound: HashSet<String> = params.iter().map(|p| p.name.clone()).collect();
    for s in body {
        collect_free_stmt(s, &mut bound, &mut free);
    }
    free
}

fn collect_free_expr(expr: &Expr, bound: &HashSet<String>, free: &mut HashSet<String>) {
    match expr {
        Expr::Ident(name, _) => {
            if !bound.contains(name) {
                free.insert(name.clone());
            }
        }
        Expr::Float(..) | Expr::Bool(..) | Expr::None(..) | Expr::StringLit(..) | Expr::HexColor(..) => {}
        Expr::BinOp { left, right, .. } => {
            collect_free_expr(left, bound, free);
            collect_free_expr(right, bound, free);
        }
        Expr::UnOp { operand, .. } => collect_free_expr(operand, bound, free),
        Expr::Ternary { condition, then_expr, else_expr, .. } => {
            collect_free_expr(condition, bound, free);
            collect_free_expr(then_expr, bound, free);
            collect_free_expr(else_expr, bound, free);
        }
        Expr::Cast { expr, .. } | Expr::Try { expr, .. } => collect_free_expr(expr, bound, free),
        Expr::Call { callee, args, named_args, .. } => {
            if !bound.contains(callee) {
                free.insert(callee.clone());
            }
            for a in args { collect_free_expr(a, bound, free); }
            for (_, a) in named_args { collect_free_expr(a, bound, free); }
        }
        Expr::Index { expr, index, .. } => {
            collect_free_expr(expr, bound, free);
            collect_free_expr(index, bound, free);
        }
        Expr::Field { expr, .. } | Expr::OptionalChain { expr, .. } => collect_free_expr(expr, bound, free),
        Expr::MethodCall { expr, args, named_args, .. } => {
            collect_free_expr(expr, bound, free);
            for a in args { collect_free_expr(a, bound, free); }
            for (_, a) in named_args { collect_free_expr(a, bound, free); }
        }
        Expr::Transform { expr, transforms, .. } => {
            collect_free_expr(expr, bound, free);
            for t in transforms { collect_free_expr(t, bound, free); }
        }
        Expr::List(items, _) => {
            for i in items { collect_free_expr(i, bound, free); }
        }
        Expr::Lambda { params, body, .. } => {
            let mut inner_bound = bound.clone();
            for p in params.iter() { inner_bound.insert(p.name.clone()); }
            for s in body.iter() { collect_free_stmt(s, &mut inner_bound, free); }
        }
    }
}

fn collect_free_stmt(stmt: &Stmt, bound: &mut HashSet<String>, free: &mut HashSet<String>) {
    match stmt {
        Stmt::VarDecl(v) => {
            collect_free_expr(&v.initializer, bound, free);
            bound.insert(v.name.clone());
        }
        Stmt::FnVar { value, name, .. } => {
            collect_free_expr(value, bound, free);
            bound.insert(name.clone());
        }
        Stmt::Assign(a) => {
            collect_free_expr(&a.value, bound, free);
            let root = &a.target.path()[0];
            if !bound.contains(root) { free.insert(root.clone()); }
            if let ast::AssignTarget::Indexed { indices, .. } = &a.target {
                for idx in indices { collect_free_expr(idx, bound, free); }
            }
        }
        Stmt::Out(o) => {
            for e in &o.shapes { collect_free_expr(e, bound, free); }
        }
        Stmt::Print(p) => {
            for e in &p.values { collect_free_expr(e, bound, free); }
        }
        Stmt::Expr(e) => collect_free_expr(e, bound, free),
        Stmt::Return(expr, _) => {
            if let Some(e) = expr { collect_free_expr(e, bound, free); }
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
        Stmt::IfLet { binding, expr, then_block, else_block, .. } => {
            collect_free_expr(expr, bound, free);
            let mut then_bound = bound.clone();
            then_bound.insert(binding.clone());
            for s in then_block { collect_free_stmt(s, &mut then_bound, free); }
            if let Some(els) = else_block {
                let mut else_bound = bound.clone();
                for s in els { collect_free_stmt(s, &mut else_bound, free); }
            }
        }
        Stmt::If(i) => {
            collect_free_expr(&i.condition, bound, free);
            let mut then_bound = bound.clone();
            for s in &i.then_block { collect_free_stmt(s, &mut then_bound, free); }
            if let Some(els) = &i.else_block {
                let mut else_bound = bound.clone();
                for s in els { collect_free_stmt(s, &mut else_bound, free); }
            }
        }
        Stmt::Match(m) => {
            collect_free_expr(&m.expr, bound, free);
            for arm in &m.arms {
                for v in &arm.values { collect_free_expr(v, bound, free); }
                let mut arm_bound = bound.clone();
                for s in &arm.body { collect_free_stmt(s, &mut arm_bound, free); }
            }
        }
        Stmt::While(w) => {
            collect_free_expr(&w.condition, bound, free);
            let mut body_bound = bound.clone();
            for s in &w.body { collect_free_stmt(s, &mut body_bound, free); }
        }
        Stmt::For(f) => {
            let mut for_bound = bound.clone();
            collect_free_stmt(&f.init, &mut for_bound, free);
            collect_free_expr(&f.condition, &for_bound, free);
            collect_free_stmt(&f.step, &mut for_bound.clone(), free);
            for s in &f.body { collect_free_stmt(s, &mut for_bound.clone(), free); }
        }
        Stmt::Foreach(f) => {
            collect_free_expr(&f.iterable, bound, free);
            let mut body_bound = bound.clone();
            body_bound.insert(f.var_name.clone());
            for s in &f.body { collect_free_stmt(s, &mut body_bound, free); }
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn safe_index(v: &Value, line: usize) -> Result<usize, RuntimeError> {
    let f = match v {
        Value::Float(x) => *x,
        other => return Err(RuntimeError::new(ErrorCode::R001, line, format!(
            "index must be a number, got `{}`", value_type_name(other)
        ))),
    };
    if f.is_nan() {
        return Err(RuntimeError::new(ErrorCode::R006, line, "index is NaN"));
    }
    if f.is_infinite() {
        return Err(RuntimeError::new(ErrorCode::R006, line, "index is infinite"));
    }
    if f < 0.0 {
        return Err(RuntimeError::new(ErrorCode::R006, line, format!("index is negative ({})", f as i64)));
    }
    Ok(f as usize)
}

fn parse_hex_color(hex: &str) -> Result<Value, RuntimeError> {
    let parse = |s: &str| u8::from_str_radix(s, 16)
        .map(|n| n as f64 / 255.0)
        .map_err(|_| RuntimeError::new(ErrorCode::R001, 0, format!("invalid hex: #{hex}")));
    match hex.len() {
        6 => Ok(Value::Color { r: parse(&hex[0..2])?, g: parse(&hex[2..4])?, b: parse(&hex[4..6])?, a: 1.0 }),
        8 => Ok(Value::Color { r: parse(&hex[0..2])?, g: parse(&hex[2..4])?, b: parse(&hex[4..6])?, a: parse(&hex[6..8])? }),
        _ => Err(RuntimeError::new(ErrorCode::R001, 0, format!("invalid hex color length: #{hex}"))),
    }
}
