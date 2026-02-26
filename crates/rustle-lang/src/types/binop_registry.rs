//! Operator overload registry — maps (BinOp, lhs_type, rhs_type) → implementation.
//!
//! Adding operators for a new type = calling `register()` here.
//! No edits to interpreter.rs needed.

use std::collections::HashMap;

use crate::syntax::ast::BinOp;
use crate::error::RuntimeError;
use crate::runtime::value::Value;
use crate::types::registry::value_type_key;

// ─── Function pointer ─────────────────────────────────────────────────────────

pub type BinopFn = fn(Value, Value, usize) -> Result<Value, RuntimeError>;

// ─── Registry ─────────────────────────────────────────────────────────────────

pub struct BinopRegistry {
    ops: HashMap<(BinOp, &'static str, &'static str), (&'static str, BinopFn)>,
}

impl BinopRegistry {
    pub fn new() -> Self {
        Self { ops: HashMap::new() }
    }

    pub fn register(&mut self, op: BinOp, lhs: &'static str, rhs: &'static str, ret: &'static str, f: BinopFn) {
        self.ops.insert((op, lhs, rhs), (ret, f));
    }

    /// Return the result type key for `lhs op rhs`, or `None` if not registered.
    /// Used by the type checker at compile time.
    pub fn result_type(&self, op: &BinOp, lhs: &'static str, rhs: &'static str) -> Option<&'static str> {
        self.ops.get(&(op.clone(), lhs, rhs)).map(|(ret, _)| *ret)
    }

    /// Evaluate `l op r`. Returns `None` if no handler is registered for this
    /// type combination — the caller should produce an appropriate error.
    pub fn eval(
        &self,
        op:   &BinOp,
        l:    Value,
        r:    Value,
        line: usize,
    ) -> Option<Result<Value, RuntimeError>> {
        let lkey = value_type_key(&l);
        let rkey = value_type_key(&r);
        self.ops.get(&(op.clone(), lkey, rkey)).map(|(_, f)| f(l, r, line))
    }
}

// ─── Compile-time type helpers ────────────────────────────────────────────────

use crate::syntax::ast::Type;

/// Map a `Type` to its BinopRegistry key. Returns `None` for generic/compound types.
pub fn type_to_key(ty: &Type) -> Option<&'static str> {
    match ty {
        Type::Float                          => Some("float"),
        Type::Bool                           => Some("bool"),
        Type::Named(n) => match n.as_str() {
            "vec2"      => Some("vec2"),
            "vec3"      => Some("vec3"),
            "vec4"      => Some("vec4"),
            "color"     => Some("color"),
            "mat3"      => Some("mat3"),
            "mat4"      => Some("mat4"),
            "transform" => Some("transform"),
            "shape"     => Some("shape"),
            _           => None,
        },
        _ => None,
    }
}

/// Map a BinopRegistry return-type key back to a `Type`.
pub fn key_to_type(key: &str) -> Type {
    match key {
        "float" => Type::Float,
        "bool"  => Type::Bool,
        n       => Type::Named(n.to_string()),
    }
}

impl Default for BinopRegistry {
    fn default() -> Self {
        let mut r = Self::new();
        register_float(&mut r);
        register_vec2(&mut r);
        register_vec3(&mut r);
        register_vec4(&mut r);
        register_color(&mut r);
        register_bool(&mut r);
        register_mat3(&mut r);
        register_mat4(&mut r);
        r
    }
}

// ─── float ────────────────────────────────────────────────────────────────────

fn register_float(r: &mut BinopRegistry) {
    use BinOp::*;
    r.register(Add, "float", "float", "float", |l, r, _| {
        let (Value::Float(a), Value::Float(b)) = (l, r) else { unreachable!() };
        Ok(Value::Float(a + b))
    });
    r.register(Sub, "float", "float", "float", |l, r, _| {
        let (Value::Float(a), Value::Float(b)) = (l, r) else { unreachable!() };
        Ok(Value::Float(a - b))
    });
    r.register(Mul, "float", "float", "float", |l, r, _| {
        let (Value::Float(a), Value::Float(b)) = (l, r) else { unreachable!() };
        Ok(Value::Float(a * b))
    });
    r.register(Div, "float", "float", "float", |l, r, line| {
        let (Value::Float(a), Value::Float(b)) = (l, r) else { unreachable!() };
        if b == 0.0 { Err(RuntimeError::new(line, "division by zero")) }
        else { Ok(Value::Float(a / b)) }
    });
    r.register(Mod, "float", "float", "float", |l, r, line| {
        let (Value::Float(a), Value::Float(b)) = (l, r) else { unreachable!() };
        if b == 0.0 { Err(RuntimeError::new(line, "mod by zero")) }
        else { Ok(Value::Float(a % b)) }
    });
    r.register(Lt,   "float", "float", "bool", |l, r, _| { let (Value::Float(a), Value::Float(b)) = (l, r) else { unreachable!() }; Ok(Value::Bool(a <  b)) });
    r.register(LtEq, "float", "float", "bool", |l, r, _| { let (Value::Float(a), Value::Float(b)) = (l, r) else { unreachable!() }; Ok(Value::Bool(a <= b)) });
    r.register(Gt,   "float", "float", "bool", |l, r, _| { let (Value::Float(a), Value::Float(b)) = (l, r) else { unreachable!() }; Ok(Value::Bool(a >  b)) });
    r.register(GtEq, "float", "float", "bool", |l, r, _| { let (Value::Float(a), Value::Float(b)) = (l, r) else { unreachable!() }; Ok(Value::Bool(a >= b)) });
    r.register(Eq,   "float", "float", "bool", |l, r, _| { let (Value::Float(a), Value::Float(b)) = (l, r) else { unreachable!() }; Ok(Value::Bool(a == b)) });
    r.register(NotEq,"float", "float", "bool", |l, r, _| { let (Value::Float(a), Value::Float(b)) = (l, r) else { unreachable!() }; Ok(Value::Bool(a != b)) });
}

// ─── vec2 ─────────────────────────────────────────────────────────────────────

fn register_vec2(r: &mut BinopRegistry) {
    use BinOp::*;
    r.register(Add, "vec2", "vec2", "vec2", |l, r, _| {
        let (Value::Vec2(ax, ay), Value::Vec2(bx, by)) = (l, r) else { unreachable!() };
        Ok(Value::Vec2(ax + bx, ay + by))
    });
    r.register(Sub, "vec2", "vec2", "vec2", |l, r, _| {
        let (Value::Vec2(ax, ay), Value::Vec2(bx, by)) = (l, r) else { unreachable!() };
        Ok(Value::Vec2(ax - bx, ay - by))
    });
    r.register(Mul, "vec2", "float", "vec2", |l, r, _| {
        let (Value::Vec2(x, y), Value::Float(s)) = (l, r) else { unreachable!() };
        Ok(Value::Vec2(x * s, y * s))
    });
    r.register(Mul, "float", "vec2", "vec2", |l, r, _| {
        let (Value::Float(s), Value::Vec2(x, y)) = (l, r) else { unreachable!() };
        Ok(Value::Vec2(x * s, y * s))
    });
    r.register(Div, "vec2", "float", "vec2", |l, r, line| {
        let (Value::Vec2(x, y), Value::Float(s)) = (l, r) else { unreachable!() };
        if s == 0.0 { Err(RuntimeError::new(line, "division by zero")) }
        else { Ok(Value::Vec2(x / s, y / s)) }
    });
    r.register(Eq,    "vec2", "vec2", "bool", |l, r, _| { let (Value::Vec2(ax,ay), Value::Vec2(bx,by)) = (l,r) else { unreachable!() }; Ok(Value::Bool(ax==bx && ay==by)) });
    r.register(NotEq, "vec2", "vec2", "bool", |l, r, _| { let (Value::Vec2(ax,ay), Value::Vec2(bx,by)) = (l,r) else { unreachable!() }; Ok(Value::Bool(ax!=bx || ay!=by)) });
}

// ─── vec3 ─────────────────────────────────────────────────────────────────────

fn register_vec3(r: &mut BinopRegistry) {
    use BinOp::*;
    r.register(Add, "vec3", "vec3", "vec3", |l, r, _| {
        let (Value::Vec3(ax,ay,az), Value::Vec3(bx,by,bz)) = (l, r) else { unreachable!() };
        Ok(Value::Vec3(ax+bx, ay+by, az+bz))
    });
    r.register(Sub, "vec3", "vec3", "vec3", |l, r, _| {
        let (Value::Vec3(ax,ay,az), Value::Vec3(bx,by,bz)) = (l, r) else { unreachable!() };
        Ok(Value::Vec3(ax-bx, ay-by, az-bz))
    });
    r.register(Mul, "vec3", "float", "vec3", |l, r, _| {
        let (Value::Vec3(x,y,z), Value::Float(s)) = (l, r) else { unreachable!() };
        Ok(Value::Vec3(x*s, y*s, z*s))
    });
    r.register(Mul, "float", "vec3", "vec3", |l, r, _| {
        let (Value::Float(s), Value::Vec3(x,y,z)) = (l, r) else { unreachable!() };
        Ok(Value::Vec3(x*s, y*s, z*s))
    });
    r.register(Div, "vec3", "float", "vec3", |l, r, line| {
        let (Value::Vec3(x,y,z), Value::Float(s)) = (l, r) else { unreachable!() };
        if s == 0.0 { Err(RuntimeError::new(line, "division by zero")) }
        else { Ok(Value::Vec3(x/s, y/s, z/s)) }
    });
    r.register(Eq,    "vec3", "vec3", "bool", |l, r, _| { let (Value::Vec3(ax,ay,az), Value::Vec3(bx,by,bz)) = (l,r) else { unreachable!() }; Ok(Value::Bool(ax==bx && ay==by && az==bz)) });
    r.register(NotEq, "vec3", "vec3", "bool", |l, r, _| { let (Value::Vec3(ax,ay,az), Value::Vec3(bx,by,bz)) = (l,r) else { unreachable!() }; Ok(Value::Bool(ax!=bx || ay!=by || az!=bz)) });
}

// ─── vec4 ─────────────────────────────────────────────────────────────────────

fn register_vec4(r: &mut BinopRegistry) {
    use BinOp::*;
    r.register(Add, "vec4", "vec4", "vec4", |l, r, _| {
        let (Value::Vec4(ax,ay,az,aw), Value::Vec4(bx,by,bz,bw)) = (l, r) else { unreachable!() };
        Ok(Value::Vec4(ax+bx, ay+by, az+bz, aw+bw))
    });
    r.register(Sub, "vec4", "vec4", "vec4", |l, r, _| {
        let (Value::Vec4(ax,ay,az,aw), Value::Vec4(bx,by,bz,bw)) = (l, r) else { unreachable!() };
        Ok(Value::Vec4(ax-bx, ay-by, az-bz, aw-bw))
    });
    r.register(Mul, "vec4", "float", "vec4", |l, r, _| {
        let (Value::Vec4(x,y,z,w), Value::Float(s)) = (l, r) else { unreachable!() };
        Ok(Value::Vec4(x*s, y*s, z*s, w*s))
    });
    r.register(Mul, "float", "vec4", "vec4", |l, r, _| {
        let (Value::Float(s), Value::Vec4(x,y,z,w)) = (l, r) else { unreachable!() };
        Ok(Value::Vec4(x*s, y*s, z*s, w*s))
    });
    r.register(Div, "vec4", "float", "vec4", |l, r, line| {
        let (Value::Vec4(x,y,z,w), Value::Float(s)) = (l, r) else { unreachable!() };
        if s == 0.0 { Err(RuntimeError::new(line, "division by zero")) }
        else { Ok(Value::Vec4(x/s, y/s, z/s, w/s)) }
    });
    r.register(Eq,    "vec4", "vec4", "bool", |l, r, _| { let (Value::Vec4(ax,ay,az,aw), Value::Vec4(bx,by,bz,bw)) = (l,r) else { unreachable!() }; Ok(Value::Bool(ax==bx && ay==by && az==bz && aw==bw)) });
    r.register(NotEq, "vec4", "vec4", "bool", |l, r, _| { let (Value::Vec4(ax,ay,az,aw), Value::Vec4(bx,by,bz,bw)) = (l,r) else { unreachable!() }; Ok(Value::Bool(ax!=bx || ay!=by || az!=bz || aw!=bw)) });
}

// ─── color ────────────────────────────────────────────────────────────────────

fn register_color(r: &mut BinopRegistry) {
    use BinOp::*;
    r.register(Add, "color", "color", "color", |l, r, _| {
        let (Value::Color { r: ar, g: ag, b: ab, a: aa }, Value::Color { r: br, g: bg, b: bb, a: ba }) = (l, r) else { unreachable!() };
        Ok(Value::Color { r: (ar+br).min(1.0), g: (ag+bg).min(1.0), b: (ab+bb).min(1.0), a: (aa+ba).min(1.0) })
    });
    r.register(Mul, "color", "float", "color", |l, r, _| {
        let (Value::Color { r: cr, g: cg, b: cb, a: ca }, Value::Float(s)) = (l, r) else { unreachable!() };
        Ok(Value::Color { r: (cr*s).min(1.0), g: (cg*s).min(1.0), b: (cb*s).min(1.0), a: (ca*s).min(1.0) })
    });
    r.register(Eq, "color", "color", "bool", |l, r, _| {
        let (Value::Color { r: ar, g: ag, b: ab, a: aa }, Value::Color { r: br, g: bg, b: bb, a: ba }) = (l, r) else { unreachable!() };
        Ok(Value::Bool(ar==br && ag==bg && ab==bb && aa==ba))
    });
    r.register(NotEq, "color", "color", "bool", |l, r, _| {
        let (Value::Color { r: ar, g: ag, b: ab, a: aa }, Value::Color { r: br, g: bg, b: bb, a: ba }) = (l, r) else { unreachable!() };
        Ok(Value::Bool(ar!=br || ag!=bg || ab!=bb || aa!=ba))
    });
}

// ─── bool ─────────────────────────────────────────────────────────────────────

fn register_bool(r: &mut BinopRegistry) {
    use BinOp::*;
    r.register(And, "bool", "bool", "bool", |l, r, _| {
        let (Value::Bool(a), Value::Bool(b)) = (l, r) else { unreachable!() };
        Ok(Value::Bool(a && b))
    });
    r.register(Or, "bool", "bool", "bool", |l, r, _| {
        let (Value::Bool(a), Value::Bool(b)) = (l, r) else { unreachable!() };
        Ok(Value::Bool(a || b))
    });
    r.register(Eq,    "bool", "bool", "bool", |l, r, _| { let (Value::Bool(a), Value::Bool(b)) = (l, r) else { unreachable!() }; Ok(Value::Bool(a == b)) });
    r.register(NotEq, "bool", "bool", "bool", |l, r, _| { let (Value::Bool(a), Value::Bool(b)) = (l, r) else { unreachable!() }; Ok(Value::Bool(a != b)) });
}

// ─── mat3 ─────────────────────────────────────────────────────────────────────

fn register_mat3(r: &mut BinopRegistry) {
    use BinOp::*;
    use crate::types::mat::*;
    r.register(Mul, "mat3", "mat3", "mat3", |l, r, _| {
        let (Value::Mat3(a), Value::Mat3(b)) = (l, r) else { unreachable!() };
        Ok(Value::Mat3(Box::new(m3_mul(&a, &b))))
    });
    r.register(Mul, "mat3", "vec3", "vec3", |l, r, _| {
        let (Value::Mat3(m), Value::Vec3(x, y, z)) = (l, r) else { unreachable!() };
        let (rx, ry, rz) = m3_mul_vec(&m, (x, y, z));
        Ok(Value::Vec3(rx, ry, rz))
    });
    r.register(Mul, "mat3", "float", "mat3", |l, r, _| {
        let (Value::Mat3(m), Value::Float(s)) = (l, r) else { unreachable!() };
        Ok(Value::Mat3(Box::new(m3_scale(&m, s))))
    });
    r.register(Mul, "float", "mat3", "mat3", |l, r, _| {
        let (Value::Float(s), Value::Mat3(m)) = (l, r) else { unreachable!() };
        Ok(Value::Mat3(Box::new(m3_scale(&m, s))))
    });
}

// ─── mat4 ─────────────────────────────────────────────────────────────────────

fn register_mat4(r: &mut BinopRegistry) {
    use BinOp::*;
    use crate::types::mat::*;
    r.register(Mul, "mat4", "mat4", "mat4", |l, r, _| {
        let (Value::Mat4(a), Value::Mat4(b)) = (l, r) else { unreachable!() };
        Ok(Value::Mat4(Box::new(m4_mul(&a, &b))))
    });
    r.register(Mul, "mat4", "vec4", "vec4", |l, r, _| {
        let (Value::Mat4(m), Value::Vec4(x, y, z, w)) = (l, r) else { unreachable!() };
        let (rx, ry, rz, rw) = m4_mul_vec(&m, (x, y, z, w));
        Ok(Value::Vec4(rx, ry, rz, rw))
    });
    r.register(Mul, "mat4", "float", "mat4", |l, r, _| {
        let (Value::Mat4(m), Value::Float(s)) = (l, r) else { unreachable!() };
        Ok(Value::Mat4(Box::new(m4_scale(&m, s))))
    });
    r.register(Mul, "float", "mat4", "mat4", |l, r, _| {
        let (Value::Float(s), Value::Mat4(m)) = (l, r) else { unreachable!() };
        Ok(Value::Mat4(Box::new(m4_scale(&m, s))))
    });
}
