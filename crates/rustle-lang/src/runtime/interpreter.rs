//! Tree-walking interpreter. Evaluates AST → Vec<DrawCommand>.
//! All domain-specific calls are dispatched through the `NamespaceRegistry`.
//! The interpreter itself contains no hardcoded function implementations.

use crate::syntax::ast::{self, AssignTarget, BinOp, Expr, Item, Param, Span, Stmt, UnOp};
use crate::types::draw::DrawCommand;
use crate::types::binop_registry::BinopRegistry;
use crate::types::registry::TypeRegistry;
use crate::error::{ErrorCode, RuntimeError};
use crate::namespaces::{as_float, value_type_name, NamespaceRegistry, RuntimeState};
use crate::{Input, State, Value};
use crate::runtime::value::ClosureData;
use crate::runtime::object::{StructInstance, StructMethodDef, StructMethods};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

// ─── Environment ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub(crate) struct Env {
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
        self.scopes.last_mut().expect("scope stack is never empty").insert(name.to_string(), val);
    }

    fn set(&mut self, name: &str, val: Value) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(slot) = scope.get_mut(name) {
                *slot = val;
                return true;
            }
        }
        false
    }

    fn get(&self, name: &str) -> Option<&Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) { return Some(v); }
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
    struct_defs: HashMap<&'a str, &'a ast::StructDef>,
    method_cache: HashMap<String, Rc<StructMethods>>,
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

/// Extract a non-negative float as usize, with proper error reporting.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn float_to_index(v: &Value, name: &str, line: usize) -> Result<usize, RuntimeError> {
    match v {
        Value::Float(f) if *f < 0.0 => Err(RuntimeError::new(
            ErrorCode::R011, line, 0,
            format!("`{name}` index must be non-negative, got {f}"),
        )),
        Value::Float(f) => Ok(*f as usize),
        _ => Err(RuntimeError::new(
            ErrorCode::R001, line, 0,
            format!("`{name}` expects float index"),
        )),
    }
}

impl<'a> Interpreter<'a> {
    #[must_use]
    pub fn new(
        program: &'a ast::Program,
        registry: &'a NamespaceRegistry,
        types: &'a TypeRegistry,
        binops: &'a BinopRegistry,
    ) -> Self {
        let fn_table = program.items.iter().filter_map(|item| match item {
            ast::Item::FnDef(f) => Some((f.name.as_str(), f)),
            ast::Item::Stmt(_) | ast::Item::Struct(_) | ast::Item::Enum(_) => None,
        }).collect();
        let struct_defs: HashMap<&str, &ast::StructDef> = program.items.iter().filter_map(|item| match item {
            ast::Item::Struct(s) => Some((s.name.as_str(), s)),
            _ => None,
        }).collect();
        let method_cache = struct_defs.iter().map(|(&name, sdef)| {
            let methods: StructMethods = sdef.methods.iter().map(|m| {
                let vis = match m.visibility {
                    ast::Visibility::Public => crate::runtime::object::Visibility::Public,
                    ast::Visibility::Private => crate::runtime::object::Visibility::Private,
                };
                (m.def.name.clone(), StructMethodDef {
                    visibility: vis,
                    params: m.def.params.clone(),
                    body: m.def.body.clone(),
                    return_ty: m.def.return_ty.clone(),
                })
            }).collect();
            (name.to_string(), Rc::new(methods))
        }).collect();
        Self {
            program,
            fn_table,
            struct_defs,
            method_cache,
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

    /// Seed the interpreter with persisted runtime state (`coord_meta`, etc.) from
    /// a prior init or tick so that resolution/origin survive across frames.
    #[must_use] 
    pub fn with_runtime_state(mut self, rs: RuntimeState) -> Self {
        self.runtime_state = rs;
        self
    }

    /// Set a cancellation token. When the flag is set to `true`, the interpreter
    /// will abort at the next loop iteration or function call boundary.
    #[must_use]
    pub fn with_cancel(mut self, cancel: Arc<AtomicBool>) -> Self {
        self.cancel = Some(cancel);
        self
    }

    /// Extract the current environment for caching (e.g. after top-level setup).
    #[must_use]
    pub(crate) fn take_env(&self) -> Env {
        self.env.clone()
    }

    /// Restore a cached environment's scopes with a fresh output buffer.
    #[must_use]
    pub(crate) fn with_env(mut self, env: Env) -> Self {
        self.env = Env {
            scopes: env.scopes,
            output: Rc::new(RefCell::new(Vec::new())),
        };
        self
    }

    fn check_cancel(&self, line: usize) -> Result<(), RuntimeError> {
        if let Some(ref c) = self.cancel
            && c.load(Ordering::Relaxed) {
                return Err(self.err(ErrorCode::R010, line, "script cancelled"));
            }
        Ok(())
    }

    /// Check whether a named function is defined in the program.
    #[must_use] 
    pub fn has_fn(&self, name: &str) -> bool {
        self.fn_table.contains_key(name)
    }

    /// Extract the final runtime state after running (captures resolution/origin calls).
    #[must_use] 
    pub fn take_runtime_state(&self) -> RuntimeState {
        self.runtime_state.clone()
    }

    fn should_stop_block(&self) -> bool {
        self.return_value.is_some() || self.break_flag || self.continue_flag
    }

    #[expect(clippy::unused_self, reason = "kept as method for convenient call site syntax")]
    fn err(&self, code: ErrorCode, line: usize, msg: impl Into<String>) -> RuntimeError {
        RuntimeError::new(code, line, 0, msg)
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

    /// # Errors
    /// Returns a runtime error if any top-level statement fails.
    pub fn run_top_level(&mut self) -> Result<(), RuntimeError> {
        self.setup_imports();
        let program = self.program;
        for item in &program.items {
            if let Item::Stmt(s) = item { self.exec_stmt(s)?; }
        }
        Ok(())
    }

    /// # Errors
    /// Returns a runtime error if `on_update` fails during execution.
    pub fn run_update(&mut self, state: State, input: &Input) -> Result<State, RuntimeError> {
        let Some(f) = self.fn_table.get("on_update").copied() else { return Ok(state); };
        let input_val = Value::Input {
            dt: input.dt,
            mouse_x: input.mouse_x,
            mouse_y: input.mouse_y,
            mouse_down: input.mouse_down,
            mouse_pressed: input.mouse_pressed,
            mouse_released: input.mouse_released,
            key_pressed: input.key_pressed.clone(),
            key_down: input.key_down.clone(),
            key_released: input.key_released.clone(),
        };
        self.run_lifecycle("on_update", f, state, &[("input", input_val)])
    }

    /// # Errors
    /// Returns a runtime error if `on_init` fails during execution.
    pub fn run_init(&mut self, state: State) -> Result<State, RuntimeError> {
        let Some(f) = self.fn_table.get("on_init").copied() else { return Ok(state); };
        self.run_lifecycle("on_init", f, state, &[])
    }

    /// # Errors
    /// Returns a runtime error if `on_exit` fails during execution.
    pub fn run_on_exit(&mut self, state: State) -> Result<State, RuntimeError> {
        let Some(f) = self.fn_table.get("on_exit").copied() else { return Ok(state); };
        self.run_lifecycle("on_exit", f, state, &[])
    }

    fn run_lifecycle(
        &mut self,
        name: &str,
        f: &ast::FnDef,
        state: State,
        extra_params: &[(&str, Value)],
    ) -> Result<State, RuntimeError> {
        let state_rc = Rc::new(RefCell::new(state.0));
        let state_val = Value::State(state_rc.clone());

        self.env.push_scope();
        if let Some(p) = f.params.first() { self.env.declare(&p.name, state_val); }
        for (i, (_param_name_override, val)) in extra_params.iter().enumerate() {
            if let Some(p) = f.params.get(i + 1) { self.env.declare(&p.name, val.clone()); }
        }

        self.return_value = None;
        for stmt in f.body.iter() {
            match self.exec_stmt(stmt) {
                Ok(()) => {}
                Err(mut e) => {
                    e.push_frame(name, 0);
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

    #[must_use] 
    pub fn take_output(&self) -> Vec<DrawCommand> {
        self.env.output.borrow_mut().drain(..).collect()
    }

    // ─── Expression evaluator ─────────────────────────────────────────────────

    /// # Errors
    /// Returns a runtime error if the expression cannot be evaluated.
    #[expect(clippy::too_many_lines, reason = "large match on Expr variants; splitting would add indirection")]
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        match expr {
            Expr::Float(v, _)     => Ok(Value::Float(*v)),
            Expr::Bool(v, _)      => Ok(Value::Bool(*v)),
            Expr::Interpolated(parts, _) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        ast::InterpolPart::Lit(s) => result.push_str(s),
                        ast::InterpolPart::Expr(e) => {
                            let val = self.eval_expr(e)?;
                            result.push_str(&val.to_string());
                        }
                    }
                }
                Ok(Value::Str(result))
            }
            Expr::None(_)         => Ok(Value::None),
            Expr::StringLit(s, _) => Ok(Value::Str(s.clone())),
            Expr::HexColor(s, _)  => parse_hex_color(s),

            Expr::Ident(name, span) => {
                self.env.get(name).cloned()
                    .or_else(|| self.registry.get_constant(name))
                    .or_else(|| {
                        self.fn_table.get(name.as_str()).map(|f| Value::Closure(Box::new(ClosureData {
                            params: f.params.clone(),
                            body: f.body.clone(),
                            captured: HashMap::new(),
                        })))
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
                if matches!(op, BinOp::Eq | BinOp::NotEq)
                    && (matches!(&l, Value::None) || matches!(&r, Value::None)) {
                        let both_none = matches!((&l, &r), (Value::None, Value::None));
                        return Ok(Value::Bool(if *op == BinOp::Eq { both_none } else { !both_none }));
                    }
                eval_binop(op, l, r, span.line, self.binops)
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

            Expr::Cast { expr, ty, span } => {
                let val = self.eval_expr(expr)?;
                cast_value(val, ty, span.line)
            }

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
                eval_field(self.types, &obj, field, span.line)
            }

            Expr::OptionalChain { expr, field, span } => {
                let obj = self.eval_expr(expr)?;
                match obj {
                    Value::None => Ok(Value::None),
                    other => eval_field(self.types, &other, field, span.line),
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
                Ok(Value::Closure(Box::new(ClosureData { params: params.clone(), body: body.clone(), captured })))
            }

            Expr::StructConstruction { name, fields, span } => {
                let struct_def = self.struct_defs.get(name.as_str())
                    .ok_or_else(|| self.err(ErrorCode::R002, span.line, format!("undefined struct: `{name}`")))?;
                // Build field values: start with defaults, then override with provided
                let mut field_values = HashMap::new();
                for field_def in &struct_def.fields {
                    if let Some(ref default_expr) = field_def.default {
                        field_values.insert(field_def.name.clone(), self.eval_expr(default_expr)?);
                    }
                }
                for (field_name, field_expr) in fields {
                    field_values.insert(field_name.clone(), self.eval_expr(field_expr)?);
                }
                let methods = self.method_cache.get(name.as_str())
                    .cloned()
                    .unwrap_or_else(|| Rc::new(HashMap::new()));
                let instance = StructInstance {
                    type_name: name.clone(),
                    fields: field_values,
                    methods,
                };
                Ok(Value::Object(Rc::new(RefCell::new(Box::new(instance)))))
            }

            Expr::EnumConstruction { enum_name, variant, fields, .. } => {
                let mut field_values = HashMap::new();
                for (field_name, field_expr) in fields {
                    field_values.insert(field_name.clone(), self.eval_expr(field_expr)?);
                }
                Ok(Value::EnumVariant {
                    enum_name: enum_name.clone(),
                    variant: variant.clone(),
                    fields: field_values,
                })
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
        if let Some(val) = self.env.get(callee).cloned() {
            match val {
                Value::NativeFn(ref name) => {
                    let n = name.clone();
                    return self.registry.call_any(&n, &arg_vals, &named, &mut self.runtime_state, span.line)?
                        .ok_or_else(|| self.err(ErrorCode::R004, span.line, format!("unknown native fn: `{n}`")));
                }
                Value::Closure(data) => {
                    let pre: Vec<(&str, Value)> = data.captured.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
                    return self.call_body(callee, &data.params, &data.body, &pre, &arg_vals, span.line);
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
            return self.call_body(&f.name, &f.params, &f.body, &[], &arg_vals, span.line);
        }

        Err(self.err(ErrorCode::R002, span.line, format!("undefined function: `{callee}`")))
    }

    fn call_body(
        &mut self,
        name: &str,
        params: &[Param],
        body: &[Stmt],
        pre_bindings: &[(&str, Value)],
        arg_vals: &[Value],
        call_line: usize,
    ) -> Result<Value, RuntimeError> {
        if params.len() != arg_vals.len() {
            return Err(self.err(ErrorCode::R008, call_line, format!(
                "`{name}` expects {} args, got {}", params.len(), arg_vals.len()
            )));
        }
        self.check_cancel(call_line)?;
        if self.call_depth >= MAX_CALL_DEPTH {
            return Err(self.err(ErrorCode::R011, call_line,
                format!("maximum call depth ({MAX_CALL_DEPTH}) exceeded — possible infinite recursion")));
        }
        self.call_depth += 1;
        self.env.push_scope();
        for (k, v) in pre_bindings { self.env.declare(k, v.clone()); }
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

    #[expect(clippy::needless_pass_by_value, reason = "obj is used by-ref internally; taking by value keeps call sites clean")]
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
            if let Some(ns) = self.registry.get(&ns_name)
                && let Some(export) = ns.get_export(method) {
                    use crate::namespaces::ExportKind;
                    if export.kind == ExportKind::Constant {
                        return ns.get_constant(method)
                            .or_else(|| self.registry.get_constant(method))
                            .ok_or_else(|| self.err(ErrorCode::R004, span.line, format!(
                                "`{ns_name}.{method}` has no runtime value"
                            )));
                    }
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
            return Err(self.err(ErrorCode::R004, span.line, format!("`{ns_name}` has no member `{method}`")));
        }

        // Object method dispatch — struct instances
        if let Value::Object(rc) = &obj {
            // Built-in clone
            if method == "clone" {
                let cloned = rc.borrow().clone_deep();
                return Ok(Value::Object(Rc::new(RefCell::new(cloned))));
            }
            // Look up method in the struct's method table via method_cache
            let type_name = rc.borrow().type_name().to_string();
            let method_def = self.method_cache.get(&type_name)
                .and_then(|methods| methods.get(method).cloned());
            if let Some(mdef) = method_def {
                let arg_vals: Vec<Value> = args.iter()
                    .map(|a| self.eval_expr(a))
                    .collect::<Result<_, _>>()?;
                let caller = format!("{type_name}.{method}");
                return self.call_body(
                    &caller,
                    &mdef.params,
                    &mdef.body,
                    &[("this", Value::Object(rc.clone()))],
                    &arg_vals,
                    span.line,
                );
            }
            return Err(self.err(ErrorCode::R004, span.line, format!(
                "`{type_name}` has no method `{method}`"
            )));
        }

        // List higher-order methods — need interpreter access to call closures.
        if let Value::List(ref items_rc) = obj {
            match method {
                "map" | "filter" | "any" | "all" | "sort" | "search" | "bsearch" | "take" | "drop" | "cut" | "paste" => {
                    let arg_vals: Vec<Value> = args.iter()
                        .map(|a| self.eval_expr(a))
                        .collect::<Result<_, _>>()?;
                    return self.eval_list_higher_order(items_rc, method, &arg_vals, span.line);
                }
                _ => {}
            }
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

    // ─── List higher-order methods ─────────────────────────────────────────────

    fn call_closure_value(
        &mut self,
        closure: &Value,
        args: &[Value],
        line: usize,
    ) -> Result<Value, RuntimeError> {
        match closure {
            Value::Closure(data) => {
                let pre: Vec<(&str, Value)> = data.captured.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
                self.call_body("<closure>", &data.params, &data.body, &pre, args, line)
            }
            _ => Err(self.err(ErrorCode::R001, line, "expected a function/closure argument".to_string())),
        }
    }

    fn eval_list_higher_order(
        &mut self,
        items_rc: &Rc<RefCell<Vec<Value>>>,
        method: &str,
        args: &[Value],
        line: usize,
    ) -> Result<Value, RuntimeError> {
        match method {
            "map" => {
                if args.len() != 1 {
                    return Err(self.err(ErrorCode::R008, line, format!("`map` expects 1 argument, got {}", args.len())));
                }
                let closure = args[0].clone();
                // Clone is necessary: the closure may mutate this list during iteration,
                // so we can't hold a borrow across closure calls.
                let items = items_rc.borrow().clone();
                let mut result = Vec::with_capacity(items.len());
                for item in items {
                    let val = self.call_closure_value(&closure, &[item], line)?;
                    result.push(val);
                }
                Ok(Value::List(Rc::new(RefCell::new(result))))
            }
            "filter" => {
                if args.len() != 1 {
                    return Err(self.err(ErrorCode::R008, line, format!("`filter` expects 1 argument, got {}", args.len())));
                }
                let closure = args[0].clone();
                // Clone is necessary: the closure may mutate this list during iteration,
                // so we can't hold a borrow across closure calls.
                let items = items_rc.borrow().clone();
                let mut result = Vec::new();
                for item in items {
                    let val = self.call_closure_value(&closure, std::slice::from_ref(&item), line)?;
                    if val.is_truthy() {
                        result.push(item);
                    }
                }
                Ok(Value::List(Rc::new(RefCell::new(result))))
            }
            "search" => {
                if args.len() != 1 {
                    return Err(self.err(ErrorCode::R008, line, format!("`search` expects 1 argument, got {}", args.len())));
                }
                let target = &args[0];
                let items = items_rc.borrow();
                for (i, item) in items.iter().enumerate() {
                    if values_equal(item, target) {
                        return Ok(Value::Float(i as f64));
                    }
                }
                Ok(Value::Float(-1.0))
            }
            "bsearch" => {
                if args.len() != 1 {
                    return Err(self.err(ErrorCode::R008, line, format!("`bsearch` expects 1 argument, got {}", args.len())));
                }
                let target = &args[0];
                let items = items_rc.borrow();
                let result = items.binary_search_by(|item| {
                    match (item, target) {
                        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal),
                        (Value::Str(a), Value::Str(b)) => a.cmp(b),
                        _ => std::cmp::Ordering::Equal,
                    }
                });
                match result {
                    Ok(idx) => Ok(Value::Float(idx as f64)),
                    Err(_) => Ok(Value::Float(-1.0)),
                }
            }
            "sort" => {
                if args.is_empty() {
                    // Default sort: ascending numeric/string
                    let mut items = items_rc.borrow_mut();
                    items.sort_by(|a, b| {
                        match (a, b) {
                            (Value::Float(x), Value::Float(y)) => x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                            (Value::Str(x), Value::Str(y)) => x.cmp(y),
                            _ => std::cmp::Ordering::Equal,
                        }
                    });
                    Ok(Value::Float(0.0)) // void — return dummy
                } else if args.len() == 1 {
                    // Sort with comparator closure
                    let closure = args[0].clone();
                    let mut items_vec = items_rc.borrow().clone();
                    // Use a cell to propagate errors out of sort_by
                    let mut sort_error: Option<RuntimeError> = None;
                    // Safety: Rust's sort_by is memory-safe even with non-transitive comparators.
                    // A side-effectful closure may produce inconsistent ordering, but this only
                    // affects sort output correctness, not memory safety. Errors are captured
                    // in sort_error and propagated after sort completes.
                    items_vec.sort_by(|a, b| {
                        if sort_error.is_some() {
                            return std::cmp::Ordering::Equal;
                        }
                        match self.call_closure_value(&closure, &[a.clone(), b.clone()], line) {
                            Ok(Value::Float(v)) => {
                                if v < 0.0 { std::cmp::Ordering::Less }
                                else if v > 0.0 { std::cmp::Ordering::Greater }
                                else { std::cmp::Ordering::Equal }
                            }
                            Ok(_) => std::cmp::Ordering::Equal,
                            Err(e) => { sort_error = Some(e); std::cmp::Ordering::Equal }
                        }
                    });
                    if let Some(e) = sort_error {
                        return Err(e);
                    }
                    *items_rc.borrow_mut() = items_vec;
                    Ok(Value::Float(0.0))
                } else {
                    Err(self.err(ErrorCode::R008, line, format!("`sort` expects 0 or 1 arguments, got {}", args.len())))
                }
            }
            "take" => {
                if args.len() != 2 {
                    return Err(self.err(ErrorCode::R008, line, format!("`take` expects 2 arguments, got {}", args.len())));
                }
                let start = float_to_index(&args[0], "take", line)?;
                let end = float_to_index(&args[1], "take", line)?;
                let items = items_rc.borrow();
                let len = items.len();
                let start = start.min(len);
                let end = end.min(len);
                let slice = items[start..end].to_vec();
                Ok(Value::List(Rc::new(RefCell::new(slice))))
            }
            "drop" => {
                if args.len() != 2 {
                    return Err(self.err(ErrorCode::R008, line, format!("`drop` expects 2 arguments, got {}", args.len())));
                }
                let start = float_to_index(&args[0], "drop", line)?;
                let end = float_to_index(&args[1], "drop", line)?;
                let items = items_rc.borrow();
                let len = items.len();
                let start = start.min(len);
                let end = end.min(len);
                let mut result = items[..start].to_vec();
                result.extend_from_slice(&items[end..]);
                Ok(Value::List(Rc::new(RefCell::new(result))))
            }
            "cut" => {
                if args.len() != 2 {
                    return Err(self.err(ErrorCode::R008, line, format!("`cut` expects 2 arguments, got {}", args.len())));
                }
                let start = float_to_index(&args[0], "cut", line)?;
                let end = float_to_index(&args[1], "cut", line)?;
                let mut items = items_rc.borrow_mut();
                let len = items.len();
                let start = start.min(len);
                let end = end.min(len);
                let removed: Vec<Value> = items.drain(start..end).collect();
                Ok(Value::List(Rc::new(RefCell::new(removed))))
            }
            "paste" => {
                if args.len() != 2 {
                    return Err(self.err(ErrorCode::R008, line, format!("`paste` expects 2 arguments, got {}", args.len())));
                }
                let index = float_to_index(&args[0], "paste", line)?;
                match &args[1] {
                    Value::List(other_rc) => {
                        let other_items = other_rc.borrow().clone();
                        let mut items = items_rc.borrow_mut();
                        let idx = index.min(items.len());
                        for (i, item) in other_items.into_iter().enumerate() {
                            items.insert(idx + i, item);
                        }
                    }
                    single_value => {
                        let mut items = items_rc.borrow_mut();
                        let idx = index.min(items.len());
                        items.insert(idx, single_value.clone());
                    }
                }
                Ok(Value::Float(0.0))
            }
            "any" => {
                if args.len() != 1 {
                    return Err(self.err(ErrorCode::R008, line, format!("`any` expects 1 argument, got {}", args.len())));
                }
                let closure = args[0].clone();
                // Clone is necessary: the closure may mutate this list during iteration,
                // so we can't hold a borrow across closure calls.
                let items = items_rc.borrow().clone();
                for item in items {
                    let val = self.call_closure_value(&closure, &[item], line)?;
                    if val.is_truthy() {
                        return Ok(Value::Bool(true));
                    }
                }
                Ok(Value::Bool(false))
            }
            "all" => {
                if args.len() != 1 {
                    return Err(self.err(ErrorCode::R008, line, format!("`all` expects 1 argument, got {}", args.len())));
                }
                let closure = args[0].clone();
                // Clone is necessary: the closure may mutate this list during iteration,
                // so we can't hold a borrow across closure calls.
                let items = items_rc.borrow().clone();
                for item in items {
                    let val = self.call_closure_value(&closure, &[item], line)?;
                    if !val.is_truthy() {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }
            _ => unreachable!(),
        }
    }

    // ─── Statement executor ───────────────────────────────────────────────────

    /// # Errors
    /// Returns a runtime error if the statement cannot be executed.
    #[expect(clippy::too_many_lines, reason = "large match on Stmt variants; splitting would add indirection")]
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
                        let obj = self.env.get(root).cloned()
                            .ok_or_else(|| self.err(ErrorCode::R002, a.span.line, format!("undefined: `{root}`")))?;
                        if let Value::State(rc) = &obj {
                            assign_state_path(rc, &p[1..], val, a.span.line, self.types)?;
                        } else if let Value::Object(rc) = &obj {
                            assign_object_path(rc, &p[1..], val, a.span.line, self.types)?;
                        } else {
                            let updated = set_field_path(self.types, obj, &p[1..], val, a.span.line)?;
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
                    match &arm.pattern {
                        ast::MatchPattern::Else => {
                            matched = true;
                        }
                        ast::MatchPattern::Values(values) => {
                            for val_expr in values {
                                let val = self.eval_expr(val_expr)?;
                                if values_equal(&scrut, &val) {
                                    matched = true;
                                    break;
                                }
                            }
                        }
                        ast::MatchPattern::EnumVariant { enum_name: _, variant, .. } => {
                            // Check if the scrutinee is an enum value matching this variant
                            if let Value::EnumVariant { variant: scrut_variant, .. } = &scrut {
                                if scrut_variant == variant {
                                    matched = true;
                                }
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
            AssignTarget::Path(p) if p.len() == 1 => self.env.get(&p[0]).cloned()
                .ok_or_else(|| self.err(ErrorCode::R002, line, format!("undefined: `{}`", p[0]))),
            AssignTarget::Path(p) => {
                let root = self.env.get(&p[0]).cloned()
                    .ok_or_else(|| self.err(ErrorCode::R002, line, format!("undefined: `{}`", p[0])))?;
                let mut v = root;
                for seg in &p[1..] {
                    v = eval_field(self.types, &v, seg, line)?;
                }
                Ok(v)
            }
            AssignTarget::Indexed { path: p, indices } => {
                let mut coll = self.env.get(&p[0]).cloned()
                    .ok_or_else(|| self.err(ErrorCode::R002, line, format!("undefined: `{}`", p[0])))?
                    ;
                for seg in &p[1..] {
                    coll = eval_field(self.types, &coll, seg, line)?;
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
                let obj = self.env.get(root).cloned()
                    .ok_or_else(|| self.err(ErrorCode::R002, line, format!("undefined: `{root}`")))?;
                if let Value::State(rc) = &obj {
                    assign_state_path(rc, &p[1..], val, line, self.types)?;
                } else if let Value::Object(rc) = &obj {
                    assign_object_path(rc, &p[1..], val, line, self.types)?;
                } else {
                    let updated = set_field_path(self.types, obj, &p[1..], val, line)?;
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
        let mut coll = self.env.get(root).cloned()
            .ok_or_else(|| self.err(ErrorCode::R002, line, format!("undefined: `{root}`")))?
            ;
        for p in path.iter().skip(1) {
            coll = eval_field(self.types, &coll, p, line)?;
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
        let last_idx = indices.last().expect("indices is non-empty");
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

/// Convert an assignable Expr (Ident, Field, Index) to `AssignTarget`.
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
                let mut keys: Vec<&str> = guard.keys().map(std::string::String::as_str).collect();
                keys.sort_unstable();
                RuntimeError::new(ErrorCode::R003, line, 0,
                    format!("state has no field `{field}` (available: {})", keys.join(", ")))
            });
    }
    // Enum variant fields — accessed after match narrows the variant.
    if let Value::EnumVariant { enum_name, variant, fields } = obj {
        return fields.get(field).cloned().ok_or_else(|| {
            let available: Vec<&str> = fields.keys().map(String::as_str).collect();
            if available.is_empty() {
                RuntimeError::new(ErrorCode::R003, line, 0,
                    format!("`{enum_name}.{variant}` has no fields"))
            } else {
                RuntimeError::new(ErrorCode::R003, line, 0,
                    format!("`{enum_name}.{variant}` has no field `{field}` (available: {})", available.join(", ")))
            }
        });
    }
    // Object (struct) fields — reference type with dynamic fields.
    if let Value::Object(rc) = obj {
        let guard = rc.borrow();
        return guard.get_field(field).ok_or_else(|| {
            let names = guard.field_names();
            RuntimeError::new(ErrorCode::R003, line, 0,
                format!("'{}' has no field `{field}` (available: {})", guard.type_name(), names.join(", ")))
        });
    }
    match types.get_field(obj, field) {
        Some(result) => result,
        None => Err(RuntimeError::new(ErrorCode::R003, line, 0, format!(
            "`{}` has no field `{field}`", value_type_name(obj)
        ))),
    }
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
                let mut keys: Vec<&str> = guard.keys().map(std::string::String::as_str).collect();
                keys.sort_unstable();
                RuntimeError::new(ErrorCode::R003, line, 0,
                    format!("state has no field `{field}` (available: {})", keys.join(", ")))
            })?;
        drop(guard);
        if let Value::Object(obj_rc) = &intermediate {
            assign_object_path(obj_rc, &path[1..], val, line, types)?;
        } else {
            let updated = set_field_path(types, intermediate, &path[1..], val, line)?;
            rc.borrow_mut().insert(field.clone(), updated);
        }
    }
    Ok(())
}

/// Assign into an Object value at `path`, mutating through the Rc in-place.
fn assign_object_path(
    rc: &Rc<RefCell<Box<dyn crate::runtime::object::RustleObject>>>,
    path: &[String],
    val: Value,
    line: usize,
    types: &TypeRegistry,
) -> Result<(), RuntimeError> {
    let field = &path[0];
    if path.len() == 1 {
        let mut guard = rc.borrow_mut();
        if !guard.set_field(field, val) {
            let names = guard.field_names();
            return Err(RuntimeError::new(ErrorCode::R003, line, 0,
                format!("'{}' has no field `{field}` (available: {})", guard.type_name(), names.join(", "))));
        }
    } else {
        // Multi-level: get intermediate, check if it's also an Object (recurse) or value type (rebuild)
        let intermediate = {
            let guard = rc.borrow();
            guard.get_field(field).ok_or_else(|| {
                let names = guard.field_names();
                RuntimeError::new(ErrorCode::R003, line, 0,
                    format!("'{}' has no field `{field}` (available: {})", guard.type_name(), names.join(", ")))
            })?
        };
        if let Value::Object(inner_rc) = &intermediate {
            assign_object_path(inner_rc, &path[1..], val, line, types)?;
        } else {
            let updated = set_field_path(types, intermediate, &path[1..], val, line)?;
            let mut guard = rc.borrow_mut();
            guard.set_field(field, updated);
        }
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
        let sub = match types.get_field(&obj, field) {
            Some(result) => result?,
            None => return Err(RuntimeError::new(ErrorCode::R003, line, 0, format!(
                "`{}` has no field `{field}`", value_type_name(&obj)
            ))),
        };
        set_field_path(types, sub, &path[1..], val, line)?
    } else {
        val
    };
    types.set_field(obj, field, new_val)
        .ok_or_else(|| RuntimeError::new(ErrorCode::R003, line, 0, format!(
            "cannot assign field `{field}` (read-only or unknown)"
        )))
}

// ─── Transform application ────────────────────────────────────────────────────

fn apply_transform(shape: Value, tf: Value, line: usize) -> Result<Value, RuntimeError> {
    let Value::Transform(td) = tf else {
        return Err(RuntimeError::new(ErrorCode::R001, line, 0, format!(
            "@  requires transform, got `{}`", value_type_name(&tf)
        )));
    };
    let Value::Shape(mut data) = shape else {
        return Err(RuntimeError::new(ErrorCode::R001, line, 0, format!(
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
        Err(RuntimeError::new(ErrorCode::R011, line, 0, format!(
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
            other => Err(RuntimeError::new(ErrorCode::R011, line, 0, format!(
                "unary `-` not supported on `{}`", value_type_name(&other)
            ))),
        },
        UnOp::Not => Ok(Value::Bool(!v.is_truthy())),
    }
}

// ─── Cast ─────────────────────────────────────────────────────────────────────

fn cast_value(val: Value, target: &ast::Type, line: usize) -> Result<Value, RuntimeError> {
    use ast::Type;
    match (&val, target) {
        // Same type — no-op
        (Value::Float(_), Type::Float)
        | (Value::Bool(_), Type::Bool)
        | (Value::Str(_), Type::String) => Ok(val),

        // float → bool
        (Value::Float(x), Type::Bool) => Ok(Value::Bool(*x != 0.0 && !x.is_nan())),
        // bool → float
        (Value::Bool(b), Type::Float) => Ok(Value::Float(if *b { 1.0 } else { 0.0 })),

        // float/bool → string
        (Value::Float(_) | Value::Bool(_), Type::String) => Ok(Value::Str(val.to_string())),

        // string → float
        (Value::Str(s), Type::Float) => s.parse::<f64>().map(Value::Float).map_err(|_| {
            RuntimeError::new(
                ErrorCode::R001,
                line,
                0,
                format!("cannot convert string `{s}` to float"),
            )
        }),
        // string → bool (empty = false, non-empty = true)
        (Value::Str(s), Type::Bool) => Ok(Value::Bool(!s.is_empty())),

        // Unsupported conversion
        _ => Err(RuntimeError::new(
            ErrorCode::R001,
            line,
            0,
            format!(
                "cannot cast `{}` to `{}`",
                value_type_name(&val),
                crate::analysis::checker::type_name(target)
            ),
        )),
    }
}

// ─── Utilities ────────────────────────────────────────────────────────────────

// Language-level equality uses exact float comparison (== on f64), matching the spec.
#[allow(clippy::float_cmp)]
/// Language-level equality using bitwise float comparison.
/// NaN == NaN is true, +0.0 != -0.0 (totalOrder semantics).
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Float(x), Value::Float(y)) => x.to_bits() == y.to_bits(),
        (Value::Bool(x),   Value::Bool(y))   => x == y,
        (Value::Str(x),    Value::Str(y))    => x == y,
        (Value::Vec2(ax, ay), Value::Vec2(bx, by)) =>
            ax.to_bits() == bx.to_bits() && ay.to_bits() == by.to_bits(),
        (Value::Vec3(ax,ay,az), Value::Vec3(bx,by,bz)) =>
            ax.to_bits() == bx.to_bits() && ay.to_bits() == by.to_bits() && az.to_bits() == bz.to_bits(),
        (Value::Vec4(ax,ay,az,aw), Value::Vec4(bx,by,bz,bw)) =>
            ax.to_bits() == bx.to_bits() && ay.to_bits() == by.to_bits() && az.to_bits() == bz.to_bits() && aw.to_bits() == bw.to_bits(),
        (Value::Color { r: ar, g: ag, b: ab, a: aa }, Value::Color { r: br, g: bg, b: bb, a: ba }) =>
            ar.to_bits() == br.to_bits() && ag.to_bits() == bg.to_bits() && ab.to_bits() == bb.to_bits() && aa.to_bits() == ba.to_bits(),
        (Value::Object(a), Value::Object(b)) => Rc::ptr_eq(a, b),
        (Value::EnumVariant { enum_name: en_a, variant: v_a, fields: f_a },
         Value::EnumVariant { enum_name: en_b, variant: v_b, fields: f_b }) => {
            en_a == en_b && v_a == v_b && f_a.len() == f_b.len()
                && f_a.iter().all(|(k, va)| f_b.get(k).map_or(false, |vb| values_equal(va, vb)))
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
        Expr::Interpolated(parts, _) => {
            for part in parts {
                if let ast::InterpolPart::Expr(e) = part {
                    collect_free_expr(e, bound, free);
                }
            }
        }
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
        Expr::Cast { expr, .. } | Expr::Try { expr, .. }
        | Expr::Field { expr, .. } | Expr::OptionalChain { expr, .. } => collect_free_expr(expr, bound, free),
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
        Expr::StructConstruction { fields, .. } => {
            for (_, e) in fields { collect_free_expr(e, bound, free); }
        }
        Expr::EnumConstruction { fields, .. } => {
            for (_, e) in fields { collect_free_expr(e, bound, free); }
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
                if let ast::MatchPattern::Values(values) = &arm.pattern {
                    for v in values { collect_free_expr(v, bound, free); }
                }
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
        other => return Err(RuntimeError::new(ErrorCode::R001, line, 0, format!(
            "index must be a number, got `{}`", value_type_name(other)
        ))),
    };
    if f.is_nan() {
        return Err(RuntimeError::new(ErrorCode::R006, line, 0, "index is NaN"));
    }
    if f.is_infinite() {
        return Err(RuntimeError::new(ErrorCode::R006, line, 0, "index is infinite"));
    }
    if f < 0.0 {
        #[allow(clippy::cast_possible_truncation)]
        return Err(RuntimeError::new(ErrorCode::R006, line, 0, format!("index is negative ({})", f as i64)));
    }
    if f.fract() != 0.0 {
        return Err(RuntimeError::new(ErrorCode::R006, line, 0, format!("index must be an integer, got {f}")));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(f as usize)
}

fn parse_hex_color(hex: &str) -> Result<Value, RuntimeError> {
    let parse = |s: &str| u8::from_str_radix(s, 16)
        .map(|n| f64::from(n) / 255.0)
        .map_err(|_| RuntimeError::new(ErrorCode::R001, 0, 0, format!("invalid hex: #{hex}")));
    match hex.len() {
        6 => Ok(Value::Color { r: parse(&hex[0..2])?, g: parse(&hex[2..4])?, b: parse(&hex[4..6])?, a: 1.0 }),
        8 => Ok(Value::Color { r: parse(&hex[0..2])?, g: parse(&hex[2..4])?, b: parse(&hex[4..6])?, a: parse(&hex[6..8])? }),
        _ => Err(RuntimeError::new(ErrorCode::R001, 0, 0, format!("invalid hex color length: #{hex}"))),
    }
}
