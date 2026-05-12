#![expect(clippy::match_same_arms, reason = "each type variant has its own arm for clarity and future divergence")]
//! Static compile-time type information for the resolver.
//!
//! Pure functions only — no runtime types, no Value, no `RuntimeError`, no closures.
//! Depends only on `crate::syntax::ast::{Type, BinOp}`.
//!
//! These replace all `TypeRegistry` and `BinopRegistry` queries in the resolver.

use crate::syntax::ast::{BinOp, Type};

// ─── field_type ───────────────────────────────────────────────────────────────

/// Return the type of `field` on `ty`, or `None` if no such field exists.
#[must_use]
pub fn field_type(ty: &Type, field: &str) -> Option<Type> {
    match ty {
        Type::Vec2 => match field {
            "x" | "y" => Some(Type::Float),
            _ => None,
        },
        Type::Vec3 => match field {
            "x" | "y" | "z" => Some(Type::Float),
            _ => None,
        },
        Type::Vec4 => match field {
            "x" | "y" | "z" | "w" => Some(Type::Float),
            _ => None,
        },
        Type::Color => match field {
            "r" | "g" | "b" | "a" => Some(Type::Float),
            _ => None,
        },
        Type::Circle => match field {
            "center" => Some(Type::Vec2),
            "radius" => Some(Type::Float),
            _ => None,
        },
        Type::Rect => match field {
            "center" | "size" => Some(Type::Vec2),
            _ => None,
        },
        Type::Line => match field {
            "from" | "to" => Some(Type::Vec2),
            _ => None,
        },
        Type::TextShape => match field {
            "pos" => Some(Type::Vec2),
            "content" => Some(Type::String),
            "size" => Some(Type::Float),
            _ => None,
        },
        Type::List(_) => match field {
            "len" => Some(Type::Float),
            _ => None,
        },
        Type::Res(inner) => match field {
            "ok" => Some(Type::Bool),
            "value" => Some(*inner.clone()),
            "error" => Some(Type::String),
            _ => None,
        },
        Type::Input => match field {
            "dt" | "mouse_x" | "mouse_y" => Some(Type::Float),
            "mouse_down" | "mouse_pressed" | "mouse_released" => Some(Type::Bool),
            "key_pressed" | "key_down" | "key_released" => Some(Type::String),
            _ => None,
        },
        // Types with no fields
        Type::Float
        | Type::Bool
        | Type::String
        | Type::Mat3
        | Type::Mat4
        | Type::Transform
        | Type::Shape
        | Type::Polygon
        | Type::Optional(_) => None,
        _ => None,
    }
}

// ─── field_names ──────────────────────────────────────────────────────────────

/// Return all field names available on `ty`.
#[must_use]
pub fn field_names(ty: &Type) -> &'static [&'static str] {
    match ty {
        Type::Vec2 => &["x", "y"],
        Type::Vec3 => &["x", "y", "z"],
        Type::Vec4 => &["x", "y", "z", "w"],
        Type::Color => &["r", "g", "b", "a"],
        Type::Circle => &["center", "radius"],
        Type::Rect => &["center", "size"],
        Type::Line => &["from", "to"],
        Type::TextShape => &["pos", "content", "size"],
        Type::List(_) => &["len"],
        Type::Res(_) => &["ok", "value", "error"],
        Type::Input => &[
            "dt",
            "mouse_x",
            "mouse_y",
            "mouse_down",
            "mouse_pressed",
            "mouse_released",
            "key_pressed",
            "key_down",
            "key_released",
        ],
        _ => &[],
    }
}

// ─── method_names ─────────────────────────────────────────────────────────────

/// Return all method names available on `ty`.
#[must_use]
pub fn method_names(ty: &Type) -> &'static [&'static str] {
    match ty {
        Type::String => &[
            "len", "contains", "starts_with", "ends_with", "trim",
            "to_upper", "to_lower", "replace", "split", "slice",
        ],
        Type::Vec2 => &[
            "length", "normalize", "dot", "lerp", "distance",
            "abs", "floor", "ceil", "min", "max", "perp", "angle",
        ],
        Type::Vec3 => &[
            "length", "normalize", "dot", "cross", "lerp",
            "abs", "min", "max", "reflect",
        ],
        Type::Vec4 => &[
            "length", "normalize", "dot", "lerp", "abs", "min", "max",
        ],
        Type::Color => &["lerp", "with_alpha", "to_vec4"],
        Type::Mat3 => &["transpose", "det", "inverse", "mul_vec", "scale"],
        Type::Mat4 => &["transpose", "det", "inverse", "mul_vec", "scale"],
        Type::Transform => &["move", "translate", "scale", "rotate"],
        Type::Shape | Type::Circle | Type::Rect | Type::Line | Type::Polygon => &["in"],
        Type::TextShape => &[],
        Type::List(_) => &[
            "push", "pop", "len", "clone", "map", "filter",
            "any", "all", "search", "bsearch", "sort",
            "take", "drop", "cut", "paste",
        ],
        Type::State => &["keys", "len"],
        _ => &[],
    }
}

// ─── method_signature ─────────────────────────────────────────────────────────

/// Return `(param_types, return_type)` for `method` on `ty`, or `None` if not found.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn method_signature(ty: &Type, method: &str) -> Option<(Vec<Type>, Option<Type>)> {
    match ty {
        // ── string ────────────────────────────────────────────────────────────
        Type::String => match method {
            "len" => Some((vec![], Some(Type::Float))),
            "contains" | "starts_with" | "ends_with" => {
                Some((vec![Type::String], Some(Type::Bool)))
            }
            "trim" | "to_upper" | "to_lower" => Some((vec![], Some(Type::String))),
            "replace" => Some((vec![Type::String, Type::String], Some(Type::String))),
            "split" => Some((
                vec![Type::String],
                Some(Type::List(Box::new(Type::String))),
            )),
            "slice" => Some((vec![Type::Float, Type::Float], Some(Type::String))),
            _ => None,
        },
        // ── vec2 ──────────────────────────────────────────────────────────────
        Type::Vec2 => match method {
            "length" => Some((vec![], Some(Type::Float))),
            "normalize" => Some((vec![], Some(Type::Vec2))),
            "dot" => Some((vec![Type::Vec2], Some(Type::Float))),
            "lerp" => Some((vec![Type::Vec2, Type::Float], Some(Type::Vec2))),
            "distance" => Some((vec![Type::Vec2], Some(Type::Float))),
            "abs" | "floor" | "ceil" | "perp" => Some((vec![], Some(Type::Vec2))),
            "min" | "max" => Some((vec![Type::Vec2], Some(Type::Vec2))),
            "angle" => Some((vec![], Some(Type::Float))),
            _ => None,
        },
        // ── vec3 ──────────────────────────────────────────────────────────────
        Type::Vec3 => match method {
            "length" => Some((vec![], Some(Type::Float))),
            "normalize" | "abs" => Some((vec![], Some(Type::Vec3))),
            "dot" => Some((vec![Type::Vec3], Some(Type::Float))),
            "cross" | "reflect" => Some((vec![Type::Vec3], Some(Type::Vec3))),
            "lerp" => Some((vec![Type::Vec3, Type::Float], Some(Type::Vec3))),
            "min" | "max" => Some((vec![Type::Vec3], Some(Type::Vec3))),
            _ => None,
        },
        // ── vec4 ──────────────────────────────────────────────────────────────
        Type::Vec4 => match method {
            "length" => Some((vec![], Some(Type::Float))),
            "normalize" | "abs" => Some((vec![], Some(Type::Vec4))),
            "dot" => Some((vec![Type::Vec4], Some(Type::Float))),
            "lerp" => Some((vec![Type::Vec4, Type::Float], Some(Type::Vec4))),
            "min" | "max" => Some((vec![Type::Vec4], Some(Type::Vec4))),
            _ => None,
        },
        // ── color ─────────────────────────────────────────────────────────────
        Type::Color => match method {
            "lerp" => Some((vec![Type::Color, Type::Float], Some(Type::Color))),
            "with_alpha" => Some((vec![Type::Float], Some(Type::Color))),
            "to_vec4" => Some((vec![], Some(Type::Vec4))),
            _ => None,
        },
        // ── mat3 ──────────────────────────────────────────────────────────────
        Type::Mat3 => match method {
            "transpose" | "inverse" => Some((vec![], Some(Type::Mat3))),
            "det" => Some((vec![], Some(Type::Float))),
            "mul_vec" => Some((vec![Type::Vec3], Some(Type::Vec3))),
            "scale" => Some((vec![Type::Float], Some(Type::Mat3))),
            _ => None,
        },
        // ── mat4 ──────────────────────────────────────────────────────────────
        Type::Mat4 => match method {
            "transpose" | "inverse" => Some((vec![], Some(Type::Mat4))),
            "det" => Some((vec![], Some(Type::Float))),
            "mul_vec" => Some((vec![Type::Vec4], Some(Type::Vec4))),
            "scale" => Some((vec![Type::Float], Some(Type::Mat4))),
            _ => None,
        },
        // ── transform ─────────────────────────────────────────────────────────
        Type::Transform => match method {
            "move" | "translate" => Some((vec![Type::Float, Type::Float], Some(Type::Transform))),
            "scale" | "rotate" => Some((vec![Type::Float], Some(Type::Transform))),
            _ => None,
        },
        // ── shapes ────────────────────────────────────────────────────────────
        Type::Shape | Type::Circle | Type::Rect | Type::Line | Type::Polygon => match method {
            "in" => Some((vec![Type::Float, Type::Float], Some(Type::Vec2))),
            _ => None,
        },
        Type::TextShape => None,
        // ── list<T> ───────────────────────────────────────────────────────────
        Type::List(elem) => {
            let elem_ty = *elem.clone();
            match method {
                "push" => Some((vec![elem_ty], None)),
                "pop" => Some((vec![], Some(elem_ty))),
                "len" => Some((vec![], Some(Type::Float))),
                "clone" => Some((vec![], Some(ty.clone()))),
                "map" => {
                    let fn_ty = Type::Fn(vec![elem_ty.clone()], Some(Box::new(elem_ty.clone())));
                    Some((vec![fn_ty], Some(Type::List(Box::new(elem_ty)))))
                }
                "filter" => {
                    let fn_ty =
                        Type::Fn(vec![elem_ty], Some(Box::new(Type::Bool)));
                    Some((vec![fn_ty], Some(ty.clone())))
                }
                "any" | "all" => {
                    let fn_ty =
                        Type::Fn(vec![elem_ty], Some(Box::new(Type::Bool)));
                    Some((vec![fn_ty], Some(Type::Bool)))
                }
                "search" | "bsearch" => Some((vec![elem_ty], Some(Type::Float))),
                "sort" => Some((vec![], None)),
                "take" | "drop" | "cut" => {
                    Some((vec![Type::Float, Type::Float], Some(Type::List(elem.clone()))))
                }
                "paste" => Some((vec![Type::Float, elem_ty], None)),
                _ => None,
            }
        }
        // ── State ─────────────────────────────────────────────────────────────
        Type::State => match method {
            "keys" => Some((vec![], Some(Type::List(Box::new(Type::String))))),
            "len" => Some((vec![], Some(Type::Float))),
            _ => None,
        },
        _ => None,
    }
}

// ─── binop_result_type ────────────────────────────────────────────────────────

/// Return the result type of `lhs op rhs`, or `None` if not applicable.
#[must_use]
pub fn binop_result_type(op: &BinOp, lhs: &Type, rhs: &Type) -> Option<Type> {
    use BinOp::{Add, Sub, Mul, Div, Mod, Lt, LtEq, Gt, GtEq, Eq, NotEq};

    // Eq / NotEq: same type on both sides → Bool
    if matches!(op, Eq | NotEq) && lhs == rhs {
        return Some(Type::Bool);
    }

    match (op, lhs, rhs) {
        // ── float ⊕ float ──────────────────────────────────────────────────
        (Add | Sub | Mul | Div | Mod, Type::Float, Type::Float) => Some(Type::Float),
        (Lt | LtEq | Gt | GtEq, Type::Float, Type::Float) => Some(Type::Bool),

        // ── bool arithmetic ────────────────────────────────────────────────
        (Add | Sub | Mul | Div | Mod, Type::Bool, Type::Float) => Some(Type::Float),
        (Add | Sub | Mul | Div | Mod, Type::Float, Type::Bool) => Some(Type::Float),
        (Add | Sub | Mul | Div | Mod, Type::Bool, Type::Bool) => Some(Type::Float),

        // ── vec2 ──────────────────────────────────────────────────────────
        (Add | Sub, Type::Vec2, Type::Vec2) => Some(Type::Vec2),
        (Mul, Type::Vec2, Type::Float) | (Mul, Type::Float, Type::Vec2) => Some(Type::Vec2),
        (Div, Type::Vec2, Type::Float) => Some(Type::Vec2),

        // ── vec3 ──────────────────────────────────────────────────────────
        (Add | Sub, Type::Vec3, Type::Vec3) => Some(Type::Vec3),
        (Mul, Type::Vec3, Type::Float) | (Mul, Type::Float, Type::Vec3) => Some(Type::Vec3),
        (Div, Type::Vec3, Type::Float) => Some(Type::Vec3),

        // ── vec4 ──────────────────────────────────────────────────────────
        (Add | Sub, Type::Vec4, Type::Vec4) => Some(Type::Vec4),
        (Mul, Type::Vec4, Type::Float) | (Mul, Type::Float, Type::Vec4) => Some(Type::Vec4),
        (Div, Type::Vec4, Type::Float) => Some(Type::Vec4),

        // ── color ─────────────────────────────────────────────────────────
        (Add, Type::Color, Type::Color) => Some(Type::Color),
        (Mul, Type::Color, Type::Float) => Some(Type::Color),

        // ── mat3 ──────────────────────────────────────────────────────────
        (Mul, Type::Mat3, Type::Mat3) => Some(Type::Mat3),
        (Mul, Type::Mat3, Type::Vec3) => Some(Type::Vec3),
        (Mul, Type::Mat3, Type::Float) | (Mul, Type::Float, Type::Mat3) => Some(Type::Mat3),

        // ── mat4 ──────────────────────────────────────────────────────────
        (Mul, Type::Mat4, Type::Mat4) => Some(Type::Mat4),
        (Mul, Type::Mat4, Type::Vec4) => Some(Type::Vec4),
        (Mul, Type::Mat4, Type::Float) | (Mul, Type::Float, Type::Mat4) => Some(Type::Mat4),

        // ── string ────────────────────────────────────────────────────────
        (Add, Type::String, Type::String) => Some(Type::String),
        (Lt | LtEq | Gt | GtEq, Type::String, Type::String) => Some(Type::Bool),

        _ => None,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── field_type ────────────────────────────────────────────────────────────

    #[test]
    fn field_type_vec2() {
        assert_eq!(field_type(&Type::Vec2, "x"), Some(Type::Float));
        assert_eq!(field_type(&Type::Vec2, "y"), Some(Type::Float));
        assert_eq!(field_type(&Type::Vec2, "z"), None);
    }

    #[test]
    fn field_type_vec3() {
        assert_eq!(field_type(&Type::Vec3, "x"), Some(Type::Float));
        assert_eq!(field_type(&Type::Vec3, "y"), Some(Type::Float));
        assert_eq!(field_type(&Type::Vec3, "z"), Some(Type::Float));
        assert_eq!(field_type(&Type::Vec3, "w"), None);
    }

    #[test]
    fn field_type_vec4() {
        for f in ["x", "y", "z", "w"] {
            assert_eq!(field_type(&Type::Vec4, f), Some(Type::Float));
        }
        assert_eq!(field_type(&Type::Vec4, "a"), None);
    }

    #[test]
    fn field_type_color() {
        for f in ["r", "g", "b", "a"] {
            assert_eq!(field_type(&Type::Color, f), Some(Type::Float));
        }
        assert_eq!(field_type(&Type::Color, "x"), None);
    }

    #[test]
    fn field_type_circle() {
        assert_eq!(field_type(&Type::Circle, "center"), Some(Type::Vec2));
        assert_eq!(field_type(&Type::Circle, "radius"), Some(Type::Float));
        assert_eq!(field_type(&Type::Circle, "size"), None);
    }

    #[test]
    fn field_type_rect() {
        assert_eq!(field_type(&Type::Rect, "center"), Some(Type::Vec2));
        assert_eq!(field_type(&Type::Rect, "size"), Some(Type::Vec2));
        assert_eq!(field_type(&Type::Rect, "radius"), None);
    }

    #[test]
    fn field_type_line() {
        assert_eq!(field_type(&Type::Line, "from"), Some(Type::Vec2));
        assert_eq!(field_type(&Type::Line, "to"), Some(Type::Vec2));
        assert_eq!(field_type(&Type::Line, "center"), None);
    }

    #[test]
    fn field_type_text_shape() {
        assert_eq!(field_type(&Type::TextShape, "pos"), Some(Type::Vec2));
        assert_eq!(field_type(&Type::TextShape, "content"), Some(Type::String));
        assert_eq!(field_type(&Type::TextShape, "size"), Some(Type::Float));
        assert_eq!(field_type(&Type::TextShape, "color"), None);
    }

    #[test]
    fn field_type_list() {
        let list_float = Type::List(Box::new(Type::Float));
        assert_eq!(field_type(&list_float, "len"), Some(Type::Float));
        assert_eq!(field_type(&list_float, "push"), None);
    }

    #[test]
    fn field_type_res() {
        let res_str = Type::Res(Box::new(Type::String));
        assert_eq!(field_type(&res_str, "ok"), Some(Type::Bool));
        assert_eq!(field_type(&res_str, "value"), Some(Type::String));
        assert_eq!(field_type(&res_str, "error"), Some(Type::String));
        assert_eq!(field_type(&res_str, "data"), None);
    }

    #[test]
    fn field_type_res_inner_type_preserved() {
        let res_vec2 = Type::Res(Box::new(Type::Vec2));
        assert_eq!(field_type(&res_vec2, "value"), Some(Type::Vec2));
    }

    #[test]
    fn field_type_input() {
        assert_eq!(field_type(&Type::Input, "dt"), Some(Type::Float));
        assert_eq!(field_type(&Type::Input, "mouse_x"), Some(Type::Float));
        assert_eq!(field_type(&Type::Input, "mouse_y"), Some(Type::Float));
        assert_eq!(field_type(&Type::Input, "mouse_down"), Some(Type::Bool));
        assert_eq!(field_type(&Type::Input, "mouse_pressed"), Some(Type::Bool));
        assert_eq!(field_type(&Type::Input, "mouse_released"), Some(Type::Bool));
        assert_eq!(field_type(&Type::Input, "key_pressed"), Some(Type::String));
        assert_eq!(field_type(&Type::Input, "key_down"), Some(Type::String));
        assert_eq!(field_type(&Type::Input, "key_released"), Some(Type::String));
        assert_eq!(field_type(&Type::Input, "unknown"), None);
    }

    #[test]
    fn field_type_no_fields() {
        assert_eq!(field_type(&Type::Float, "x"), None);
        assert_eq!(field_type(&Type::Bool, "x"), None);
        assert_eq!(field_type(&Type::String, "x"), None);
        assert_eq!(field_type(&Type::Mat3, "x"), None);
        assert_eq!(field_type(&Type::Mat4, "x"), None);
        assert_eq!(field_type(&Type::Transform, "x"), None);
        assert_eq!(field_type(&Type::Shape, "x"), None);
        assert_eq!(field_type(&Type::Polygon, "x"), None);
    }

    #[test]
    fn field_type_optional_is_none() {
        let opt = Type::Optional(Box::new(Type::Float));
        assert_eq!(field_type(&opt, "x"), None);
    }

    // ── field_names ───────────────────────────────────────────────────────────

    #[test]
    fn field_names_vec2() {
        assert_eq!(field_names(&Type::Vec2), &["x", "y"]);
    }

    #[test]
    fn field_names_color() {
        assert_eq!(field_names(&Type::Color), &["r", "g", "b", "a"]);
    }

    #[test]
    fn field_names_input() {
        let names = field_names(&Type::Input);
        assert!(names.contains(&"dt"));
        assert!(names.contains(&"mouse_x"));
        assert!(names.contains(&"key_pressed"));
        assert_eq!(names.len(), 9);
    }

    #[test]
    fn field_names_res() {
        let names = field_names(&Type::Res(Box::new(Type::Float)));
        assert_eq!(names, &["ok", "value", "error"]);
    }

    #[test]
    fn field_names_list() {
        let names = field_names(&Type::List(Box::new(Type::Float)));
        assert_eq!(names, &["len"]);
    }

    #[test]
    fn field_names_empty_types() {
        assert!(field_names(&Type::Float).is_empty());
        assert!(field_names(&Type::Bool).is_empty());
        assert!(field_names(&Type::Mat3).is_empty());
        assert!(field_names(&Type::Transform).is_empty());
    }

    // ── method_names ──────────────────────────────────────────────────────────

    #[test]
    fn method_names_string() {
        let names = method_names(&Type::String);
        assert!(names.contains(&"len"));
        assert!(names.contains(&"contains"));
        assert!(names.contains(&"split"));
        assert!(names.contains(&"slice"));
        assert_eq!(names.len(), 10);
    }

    #[test]
    fn method_names_vec2() {
        let names = method_names(&Type::Vec2);
        assert!(names.contains(&"length"));
        assert!(names.contains(&"normalize"));
        assert!(names.contains(&"perp"));
        assert!(names.contains(&"angle"));
        assert_eq!(names.len(), 12);
    }

    #[test]
    fn method_names_list() {
        let names = method_names(&Type::List(Box::new(Type::Float)));
        assert!(names.contains(&"push"));
        assert!(names.contains(&"pop"));
        assert!(names.contains(&"map"));
        assert!(names.contains(&"filter"));
        assert_eq!(names.len(), 15);
    }

    #[test]
    fn method_names_shapes() {
        assert_eq!(method_names(&Type::Shape), &["in"]);
        assert_eq!(method_names(&Type::Circle), &["in"]);
        assert_eq!(method_names(&Type::Rect), &["in"]);
        assert_eq!(method_names(&Type::Line), &["in"]);
        assert_eq!(method_names(&Type::Polygon), &["in"]);
    }

    #[test]
    fn method_names_text_shape_empty() {
        assert!(method_names(&Type::TextShape).is_empty());
    }

    #[test]
    fn method_names_state() {
        let names = method_names(&Type::State);
        assert!(names.contains(&"keys"));
        assert!(names.contains(&"len"));
    }

    // ── method_signature ──────────────────────────────────────────────────────

    #[test]
    fn method_sig_string_len() {
        let (params, ret) = method_signature(&Type::String, "len").unwrap();
        assert!(params.is_empty());
        assert_eq!(ret, Some(Type::Float));
    }

    #[test]
    fn method_sig_string_contains() {
        let (params, ret) = method_signature(&Type::String, "contains").unwrap();
        assert_eq!(params, vec![Type::String]);
        assert_eq!(ret, Some(Type::Bool));
    }

    #[test]
    fn method_sig_string_replace() {
        let (params, ret) = method_signature(&Type::String, "replace").unwrap();
        assert_eq!(params, vec![Type::String, Type::String]);
        assert_eq!(ret, Some(Type::String));
    }

    #[test]
    fn method_sig_string_split() {
        let (params, ret) = method_signature(&Type::String, "split").unwrap();
        assert_eq!(params, vec![Type::String]);
        assert_eq!(ret, Some(Type::List(Box::new(Type::String))));
    }

    #[test]
    fn method_sig_string_slice() {
        let (params, ret) = method_signature(&Type::String, "slice").unwrap();
        assert_eq!(params, vec![Type::Float, Type::Float]);
        assert_eq!(ret, Some(Type::String));
    }

    #[test]
    fn method_sig_vec2_length() {
        let (params, ret) = method_signature(&Type::Vec2, "length").unwrap();
        assert!(params.is_empty());
        assert_eq!(ret, Some(Type::Float));
    }

    #[test]
    fn method_sig_vec2_lerp() {
        let (params, ret) = method_signature(&Type::Vec2, "lerp").unwrap();
        assert_eq!(params, vec![Type::Vec2, Type::Float]);
        assert_eq!(ret, Some(Type::Vec2));
    }

    #[test]
    fn method_sig_vec2_dot() {
        let (params, ret) = method_signature(&Type::Vec2, "dot").unwrap();
        assert_eq!(params, vec![Type::Vec2]);
        assert_eq!(ret, Some(Type::Float));
    }

    #[test]
    fn method_sig_vec3_cross() {
        let (params, ret) = method_signature(&Type::Vec3, "cross").unwrap();
        assert_eq!(params, vec![Type::Vec3]);
        assert_eq!(ret, Some(Type::Vec3));
    }

    #[test]
    fn method_sig_vec4_lerp() {
        let (params, ret) = method_signature(&Type::Vec4, "lerp").unwrap();
        assert_eq!(params, vec![Type::Vec4, Type::Float]);
        assert_eq!(ret, Some(Type::Vec4));
    }

    #[test]
    fn method_sig_color_lerp() {
        let (params, ret) = method_signature(&Type::Color, "lerp").unwrap();
        assert_eq!(params, vec![Type::Color, Type::Float]);
        assert_eq!(ret, Some(Type::Color));
    }

    #[test]
    fn method_sig_color_with_alpha() {
        let (params, ret) = method_signature(&Type::Color, "with_alpha").unwrap();
        assert_eq!(params, vec![Type::Float]);
        assert_eq!(ret, Some(Type::Color));
    }

    #[test]
    fn method_sig_color_to_vec4() {
        let (params, ret) = method_signature(&Type::Color, "to_vec4").unwrap();
        assert!(params.is_empty());
        assert_eq!(ret, Some(Type::Vec4));
    }

    #[test]
    fn method_sig_mat3_transpose() {
        let (params, ret) = method_signature(&Type::Mat3, "transpose").unwrap();
        assert!(params.is_empty());
        assert_eq!(ret, Some(Type::Mat3));
    }

    #[test]
    fn method_sig_mat3_mul_vec() {
        let (params, ret) = method_signature(&Type::Mat3, "mul_vec").unwrap();
        assert_eq!(params, vec![Type::Vec3]);
        assert_eq!(ret, Some(Type::Vec3));
    }

    #[test]
    fn method_sig_mat4_mul_vec() {
        let (params, ret) = method_signature(&Type::Mat4, "mul_vec").unwrap();
        assert_eq!(params, vec![Type::Vec4]);
        assert_eq!(ret, Some(Type::Vec4));
    }

    #[test]
    fn method_sig_transform_move() {
        let (params, ret) = method_signature(&Type::Transform, "move").unwrap();
        assert_eq!(params, vec![Type::Float, Type::Float]);
        assert_eq!(ret, Some(Type::Transform));
    }

    #[test]
    fn method_sig_transform_scale() {
        let (params, ret) = method_signature(&Type::Transform, "scale").unwrap();
        assert_eq!(params, vec![Type::Float]);
        assert_eq!(ret, Some(Type::Transform));
    }

    #[test]
    fn method_sig_shape_in() {
        for ty in [Type::Shape, Type::Circle, Type::Rect, Type::Line, Type::Polygon] {
            let (params, ret) = method_signature(&ty, "in").unwrap();
            assert_eq!(params, vec![Type::Float, Type::Float]);
            assert_eq!(ret, Some(Type::Vec2));
        }
    }

    #[test]
    fn method_sig_text_shape_none() {
        assert!(method_signature(&Type::TextShape, "anything").is_none());
    }

    #[test]
    fn method_sig_list_push() {
        let list_float = Type::List(Box::new(Type::Float));
        let (params, ret) = method_signature(&list_float, "push").unwrap();
        assert_eq!(params, vec![Type::Float]);
        assert_eq!(ret, None);
    }

    #[test]
    fn method_sig_list_pop() {
        let list_str = Type::List(Box::new(Type::String));
        let (params, ret) = method_signature(&list_str, "pop").unwrap();
        assert!(params.is_empty());
        assert_eq!(ret, Some(Type::String));
    }

    #[test]
    fn method_sig_list_len() {
        let list_float = Type::List(Box::new(Type::Float));
        let (params, ret) = method_signature(&list_float, "len").unwrap();
        assert!(params.is_empty());
        assert_eq!(ret, Some(Type::Float));
    }

    #[test]
    fn method_sig_list_clone() {
        let list_float = Type::List(Box::new(Type::Float));
        let (params, ret) = method_signature(&list_float, "clone").unwrap();
        assert!(params.is_empty());
        assert_eq!(ret, Some(list_float));
    }

    #[test]
    fn method_sig_list_map() {
        let list_float = Type::List(Box::new(Type::Float));
        let (params, ret) = method_signature(&list_float, "map").unwrap();
        assert_eq!(params.len(), 1);
        // Returns list of same element type
        assert_eq!(ret, Some(Type::List(Box::new(Type::Float))));
    }

    #[test]
    fn method_sig_list_filter() {
        let list_float = Type::List(Box::new(Type::Float));
        let (params, ret) = method_signature(&list_float, "filter").unwrap();
        assert_eq!(params.len(), 1);
        assert_eq!(ret, Some(list_float));
    }

    #[test]
    fn method_sig_list_any_all() {
        let list_float = Type::List(Box::new(Type::Float));
        for m in ["any", "all"] {
            let (params, ret) = method_signature(&list_float, m).unwrap();
            assert_eq!(params.len(), 1);
            assert_eq!(ret, Some(Type::Bool));
        }
    }

    #[test]
    fn method_sig_list_sort() {
        let list_float = Type::List(Box::new(Type::Float));
        let (params, ret) = method_signature(&list_float, "sort").unwrap();
        assert!(params.is_empty());
        assert_eq!(ret, None);
    }

    #[test]
    fn method_sig_list_take_drop_cut() {
        let list_float = Type::List(Box::new(Type::Float));
        for m in ["take", "drop", "cut"] {
            let (params, ret) = method_signature(&list_float, m).unwrap();
            assert_eq!(params, vec![Type::Float, Type::Float]);
            assert_eq!(ret, Some(Type::List(Box::new(Type::Float))));
        }
    }

    #[test]
    fn method_sig_list_paste() {
        let list_float = Type::List(Box::new(Type::Float));
        let (params, ret) = method_signature(&list_float, "paste").unwrap();
        assert_eq!(params, vec![Type::Float, Type::Float]);
        assert_eq!(ret, None);
    }

    #[test]
    fn method_sig_list_search_bsearch() {
        let list_float = Type::List(Box::new(Type::Float));
        for m in ["search", "bsearch"] {
            let (params, ret) = method_signature(&list_float, m).unwrap();
            assert_eq!(params, vec![Type::Float]);
            assert_eq!(ret, Some(Type::Float));
        }
    }

    #[test]
    fn method_sig_state_keys() {
        let (params, ret) = method_signature(&Type::State, "keys").unwrap();
        assert!(params.is_empty());
        assert_eq!(ret, Some(Type::List(Box::new(Type::String))));
    }

    #[test]
    fn method_sig_state_len() {
        let (params, ret) = method_signature(&Type::State, "len").unwrap();
        assert!(params.is_empty());
        assert_eq!(ret, Some(Type::Float));
    }

    // ── binop_result_type ─────────────────────────────────────────────────────

    #[test]
    fn binop_float_arithmetic() {
        use BinOp::{Add, Sub, Mul, Div, Mod};
        for op in [Add, Sub, Mul, Div, Mod] {
            assert_eq!(
                binop_result_type(&op, &Type::Float, &Type::Float),
                Some(Type::Float)
            );
        }
    }

    #[test]
    fn binop_float_comparison() {
        use BinOp::{Lt, LtEq, Gt, GtEq};
        for op in [Lt, LtEq, Gt, GtEq] {
            assert_eq!(
                binop_result_type(&op, &Type::Float, &Type::Float),
                Some(Type::Bool)
            );
        }
    }

    #[test]
    fn binop_eq_neq_same_types() {
        use BinOp::{Eq, NotEq};
        for ty in [Type::Float, Type::Bool, Type::String, Type::Vec2, Type::Vec3, Type::Vec4, Type::Color] {
            assert_eq!(binop_result_type(&Eq, &ty, &ty), Some(Type::Bool));
            assert_eq!(binop_result_type(&NotEq, &ty, &ty), Some(Type::Bool));
        }
    }

    #[test]
    fn binop_eq_neq_different_types() {
        use BinOp::Eq;
        assert_eq!(binop_result_type(&Eq, &Type::Float, &Type::String), None);
    }

    #[test]
    fn binop_bool_arithmetic() {
        use BinOp::Add;
        assert_eq!(binop_result_type(&Add, &Type::Bool, &Type::Float), Some(Type::Float));
        assert_eq!(binop_result_type(&Add, &Type::Float, &Type::Bool), Some(Type::Float));
        assert_eq!(binop_result_type(&Add, &Type::Bool, &Type::Bool), Some(Type::Float));
    }

    #[test]
    fn binop_vec2_ops() {
        use BinOp::{Add, Sub, Mul, Div};
        assert_eq!(binop_result_type(&Add, &Type::Vec2, &Type::Vec2), Some(Type::Vec2));
        assert_eq!(binop_result_type(&Sub, &Type::Vec2, &Type::Vec2), Some(Type::Vec2));
        assert_eq!(binop_result_type(&Mul, &Type::Vec2, &Type::Float), Some(Type::Vec2));
        assert_eq!(binop_result_type(&Mul, &Type::Float, &Type::Vec2), Some(Type::Vec2));
        assert_eq!(binop_result_type(&Div, &Type::Vec2, &Type::Float), Some(Type::Vec2));
        assert_eq!(binop_result_type(&Mul, &Type::Vec2, &Type::Vec2), None);
    }

    #[test]
    fn binop_vec3_ops() {
        use BinOp::{Add, Sub, Mul, Div};
        assert_eq!(binop_result_type(&Add, &Type::Vec3, &Type::Vec3), Some(Type::Vec3));
        assert_eq!(binop_result_type(&Sub, &Type::Vec3, &Type::Vec3), Some(Type::Vec3));
        assert_eq!(binop_result_type(&Mul, &Type::Vec3, &Type::Float), Some(Type::Vec3));
        assert_eq!(binop_result_type(&Mul, &Type::Float, &Type::Vec3), Some(Type::Vec3));
        assert_eq!(binop_result_type(&Div, &Type::Vec3, &Type::Float), Some(Type::Vec3));
    }

    #[test]
    fn binop_vec4_ops() {
        use BinOp::{Add, Sub, Mul, Div};
        assert_eq!(binop_result_type(&Add, &Type::Vec4, &Type::Vec4), Some(Type::Vec4));
        assert_eq!(binop_result_type(&Sub, &Type::Vec4, &Type::Vec4), Some(Type::Vec4));
        assert_eq!(binop_result_type(&Mul, &Type::Vec4, &Type::Float), Some(Type::Vec4));
        assert_eq!(binop_result_type(&Mul, &Type::Float, &Type::Vec4), Some(Type::Vec4));
        assert_eq!(binop_result_type(&Div, &Type::Vec4, &Type::Float), Some(Type::Vec4));
    }

    #[test]
    fn binop_color_ops() {
        use BinOp::{Add, Mul};
        assert_eq!(binop_result_type(&Add, &Type::Color, &Type::Color), Some(Type::Color));
        assert_eq!(binop_result_type(&Mul, &Type::Color, &Type::Float), Some(Type::Color));
        assert_eq!(binop_result_type(&Mul, &Type::Float, &Type::Color), None); // not registered
    }

    #[test]
    fn binop_mat3_ops() {
        use BinOp::Mul;
        assert_eq!(binop_result_type(&Mul, &Type::Mat3, &Type::Mat3), Some(Type::Mat3));
        assert_eq!(binop_result_type(&Mul, &Type::Mat3, &Type::Vec3), Some(Type::Vec3));
        assert_eq!(binop_result_type(&Mul, &Type::Mat3, &Type::Float), Some(Type::Mat3));
        assert_eq!(binop_result_type(&Mul, &Type::Float, &Type::Mat3), Some(Type::Mat3));
    }

    #[test]
    fn binop_mat4_ops() {
        use BinOp::Mul;
        assert_eq!(binop_result_type(&Mul, &Type::Mat4, &Type::Mat4), Some(Type::Mat4));
        assert_eq!(binop_result_type(&Mul, &Type::Mat4, &Type::Vec4), Some(Type::Vec4));
        assert_eq!(binop_result_type(&Mul, &Type::Mat4, &Type::Float), Some(Type::Mat4));
        assert_eq!(binop_result_type(&Mul, &Type::Float, &Type::Mat4), Some(Type::Mat4));
    }

    #[test]
    fn binop_string_ops() {
        use BinOp::{Add, Lt, LtEq, Gt, GtEq};
        assert_eq!(binop_result_type(&Add, &Type::String, &Type::String), Some(Type::String));
        for op in [Lt, LtEq, Gt, GtEq] {
            assert_eq!(binop_result_type(&op, &Type::String, &Type::String), Some(Type::Bool));
        }
    }

    #[test]
    fn binop_invalid_combinations() {
        use BinOp::Add;
        assert_eq!(binop_result_type(&Add, &Type::Float, &Type::String), None);
        assert_eq!(binop_result_type(&Add, &Type::Vec2, &Type::Vec3), None);
        assert_eq!(binop_result_type(&Add, &Type::Mat3, &Type::Mat3), None);
    }
}
