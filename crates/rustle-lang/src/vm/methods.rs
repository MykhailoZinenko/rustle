#![expect(clippy::cast_possible_truncation, reason = "VM indices are guaranteed small by construction")]
#![expect(clippy::cast_sign_loss, reason = "VM indices are non-negative by construction")]
#![expect(clippy::cast_precision_loss, reason = "list lengths fit in f64 mantissa for practical sizes")]
//! Native method dispatch for all built-in heap types.
//!
//! This module implements non-HOF methods for String, Vec2/3/4, Color, Mat3/4,
//! Transform, Shape, List, and State.  Higher-order list functions (map, filter,
//! sort, etc.) are handled by `list_ops.rs` because they need the VM's
//! closure-calling mechanism.

use crate::{
    error::{ErrorCode, RuntimeError},
    types::{
        draw::TransformData,
        mat::{
            m3_det, m3_inverse, m3_mul_vec, m3_scale, m3_transpose,
            m4_det, m4_inverse, m4_mul_vec, m4_scale, m4_transpose,
        },
    },
};

use super::{
    heap::Heap,
    util::{check_arity, require_float, require_heap_ref},
    value::{deep_clone, HeapObject, StackValue},
};

// ─── Public sentinel ──────────────────────────────────────────────────────────

/// Returns `true` when `method` is a list higher-order function that requires
/// the VM's closure-calling loop.  The caller should route these to `list_ops`.
#[must_use]
pub fn is_list_hof(method: &str) -> bool {
    matches!(
        method,
        "map" | "filter" | "any" | "all" | "sort"
            | "search" | "bsearch" | "take" | "drop" | "cut" | "paste"
    )
}

// ─── Main dispatch ────────────────────────────────────────────────────────────

/// Call a built-in method on the heap object at `receiver_idx`.
///
/// `state_fields` provides the ordered field names for `State.keys()` /
/// `State.len()`.  Pass an empty slice if the receiver is not a `State`.
///
/// HOF list methods are **not** handled here; callers should check
/// [`is_list_hof`] first and route those to `list_ops`.
///
/// # Errors
/// Returns a [`RuntimeError`] on type errors, arity mismatches, or
/// operations that are mathematically undefined (e.g. normalising a zero
/// vector, inverting a singular matrix, popping an empty list).
pub fn call_method(
    heap: &mut Heap,
    receiver_idx: u32,
    method: &str,
    args: &[StackValue],
    state_fields: &[String],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    // Peek at the discriminant without holding a reference across allocations.
    // We clone the minimal data needed to decide which handler to call.
    match heap.get(receiver_idx) {
        HeapObject::Str(_)       => call_str_method(heap, receiver_idx, method, args, line),
        HeapObject::Vec2(_, _)   => call_vec2_method(heap, receiver_idx, method, args, line),
        HeapObject::Vec3(_, _, _)=> call_vec3_method(heap, receiver_idx, method, args, line),
        HeapObject::Vec4(_, _, _, _) => call_vec4_method(heap, receiver_idx, method, args, line),
        HeapObject::Color { .. } => call_color_method(heap, receiver_idx, method, args, line),
        HeapObject::Mat3(_)      => call_mat3_method(heap, receiver_idx, method, args, line),
        HeapObject::Mat4(_)      => call_mat4_method(heap, receiver_idx, method, args, line),
        HeapObject::Transform(_) => call_transform_method(heap, receiver_idx, method, args, line),
        HeapObject::Shape(_)     => call_shape_method(heap, receiver_idx, method, args, line),
        HeapObject::List(_)      => call_list_method(heap, receiver_idx, method, args, line),
        HeapObject::State(_)     => call_state_method(heap, receiver_idx, method, args, state_fields, line),
        _ => Err(method_not_found(method, "value", line)),
    }
}

fn method_not_found(method: &str, ty: &str, line: usize) -> RuntimeError {
    RuntimeError::new(
        ErrorCode::R004,
        line,
        0,
        format!("method `{method}` not found on {ty}"),
    )
}

// ─── String methods ───────────────────────────────────────────────────────────

fn call_str_method(
    heap: &mut Heap,
    receiver_idx: u32,
    method: &str,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    // Clone the string out so the borrow on `heap` ends before any allocation.
    let s = match heap.get(receiver_idx) {
        HeapObject::Str(s) => s.clone(),
        _ => unreachable!(),
    };

    match method {
        "len" => {
            check_arity(method, args, 0, line)?;
            Ok(StackValue::Float(s.chars().count() as f64))
        }
        "contains" => {
            check_arity(method, args, 1, line)?;
            let needle = get_str_arg(heap, args[0], method, line)?;
            Ok(StackValue::Bool(s.contains(needle.as_str())))
        }
        "starts_with" => {
            check_arity(method, args, 1, line)?;
            let prefix = get_str_arg(heap, args[0], method, line)?;
            Ok(StackValue::Bool(s.starts_with(prefix.as_str())))
        }
        "ends_with" => {
            check_arity(method, args, 1, line)?;
            let suffix = get_str_arg(heap, args[0], method, line)?;
            Ok(StackValue::Bool(s.ends_with(suffix.as_str())))
        }
        "trim" => {
            check_arity(method, args, 0, line)?;
            let result = s.trim().to_string();
            Ok(heap.alloc(HeapObject::Str(result)))
        }
        "to_upper" => {
            check_arity(method, args, 0, line)?;
            Ok(heap.alloc(HeapObject::Str(s.to_uppercase())))
        }
        "to_lower" => {
            check_arity(method, args, 0, line)?;
            Ok(heap.alloc(HeapObject::Str(s.to_lowercase())))
        }
        "replace" => {
            check_arity(method, args, 2, line)?;
            let from = get_str_arg(heap, args[0], method, line)?;
            let to   = get_str_arg(heap, args[1], method, line)?;
            let result = s.replace(from.as_str(), to.as_str());
            Ok(heap.alloc(HeapObject::Str(result)))
        }
        "split" => {
            check_arity(method, args, 1, line)?;
            let delim = get_str_arg(heap, args[0], method, line)?;
            // Borrow is released; now we can allocate freely.
            let parts: Vec<String> = s.split(delim.as_str()).map(str::to_string).collect();
            let refs: Vec<StackValue> = parts
                .into_iter()
                .map(|p| heap.alloc(HeapObject::Str(p)))
                .collect();
            Ok(heap.alloc(HeapObject::List(refs)))
        }
        "slice" => {
            check_arity(method, args, 2, line)?;
            let start_f = require_float(args[0], line)?;
            let end_f   = require_float(args[1], line)?;
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len();
            let start = (start_f as usize).min(len);
            let end   = (end_f   as usize).min(len).max(start);
            let result: String = chars[start..end].iter().collect();
            Ok(heap.alloc(HeapObject::Str(result)))
        }
        _ => Err(method_not_found(method, "string", line)),
    }
}

/// Resolve a string argument: it must be a `HeapRef` pointing to a Str.
fn get_str_arg(heap: &Heap, sv: StackValue, method: &str, line: usize) -> Result<String, RuntimeError> {
    let idx = require_heap_ref(sv, line)?;
    match heap.get(idx) {
        HeapObject::Str(s) => Ok(s.clone()),
        _ => Err(RuntimeError::new(
            ErrorCode::R001,
            line,
            0,
            format!("`{method}` argument must be a string"),
        )),
    }
}

// ─── Vec2 methods ─────────────────────────────────────────────────────────────

fn call_vec2_method(
    heap: &mut Heap,
    receiver_idx: u32,
    method: &str,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    let (x, y) = match heap.get(receiver_idx) {
        HeapObject::Vec2(x, y) => (*x, *y),
        _ => unreachable!(),
    };

    match method {
        "length" => {
            check_arity(method, args, 0, line)?;
            Ok(StackValue::Float((x * x + y * y).sqrt()))
        }
        "normalize" => {
            check_arity(method, args, 0, line)?;
            let len = (x * x + y * y).sqrt();
            if len == 0.0 {
                return Err(RuntimeError::new(ErrorCode::R011, line, 0, "normalize on zero vector"));
            }
            Ok(heap.alloc(HeapObject::Vec2(x / len, y / len)))
        }
        "dot" => {
            check_arity(method, args, 1, line)?;
            let (bx, by) = get_vec2_arg(heap, args[0], method, line)?;
            Ok(StackValue::Float(x * bx + y * by))
        }
        "lerp" => {
            check_arity(method, args, 2, line)?;
            let (bx, by) = get_vec2_arg(heap, args[0], method, line)?;
            let t = require_float(args[1], line)?;
            Ok(heap.alloc(HeapObject::Vec2(x + (bx - x) * t, y + (by - y) * t)))
        }
        "distance" => {
            check_arity(method, args, 1, line)?;
            let (bx, by) = get_vec2_arg(heap, args[0], method, line)?;
            let dx = x - bx;
            let dy = y - by;
            Ok(StackValue::Float((dx * dx + dy * dy).sqrt()))
        }
        "abs" => {
            check_arity(method, args, 0, line)?;
            Ok(heap.alloc(HeapObject::Vec2(x.abs(), y.abs())))
        }
        "floor" => {
            check_arity(method, args, 0, line)?;
            Ok(heap.alloc(HeapObject::Vec2(x.floor(), y.floor())))
        }
        "ceil" => {
            check_arity(method, args, 0, line)?;
            Ok(heap.alloc(HeapObject::Vec2(x.ceil(), y.ceil())))
        }
        "min" => {
            check_arity(method, args, 1, line)?;
            let (bx, by) = get_vec2_arg(heap, args[0], method, line)?;
            Ok(heap.alloc(HeapObject::Vec2(x.min(bx), y.min(by))))
        }
        "max" => {
            check_arity(method, args, 1, line)?;
            let (bx, by) = get_vec2_arg(heap, args[0], method, line)?;
            Ok(heap.alloc(HeapObject::Vec2(x.max(bx), y.max(by))))
        }
        "perp" => {
            check_arity(method, args, 0, line)?;
            Ok(heap.alloc(HeapObject::Vec2(-y, x)))
        }
        "angle" => {
            check_arity(method, args, 0, line)?;
            Ok(StackValue::Float(y.atan2(x)))
        }
        _ => Err(method_not_found(method, "vec2", line)),
    }
}

fn get_vec2_arg(heap: &Heap, sv: StackValue, method: &str, line: usize) -> Result<(f64, f64), RuntimeError> {
    let idx = require_heap_ref(sv, line)?;
    match heap.get(idx) {
        HeapObject::Vec2(x, y) => Ok((*x, *y)),
        _ => Err(RuntimeError::new(
            ErrorCode::R001,
            line,
            0,
            format!("`{method}` argument must be a vec2"),
        )),
    }
}

// ─── Vec3 methods ─────────────────────────────────────────────────────────────

fn call_vec3_method(
    heap: &mut Heap,
    receiver_idx: u32,
    method: &str,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    let (x, y, z) = match heap.get(receiver_idx) {
        HeapObject::Vec3(x, y, z) => (*x, *y, *z),
        _ => unreachable!(),
    };

    match method {
        "length" => {
            check_arity(method, args, 0, line)?;
            Ok(StackValue::Float((x * x + y * y + z * z).sqrt()))
        }
        "normalize" => {
            check_arity(method, args, 0, line)?;
            let len = (x * x + y * y + z * z).sqrt();
            if len == 0.0 {
                return Err(RuntimeError::new(ErrorCode::R011, line, 0, "normalize on zero vector"));
            }
            Ok(heap.alloc(HeapObject::Vec3(x / len, y / len, z / len)))
        }
        "dot" => {
            check_arity(method, args, 1, line)?;
            let (bx, by, bz) = get_vec3_arg(heap, args[0], method, line)?;
            Ok(StackValue::Float(x * bx + y * by + z * bz))
        }
        "cross" => {
            check_arity(method, args, 1, line)?;
            let (bx, by, bz) = get_vec3_arg(heap, args[0], method, line)?;
            Ok(heap.alloc(HeapObject::Vec3(
                y * bz - z * by,
                z * bx - x * bz,
                x * by - y * bx,
            )))
        }
        "lerp" => {
            check_arity(method, args, 2, line)?;
            let (bx, by, bz) = get_vec3_arg(heap, args[0], method, line)?;
            let t = require_float(args[1], line)?;
            Ok(heap.alloc(HeapObject::Vec3(
                x + (bx - x) * t,
                y + (by - y) * t,
                z + (bz - z) * t,
            )))
        }
        "abs" => {
            check_arity(method, args, 0, line)?;
            Ok(heap.alloc(HeapObject::Vec3(x.abs(), y.abs(), z.abs())))
        }
        "min" => {
            check_arity(method, args, 1, line)?;
            let (bx, by, bz) = get_vec3_arg(heap, args[0], method, line)?;
            Ok(heap.alloc(HeapObject::Vec3(x.min(bx), y.min(by), z.min(bz))))
        }
        "max" => {
            check_arity(method, args, 1, line)?;
            let (bx, by, bz) = get_vec3_arg(heap, args[0], method, line)?;
            Ok(heap.alloc(HeapObject::Vec3(x.max(bx), y.max(by), z.max(bz))))
        }
        "reflect" => {
            check_arity(method, args, 1, line)?;
            let (nx, ny, nz) = get_vec3_arg(heap, args[0], method, line)?;
            let d2 = 2.0 * (x * nx + y * ny + z * nz);
            Ok(heap.alloc(HeapObject::Vec3(x - d2 * nx, y - d2 * ny, z - d2 * nz)))
        }
        _ => Err(method_not_found(method, "vec3", line)),
    }
}

fn get_vec3_arg(heap: &Heap, sv: StackValue, method: &str, line: usize) -> Result<(f64, f64, f64), RuntimeError> {
    let idx = require_heap_ref(sv, line)?;
    match heap.get(idx) {
        HeapObject::Vec3(x, y, z) => Ok((*x, *y, *z)),
        _ => Err(RuntimeError::new(
            ErrorCode::R001,
            line,
            0,
            format!("`{method}` argument must be a vec3"),
        )),
    }
}

// ─── Vec4 methods ─────────────────────────────────────────────────────────────

fn call_vec4_method(
    heap: &mut Heap,
    receiver_idx: u32,
    method: &str,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    let (x, y, z, w) = match heap.get(receiver_idx) {
        HeapObject::Vec4(x, y, z, w) => (*x, *y, *z, *w),
        _ => unreachable!(),
    };

    match method {
        "length" => {
            check_arity(method, args, 0, line)?;
            Ok(StackValue::Float((x * x + y * y + z * z + w * w).sqrt()))
        }
        "normalize" => {
            check_arity(method, args, 0, line)?;
            let len = (x * x + y * y + z * z + w * w).sqrt();
            if len == 0.0 {
                return Err(RuntimeError::new(ErrorCode::R011, line, 0, "normalize on zero vector"));
            }
            Ok(heap.alloc(HeapObject::Vec4(x / len, y / len, z / len, w / len)))
        }
        "dot" => {
            check_arity(method, args, 1, line)?;
            let (bx, by, bz, bw) = get_vec4_arg(heap, args[0], method, line)?;
            Ok(StackValue::Float(x * bx + y * by + z * bz + w * bw))
        }
        "lerp" => {
            check_arity(method, args, 2, line)?;
            let (bx, by, bz, bw) = get_vec4_arg(heap, args[0], method, line)?;
            let t = require_float(args[1], line)?;
            Ok(heap.alloc(HeapObject::Vec4(
                x + (bx - x) * t,
                y + (by - y) * t,
                z + (bz - z) * t,
                w + (bw - w) * t,
            )))
        }
        "abs" => {
            check_arity(method, args, 0, line)?;
            Ok(heap.alloc(HeapObject::Vec4(x.abs(), y.abs(), z.abs(), w.abs())))
        }
        "min" => {
            check_arity(method, args, 1, line)?;
            let (bx, by, bz, bw) = get_vec4_arg(heap, args[0], method, line)?;
            Ok(heap.alloc(HeapObject::Vec4(x.min(bx), y.min(by), z.min(bz), w.min(bw))))
        }
        "max" => {
            check_arity(method, args, 1, line)?;
            let (bx, by, bz, bw) = get_vec4_arg(heap, args[0], method, line)?;
            Ok(heap.alloc(HeapObject::Vec4(x.max(bx), y.max(by), z.max(bz), w.max(bw))))
        }
        _ => Err(method_not_found(method, "vec4", line)),
    }
}

fn get_vec4_arg(heap: &Heap, sv: StackValue, method: &str, line: usize) -> Result<(f64, f64, f64, f64), RuntimeError> {
    let idx = require_heap_ref(sv, line)?;
    match heap.get(idx) {
        HeapObject::Vec4(x, y, z, w) => Ok((*x, *y, *z, *w)),
        _ => Err(RuntimeError::new(
            ErrorCode::R001,
            line,
            0,
            format!("`{method}` argument must be a vec4"),
        )),
    }
}

// ─── Color methods ────────────────────────────────────────────────────────────

fn call_color_method(
    heap: &mut Heap,
    receiver_idx: u32,
    method: &str,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    let (r, g, b, a) = match heap.get(receiver_idx) {
        HeapObject::Color { r, g, b, a } => (*r, *g, *b, *a),
        _ => unreachable!(),
    };

    match method {
        "lerp" => {
            check_arity(method, args, 2, line)?;
            let (br, bg, bb, ba) = get_color_arg(heap, args[0], method, line)?;
            let t = require_float(args[1], line)?;
            Ok(heap.alloc(HeapObject::Color {
                r: r + (br - r) * t,
                g: g + (bg - g) * t,
                b: b + (bb - b) * t,
                a: a + (ba - a) * t,
            }))
        }
        "with_alpha" => {
            check_arity(method, args, 1, line)?;
            let new_a = require_float(args[0], line)?;
            Ok(heap.alloc(HeapObject::Color { r, g, b, a: new_a }))
        }
        "to_vec4" => {
            check_arity(method, args, 0, line)?;
            Ok(heap.alloc(HeapObject::Vec4(r, g, b, a)))
        }
        _ => Err(method_not_found(method, "color", line)),
    }
}

fn get_color_arg(heap: &Heap, sv: StackValue, method: &str, line: usize) -> Result<(f64, f64, f64, f64), RuntimeError> {
    let idx = require_heap_ref(sv, line)?;
    match heap.get(idx) {
        HeapObject::Color { r, g, b, a } => Ok((*r, *g, *b, *a)),
        _ => Err(RuntimeError::new(
            ErrorCode::R001,
            line,
            0,
            format!("`{method}` argument must be a color"),
        )),
    }
}

// ─── Mat3 methods ─────────────────────────────────────────────────────────────

fn call_mat3_method(
    heap: &mut Heap,
    receiver_idx: u32,
    method: &str,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    // Copy the 9 floats out — derefs the Box then copies the array.
    let m: [f64; 9] = match heap.get(receiver_idx) {
        HeapObject::Mat3(boxed) => **boxed,
        _ => unreachable!(),
    };

    match method {
        "transpose" => {
            check_arity(method, args, 0, line)?;
            Ok(heap.alloc(HeapObject::Mat3(Box::new(m3_transpose(&m)))))
        }
        "det" => {
            check_arity(method, args, 0, line)?;
            Ok(StackValue::Float(m3_det(&m)))
        }
        "inverse" => {
            check_arity(method, args, 0, line)?;
            let inv = m3_inverse(&m, line)?;
            Ok(heap.alloc(HeapObject::Mat3(Box::new(inv))))
        }
        "mul_vec" => {
            check_arity(method, args, 1, line)?;
            let (vx, vy, vz) = get_vec3_arg(heap, args[0], method, line)?;
            let (rx, ry, rz) = m3_mul_vec(&m, (vx, vy, vz));
            Ok(heap.alloc(HeapObject::Vec3(rx, ry, rz)))
        }
        "scale" => {
            check_arity(method, args, 1, line)?;
            let s = require_float(args[0], line)?;
            Ok(heap.alloc(HeapObject::Mat3(Box::new(m3_scale(&m, s)))))
        }
        _ => Err(method_not_found(method, "mat3", line)),
    }
}

// ─── Mat4 methods ─────────────────────────────────────────────────────────────

fn call_mat4_method(
    heap: &mut Heap,
    receiver_idx: u32,
    method: &str,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    let m: [f64; 16] = match heap.get(receiver_idx) {
        HeapObject::Mat4(boxed) => **boxed,
        _ => unreachable!(),
    };

    match method {
        "transpose" => {
            check_arity(method, args, 0, line)?;
            Ok(heap.alloc(HeapObject::Mat4(Box::new(m4_transpose(&m)))))
        }
        "det" => {
            check_arity(method, args, 0, line)?;
            Ok(StackValue::Float(m4_det(&m)))
        }
        "inverse" => {
            check_arity(method, args, 0, line)?;
            let inv = m4_inverse(&m, line)?;
            Ok(heap.alloc(HeapObject::Mat4(Box::new(inv))))
        }
        "mul_vec" => {
            check_arity(method, args, 1, line)?;
            let (vx, vy, vz, vw) = get_vec4_arg(heap, args[0], method, line)?;
            let (rx, ry, rz, rw) = m4_mul_vec(&m, (vx, vy, vz, vw));
            Ok(heap.alloc(HeapObject::Vec4(rx, ry, rz, rw)))
        }
        "scale" => {
            check_arity(method, args, 1, line)?;
            let s = require_float(args[0], line)?;
            Ok(heap.alloc(HeapObject::Mat4(Box::new(m4_scale(&m, s)))))
        }
        _ => Err(method_not_found(method, "mat4", line)),
    }
}

// ─── Transform methods ────────────────────────────────────────────────────────

fn call_transform_method(
    heap: &mut Heap,
    receiver_idx: u32,
    method: &str,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    let td: TransformData = match heap.get(receiver_idx) {
        HeapObject::Transform(t) => t.clone(),
        _ => unreachable!(),
    };

    match method {
        // "move" is a Rust keyword — matched as a string literal.
        "move" | "translate" => {
            check_arity(method, args, 2, line)?;
            let dx = require_float(args[0], line)?;
            let dy = require_float(args[1], line)?;
            let mut t = td;
            t.tx += dx;
            t.ty += dy;
            Ok(heap.alloc(HeapObject::Transform(t)))
        }
        "scale" => {
            check_arity(method, args, 1, line)?;
            let s = require_float(args[0], line)?;
            let mut t = td;
            t.sx *= s;
            t.sy *= s;
            Ok(heap.alloc(HeapObject::Transform(t)))
        }
        "rotate" => {
            check_arity(method, args, 1, line)?;
            let degrees = require_float(args[0], line)?;
            let mut t = td;
            t.angle += degrees.to_radians();
            Ok(heap.alloc(HeapObject::Transform(t)))
        }
        _ => Err(method_not_found(method, "transform", line)),
    }
}

// ─── Shape methods ────────────────────────────────────────────────────────────

fn call_shape_method(
    heap: &mut Heap,
    receiver_idx: u32,
    method: &str,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    match method {
        "in" => {
            check_arity(method, args, 2, line)?;
            // Extract the anchor before we allocate the result vec2.
            let anchor: (f64, f64) = match heap.get(receiver_idx) {
                HeapObject::Shape(sd) => sd.desc.anchor(),
                _ => unreachable!(),
            };
            let dx = require_float(args[0], line)?;
            let dy = require_float(args[1], line)?;
            Ok(heap.alloc(HeapObject::Vec2(anchor.0 + dx, anchor.1 + dy)))
        }
        _ => Err(method_not_found(method, "shape", line)),
    }
}

// ─── List non-HOF methods ─────────────────────────────────────────────────────

fn call_list_method(
    heap: &mut Heap,
    receiver_idx: u32,
    method: &str,
    args: &[StackValue],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    if is_list_hof(method) {
        // Should have been caught by the caller; return a clear error.
        return Err(RuntimeError::new(
            ErrorCode::R004,
            line,
            0,
            format!("`{method}` is a higher-order list method and must be called via the VM closure loop"),
        ));
    }

    match method {
        "len" => {
            check_arity(method, args, 0, line)?;
            let len = match heap.get(receiver_idx) {
                HeapObject::List(v) => v.len(),
                _ => unreachable!(),
            };
            Ok(StackValue::Float(len as f64))
        }
        "push" => {
            check_arity(method, args, 1, line)?;
            let val = if !Heap::is_frame_ref(receiver_idx) {
                heap.promote(args[0])
            } else {
                args[0]
            };
            match heap.get_mut(receiver_idx) {
                HeapObject::List(v) => v.push(val),
                _ => unreachable!(),
            }
            Ok(StackValue::HeapRef(receiver_idx))
        }
        "pop" => {
            check_arity(method, args, 0, line)?;
            match heap.get_mut(receiver_idx) {
                HeapObject::List(v) => v.pop().ok_or_else(|| {
                    RuntimeError::new(ErrorCode::R011, line, 0, "pop on empty list")
                }),
                _ => unreachable!(),
            }
        }
        "clone" => {
            check_arity(method, args, 0, line)?;
            Ok(deep_clone(StackValue::HeapRef(receiver_idx), heap))
        }
        _ => Err(method_not_found(method, "list", line)),
    }
}

// ─── State methods ────────────────────────────────────────────────────────────

fn call_state_method(
    heap: &mut Heap,
    receiver_idx: u32,
    method: &str,
    args: &[StackValue],
    state_fields: &[String],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    match method {
        "keys" => {
            check_arity(method, args, 0, line)?;
            // Sort alphabetically to match tree-walker semantics.
            let mut names: Vec<String> = state_fields.to_vec();
            names.sort_unstable();
            let refs: Vec<StackValue> = names
                .into_iter()
                .map(|n| heap.alloc(HeapObject::Str(n)))
                .collect();
            Ok(heap.alloc(HeapObject::List(refs)))
        }
        "len" => {
            check_arity(method, args, 0, line)?;
            // Use state_fields length (matches the field count stored in the State).
            let _ = receiver_idx; // not needed for len
            Ok(StackValue::Float(state_fields.len() as f64))
        }
        _ => Err(method_not_found(method, "state", line)),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        types::draw::{CoordMeta, RenderMode, ShapeData, ShapeDesc, TransformData},
        vm::{heap::Heap, value::HeapObject},
    };

    // ── helpers ──────────────────────────────────────────────────────────────

    fn alloc_str(heap: &mut Heap, s: &str) -> StackValue {
        heap.alloc(HeapObject::Str(s.into()))
    }

    fn alloc_vec2(heap: &mut Heap, x: f64, y: f64) -> StackValue {
        heap.alloc(HeapObject::Vec2(x, y))
    }

    fn alloc_vec3(heap: &mut Heap, x: f64, y: f64, z: f64) -> StackValue {
        heap.alloc(HeapObject::Vec3(x, y, z))
    }

    fn alloc_vec4(heap: &mut Heap, x: f64, y: f64, z: f64, w: f64) -> StackValue {
        heap.alloc(HeapObject::Vec4(x, y, z, w))
    }

    fn alloc_color(heap: &mut Heap, r: f64, g: f64, b: f64, a: f64) -> StackValue {
        heap.alloc(HeapObject::Color { r, g, b, a })
    }

    fn idx(sv: StackValue) -> u32 {
        sv.as_heap_ref().unwrap()
    }

    fn dispatch(heap: &mut Heap, receiver: StackValue, method: &str, args: &[StackValue]) -> Result<StackValue, RuntimeError> {
        call_method(heap, idx(receiver), method, args, &[], 1)
    }

    fn dispatch_state(heap: &mut Heap, receiver: StackValue, method: &str, args: &[StackValue], fields: &[String]) -> Result<StackValue, RuntimeError> {
        call_method(heap, idx(receiver), method, args, fields, 1)
    }

    // ── is_list_hof ──────────────────────────────────────────────────────────

    #[test]
    fn hof_names_are_detected() {
        for name in &["map", "filter", "any", "all", "sort", "search", "bsearch",
                      "take", "drop", "cut", "paste"] {
            assert!(is_list_hof(name), "{name} should be HOF");
        }
        for name in &["push", "pop", "len", "clone"] {
            assert!(!is_list_hof(name), "{name} should NOT be HOF");
        }
    }

    // ── String methods ────────────────────────────────────────────────────────

    #[test]
    fn str_len() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "hello");
        let r = dispatch(&mut h, s, "len", &[]).unwrap();
        assert!(matches!(r, StackValue::Float(f) if f == 5.0));
    }

    #[test]
    fn str_len_unicode() {
        let mut h = Heap::new();
        // "café" has 4 chars but 5 bytes
        let s = alloc_str(&mut h, "café");
        let r = dispatch(&mut h, s, "len", &[]).unwrap();
        assert!(matches!(r, StackValue::Float(f) if f == 4.0));
    }

    #[test]
    fn str_contains_true() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "hello world");
        let needle = alloc_str(&mut h, "world");
        let r = dispatch(&mut h, s, "contains", &[needle]).unwrap();
        assert!(matches!(r, StackValue::Bool(true)));
    }

    #[test]
    fn str_contains_false() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "hello world");
        let needle = alloc_str(&mut h, "xyz");
        let r = dispatch(&mut h, s, "contains", &[needle]).unwrap();
        assert!(matches!(r, StackValue::Bool(false)));
    }

    #[test]
    fn str_starts_with() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "hello");
        let prefix = alloc_str(&mut h, "hel");
        let r = dispatch(&mut h, s, "starts_with", &[prefix]).unwrap();
        assert!(matches!(r, StackValue::Bool(true)));
    }

    #[test]
    fn str_ends_with() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "hello");
        let suffix = alloc_str(&mut h, "llo");
        let r = dispatch(&mut h, s, "ends_with", &[suffix]).unwrap();
        assert!(matches!(r, StackValue::Bool(true)));
    }

    #[test]
    fn str_trim() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "  hi  ");
        let r = dispatch(&mut h, s, "trim", &[]).unwrap();
        let ri = idx(r);
        assert!(matches!(h.get(ri), HeapObject::Str(s) if s == "hi"));
    }

    #[test]
    fn str_to_upper() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "hello");
        let r = dispatch(&mut h, s, "to_upper", &[]).unwrap();
        let ri = idx(r);
        assert!(matches!(h.get(ri), HeapObject::Str(s) if s == "HELLO"));
    }

    #[test]
    fn str_to_lower() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "WORLD");
        let r = dispatch(&mut h, s, "to_lower", &[]).unwrap();
        let ri = idx(r);
        assert!(matches!(h.get(ri), HeapObject::Str(s) if s == "world"));
    }

    #[test]
    fn str_replace() {
        let mut h = Heap::new();
        let s    = alloc_str(&mut h, "aabbcc");
        let from = alloc_str(&mut h, "bb");
        let to   = alloc_str(&mut h, "XX");
        let r = dispatch(&mut h, s, "replace", &[from, to]).unwrap();
        let ri = idx(r);
        assert!(matches!(h.get(ri), HeapObject::Str(s) if s == "aaXXcc"));
    }

    #[test]
    fn str_split_returns_list() {
        let mut h = Heap::new();
        let s     = alloc_str(&mut h, "a,b,c");
        let delim = alloc_str(&mut h, ",");
        let r = dispatch(&mut h, s, "split", &[delim]).unwrap();
        let ri = idx(r);
        let items = match h.get(ri) {
            HeapObject::List(v) => v.clone(),
            _ => panic!("expected list"),
        };
        assert_eq!(items.len(), 3);
        // Check first element is "a"
        let first_idx = idx(items[0]);
        assert!(matches!(h.get(first_idx), HeapObject::Str(s) if s == "a"));
    }

    #[test]
    fn str_slice_basic() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "abcdef");
        let r = dispatch(&mut h, s, "slice", &[StackValue::Float(1.0), StackValue::Float(4.0)]).unwrap();
        let ri = idx(r);
        assert!(matches!(h.get(ri), HeapObject::Str(s) if s == "bcd"));
    }

    #[test]
    fn str_slice_clamps_end() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "abc");
        let r = dispatch(&mut h, s, "slice", &[StackValue::Float(0.0), StackValue::Float(100.0)]).unwrap();
        let ri = idx(r);
        assert!(matches!(h.get(ri), HeapObject::Str(s) if s == "abc"));
    }

    #[test]
    fn str_slice_start_clamped_to_end() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "abc");
        // start > len → empty
        let r = dispatch(&mut h, s, "slice", &[StackValue::Float(10.0), StackValue::Float(12.0)]).unwrap();
        let ri = idx(r);
        assert!(matches!(h.get(ri), HeapObject::Str(s) if s == ""));
    }

    #[test]
    fn str_wrong_arity() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "hello");
        let err = dispatch(&mut h, s, "len", &[StackValue::Float(1.0)]).unwrap_err();
        assert_eq!(err.code, ErrorCode::R008);
    }

    #[test]
    fn str_unknown_method() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "hello");
        let err = dispatch(&mut h, s, "nonexistent", &[]).unwrap_err();
        assert_eq!(err.code, ErrorCode::R004);
    }

    // ── Vec2 methods ──────────────────────────────────────────────────────────

    #[test]
    fn vec2_length() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 3.0, 4.0);
        let r = dispatch(&mut h, v, "length", &[]).unwrap();
        assert!(matches!(r, StackValue::Float(f) if (f - 5.0).abs() < 1e-10));
    }

    #[test]
    fn vec2_normalize() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 3.0, 4.0);
        let r = dispatch(&mut h, v, "normalize", &[]).unwrap();
        let ri = idx(r);
        match h.get(ri) {
            HeapObject::Vec2(x, y) => {
                assert!((x - 0.6).abs() < 1e-10);
                assert!((y - 0.8).abs() < 1e-10);
            }
            _ => panic!("expected vec2"),
        }
    }

    #[test]
    fn vec2_normalize_zero_vector_errors() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 0.0, 0.0);
        let err = dispatch(&mut h, v, "normalize", &[]).unwrap_err();
        assert_eq!(err.code, ErrorCode::R011);
    }

    #[test]
    fn vec2_dot() {
        let mut h = Heap::new();
        let a = alloc_vec2(&mut h, 1.0, 2.0);
        let b = alloc_vec2(&mut h, 3.0, 4.0);
        let r = dispatch(&mut h, a, "dot", &[b]).unwrap();
        // 1*3 + 2*4 = 11
        assert!(matches!(r, StackValue::Float(f) if (f - 11.0).abs() < 1e-10));
    }

    #[test]
    fn vec2_lerp() {
        let mut h = Heap::new();
        let a = alloc_vec2(&mut h, 0.0, 0.0);
        let b = alloc_vec2(&mut h, 10.0, 20.0);
        let r = dispatch(&mut h, a, "lerp", &[b, StackValue::Float(0.5)]).unwrap();
        let ri = idx(r);
        match h.get(ri) {
            HeapObject::Vec2(x, y) => {
                assert!((x - 5.0).abs() < 1e-10);
                assert!((y - 10.0).abs() < 1e-10);
            }
            _ => panic!("expected vec2"),
        }
    }

    #[test]
    fn vec2_perp() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 1.0, 2.0);
        let r = dispatch(&mut h, v, "perp", &[]).unwrap();
        let ri = idx(r);
        match h.get(ri) {
            HeapObject::Vec2(x, y) => {
                assert!((x - (-2.0)).abs() < 1e-10);
                assert!((y - 1.0).abs() < 1e-10);
            }
            _ => panic!("expected vec2"),
        }
    }

    #[test]
    fn vec2_angle() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 1.0, 0.0);
        let r = dispatch(&mut h, v, "angle", &[]).unwrap();
        assert!(matches!(r, StackValue::Float(f) if f.abs() < 1e-10));

        let v2 = alloc_vec2(&mut h, 0.0, 1.0);
        let r2 = dispatch(&mut h, v2, "angle", &[]).unwrap();
        let angle = r2.as_float().unwrap();
        assert!((angle - std::f64::consts::FRAC_PI_2).abs() < 1e-10);
    }

    #[test]
    fn vec2_distance() {
        let mut h = Heap::new();
        let a = alloc_vec2(&mut h, 0.0, 0.0);
        let b = alloc_vec2(&mut h, 3.0, 4.0);
        let r = dispatch(&mut h, a, "distance", &[b]).unwrap();
        assert!(matches!(r, StackValue::Float(f) if (f - 5.0).abs() < 1e-10));
    }

    #[test]
    fn vec2_abs_floor_ceil() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, -1.7, 2.3);
        let abs_r = dispatch(&mut h, v, "abs", &[]).unwrap();
        match h.get(idx(abs_r)) {
            HeapObject::Vec2(x, y) => {
                assert!((x - 1.7).abs() < 1e-10);
                assert!((y - 2.3).abs() < 1e-10);
            }
            _ => panic!(),
        }

        let floor_r = dispatch(&mut h, v, "floor", &[]).unwrap();
        match h.get(idx(floor_r)) {
            HeapObject::Vec2(x, y) => {
                assert_eq!(*x, -2.0);
                assert_eq!(*y, 2.0);
            }
            _ => panic!(),
        }

        let ceil_r = dispatch(&mut h, v, "ceil", &[]).unwrap();
        match h.get(idx(ceil_r)) {
            HeapObject::Vec2(x, y) => {
                assert_eq!(*x, -1.0);
                assert_eq!(*y, 3.0);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn vec2_min_max() {
        let mut h = Heap::new();
        let a = alloc_vec2(&mut h, 1.0, 5.0);
        let b = alloc_vec2(&mut h, 3.0, 2.0);
        let mn = dispatch(&mut h, a, "min", &[b]).unwrap();
        match h.get(idx(mn)) {
            HeapObject::Vec2(x, y) => { assert_eq!(*x, 1.0); assert_eq!(*y, 2.0); }
            _ => panic!(),
        }
        let mx = dispatch(&mut h, a, "max", &[b]).unwrap();
        match h.get(idx(mx)) {
            HeapObject::Vec2(x, y) => { assert_eq!(*x, 3.0); assert_eq!(*y, 5.0); }
            _ => panic!(),
        }
    }

    // ── Vec3 methods ──────────────────────────────────────────────────────────

    #[test]
    fn vec3_cross() {
        let mut h = Heap::new();
        let a = alloc_vec3(&mut h, 1.0, 0.0, 0.0);
        let b = alloc_vec3(&mut h, 0.0, 1.0, 0.0);
        let r = dispatch(&mut h, a, "cross", &[b]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Vec3(x, y, z) => {
                assert!(x.abs() < 1e-10);
                assert!(y.abs() < 1e-10);
                assert!((z - 1.0).abs() < 1e-10);
            }
            _ => panic!("expected vec3"),
        }
    }

    #[test]
    fn vec3_reflect() {
        let mut h = Heap::new();
        // v = (1, -1, 0), n = (0, 1, 0) → reflect = (1, 1, 0)
        let v = alloc_vec3(&mut h, 1.0, -1.0, 0.0);
        let n = alloc_vec3(&mut h, 0.0, 1.0, 0.0);
        let r = dispatch(&mut h, v, "reflect", &[n]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Vec3(x, y, z) => {
                assert!((x - 1.0).abs() < 1e-10);
                assert!((y - 1.0).abs() < 1e-10);
                assert!(z.abs() < 1e-10);
            }
            _ => panic!("expected vec3"),
        }
    }

    #[test]
    fn vec3_normalize_zero_errors() {
        let mut h = Heap::new();
        let v = alloc_vec3(&mut h, 0.0, 0.0, 0.0);
        let err = dispatch(&mut h, v, "normalize", &[]).unwrap_err();
        assert_eq!(err.code, ErrorCode::R011);
    }

    #[test]
    fn vec3_length_and_dot() {
        let mut h = Heap::new();
        let v = alloc_vec3(&mut h, 1.0, 2.0, 2.0);
        let len = dispatch(&mut h, v, "length", &[]).unwrap();
        assert!(matches!(len, StackValue::Float(f) if (f - 3.0).abs() < 1e-10));

        let b = alloc_vec3(&mut h, 1.0, 0.0, 0.0);
        let dot = dispatch(&mut h, v, "dot", &[b]).unwrap();
        assert!(matches!(dot, StackValue::Float(f) if (f - 1.0).abs() < 1e-10));
    }

    #[test]
    fn vec3_lerp_abs_min_max() {
        let mut h = Heap::new();
        let a = alloc_vec3(&mut h, 0.0, 0.0, 0.0);
        let b = alloc_vec3(&mut h, 10.0, 10.0, 10.0);
        let lr = dispatch(&mut h, a, "lerp", &[b, StackValue::Float(0.5)]).unwrap();
        match h.get(idx(lr)) {
            HeapObject::Vec3(x, y, z) => {
                assert!((x - 5.0).abs() < 1e-10);
                assert!((y - 5.0).abs() < 1e-10);
                assert!((z - 5.0).abs() < 1e-10);
            }
            _ => panic!(),
        }

        let neg = alloc_vec3(&mut h, -3.0, -2.0, -1.0);
        let ab = dispatch(&mut h, neg, "abs", &[]).unwrap();
        match h.get(idx(ab)) {
            HeapObject::Vec3(x, y, z) => {
                assert_eq!(*x, 3.0);
                assert_eq!(*y, 2.0);
                assert_eq!(*z, 1.0);
            }
            _ => panic!(),
        }
    }

    // ── Vec4 methods ──────────────────────────────────────────────────────────

    #[test]
    fn vec4_length() {
        let mut h = Heap::new();
        let v = alloc_vec4(&mut h, 1.0, 0.0, 0.0, 0.0);
        let r = dispatch(&mut h, v, "length", &[]).unwrap();
        assert!(matches!(r, StackValue::Float(f) if (f - 1.0).abs() < 1e-10));
    }

    #[test]
    fn vec4_normalize() {
        let mut h = Heap::new();
        let v = alloc_vec4(&mut h, 2.0, 0.0, 0.0, 0.0);
        let r = dispatch(&mut h, v, "normalize", &[]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Vec4(x, y, z, w) => {
                assert!((x - 1.0).abs() < 1e-10);
                assert!(y.abs() < 1e-10);
                assert!(z.abs() < 1e-10);
                assert!(w.abs() < 1e-10);
            }
            _ => panic!("expected vec4"),
        }
    }

    #[test]
    fn vec4_normalize_zero_errors() {
        let mut h = Heap::new();
        let v = alloc_vec4(&mut h, 0.0, 0.0, 0.0, 0.0);
        let err = dispatch(&mut h, v, "normalize", &[]).unwrap_err();
        assert_eq!(err.code, ErrorCode::R011);
    }

    #[test]
    fn vec4_dot_lerp_abs_min_max() {
        let mut h = Heap::new();
        let a = alloc_vec4(&mut h, 1.0, 2.0, 3.0, 4.0);
        let b = alloc_vec4(&mut h, 1.0, 1.0, 1.0, 1.0);
        let d = dispatch(&mut h, a, "dot", &[b]).unwrap();
        assert!(matches!(d, StackValue::Float(f) if (f - 10.0).abs() < 1e-10));

        let z = alloc_vec4(&mut h, 0.0, 0.0, 0.0, 0.0);
        let lr = dispatch(&mut h, z, "lerp", &[a, StackValue::Float(1.0)]).unwrap();
        match h.get(idx(lr)) {
            HeapObject::Vec4(x, y, z2, w) => {
                assert!((x - 1.0).abs() < 1e-10);
                assert!((y - 2.0).abs() < 1e-10);
                assert!((z2 - 3.0).abs() < 1e-10);
                assert!((w - 4.0).abs() < 1e-10);
            }
            _ => panic!(),
        }
    }

    // ── Color methods ─────────────────────────────────────────────────────────

    #[test]
    fn color_lerp() {
        let mut h = Heap::new();
        let a = alloc_color(&mut h, 0.0, 0.0, 0.0, 1.0);
        let b = alloc_color(&mut h, 1.0, 1.0, 1.0, 1.0);
        let r = dispatch(&mut h, a, "lerp", &[b, StackValue::Float(0.5)]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Color { r, g, b, a } => {
                assert!((r - 0.5).abs() < 1e-10);
                assert!((g - 0.5).abs() < 1e-10);
                assert!((b - 0.5).abs() < 1e-10);
                assert!((a - 1.0).abs() < 1e-10);
            }
            _ => panic!("expected color"),
        }
    }

    #[test]
    fn color_with_alpha() {
        let mut h = Heap::new();
        let c = alloc_color(&mut h, 1.0, 0.5, 0.0, 1.0);
        let r = dispatch(&mut h, c, "with_alpha", &[StackValue::Float(0.3)]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Color { r, g: _, b: _, a } => {
                assert!((r - 1.0).abs() < 1e-10);
                assert!((a - 0.3).abs() < 1e-10);
            }
            _ => panic!("expected color"),
        }
    }

    #[test]
    fn color_to_vec4() {
        let mut h = Heap::new();
        let c = alloc_color(&mut h, 0.1, 0.2, 0.3, 0.4);
        let r = dispatch(&mut h, c, "to_vec4", &[]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Vec4(x, y, z, w) => {
                assert!((x - 0.1).abs() < 1e-10);
                assert!((y - 0.2).abs() < 1e-10);
                assert!((z - 0.3).abs() < 1e-10);
                assert!((w - 0.4).abs() < 1e-10);
            }
            _ => panic!("expected vec4"),
        }
    }

    // ── Mat3 methods ──────────────────────────────────────────────────────────

    fn alloc_mat3(heap: &mut Heap, m: [f64; 9]) -> StackValue {
        heap.alloc(HeapObject::Mat3(Box::new(m)))
    }

    fn alloc_mat4(heap: &mut Heap, m: [f64; 16]) -> StackValue {
        heap.alloc(HeapObject::Mat4(Box::new(m)))
    }

    fn identity3() -> [f64; 9] {
        [1., 0., 0.,
         0., 1., 0.,
         0., 0., 1.]
    }

    fn identity4() -> [f64; 16] {
        [1., 0., 0., 0.,
         0., 1., 0., 0.,
         0., 0., 1., 0.,
         0., 0., 0., 1.]
    }

    #[test]
    fn mat3_transpose() {
        let mut h = Heap::new();
        let m = alloc_mat3(&mut h, [1., 2., 3., 4., 5., 6., 7., 8., 9.]);
        let r = dispatch(&mut h, m, "transpose", &[]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Mat3(b) => {
                // Row 0 of transpose = col 0 of original
                assert_eq!(b[0], 1.); assert_eq!(b[1], 4.); assert_eq!(b[2], 7.);
            }
            _ => panic!("expected mat3"),
        }
    }

    #[test]
    fn mat3_det_identity() {
        let mut h = Heap::new();
        let m = alloc_mat3(&mut h, identity3());
        let r = dispatch(&mut h, m, "det", &[]).unwrap();
        assert!(matches!(r, StackValue::Float(f) if (f - 1.0).abs() < 1e-10));
    }

    #[test]
    fn mat3_inverse_identity() {
        let mut h = Heap::new();
        let m = alloc_mat3(&mut h, identity3());
        let r = dispatch(&mut h, m, "inverse", &[]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Mat3(b) => {
                assert!((b[0] - 1.0).abs() < 1e-10);
                assert!(b[1].abs() < 1e-10);
            }
            _ => panic!("expected mat3"),
        }
    }

    #[test]
    fn mat3_inverse_singular_errors() {
        let mut h = Heap::new();
        // All-zero matrix → singular
        let m = alloc_mat3(&mut h, [0.; 9]);
        let err = dispatch(&mut h, m, "inverse", &[]).unwrap_err();
        assert_eq!(err.code, ErrorCode::R011);
    }

    #[test]
    fn mat3_mul_vec() {
        let mut h = Heap::new();
        let m = alloc_mat3(&mut h, identity3());
        let v = alloc_vec3(&mut h, 1.0, 2.0, 3.0);
        let r = dispatch(&mut h, m, "mul_vec", &[v]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Vec3(x, y, z) => {
                assert!((x - 1.0).abs() < 1e-10);
                assert!((y - 2.0).abs() < 1e-10);
                assert!((z - 3.0).abs() < 1e-10);
            }
            _ => panic!("expected vec3"),
        }
    }

    #[test]
    fn mat3_scale() {
        let mut h = Heap::new();
        let m = alloc_mat3(&mut h, identity3());
        let r = dispatch(&mut h, m, "scale", &[StackValue::Float(2.0)]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Mat3(b) => {
                assert!((b[0] - 2.0).abs() < 1e-10);
            }
            _ => panic!("expected mat3"),
        }
    }

    // ── Mat4 methods ──────────────────────────────────────────────────────────

    #[test]
    fn mat4_transpose() {
        let mut h = Heap::new();
        let mut raw = [0.0f64; 16];
        for i in 0..16 { raw[i] = i as f64; }
        let m = alloc_mat4(&mut h, raw);
        let r = dispatch(&mut h, m, "transpose", &[]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Mat4(b) => {
                // Element [0][1] (row 0, col 1) = original [1][0] (index 4)
                assert!((b[1] - 4.0).abs() < 1e-10);
            }
            _ => panic!("expected mat4"),
        }
    }

    #[test]
    fn mat4_inverse_identity() {
        let mut h = Heap::new();
        let m = alloc_mat4(&mut h, identity4());
        let r = dispatch(&mut h, m, "inverse", &[]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Mat4(b) => {
                assert!((b[0] - 1.0).abs() < 1e-10);
                assert!(b[1].abs() < 1e-10);
            }
            _ => panic!("expected mat4"),
        }
    }

    #[test]
    fn mat4_inverse_singular_errors() {
        let mut h = Heap::new();
        let m = alloc_mat4(&mut h, [0.; 16]);
        let err = dispatch(&mut h, m, "inverse", &[]).unwrap_err();
        assert_eq!(err.code, ErrorCode::R011);
    }

    #[test]
    fn mat4_mul_vec() {
        let mut h = Heap::new();
        let m = alloc_mat4(&mut h, identity4());
        let v = alloc_vec4(&mut h, 1.0, 2.0, 3.0, 4.0);
        let r = dispatch(&mut h, m, "mul_vec", &[v]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Vec4(x, y, z, w) => {
                assert!((x - 1.0).abs() < 1e-10);
                assert!((y - 2.0).abs() < 1e-10);
                assert!((z - 3.0).abs() < 1e-10);
                assert!((w - 4.0).abs() < 1e-10);
            }
            _ => panic!("expected vec4"),
        }
    }

    // ── Transform methods ─────────────────────────────────────────────────────

    fn alloc_transform(heap: &mut Heap) -> StackValue {
        heap.alloc(HeapObject::Transform(TransformData::default()))
    }

    #[test]
    fn transform_move() {
        let mut h = Heap::new();
        let t = alloc_transform(&mut h);
        let r = dispatch(&mut h, t, "move", &[StackValue::Float(3.0), StackValue::Float(4.0)]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Transform(td) => {
                assert!((td.tx - 3.0).abs() < 1e-10);
                assert!((td.ty - 4.0).abs() < 1e-10);
            }
            _ => panic!("expected transform"),
        }
    }

    #[test]
    fn transform_translate_same_as_move() {
        let mut h = Heap::new();
        let t = alloc_transform(&mut h);
        let r = dispatch(&mut h, t, "translate", &[StackValue::Float(5.0), StackValue::Float(6.0)]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Transform(td) => {
                assert!((td.tx - 5.0).abs() < 1e-10);
                assert!((td.ty - 6.0).abs() < 1e-10);
            }
            _ => panic!("expected transform"),
        }
    }

    #[test]
    fn transform_scale() {
        let mut h = Heap::new();
        let t = alloc_transform(&mut h);
        let r = dispatch(&mut h, t, "scale", &[StackValue::Float(2.0)]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Transform(td) => {
                assert!((td.sx - 2.0).abs() < 1e-10);
                assert!((td.sy - 2.0).abs() < 1e-10);
            }
            _ => panic!("expected transform"),
        }
    }

    #[test]
    fn transform_rotate() {
        let mut h = Heap::new();
        let t = alloc_transform(&mut h);
        let r = dispatch(&mut h, t, "rotate", &[StackValue::Float(90.0)]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Transform(td) => {
                let expected = 90.0f64.to_radians();
                assert!((td.angle - expected).abs() < 1e-10);
            }
            _ => panic!("expected transform"),
        }
    }

    // ── Shape.in() ────────────────────────────────────────────────────────────

    fn make_shape(desc: ShapeDesc) -> HeapObject {
        HeapObject::Shape(ShapeData::new(desc, RenderMode::Sdf, CoordMeta::default()))
    }

    #[test]
    fn shape_in_circle() {
        let mut h = Heap::new();
        let s = h.alloc(make_shape(ShapeDesc::Circle { center: (5.0, 10.0), radius: 3.0 }));
        let r = dispatch(&mut h, s, "in", &[StackValue::Float(1.0), StackValue::Float(-2.0)]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Vec2(x, y) => {
                assert!((x - 6.0).abs() < 1e-10);
                assert!((y - 8.0).abs() < 1e-10);
            }
            _ => panic!("expected vec2"),
        }
    }

    #[test]
    fn shape_in_rect() {
        let mut h = Heap::new();
        let s = h.alloc(make_shape(ShapeDesc::Rect {
            center: (0.0, 0.0),
            size: (10.0, 10.0),
            origin: crate::types::draw::Origin::Center,
        }));
        let r = dispatch(&mut h, s, "in", &[StackValue::Float(3.0), StackValue::Float(3.0)]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Vec2(x, y) => {
                assert!((x - 3.0).abs() < 1e-10);
                assert!((y - 3.0).abs() < 1e-10);
            }
            _ => panic!("expected vec2"),
        }
    }

    #[test]
    fn shape_in_line() {
        let mut h = Heap::new();
        let s = h.alloc(make_shape(ShapeDesc::Line { from: (1.0, 2.0), to: (5.0, 6.0) }));
        let r = dispatch(&mut h, s, "in", &[StackValue::Float(0.5), StackValue::Float(0.5)]).unwrap();
        match h.get(idx(r)) {
            HeapObject::Vec2(x, y) => {
                assert!((x - 1.5).abs() < 1e-10);
                assert!((y - 2.5).abs() < 1e-10);
            }
            _ => panic!("expected vec2"),
        }
    }

    // ── List methods ──────────────────────────────────────────────────────────

    fn alloc_list(heap: &mut Heap, items: Vec<StackValue>) -> StackValue {
        heap.alloc(HeapObject::List(items))
    }

    #[test]
    fn list_len() {
        let mut h = Heap::new();
        let l = alloc_list(&mut h, vec![StackValue::Float(1.0), StackValue::Float(2.0)]);
        let r = dispatch(&mut h, l, "len", &[]).unwrap();
        assert!(matches!(r, StackValue::Float(f) if f == 2.0));
    }

    #[test]
    fn list_push_mutates_in_place() {
        let mut h = Heap::new();
        let l = alloc_list(&mut h, vec![]);
        let l_idx = idx(l);
        let pushed_ret = dispatch(&mut h, l, "push", &[StackValue::Float(42.0)]).unwrap();
        // Return value is the same list
        assert_eq!(pushed_ret.as_heap_ref().unwrap(), l_idx);
        // Length grew
        match h.get(l_idx) {
            HeapObject::List(v) => assert_eq!(v.len(), 1),
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn list_pop_returns_value_and_shrinks() {
        let mut h = Heap::new();
        let l = alloc_list(&mut h, vec![StackValue::Float(7.0), StackValue::Float(8.0)]);
        let popped = dispatch(&mut h, l, "pop", &[]).unwrap();
        assert!(matches!(popped, StackValue::Float(f) if f == 8.0));
        let li = idx(l);
        match h.get(li) {
            HeapObject::List(v) => assert_eq!(v.len(), 1),
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn list_pop_empty_errors() {
        let mut h = Heap::new();
        let l = alloc_list(&mut h, vec![]);
        let err = dispatch(&mut h, l, "pop", &[]).unwrap_err();
        assert_eq!(err.code, ErrorCode::R011);
    }

    #[test]
    fn list_clone_is_independent() {
        let mut h = Heap::new();
        let l = alloc_list(&mut h, vec![StackValue::Float(1.0), StackValue::Float(2.0)]);
        let l_clone = dispatch(&mut h, l, "clone", &[]).unwrap();
        let orig_idx  = idx(l);
        let clone_idx = idx(l_clone);
        assert_ne!(orig_idx, clone_idx);

        // Mutate original
        if let HeapObject::List(v) = h.get_mut(orig_idx) {
            v.push(StackValue::Float(99.0));
        }
        // Clone should still have length 2
        match h.get(clone_idx) {
            HeapObject::List(v) => assert_eq!(v.len(), 2),
            _ => panic!("expected list"),
        }
    }

    // ── State methods ─────────────────────────────────────────────────────────

    fn alloc_state(heap: &mut Heap, n_fields: usize) -> StackValue {
        heap.alloc(HeapObject::State(super::super::value::StateObj {
            fields: vec![StackValue::None; n_fields],
        }))
    }

    #[test]
    fn state_len() {
        let mut h = Heap::new();
        let fields = vec!["x".to_string(), "y".to_string(), "z".to_string()];
        let s = alloc_state(&mut h, 3);
        let r = dispatch_state(&mut h, s, "len", &[], &fields).unwrap();
        assert!(matches!(r, StackValue::Float(f) if f == 3.0));
    }

    #[test]
    fn state_keys_sorted() {
        let mut h = Heap::new();
        let fields = vec!["beta".to_string(), "alpha".to_string(), "gamma".to_string()];
        let s = alloc_state(&mut h, 3);
        let r = dispatch_state(&mut h, s, "keys", &[], &fields).unwrap();
        let ri = idx(r);
        let items = match h.get(ri) {
            HeapObject::List(v) => v.clone(),
            _ => panic!("expected list"),
        };
        assert_eq!(items.len(), 3);
        let names: Vec<String> = items.iter().map(|&sv| {
            match h.get(idx(sv)) {
                HeapObject::Str(s) => s.clone(),
                _ => panic!("expected str"),
            }
        }).collect();
        assert_eq!(names, vec!["alpha", "beta", "gamma"]);
    }

    // ── Arity errors ──────────────────────────────────────────────────────────

    #[test]
    fn arity_error_vec2_length() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 1.0, 2.0);
        let err = dispatch(&mut h, v, "length", &[StackValue::Float(99.0)]).unwrap_err();
        assert_eq!(err.code, ErrorCode::R008);
    }

    #[test]
    fn arity_error_vec3_cross() {
        let mut h = Heap::new();
        let v = alloc_vec3(&mut h, 1.0, 0.0, 0.0);
        let err = dispatch(&mut h, v, "cross", &[]).unwrap_err();
        assert_eq!(err.code, ErrorCode::R008);
    }

    // ── Type errors ───────────────────────────────────────────────────────────

    #[test]
    fn type_error_calling_vec2_method_on_string() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "hello");
        // "length" exists on vec2 but not on string (it's "len" for strings)
        let err = dispatch(&mut h, s, "length", &[]).unwrap_err();
        assert_eq!(err.code, ErrorCode::R004);
    }

    #[test]
    fn type_error_dot_arg_not_vec2() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 1.0, 2.0);
        // Pass a string as the dot argument — should be a vec2
        let bad_arg = alloc_str(&mut h, "oops");
        let err = dispatch(&mut h, v, "dot", &[bad_arg]).unwrap_err();
        assert_eq!(err.code, ErrorCode::R001);
    }
}
