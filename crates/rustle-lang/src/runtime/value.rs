use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::cell::RefCell;

use crate::syntax::ast::{Param, Stmt};
use crate::types::draw::{RenderMode, ShapeData, TransformData};

#[derive(Debug, Clone)]
pub enum Value {
    Float(f64),
    Bool(bool),
    Str(String),
    Vec2(f64, f64),
    Vec3(f64, f64, f64),
    Vec4(f64, f64, f64, f64),
    Mat3(Box<[f64; 9]>),   // row-major 3×3
    Mat4(Box<[f64; 16]>),  // row-major 4×4
    Color { r: f64, g: f64, b: f64, a: f64 },
    List(Rc<RefCell<Vec<Value>>>),
    Shape(ShapeData),
    Transform(TransformData),
    RenderMode(RenderMode),
    ResOk(Box<Value>),
    ResErr(String),
    Namespace(String),
    NativeFn(String),
    Closure {
        params: Arc<[Param]>,
        body:   Arc<[Stmt]>,
        captured: HashMap<String, Value>,
    },
    State(Rc<RefCell<HashMap<String, Value>>>),
    Input { dt: f64 },
    None,
}

impl Value {
    /// Returns whether this value is "truthy" in a boolean context.
    #[must_use] 
    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Float(f) => *f != 0.0 && !f.is_nan(),
            Value::Str(s) => !s.is_empty(),
            Value::List(rc) => !rc.borrow().is_empty(),
            Value::None => false,
            _ => true,
        }
    }
}

impl fmt::Display for Value {
    #[expect(clippy::many_single_char_names, reason = "f, x, y, z, w are conventional in fmt and math contexts")]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Float(v) => {
                #[allow(clippy::cast_possible_truncation)]
                if v.fract() == 0.0 && v.abs() < 1e15 { write!(f, "{}", *v as i64) }
                else { write!(f, "{v}") }
            }
            Value::Bool(b)   => write!(f, "{b}"),
            Value::Str(s)    => write!(f, "{s}"),
            Value::Vec2(x, y)           => write!(f, "vec2({x}, {y})"),
            Value::Vec3(x, y, z)        => write!(f, "vec3({x}, {y}, {z})"),
            Value::Vec4(x, y, z, w)     => write!(f, "vec4({x}, {y}, {z}, {w})"),
            Value::Color { r, g, b, a } => write!(f, "color({r:.3}, {g:.3}, {b:.3}, {a:.3})"),
            Value::List(rc) => {
                let items = rc.borrow();
                write!(f, "[")?;
                for (i, v) in items.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Value::ResOk(v)  => write!(f, "ok({v})"),
            Value::ResErr(e) => write!(f, "err({e})"),
            Value::Mat3(_)   => write!(f, "mat3(...)"),
            Value::Mat4(_)   => write!(f, "mat4(...)"),
            Value::Shape(_)  => write!(f, "<shape>"),
            Value::Transform(_)  => write!(f, "<transform>"),
            Value::RenderMode(_) => write!(f, "<render_mode>"),
            Value::Namespace(n)  => write!(f, "<namespace:{n}>"),
            Value::NativeFn(n)   => write!(f, "<fn:{n}>"),
            Value::Closure { .. } => write!(f, "<closure>"),
            Value::State(_)  => write!(f, "<state>"),
            Value::Input { dt } => write!(f, "input(dt={dt})"),
            Value::None => write!(f, "none"),
        }
    }
}
