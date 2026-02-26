//! Tree-walking interpreter. Evaluates AST → Vec<DrawCommand>.
//! All domain-specific calls are dispatched through the NamespaceRegistry.
//! The interpreter itself contains no hardcoded function implementations.

use crate::syntax::ast::{self, BinOp, Expr, Item, Param, Span, Stmt, UnOp};
use crate::types::draw::DrawCommand;
use crate::types::binop_registry::BinopRegistry;
use crate::types::registry::TypeRegistry;
use crate::error::RuntimeError;
use crate::namespaces::{value_type_name, NamespaceRegistry, RuntimeState};
use crate::{Input, State, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

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

// ─── Interpreter ──────────────────────────────────────────────────────────────

pub struct Interpreter<'a> {
    program: &'a ast::Program,
    registry: &'a NamespaceRegistry,
    binops: BinopRegistry,
    types: TypeRegistry,
    env: Env,
    return_value: Option<Value>,
    runtime_state: RuntimeState,
}

impl<'a> Interpreter<'a> {
    pub fn new(program: &'a ast::Program, registry: &'a NamespaceRegistry) -> Self {
        Self {
            program,
            registry,
            binops: BinopRegistry::default(),
            types: TypeRegistry::default(),
            env: Env::new(),
            return_value: None,
            runtime_state: RuntimeState::default(),
        }
    }

    /// Seed the interpreter with persisted runtime state (coord_meta, etc.) from
    /// a prior init or tick so that resolution/origin survive across frames.
    pub fn with_runtime_state(mut self, rs: RuntimeState) -> Self {
        self.runtime_state = rs;
        self
    }

    /// Extract the final runtime state after running (captures resolution/origin calls).
    pub fn take_runtime_state(&self) -> RuntimeState {
        self.runtime_state.clone()
    }

    fn err(&self, line: usize, msg: impl Into<String>) -> RuntimeError {
        RuntimeError::new(line, msg)
    }

    // ─── Imports ──────────────────────────────────────────────────────────────

    /// Bind all import declarations into the current environment.
    /// - `import shapes`           → `shapes = Namespace("shapes")`
    /// - `import shapes { circle }` → `circle = NativeFn("circle")`
    /// - `import render { sdf }`   → `sdf = RenderMode(Sdf)`  (constant)
    pub fn setup_imports(&mut self) {
        let imports = self.program.imports.clone();
        for import in &imports {
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
        let items = self.program.items.clone();
        for item in &items {
            if let Item::Stmt(s) = item { self.exec_stmt(s)?; }
        }
        Ok(())
    }

    pub fn run_update(&mut self, state: State, input: &Input) -> Result<State, RuntimeError> {
        let f = self.program.items.iter().find_map(|i| match i {
            Item::FnDef(f) if f.name == "update" => Some(f.clone()),
            _ => None,
        });
        let Some(f) = f else { return Ok(state); };

        self.run_top_level()?;

        let state_rc = Rc::new(RefCell::new(state.0));
        let state_val = Value::State(state_rc.clone());
        let input_val = Value::Input { dt: input.dt };

        self.env.push_scope();
        if let Some(p) = f.params.first()  { self.env.declare(&p.name, state_val); }
        if let Some(p) = f.params.get(1)   { self.env.declare(&p.name, input_val); }

        self.return_value = None;
        for stmt in &f.body {
            self.exec_stmt(stmt)?;
            if self.return_value.is_some() { break; }
        }
        self.env.pop_scope();

        let new_map = match self.return_value.take() {
            Some(Value::State(rc)) => rc.borrow().clone(),
            _ => state_rc.borrow().clone(),
        };
        Ok(State(new_map))
    }

    pub fn run_init(&mut self, state: State) -> Result<State, RuntimeError> {
        let f = self.program.items.iter().find_map(|i| match i {
            Item::FnDef(f) if f.name == "init" => Some(f.clone()),
            _ => None,
        });
        let Some(f) = f else { return Ok(state); };

        let state_rc = Rc::new(RefCell::new(state.0));
        let state_val = Value::State(state_rc.clone());

        self.env.push_scope();
        if let Some(p) = f.params.first() { self.env.declare(&p.name, state_val); }

        self.return_value = None;
        for stmt in &f.body {
            self.exec_stmt(stmt)?;
            if self.return_value.is_some() { break; }
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
            Expr::StringLit(s, _) => Ok(Value::Str(s.clone())),
            Expr::HexColor(s, _)  => parse_hex_color(s),

            Expr::Ident(name, span) => {
                self.env.get(name)
                    .or_else(|| self.registry.get_constant(name))
                    .or_else(|| {
                        // User-defined function used as a first-class value.
                        self.program.items.iter().find_map(|i| match i {
                            Item::FnDef(f) if f.name == *name => Some(Value::Closure {
                                params: f.params.clone(),
                                body: f.body.clone(),
                                captured: HashMap::new(),
                            }),
                            _ => None,
                        })
                    })
                    .ok_or_else(|| self.err(span.line, format!("undefined: `{name}`")))
            }

            Expr::BinOp { left, op, right, span } => {
                let l = self.eval_expr(left)?;
                let r = self.eval_expr(right)?;
                eval_binop(op, l, r, span.line, &self.binops)
            }

            Expr::UnOp { op, operand, span } => {
                let v = self.eval_expr(operand)?;
                eval_unop(op, v, span.line)
            }

            Expr::Ternary { condition, then_expr, else_expr, span } => {
                match self.eval_expr(condition)? {
                    Value::Bool(true)  => self.eval_expr(then_expr),
                    Value::Bool(false) => self.eval_expr(else_expr),
                    _ => Err(self.err(span.line, "ternary condition must be bool")),
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
                let i = as_float(&idx, span.line)? as usize;
                match coll {
                    Value::List(items) => items.borrow().get(i).cloned()
                        .ok_or_else(|| self.err(span.line, "index out of bounds")),
                    _ => Err(self.err(span.line, format!(
                        "cannot index `{}`", value_type_name(&coll)
                    ))),
                }
            }

            Expr::Field { expr, field, span } => {
                let obj = self.eval_expr(expr)?;
                eval_field(&self.types, &obj, field, span.line)
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
                let captured = self.env.scopes.iter()
                    .flat_map(|s| s.iter())
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
                        .ok_or_else(|| self.err(span.line, format!("unknown native fn: `{n}`")));
                }
                Value::Closure { params, body, captured } => {
                    return self.call_closure(&params, &body, &captured, &arg_vals, span.line);
                }
                _ => {} // fall through — may be a user fn with same name
            }
        }

        // 2. Registry (all namespace providers: core, shapes, render, coords)
        if let Some(v) = self.registry.call_any(callee, &arg_vals, &named, &mut self.runtime_state, span.line)? {
            return Ok(v);
        }

        // 3. User-defined functions (FnDef items)
        let f = self.program.items.iter().find_map(|i| match i {
            Item::FnDef(f) if f.name == callee => Some(f.clone()),
            _ => None,
        });
        if let Some(f) = f {
            if f.params.len() != arg_vals.len() {
                return Err(self.err(span.line, format!(
                    "`{}` expects {} args, got {}", f.name, f.params.len(), arg_vals.len()
                )));
            }
            return self.call_fn(&f.params, &f.body, &arg_vals, span.line);
        }

        Err(self.err(span.line, format!("undefined function: `{callee}`")))
    }

    fn call_fn(
        &mut self,
        params: &[Param],
        body: &[Stmt],
        arg_vals: &[Value],
        _line: usize,
    ) -> Result<Value, RuntimeError> {
        self.env.push_scope();
        for (p, v) in params.iter().zip(arg_vals) {
            self.env.declare(&p.name, v.clone());
        }
        let saved = self.return_value.take();
        let body = body.to_vec();
        for stmt in &body {
            self.exec_stmt(stmt)?;
            if self.return_value.is_some() { break; }
        }
        let result = self.return_value.take().unwrap_or(Value::Float(0.0));
        self.return_value = saved;
        self.env.pop_scope();
        Ok(result)
    }

    fn call_closure(
        &mut self,
        params: &[Param],
        body: &[Stmt],
        captured: &HashMap<String, Value>,
        arg_vals: &[Value],
        line: usize,
    ) -> Result<Value, RuntimeError> {
        if params.len() != arg_vals.len() {
            return Err(self.err(line, format!(
                "closure expects {} args, got {}", params.len(), arg_vals.len()
            )));
        }
        self.env.push_scope();
        for (k, v) in captured { self.env.declare(k, v.clone()); }
        for (p, v) in params.iter().zip(arg_vals) { self.env.declare(&p.name, v.clone()); }
        let saved = self.return_value.take();
        let body = body.to_vec();
        for stmt in &body {
            self.exec_stmt(stmt)?;
            if self.return_value.is_some() { break; }
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
                            .ok_or_else(|| self.err(span.line, format!(
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
                            .ok_or_else(|| self.err(span.line, format!(
                                "`{ns_name}` does not implement `{method}`"
                            )));
                    }
                }
            }
            return Err(self.err(span.line, format!("`{ns_name}` has no member `{method}`")));
        }

        // All other types: evaluate args, delegate to TypeRegistry.
        let arg_vals: Vec<Value> = args.iter()
            .map(|a| self.eval_expr(a))
            .collect::<Result<_, _>>()?;

        self.types.call_method(&obj, method, &arg_vals, span.line)
            .unwrap_or_else(|| Err(self.err(span.line, format!(
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
                if a.target.len() == 1 {
                    let name = &a.target[0];
                    if !self.env.set(name, val) {
                        return Err(self.err(a.span.line, format!("undefined: `{name}`")));
                    }
                } else {
                    let root = &a.target[0];
                    let obj = self.env.get(root)
                        .ok_or_else(|| self.err(a.span.line, format!("undefined: `{root}`")))?;
                    if let Value::State(rc) = &obj {
                        // State uses Rc for in-place mutation.
                        assign_state_path(rc, &a.target[1..], val, a.span.line, &self.types)?;
                    } else {
                        // Local vars (vec2, color, …): produce new value, write back.
                        let updated = set_field_path(&self.types, obj, &a.target[1..], val, a.span.line)?;
                        self.env.set(root, updated);
                    }
                }
            }

            Stmt::Out(o) => {
                for expr in &o.shapes {
                    match self.eval_expr(expr)? {
                        Value::Shape(data) => self.env.emit(DrawCommand::DrawShape(data)),
                        other => return Err(self.err(o.span.line, format!(
                            "out << expects shape, got `{}`", value_type_name(&other)
                        ))),
                    }
                }
            }

            Stmt::If(i) => {
                let branch = match self.eval_expr(&i.condition)? {
                    Value::Bool(true)  => Some(&i.then_block),
                    Value::Bool(false) => i.else_block.as_ref(),
                    _ => return Err(self.err(i.span.line, "if condition must be bool")),
                };
                if let Some(block) = branch {
                    self.env.push_scope();
                    for s in block {
                        self.exec_stmt(s)?;
                        if self.return_value.is_some() { break; }
                    }
                    self.env.pop_scope();
                }
            }

            Stmt::While(w) => {
                loop {
                    match self.eval_expr(&w.condition)? {
                        Value::Bool(false) => break,
                        Value::Bool(true)  => {}
                        _ => return Err(self.err(w.span.line, "while condition must be bool")),
                    }
                    self.env.push_scope();
                    for s in &w.body {
                        self.exec_stmt(s)?;
                        if self.return_value.is_some() { break; }
                    }
                    self.env.pop_scope();
                    if self.return_value.is_some() { break; }
                }
            }

            Stmt::For(f) => {
                self.env.push_scope();
                self.exec_stmt(&f.init)?;
                loop {
                    match self.eval_expr(&f.condition)? {
                        Value::Bool(false) => break,
                        Value::Bool(true)  => {}
                        _ => return Err(self.err(f.span.line, "for condition must be bool")),
                    }
                    self.env.push_scope();
                    for s in &f.body {
                        self.exec_stmt(s)?;
                        if self.return_value.is_some() { break; }
                    }
                    self.env.pop_scope();
                    if self.return_value.is_some() { break; }
                    self.exec_stmt(&f.step)?;
                }
                self.env.pop_scope();
            }

            Stmt::Foreach(f) => {
                let list = match self.eval_expr(&f.iterable)? {
                    Value::List(items) => items.borrow().clone(),
                    other => return Err(self.err(f.span.line, format!(
                        "foreach expects list, got `{}`", value_type_name(&other)
                    ))),
                };
                for item in list {
                    self.env.push_scope();
                    self.env.declare(&f.var_name, item);
                    for s in &f.body {
                        self.exec_stmt(s)?;
                        if self.return_value.is_some() { break; }
                    }
                    self.env.pop_scope();
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

            Stmt::FnVar { name, value, .. } => {
                let val = self.eval_expr(value)?;
                self.env.declare(name, val);
            }

            Stmt::Expr(e) => { self.eval_expr(e)?; }
        }
        Ok(())
    }
}

// ─── Field access ─────────────────────────────────────────────────────────────

fn eval_field(types: &TypeRegistry, obj: &Value, field: &str, line: usize) -> Result<Value, RuntimeError> {
    // State fields are dynamic (per-script) — not in the static registry.
    if let Value::State(rc) = obj {
        return rc.borrow().get(field).cloned()
            .ok_or_else(|| RuntimeError::new(line, format!("state has no field `{field}`")));
    }
    types.get_field(obj, field)
        .ok_or_else(|| RuntimeError::new(line, format!(
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
        let intermediate = rc.borrow().get(field.as_str()).cloned()
            .ok_or_else(|| RuntimeError::new(line, format!("state has no field `{field}`")))?;
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
            .ok_or_else(|| RuntimeError::new(line, format!(
                "`{}` has no field `{field}`", value_type_name(&obj)
            )))?;
        set_field_path(types, sub, &path[1..], val, line)?
    } else {
        val
    };
    types.set_field(obj, field, new_val)
        .ok_or_else(|| RuntimeError::new(line, format!(
            "cannot assign field `{field}` (read-only or unknown)"
        )))
}

// ─── Transform application ────────────────────────────────────────────────────

fn apply_transform(shape: Value, tf: Value, line: usize) -> Result<Value, RuntimeError> {
    let Value::Transform(td) = tf else {
        return Err(RuntimeError::new(line, format!(
            "@  requires transform, got `{}`", value_type_name(&tf)
        )));
    };
    let Value::Shape(mut data) = shape else {
        return Err(RuntimeError::new(line, format!(
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
        Err(RuntimeError::new(line, format!(
            "operator `{}` not supported for these types",
            match op {
                BinOp::Add  => "+",  BinOp::Sub  => "-",
                BinOp::Mul  => "*",  BinOp::Div  => "/",  BinOp::Mod  => "%",
                BinOp::Lt   => "<",  BinOp::LtEq => "<=",
                BinOp::Gt   => ">",  BinOp::GtEq => ">=",
                BinOp::And  => "and", BinOp::Or  => "or",
                BinOp::Eq | BinOp::NotEq => unreachable!(),
            }
        )))
    })
}

fn eval_unop(op: &UnOp, v: Value, line: usize) -> Result<Value, RuntimeError> {
    match op {
        UnOp::Neg => match v {
            Value::Float(x)   => Ok(Value::Float(-x)),
            Value::Vec2(x, y) => Ok(Value::Vec2(-x, -y)),
            other => Err(RuntimeError::new(line, format!(
                "unary `-` not supported on `{}`", value_type_name(&other)
            ))),
        },
        UnOp::Not => match v {
            Value::Bool(b) => Ok(Value::Bool(!b)),
            other => Err(RuntimeError::new(line, format!(
                "`not` requires bool, got `{}`", value_type_name(&other)
            ))),
        },
    }
}

// ─── Utilities ────────────────────────────────────────────────────────────────

fn as_float(v: &Value, line: usize) -> Result<f64, RuntimeError> {
    match v {
        Value::Float(x) => Ok(*x),
        _ => Err(RuntimeError::new(line, format!(
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
        _ => false,
    }
}


fn parse_hex_color(hex: &str) -> Result<Value, RuntimeError> {
    let parse = |s: &str| u8::from_str_radix(s, 16)
        .map(|n| n as f64 / 255.0)
        .map_err(|_| RuntimeError::new(0, format!("invalid hex: #{hex}")));
    match hex.len() {
        6 => Ok(Value::Color { r: parse(&hex[0..2])?, g: parse(&hex[2..4])?, b: parse(&hex[4..6])?, a: 1.0 }),
        8 => Ok(Value::Color { r: parse(&hex[0..2])?, g: parse(&hex[2..4])?, b: parse(&hex[4..6])?, a: parse(&hex[6..8])? }),
        _ => Err(RuntimeError::new(0, format!("invalid hex color length: #{hex}"))),
    }
}
