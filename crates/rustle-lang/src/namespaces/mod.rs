use crate::syntax::ast::Type;
use crate::types::draw::RenderMode;
use crate::error::{ErrorCode, RuntimeError};
use crate::Value;
use std::collections::HashMap;

// ─── Runtime state ────────────────────────────────────────────────────────────

/// Interpreter-level state passed to every namespace call.
/// Holds the current coordinate context — updated by `resolution`, `default`,
/// `normalize`, `origin` and snapshotted into each `ShapeData` at build time.
#[derive(Clone)]
#[derive(Default)]
pub struct RuntimeState {
    pub coord_meta: crate::types::draw::CoordMeta,
}


pub mod core;
pub mod shapes;
pub mod render;
pub mod coords;

// ─── Export ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportKind { Function, Constant }

#[derive(Debug, Clone)]
pub struct Export {
    pub name: &'static str,
    pub kind: ExportKind,
    pub ty:   Type,
}

// ─── Compile-time interface ───────────────────────────────────────────────────

/// What the resolver needs: type exports only.
/// No dependency on `Value`, `RuntimeError`, or runtime state.
pub trait NamespaceInfo: Send + Sync {
    fn name(&self) -> &'static str;
    fn exports(&self) -> Vec<Export>;

    fn get_export(&self, name: &str) -> Option<Export> {
        self.exports().into_iter().find(|e| e.name == name)
    }
}

// ─── Runtime interface ────────────────────────────────────────────────────────

/// What the interpreter needs: call dispatch + constant lookup.
/// Extends `NamespaceInfo` so a single object serves both roles.
pub trait NamespaceProvider: NamespaceInfo {
    /// # Errors
    /// Returns an error if the call fails (wrong argument types, count, or runtime failure).
    fn call(
        &self,
        name: &str,
        args: &[Value],
        named: &HashMap<String, Value>,
        state: &mut RuntimeState,
        line: usize,
    ) -> Result<Option<Value>, RuntimeError>;

    fn get_constant(&self, name: &str) -> Option<Value>;
}

// ─── Registry ─────────────────────────────────────────────────────────────────

/// Registry of namespace providers for resolving imports and dispatching calls.
///
/// Uses dynamic dispatch (`Box<dyn NamespaceProvider>`) to support potential
/// future plugin/extension namespaces. The fixed set of built-in namespaces
/// (core, shapes, render, coords) is registered via [`Self::standard()`].
pub struct NamespaceRegistry {
    providers: Vec<Box<dyn NamespaceProvider>>,
}

impl NamespaceRegistry {
    #[must_use] 
    pub fn new() -> Self { Self { providers: Vec::new() } }

    pub fn register(&mut self, p: Box<dyn NamespaceProvider>) { self.providers.push(p); }

    #[must_use] 
    pub fn get(&self, name: &str) -> Option<&dyn NamespaceProvider> {
        self.providers.iter().find(|p| p.name() == name).map(std::convert::AsRef::as_ref)
    }

    /// # Errors
    /// Returns an error if the matching provider's call fails.
    pub fn call_any(
        &self,
        name: &str,
        args: &[Value],
        named: &HashMap<String, Value>,
        state: &mut RuntimeState,
        line: usize,
    ) -> Result<Option<Value>, RuntimeError> {
        for p in &self.providers {
            if let Some(v) = p.call(name, args, named, state, line)? {
                return Ok(Some(v));
            }
        }
        Ok(None)
    }

    #[must_use] 
    pub fn get_constant(&self, name: &str) -> Option<Value> {
        self.providers.iter().find_map(|p| p.get_constant(name))
    }

    #[must_use] 
    pub fn standard() -> Self {
        let mut r = Self::new();
        r.register(Box::new(core::CoreNamespace));
        r.register(Box::new(shapes::ShapesNamespace));
        r.register(Box::new(render::RenderNamespace));
        r.register(Box::new(coords::CoordsNamespace));
        r
    }
}

impl Default for NamespaceRegistry {
    fn default() -> Self { Self::standard() }
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

pub(crate) fn as_float(v: &Value, line: usize) -> Result<f64, RuntimeError> {
    match v {
        Value::Float(x) => Ok(*x),
        _ => Err(RuntimeError::new(ErrorCode::R001, line, format!("expected float, got {}", value_type_name(v)))),
    }
}

pub(crate) fn as_vec2(v: &Value, line: usize) -> Result<(f64, f64), RuntimeError> {
    match v {
        Value::Vec2(x, y) => Ok((*x, *y)),
        _ => Err(RuntimeError::new(ErrorCode::R001, line, format!("expected vec2, got {}", value_type_name(v)))),
    }
}

pub(crate) fn as_vertices(v: &Value, line: usize) -> Result<Vec<(f64, f64)>, RuntimeError> {
    match v {
        Value::List(items) => items.borrow().iter().map(|i| as_vec2(i, line)).collect(),
        _ => Err(RuntimeError::new(ErrorCode::R001, line, "expected list[vec2]")),
    }
}

pub(crate) fn check_argc(name: &str, args: &[Value], n: usize, line: usize) -> Result<(), RuntimeError> {
    if args.len() == n {
        Ok(())
    } else {
        Err(RuntimeError::new(ErrorCode::R008, line, format!("`{name}` expects {n} args, got {}", args.len())))
    }
}

pub(crate) fn render_mode_from_named(named: &HashMap<String, Value>, line: usize) -> Result<RenderMode, RuntimeError> {
    match named.get("render") {
        Some(Value::RenderMode(m)) => Ok(m.clone()),
        Some(_) => Err(RuntimeError::new(ErrorCode::R011, line, "`render:` must be a render_mode value")),
        None    => Ok(RenderMode::default()),
    }
}

pub(crate) fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Float(_)      => "float",
        Value::Bool(_)       => "bool",
        Value::Str(_)        => "string",
        Value::Vec2(..)      => "vec2",
        Value::Vec3(..)      => "vec3",
        Value::Vec4(..)      => "vec4",
        Value::Color { .. }  => "color",
        Value::Mat3(_)       => "mat3",
        Value::Mat4(_)       => "mat4",
        Value::List(_)       => "list",
        Value::Shape(_)      => "shape",
        Value::Transform(_)  => "transform",
        Value::RenderMode(_) => "render_mode",
        Value::ResOk(_)      => "res<ok>",
        Value::ResErr(_)     => "res<err>",
        Value::Namespace(_)  => "namespace",
        Value::NativeFn(_) | Value::Closure(_) => "fn",
        Value::State(_)      => "State",
        Value::Input {..}    => "Input",
        Value::None          => "none",
    }
}
