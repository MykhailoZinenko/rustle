use std::collections::HashMap;
use std::rc::Rc;
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
        params: Vec<Param>,
        body:   Vec<Stmt>,
        captured: HashMap<String, Value>,
    },
    State(Rc<RefCell<HashMap<String, Value>>>),
    Input { dt: f64 },
}
