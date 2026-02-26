//! Always-available built-ins — no import required.
//! Includes: math, constructors (vec2/3/4, color, transform), result helpers, constants.

use crate::syntax::ast::Type;
use crate::types::draw::TransformData;
use crate::types::mat::{
    m3_identity, m3_translate2d, m3_rotate2d, m3_scale2d,
    m4_identity, m4_translate, m4_scale_xyz, m4_rotate_x, m4_rotate_y, m4_rotate_z,
};
use crate::error::RuntimeError;
use crate::Value;
use std::collections::HashMap;
use super::{Export, ExportKind, NamespaceInfo, NamespaceProvider, RuntimeState, as_float, check_argc, value_type_name};

// ─── Type helpers ─────────────────────────────────────────────────────────────

fn f(name: &'static str, params: Vec<Type>, ret: Type) -> Export {
    Export { name, kind: ExportKind::Function, ty: Type::Fn(params, Some(Box::new(ret))) }
}

fn c(name: &'static str, ty: Type) -> Export {
    Export { name, kind: ExportKind::Constant, ty }
}

fn named(s: &str) -> Type { Type::Named(s.into()) }

// ─── Type exports (used by resolver/collector) ─────────────────────────────

pub fn core_exports() -> Vec<Export> {
    vec![
        // Math
        f("sin",   vec![Type::Float], Type::Float),
        f("cos",   vec![Type::Float], Type::Float),
        f("tan",   vec![Type::Float], Type::Float),
        f("asin",  vec![Type::Float], Type::Float),
        f("acos",  vec![Type::Float], Type::Float),
        f("atan",  vec![Type::Float], Type::Float),
        f("atan2", vec![Type::Float, Type::Float], Type::Float),
        f("sqrt",  vec![Type::Float], Type::Float),
        f("pow",   vec![Type::Float, Type::Float], Type::Float),
        f("abs",   vec![Type::Float], Type::Float),
        f("floor", vec![Type::Float], Type::Float),
        f("ceil",  vec![Type::Float], Type::Float),
        f("round", vec![Type::Float], Type::Float),
        f("sign",  vec![Type::Float], Type::Float),
        f("fract", vec![Type::Float], Type::Float),
        f("min",   vec![Type::Float, Type::Float], Type::Float),
        f("max",   vec![Type::Float, Type::Float], Type::Float),
        f("clamp", vec![Type::Float, Type::Float, Type::Float], Type::Float),
        f("lerp",  vec![Type::Float, Type::Float, Type::Float], Type::Float),

        // Constructors
        f("vec2",      vec![Type::Float, Type::Float], named("vec2")),
        f("vec3",      vec![Type::Float, Type::Float, Type::Float], named("vec3")),
        f("vec4",      vec![Type::Float, Type::Float, Type::Float, Type::Float], named("vec4")),
        f("color",     vec![Type::Float, Type::Float, Type::Float], named("color")),
        f("transform", vec![], named("transform")),
        f("mat3",      vec![], named("mat3")),
        f("mat4",      vec![], named("mat4")),
        // Mat3 2D constructors (angle in degrees)
        f("mat3_translate", vec![Type::Float, Type::Float], named("mat3")),
        f("mat3_rotate",    vec![Type::Float],               named("mat3")),
        f("mat3_scale",     vec![Type::Float, Type::Float], named("mat3")),
        // Mat4 3D constructors (angles in degrees)
        f("mat4_translate", vec![Type::Float, Type::Float, Type::Float], named("mat4")),
        f("mat4_scale",     vec![Type::Float, Type::Float, Type::Float], named("mat4")),
        f("mat4_rotate_x",  vec![Type::Float], named("mat4")),
        f("mat4_rotate_y",  vec![Type::Float], named("mat4")),
        f("mat4_rotate_z",  vec![Type::Float], named("mat4")),

        // Result helpers
        f("ok",    vec![Type::Float], Type::Res(Box::new(Type::Float))),
        f("error", vec![named("string")], Type::Res(Box::new(Type::Float))),

        // Constants
        c("PI",          Type::Float),
        c("TAU",         Type::Float),
        c("red",         named("color")),
        c("green",       named("color")),
        c("blue",        named("color")),
        c("white",       named("color")),
        c("black",       named("color")),
        c("transparent", named("color")),
    ]
}

// ─── CoreNamespace — runtime provider ────────────────────────────────────────

pub struct CoreNamespace;

impl NamespaceInfo for CoreNamespace {
    fn name(&self) -> &'static str { "core" }
    fn exports(&self) -> Vec<Export> { core_exports() }
}

impl NamespaceProvider for CoreNamespace {
    fn call(
        &self,
        name: &str,
        args: &[Value],
        _named: &HashMap<String, Value>,
        _state: &mut RuntimeState,
        line: usize,
    ) -> Result<Option<Value>, RuntimeError> {
        let v = match name {
            // ── 1-arg math ────────────────────────────────────────────────
            "sin"   => { check_argc(name, args, 1, line)?; Value::Float(as_float(&args[0], line)?.sin()) }
            "cos"   => { check_argc(name, args, 1, line)?; Value::Float(as_float(&args[0], line)?.cos()) }
            "tan"   => { check_argc(name, args, 1, line)?; Value::Float(as_float(&args[0], line)?.tan()) }
            "asin"  => { check_argc(name, args, 1, line)?; Value::Float(as_float(&args[0], line)?.asin()) }
            "acos"  => { check_argc(name, args, 1, line)?; Value::Float(as_float(&args[0], line)?.acos()) }
            "atan"  => { check_argc(name, args, 1, line)?; Value::Float(as_float(&args[0], line)?.atan()) }
            "sqrt"  => { check_argc(name, args, 1, line)?; Value::Float(as_float(&args[0], line)?.sqrt()) }
            "abs"   => { check_argc(name, args, 1, line)?; Value::Float(as_float(&args[0], line)?.abs()) }
            "floor" => { check_argc(name, args, 1, line)?; Value::Float(as_float(&args[0], line)?.floor()) }
            "ceil"  => { check_argc(name, args, 1, line)?; Value::Float(as_float(&args[0], line)?.ceil()) }
            "round" => { check_argc(name, args, 1, line)?; Value::Float(as_float(&args[0], line)?.round()) }
            "sign"  => { check_argc(name, args, 1, line)?; Value::Float(as_float(&args[0], line)?.signum()) }
            "fract" => {
                check_argc(name, args, 1, line)?;
                let x = as_float(&args[0], line)?;
                Value::Float(x - x.floor())
            }

            // ── 2-arg math ────────────────────────────────────────────────
            "atan2" => {
                check_argc(name, args, 2, line)?;
                Value::Float(as_float(&args[0], line)?.atan2(as_float(&args[1], line)?))
            }
            "pow" => {
                check_argc(name, args, 2, line)?;
                Value::Float(as_float(&args[0], line)?.powf(as_float(&args[1], line)?))
            }
            "min" => {
                check_argc(name, args, 2, line)?;
                Value::Float(as_float(&args[0], line)?.min(as_float(&args[1], line)?))
            }
            "max" => {
                check_argc(name, args, 2, line)?;
                Value::Float(as_float(&args[0], line)?.max(as_float(&args[1], line)?))
            }

            // ── 3-arg math ────────────────────────────────────────────────
            "clamp" => {
                check_argc(name, args, 3, line)?;
                let (x, lo, hi) = (as_float(&args[0], line)?, as_float(&args[1], line)?, as_float(&args[2], line)?);
                Value::Float(x.clamp(lo, hi))
            }
            "lerp" => {
                check_argc(name, args, 3, line)?;
                let (a, b, t) = (as_float(&args[0], line)?, as_float(&args[1], line)?, as_float(&args[2], line)?);
                Value::Float(a + (b - a) * t)
            }

            // ── Constructors ──────────────────────────────────────────────
            "vec2" => {
                check_argc(name, args, 2, line)?;
                Value::Vec2(as_float(&args[0], line)?, as_float(&args[1], line)?)
            }
            "vec3" => {
                check_argc(name, args, 3, line)?;
                Value::Vec3(as_float(&args[0], line)?, as_float(&args[1], line)?, as_float(&args[2], line)?)
            }
            "vec4" => {
                check_argc(name, args, 4, line)?;
                Value::Vec4(
                    as_float(&args[0], line)?, as_float(&args[1], line)?,
                    as_float(&args[2], line)?, as_float(&args[3], line)?,
                )
            }
            "color" => {
                if args.len() != 3 && args.len() != 4 {
                    return Err(RuntimeError::new(line, format!(
                        "`color` expects 3 or 4 args, got {}", args.len()
                    )));
                }
                let a = if args.len() == 4 { as_float(&args[3], line)? } else { 1.0 };
                Value::Color {
                    r: as_float(&args[0], line)?,
                    g: as_float(&args[1], line)?,
                    b: as_float(&args[2], line)?,
                    a,
                }
            }
            "transform" => {
                Value::Transform(TransformData::default())
            }
            "mat3" => {
                Value::Mat3(Box::new(m3_identity()))
            }
            "mat4" => {
                Value::Mat4(Box::new(m4_identity()))
            }
            "mat3_translate" => {
                check_argc(name, args, 2, line)?;
                Value::Mat3(Box::new(m3_translate2d(as_float(&args[0], line)?, as_float(&args[1], line)?)))
            }
            "mat3_rotate" => {
                check_argc(name, args, 1, line)?;
                Value::Mat3(Box::new(m3_rotate2d(as_float(&args[0], line)?.to_radians())))
            }
            "mat3_scale" => {
                check_argc(name, args, 2, line)?;
                Value::Mat3(Box::new(m3_scale2d(as_float(&args[0], line)?, as_float(&args[1], line)?)))
            }
            "mat4_translate" => {
                check_argc(name, args, 3, line)?;
                Value::Mat4(Box::new(m4_translate(as_float(&args[0], line)?, as_float(&args[1], line)?, as_float(&args[2], line)?)))
            }
            "mat4_scale" => {
                check_argc(name, args, 3, line)?;
                Value::Mat4(Box::new(m4_scale_xyz(as_float(&args[0], line)?, as_float(&args[1], line)?, as_float(&args[2], line)?)))
            }
            "mat4_rotate_x" => {
                check_argc(name, args, 1, line)?;
                Value::Mat4(Box::new(m4_rotate_x(as_float(&args[0], line)?.to_radians())))
            }
            "mat4_rotate_y" => {
                check_argc(name, args, 1, line)?;
                Value::Mat4(Box::new(m4_rotate_y(as_float(&args[0], line)?.to_radians())))
            }
            "mat4_rotate_z" => {
                check_argc(name, args, 1, line)?;
                Value::Mat4(Box::new(m4_rotate_z(as_float(&args[0], line)?.to_radians())))
            }

            // ── Result helpers ─────────────────────────────────────────────
            "ok" => {
                check_argc(name, args, 1, line)?;
                Value::ResOk(Box::new(args[0].clone()))
            }
            "error" => {
                check_argc(name, args, 1, line)?;
                let msg = match &args[0] {
                    Value::Str(s) => s.clone(),
                    other => format!("({})", value_type_name(other)),
                };
                Value::ResErr(msg)
            }

            _ => return Ok(None),
        };
        Ok(Some(v))
    }

    fn get_constant(&self, name: &str) -> Option<Value> {
        match name {
            "PI"          => Some(Value::Float(std::f64::consts::PI)),
            "TAU"         => Some(Value::Float(std::f64::consts::TAU)),
            "red"         => Some(Value::Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 }),
            "green"       => Some(Value::Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 }),
            "blue"        => Some(Value::Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 }),
            "white"       => Some(Value::Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 }),
            "black"       => Some(Value::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 }),
            "transparent" => Some(Value::Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }),
            _ => None,
        }
    }
}
