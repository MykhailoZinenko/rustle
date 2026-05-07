//! Native operator dispatch for all StackValue/HeapObject type combinations.
//!
//! Each function takes operands as [`StackValue`], a mutable reference to the [`Heap`]
//! (needed for reading existing objects and allocating results), and the source line
//! number for error reporting.

use crate::error::{ErrorCode, RuntimeError};
use crate::types::mat::{m3_mul, m3_mul_vec, m3_scale, m4_mul, m4_mul_vec, m4_scale};
use crate::vm::heap::Heap;
use crate::vm::value::{HeapObject, StackValue};

// ─── Helper ──────────────────────────────────────────────────────────────────

/// Coerce a Float or Bool operand to f64 for arithmetic. Returns None for anything else.
fn as_arith_float(sv: StackValue) -> Option<f64> {
    match sv {
        StackValue::Float(f) => Some(f),
        StackValue::Bool(b) => Some(if b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// Build an R011 "operator not defined for these types" error.
fn type_err(op: &str, line: usize) -> RuntimeError {
    RuntimeError::new(
        ErrorCode::R011,
        line,
        0,
        format!("operator '{op}' not defined for these types"),
    )
}

/// Build an R007 division / modulo by zero error.
fn div_zero_err(kind: &str, line: usize) -> RuntimeError {
    RuntimeError::new(ErrorCode::R007, line, 0, kind)
}

// ─── Addition ────────────────────────────────────────────────────────────────

/// Execute the `+` operator.
///
/// Supported combinations:
/// - Float + Float → Float
/// - Bool/Float mixed → Float (coerce)
/// - Vec2 + Vec2 → Vec2
/// - Vec3 + Vec3 → Vec3
/// - Vec4 + Vec4 → Vec4
/// - Color + Color → Color (each channel clamped to 1.0)
/// - Str + Str → Str (concatenation)
pub fn exec_add(
    a: StackValue,
    b: StackValue,
    heap: &mut Heap,
    line: usize,
) -> Result<StackValue, RuntimeError> {
    // Hot path: both floats
    if let (StackValue::Float(fa), StackValue::Float(fb)) = (a, b) {
        return Ok(StackValue::Float(fa + fb));
    }

    // Bool/float coercion
    if let (Some(fa), Some(fb)) = (as_arith_float(a), as_arith_float(b)) {
        // Only reach here if at least one operand was Bool (Float+Float handled above)
        return Ok(StackValue::Float(fa + fb));
    }

    // HeapRef combinations
    if let (StackValue::HeapRef(ai), StackValue::HeapRef(bi)) = (a, b) {
        match (heap.get(ai), heap.get(bi)) {
            (HeapObject::Vec2(ax, ay), HeapObject::Vec2(bx, by)) => {
                let (rx, ry) = (ax + bx, ay + by);
                return Ok(heap.alloc(HeapObject::Vec2(rx, ry)));
            }
            (HeapObject::Vec3(ax, ay, az), HeapObject::Vec3(bx, by, bz)) => {
                let (rx, ry, rz) = (ax + bx, ay + by, az + bz);
                return Ok(heap.alloc(HeapObject::Vec3(rx, ry, rz)));
            }
            (HeapObject::Vec4(ax, ay, az, aw), HeapObject::Vec4(bx, by, bz, bw)) => {
                let (rx, ry, rz, rw) = (ax + bx, ay + by, az + bz, aw + bw);
                return Ok(heap.alloc(HeapObject::Vec4(rx, ry, rz, rw)));
            }
            (
                HeapObject::Color { r: ar, g: ag, b: ab, a: aa },
                HeapObject::Color { r: br, g: bg, b: bb, a: ba },
            ) => {
                let (rr, rg, rb, ra) = (
                    (ar + br).min(1.0),
                    (ag + bg).min(1.0),
                    (ab + bb).min(1.0),
                    (aa + ba).min(1.0),
                );
                return Ok(heap.alloc(HeapObject::Color { r: rr, g: rg, b: rb, a: ra }));
            }
            (HeapObject::Str(sa), HeapObject::Str(sb)) => {
                let result = format!("{sa}{sb}");
                return Ok(heap.alloc(HeapObject::Str(result)));
            }
            _ => {}
        }
    }

    Err(type_err("+", line))
}

// ─── Subtraction ─────────────────────────────────────────────────────────────

/// Execute the `-` binary operator.
///
/// Supported combinations:
/// - Float - Float → Float
/// - Bool/Float mixed → Float
/// - Vec2 - Vec2 → Vec2
/// - Vec3 - Vec3 → Vec3
/// - Vec4 - Vec4 → Vec4
pub fn exec_sub(
    a: StackValue,
    b: StackValue,
    heap: &mut Heap,
    line: usize,
) -> Result<StackValue, RuntimeError> {
    if let (StackValue::Float(fa), StackValue::Float(fb)) = (a, b) {
        return Ok(StackValue::Float(fa - fb));
    }

    if let (Some(fa), Some(fb)) = (as_arith_float(a), as_arith_float(b)) {
        return Ok(StackValue::Float(fa - fb));
    }

    if let (StackValue::HeapRef(ai), StackValue::HeapRef(bi)) = (a, b) {
        match (heap.get(ai), heap.get(bi)) {
            (HeapObject::Vec2(ax, ay), HeapObject::Vec2(bx, by)) => {
                let (rx, ry) = (ax - bx, ay - by);
                return Ok(heap.alloc(HeapObject::Vec2(rx, ry)));
            }
            (HeapObject::Vec3(ax, ay, az), HeapObject::Vec3(bx, by, bz)) => {
                let (rx, ry, rz) = (ax - bx, ay - by, az - bz);
                return Ok(heap.alloc(HeapObject::Vec3(rx, ry, rz)));
            }
            (HeapObject::Vec4(ax, ay, az, aw), HeapObject::Vec4(bx, by, bz, bw)) => {
                let (rx, ry, rz, rw) = (ax - bx, ay - by, az - bz, aw - bw);
                return Ok(heap.alloc(HeapObject::Vec4(rx, ry, rz, rw)));
            }
            _ => {}
        }
    }

    Err(type_err("-", line))
}

// ─── Multiplication ───────────────────────────────────────────────────────────

/// Execute the `*` operator.
///
/// Supported combinations:
/// - Float * Float → Float
/// - Bool/Float mixed → Float
/// - Vec2/3/4 * Float or Float * Vec2/3/4 → scaled vector
/// - Color * Float → Color (clamped)
/// - Mat3 * Mat3 → Mat3
/// - Mat3 * Vec3 → Vec3
/// - Mat3 * Float / Float * Mat3 → Mat3
/// - Mat4 * Mat4 → Mat4
/// - Mat4 * Vec4 → Vec4
/// - Mat4 * Float / Float * Mat4 → Mat4
pub fn exec_mul(
    a: StackValue,
    b: StackValue,
    heap: &mut Heap,
    line: usize,
) -> Result<StackValue, RuntimeError> {
    if let (StackValue::Float(fa), StackValue::Float(fb)) = (a, b) {
        return Ok(StackValue::Float(fa * fb));
    }

    if let (Some(fa), Some(fb)) = (as_arith_float(a), as_arith_float(b)) {
        return Ok(StackValue::Float(fa * fb));
    }

    // HeapRef * HeapRef
    if let (StackValue::HeapRef(ai), StackValue::HeapRef(bi)) = (a, b) {
        // Vec2 * Vec2 is explicitly NOT defined
        match (heap.get(ai), heap.get(bi)) {
            (HeapObject::Vec2(_, _), HeapObject::Vec2(_, _)) => {
                return Err(type_err("*", line));
            }
            (HeapObject::Mat3(ma), HeapObject::Mat3(mb)) => {
                let (a_arr, b_arr) = (**ma, **mb);
                let result = m3_mul(&a_arr, &b_arr);
                return Ok(heap.alloc(HeapObject::Mat3(Box::new(result))));
            }
            (HeapObject::Mat3(ma), HeapObject::Vec3(vx, vy, vz)) => {
                let a_arr = **ma;
                let (vx, vy, vz) = (*vx, *vy, *vz);
                let (rx, ry, rz) = m3_mul_vec(&a_arr, (vx, vy, vz));
                return Ok(heap.alloc(HeapObject::Vec3(rx, ry, rz)));
            }
            (HeapObject::Mat4(ma), HeapObject::Mat4(mb)) => {
                let (a_arr, b_arr) = (**ma, **mb);
                let result = m4_mul(&a_arr, &b_arr);
                return Ok(heap.alloc(HeapObject::Mat4(Box::new(result))));
            }
            (HeapObject::Mat4(ma), HeapObject::Vec4(vx, vy, vz, vw)) => {
                let a_arr = **ma;
                let (vx, vy, vz, vw) = (*vx, *vy, *vz, *vw);
                let (rx, ry, rz, rw) = m4_mul_vec(&a_arr, (vx, vy, vz, vw));
                return Ok(heap.alloc(HeapObject::Vec4(rx, ry, rz, rw)));
            }
            _ => return Err(type_err("*", line)),
        }
    }

    // HeapRef * Float or Float * HeapRef
    let (heap_idx, scalar, heap_is_lhs) = match (a, b) {
        (StackValue::HeapRef(idx), StackValue::Float(s)) => (idx, s, true),
        (StackValue::Float(s), StackValue::HeapRef(idx)) => (idx, s, false),
        _ => return Err(type_err("*", line)),
    };

    match heap.get(heap_idx) {
        HeapObject::Vec2(x, y) => {
            let (rx, ry) = (x * scalar, y * scalar);
            Ok(heap.alloc(HeapObject::Vec2(rx, ry)))
        }
        HeapObject::Vec3(x, y, z) => {
            let (rx, ry, rz) = (x * scalar, y * scalar, z * scalar);
            Ok(heap.alloc(HeapObject::Vec3(rx, ry, rz)))
        }
        HeapObject::Vec4(x, y, z, w) => {
            let (rx, ry, rz, rw) = (x * scalar, y * scalar, z * scalar, w * scalar);
            Ok(heap.alloc(HeapObject::Vec4(rx, ry, rz, rw)))
        }
        HeapObject::Color { r, g, b, a } => {
            if !heap_is_lhs {
                // Float * Color is not registered — only Color * Float
                return Err(type_err("*", line));
            }
            let s = scalar;
            let (rr, rg, rb, ra) = ((r * s).min(1.0), (g * s).min(1.0), (b * s).min(1.0), (a * s).min(1.0));
            Ok(heap.alloc(HeapObject::Color { r: rr, g: rg, b: rb, a: ra }))
        }
        HeapObject::Mat3(m) => {
            let arr = **m;
            let result = m3_scale(&arr, scalar);
            Ok(heap.alloc(HeapObject::Mat3(Box::new(result))))
        }
        HeapObject::Mat4(m) => {
            let arr = **m;
            let result = m4_scale(&arr, scalar);
            Ok(heap.alloc(HeapObject::Mat4(Box::new(result))))
        }
        _ => Err(type_err("*", line)),
    }
}

// ─── Division ─────────────────────────────────────────────────────────────────

/// Execute the `/` operator.
///
/// Supported combinations:
/// - Float / Float → Float (div-by-zero → R007)
/// - Bool/Float mixed → Float (div-by-zero check after coercion)
/// - Vec2 / Float → Vec2 (scalar divisor; div-by-zero check)
/// - Vec3 / Float → Vec3
/// - Vec4 / Float → Vec4
pub fn exec_div(
    a: StackValue,
    b: StackValue,
    heap: &mut Heap,
    line: usize,
) -> Result<StackValue, RuntimeError> {
    if let (StackValue::Float(fa), StackValue::Float(fb)) = (a, b) {
        if fb == 0.0 {
            return Err(div_zero_err("division by zero", line));
        }
        return Ok(StackValue::Float(fa / fb));
    }

    if let (Some(fa), Some(fb)) = (as_arith_float(a), as_arith_float(b)) {
        if fb == 0.0 {
            return Err(div_zero_err("division by zero", line));
        }
        return Ok(StackValue::Float(fa / fb));
    }

    // Vec / Float
    if let (StackValue::HeapRef(ai), StackValue::Float(s)) = (a, b) {
        if s == 0.0 {
            return Err(div_zero_err("division by zero", line));
        }
        match heap.get(ai) {
            HeapObject::Vec2(x, y) => {
                let (rx, ry) = (x / s, y / s);
                return Ok(heap.alloc(HeapObject::Vec2(rx, ry)));
            }
            HeapObject::Vec3(x, y, z) => {
                let (rx, ry, rz) = (x / s, y / s, z / s);
                return Ok(heap.alloc(HeapObject::Vec3(rx, ry, rz)));
            }
            HeapObject::Vec4(x, y, z, w) => {
                let (rx, ry, rz, rw) = (x / s, y / s, z / s, w / s);
                return Ok(heap.alloc(HeapObject::Vec4(rx, ry, rz, rw)));
            }
            _ => {}
        }
    }

    Err(type_err("/", line))
}

// ─── Modulo ──────────────────────────────────────────────────────────────────

/// Execute the `%` operator.
///
/// Supported combinations:
/// - Float % Float → Float (mod-by-zero → R007)
/// - Bool/Float mixed → Float (mod-by-zero check after coercion)
pub fn exec_mod(
    a: StackValue,
    b: StackValue,
    _heap: &mut Heap,
    line: usize,
) -> Result<StackValue, RuntimeError> {
    if let (StackValue::Float(fa), StackValue::Float(fb)) = (a, b) {
        if fb == 0.0 {
            return Err(div_zero_err("modulo by zero", line));
        }
        return Ok(StackValue::Float(fa % fb));
    }

    if let (Some(fa), Some(fb)) = (as_arith_float(a), as_arith_float(b)) {
        if fb == 0.0 {
            return Err(div_zero_err("modulo by zero", line));
        }
        return Ok(StackValue::Float(fa % fb));
    }

    Err(type_err("%", line))
}

// ─── Unary negation ───────────────────────────────────────────────────────────

/// Execute the unary `-` (negation) operator.
///
/// Supported types: Float, Vec2, Vec3, Vec4.
pub fn exec_neg(v: StackValue, heap: &mut Heap, line: usize) -> Result<StackValue, RuntimeError> {
    match v {
        StackValue::Float(f) => Ok(StackValue::Float(-f)),
        StackValue::HeapRef(idx) => match heap.get(idx) {
            HeapObject::Vec2(x, y) => {
                let (rx, ry) = (-x, -y);
                Ok(heap.alloc(HeapObject::Vec2(rx, ry)))
            }
            HeapObject::Vec3(x, y, z) => {
                let (rx, ry, rz) = (-x, -y, -z);
                Ok(heap.alloc(HeapObject::Vec3(rx, ry, rz)))
            }
            HeapObject::Vec4(x, y, z, w) => {
                let (rx, ry, rz, rw) = (-x, -y, -z, -w);
                Ok(heap.alloc(HeapObject::Vec4(rx, ry, rz, rw)))
            }
            _ => Err(RuntimeError::new(ErrorCode::R011, line, 0, "cannot negate")),
        },
        _ => Err(RuntimeError::new(ErrorCode::R011, line, 0, "cannot negate")),
    }
}

// ─── Comparisons ─────────────────────────────────────────────────────────────

/// Execute the `>` operator.
///
/// Supported: Float > Float, Str > Str (lexicographic).
pub fn exec_gt(
    a: StackValue,
    b: StackValue,
    heap: &Heap,
    line: usize,
) -> Result<StackValue, RuntimeError> {
    cmp_op(a, b, heap, line, ">", |ord| matches!(ord, std::cmp::Ordering::Greater))
}

/// Execute the `<` operator.
///
/// Supported: Float < Float, Str < Str (lexicographic).
pub fn exec_lt(
    a: StackValue,
    b: StackValue,
    heap: &Heap,
    line: usize,
) -> Result<StackValue, RuntimeError> {
    cmp_op(a, b, heap, line, "<", |ord| matches!(ord, std::cmp::Ordering::Less))
}

/// Execute the `>=` operator.
///
/// Supported: Float >= Float, Str >= Str (lexicographic).
pub fn exec_gte(
    a: StackValue,
    b: StackValue,
    heap: &Heap,
    line: usize,
) -> Result<StackValue, RuntimeError> {
    cmp_op(a, b, heap, line, ">=", |ord| {
        matches!(ord, std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
    })
}

/// Execute the `<=` operator.
///
/// Supported: Float <= Float, Str <= Str (lexicographic).
pub fn exec_lte(
    a: StackValue,
    b: StackValue,
    heap: &Heap,
    line: usize,
) -> Result<StackValue, RuntimeError> {
    cmp_op(a, b, heap, line, "<=", |ord| {
        matches!(ord, std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
    })
}

/// Shared implementation for all ordered comparison operators.
fn cmp_op(
    a: StackValue,
    b: StackValue,
    heap: &Heap,
    line: usize,
    op: &str,
    predicate: impl Fn(std::cmp::Ordering) -> bool,
) -> Result<StackValue, RuntimeError> {
    match (a, b) {
        (StackValue::Float(fa), StackValue::Float(fb)) => {
            // partial_cmp on f64 returns None for NaN; treat as "not ordered" → false
            let result = fa.partial_cmp(&fb).map_or(false, predicate);
            Ok(StackValue::Bool(result))
        }
        (StackValue::HeapRef(ai), StackValue::HeapRef(bi)) => {
            match (heap.get(ai), heap.get(bi)) {
                (HeapObject::Str(sa), HeapObject::Str(sb)) => {
                    let ord = sa.cmp(sb);
                    Ok(StackValue::Bool(predicate(ord)))
                }
                _ => Err(type_err(op, line)),
            }
        }
        _ => Err(type_err(op, line)),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::mat::{m3_identity, m4_identity};
    use crate::vm::heap::Heap;
    use crate::vm::value::HeapObject;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn float(f: f64) -> StackValue {
        StackValue::Float(f)
    }

    fn bool_val(b: bool) -> StackValue {
        StackValue::Bool(b)
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

    fn alloc_str(heap: &mut Heap, s: &str) -> StackValue {
        heap.alloc(HeapObject::Str(s.into()))
    }

    fn alloc_mat3(heap: &mut Heap, m: [f64; 9]) -> StackValue {
        heap.alloc(HeapObject::Mat3(Box::new(m)))
    }

    fn alloc_mat4(heap: &mut Heap, m: [f64; 16]) -> StackValue {
        heap.alloc(HeapObject::Mat4(Box::new(m)))
    }

    fn get_vec2(heap: &Heap, sv: StackValue) -> (f64, f64) {
        let idx = sv.as_heap_ref().unwrap();
        match heap.get(idx) {
            HeapObject::Vec2(x, y) => (*x, *y),
            other => panic!("expected Vec2, got {:?}", other),
        }
    }

    fn get_vec3(heap: &Heap, sv: StackValue) -> (f64, f64, f64) {
        let idx = sv.as_heap_ref().unwrap();
        match heap.get(idx) {
            HeapObject::Vec3(x, y, z) => (*x, *y, *z),
            other => panic!("expected Vec3, got {:?}", other),
        }
    }

    fn get_vec4(heap: &Heap, sv: StackValue) -> (f64, f64, f64, f64) {
        let idx = sv.as_heap_ref().unwrap();
        match heap.get(idx) {
            HeapObject::Vec4(x, y, z, w) => (*x, *y, *z, *w),
            other => panic!("expected Vec4, got {:?}", other),
        }
    }

    fn get_color(heap: &Heap, sv: StackValue) -> (f64, f64, f64, f64) {
        let idx = sv.as_heap_ref().unwrap();
        match heap.get(idx) {
            HeapObject::Color { r, g, b, a } => (*r, *g, *b, *a),
            other => panic!("expected Color, got {:?}", other),
        }
    }

    fn get_str(heap: &Heap, sv: StackValue) -> String {
        let idx = sv.as_heap_ref().unwrap();
        match heap.get(idx) {
            HeapObject::Str(s) => s.clone(),
            other => panic!("expected Str, got {:?}", other),
        }
    }

    fn get_mat3(heap: &Heap, sv: StackValue) -> [f64; 9] {
        let idx = sv.as_heap_ref().unwrap();
        match heap.get(idx) {
            HeapObject::Mat3(m) => **m,
            other => panic!("expected Mat3, got {:?}", other),
        }
    }

    fn get_mat4(heap: &Heap, sv: StackValue) -> [f64; 16] {
        let idx = sv.as_heap_ref().unwrap();
        match heap.get(idx) {
            HeapObject::Mat4(m) => **m,
            other => panic!("expected Mat4, got {:?}", other),
        }
    }

    fn expect_float(sv: StackValue) -> f64 {
        sv.as_float().unwrap_or_else(|| panic!("expected Float, got {:?}", sv))
    }

    fn expect_bool(sv: StackValue) -> bool {
        sv.as_bool().unwrap_or_else(|| panic!("expected Bool, got {:?}", sv))
    }

    fn is_r011(e: &RuntimeError) -> bool {
        e.code == ErrorCode::R011
    }

    fn is_r007(e: &RuntimeError) -> bool {
        e.code == ErrorCode::R007
    }

    // ── exec_add ─────────────────────────────────────────────────────────────

    #[test]
    fn add_float_float() {
        let mut h = Heap::new();
        let r = exec_add(float(1.5), float(2.5), &mut h, 0).unwrap();
        assert_eq!(expect_float(r), 4.0);
    }

    #[test]
    fn add_bool_float_true_plus_one() {
        let mut h = Heap::new();
        // true + 1.0 = 2.0
        let r = exec_add(bool_val(true), float(1.0), &mut h, 0).unwrap();
        assert_eq!(expect_float(r), 2.0);
    }

    #[test]
    fn add_float_bool() {
        let mut h = Heap::new();
        let r = exec_add(float(3.0), bool_val(false), &mut h, 0).unwrap();
        assert_eq!(expect_float(r), 3.0);
    }

    #[test]
    fn add_bool_bool_true_plus_true() {
        let mut h = Heap::new();
        let r = exec_add(bool_val(true), bool_val(true), &mut h, 0).unwrap();
        assert_eq!(expect_float(r), 2.0);
    }

    #[test]
    fn add_bool_bool_false_plus_true() {
        let mut h = Heap::new();
        let r = exec_add(bool_val(false), bool_val(true), &mut h, 0).unwrap();
        assert_eq!(expect_float(r), 1.0);
    }

    #[test]
    fn add_vec2_vec2() {
        let mut h = Heap::new();
        let a = alloc_vec2(&mut h, 1.0, 2.0);
        let b = alloc_vec2(&mut h, 3.0, 4.0);
        let r = exec_add(a, b, &mut h, 0).unwrap();
        assert_eq!(get_vec2(&h, r), (4.0, 6.0));
    }

    #[test]
    fn add_vec3_vec3() {
        let mut h = Heap::new();
        let a = alloc_vec3(&mut h, 1.0, 2.0, 3.0);
        let b = alloc_vec3(&mut h, 4.0, 5.0, 6.0);
        let r = exec_add(a, b, &mut h, 0).unwrap();
        assert_eq!(get_vec3(&h, r), (5.0, 7.0, 9.0));
    }

    #[test]
    fn add_vec4_vec4() {
        let mut h = Heap::new();
        let a = alloc_vec4(&mut h, 1.0, 0.0, 0.0, 0.5);
        let b = alloc_vec4(&mut h, 0.0, 1.0, 0.0, 0.5);
        let r = exec_add(a, b, &mut h, 0).unwrap();
        assert_eq!(get_vec4(&h, r), (1.0, 1.0, 0.0, 1.0));
    }

    #[test]
    fn add_color_color_no_saturation() {
        let mut h = Heap::new();
        let a = alloc_color(&mut h, 0.2, 0.3, 0.1, 0.5);
        let b = alloc_color(&mut h, 0.1, 0.2, 0.3, 0.4);
        let r = exec_add(a, b, &mut h, 0).unwrap();
        let (rr, rg, rb, ra) = get_color(&h, r);
        assert!((rr - 0.3).abs() < 1e-10);
        assert!((rg - 0.5).abs() < 1e-10);
        assert!((rb - 0.4).abs() < 1e-10);
        assert!((ra - 0.9).abs() < 1e-10);
    }

    #[test]
    fn add_color_color_saturates_at_one() {
        let mut h = Heap::new();
        let a = alloc_color(&mut h, 0.8, 0.0, 0.0, 0.0);
        let b = alloc_color(&mut h, 0.5, 0.0, 0.0, 0.0);
        let r = exec_add(a, b, &mut h, 0).unwrap();
        let (rr, ..) = get_color(&h, r);
        assert_eq!(rr, 1.0); // saturated
    }

    #[test]
    fn add_str_str_concatenation() {
        let mut h = Heap::new();
        let a = alloc_str(&mut h, "hello");
        let b = alloc_str(&mut h, " world");
        let r = exec_add(a, b, &mut h, 0).unwrap();
        assert_eq!(get_str(&h, r), "hello world");
    }

    #[test]
    fn add_float_vec2_is_error() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 1.0, 2.0);
        let e = exec_add(float(1.0), v, &mut h, 5).unwrap_err();
        assert!(is_r011(&e));
    }

    #[test]
    fn add_vec2_str_is_error() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 1.0, 2.0);
        let s = alloc_str(&mut h, "hi");
        let e = exec_add(v, s, &mut h, 3).unwrap_err();
        assert!(is_r011(&e));
    }

    // ── exec_sub ─────────────────────────────────────────────────────────────

    #[test]
    fn sub_float_float() {
        let mut h = Heap::new();
        let r = exec_sub(float(5.0), float(3.0), &mut h, 0).unwrap();
        assert_eq!(expect_float(r), 2.0);
    }

    #[test]
    fn sub_bool_coercion() {
        let mut h = Heap::new();
        // true - false = 1.0 - 0.0 = 1.0
        let r = exec_sub(bool_val(true), bool_val(false), &mut h, 0).unwrap();
        assert_eq!(expect_float(r), 1.0);
    }

    #[test]
    fn sub_float_bool() {
        let mut h = Heap::new();
        // 5.0 - true = 5.0 - 1.0 = 4.0
        let r = exec_sub(float(5.0), bool_val(true), &mut h, 0).unwrap();
        assert_eq!(expect_float(r), 4.0);
    }

    #[test]
    fn sub_vec2_vec2() {
        let mut h = Heap::new();
        let a = alloc_vec2(&mut h, 5.0, 3.0);
        let b = alloc_vec2(&mut h, 2.0, 1.0);
        let r = exec_sub(a, b, &mut h, 0).unwrap();
        assert_eq!(get_vec2(&h, r), (3.0, 2.0));
    }

    #[test]
    fn sub_vec3_vec3() {
        let mut h = Heap::new();
        let a = alloc_vec3(&mut h, 10.0, 8.0, 6.0);
        let b = alloc_vec3(&mut h, 1.0, 2.0, 3.0);
        let r = exec_sub(a, b, &mut h, 0).unwrap();
        assert_eq!(get_vec3(&h, r), (9.0, 6.0, 3.0));
    }

    #[test]
    fn sub_vec4_vec4() {
        let mut h = Heap::new();
        let a = alloc_vec4(&mut h, 4.0, 3.0, 2.0, 1.0);
        let b = alloc_vec4(&mut h, 1.0, 1.0, 1.0, 1.0);
        let r = exec_sub(a, b, &mut h, 0).unwrap();
        assert_eq!(get_vec4(&h, r), (3.0, 2.0, 1.0, 0.0));
    }

    #[test]
    fn sub_float_vec2_is_error() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 1.0, 2.0);
        let e = exec_sub(float(1.0), v, &mut h, 0).unwrap_err();
        assert!(is_r011(&e));
    }

    // ── exec_mul ─────────────────────────────────────────────────────────────

    #[test]
    fn mul_float_float() {
        let mut h = Heap::new();
        let r = exec_mul(float(3.0), float(4.0), &mut h, 0).unwrap();
        assert_eq!(expect_float(r), 12.0);
    }

    #[test]
    fn mul_bool_coercion() {
        let mut h = Heap::new();
        // true * 5.0 = 1.0 * 5.0 = 5.0
        let r = exec_mul(bool_val(true), float(5.0), &mut h, 0).unwrap();
        assert_eq!(expect_float(r), 5.0);
    }

    #[test]
    fn mul_vec2_float() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 2.0, 3.0);
        let r = exec_mul(v, float(4.0), &mut h, 0).unwrap();
        assert_eq!(get_vec2(&h, r), (8.0, 12.0));
    }

    #[test]
    fn mul_float_vec2() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 2.0, 3.0);
        let r = exec_mul(float(4.0), v, &mut h, 0).unwrap();
        assert_eq!(get_vec2(&h, r), (8.0, 12.0));
    }

    #[test]
    fn mul_vec3_float() {
        let mut h = Heap::new();
        let v = alloc_vec3(&mut h, 1.0, 2.0, 3.0);
        let r = exec_mul(v, float(2.0), &mut h, 0).unwrap();
        assert_eq!(get_vec3(&h, r), (2.0, 4.0, 6.0));
    }

    #[test]
    fn mul_float_vec3() {
        let mut h = Heap::new();
        let v = alloc_vec3(&mut h, 1.0, 2.0, 3.0);
        let r = exec_mul(float(2.0), v, &mut h, 0).unwrap();
        assert_eq!(get_vec3(&h, r), (2.0, 4.0, 6.0));
    }

    #[test]
    fn mul_vec4_float() {
        let mut h = Heap::new();
        let v = alloc_vec4(&mut h, 1.0, 2.0, 3.0, 4.0);
        let r = exec_mul(v, float(0.5), &mut h, 0).unwrap();
        assert_eq!(get_vec4(&h, r), (0.5, 1.0, 1.5, 2.0));
    }

    #[test]
    fn mul_float_vec4() {
        let mut h = Heap::new();
        let v = alloc_vec4(&mut h, 1.0, 2.0, 3.0, 4.0);
        let r = exec_mul(float(0.5), v, &mut h, 0).unwrap();
        assert_eq!(get_vec4(&h, r), (0.5, 1.0, 1.5, 2.0));
    }

    #[test]
    fn mul_color_float_with_saturation() {
        let mut h = Heap::new();
        let c = alloc_color(&mut h, 0.8, 0.5, 0.2, 1.0);
        let r = exec_mul(c, float(2.0), &mut h, 0).unwrap();
        let (rr, rg, rb, ra) = get_color(&h, r);
        assert_eq!(rr, 1.0); // clamped: 0.8 * 2.0 = 1.6 → 1.0
        assert_eq!(rg, 1.0); // 0.5 * 2.0 = 1.0 → 1.0
        assert!((rb - 0.4).abs() < 1e-10); // 0.2 * 2.0 = 0.4
        assert_eq!(ra, 1.0); // 1.0 * 2.0 = 2.0 → 1.0
    }

    #[test]
    fn mul_float_color_is_error() {
        // Float * Color is NOT registered — only Color * Float
        let mut h = Heap::new();
        let c = alloc_color(&mut h, 0.5, 0.5, 0.5, 1.0);
        let e = exec_mul(float(2.0), c, &mut h, 0).unwrap_err();
        assert!(is_r011(&e));
    }

    #[test]
    fn mul_mat3_mat3_identity() {
        let mut h = Heap::new();
        let id = m3_identity();
        let ma = alloc_mat3(&mut h, id);
        let mb = alloc_mat3(&mut h, id);
        let r = exec_mul(ma, mb, &mut h, 0).unwrap();
        let result = get_mat3(&h, r);
        for (i, (&a, &b)) in result.iter().zip(id.iter()).enumerate() {
            assert!((a - b).abs() < 1e-10, "mat3*mat3 identity mismatch at index {i}");
        }
    }

    #[test]
    fn mul_mat3_vec3_identity() {
        let mut h = Heap::new();
        let id = m3_identity();
        let ma = alloc_mat3(&mut h, id);
        let v = alloc_vec3(&mut h, 1.0, 2.0, 3.0);
        let r = exec_mul(ma, v, &mut h, 0).unwrap();
        assert_eq!(get_vec3(&h, r), (1.0, 2.0, 3.0));
    }

    #[test]
    fn mul_mat3_float() {
        let mut h = Heap::new();
        let id = m3_identity();
        let ma = alloc_mat3(&mut h, id);
        let r = exec_mul(ma, float(2.0), &mut h, 0).unwrap();
        let result = get_mat3(&h, r);
        // Identity * 2 should give 2 on diagonal, 0 elsewhere
        assert_eq!(result[0], 2.0);
        assert_eq!(result[1], 0.0);
        assert_eq!(result[4], 2.0);
    }

    #[test]
    fn mul_float_mat3() {
        let mut h = Heap::new();
        let id = m3_identity();
        let ma = alloc_mat3(&mut h, id);
        let r = exec_mul(float(3.0), ma, &mut h, 0).unwrap();
        let result = get_mat3(&h, r);
        assert_eq!(result[0], 3.0);
        assert_eq!(result[4], 3.0);
        assert_eq!(result[8], 3.0);
    }

    #[test]
    fn mul_mat4_mat4_identity() {
        let mut h = Heap::new();
        let id = m4_identity();
        let ma = alloc_mat4(&mut h, id);
        let mb = alloc_mat4(&mut h, id);
        let r = exec_mul(ma, mb, &mut h, 0).unwrap();
        let result = get_mat4(&h, r);
        for (i, (&a, &b)) in result.iter().zip(id.iter()).enumerate() {
            assert!((a - b).abs() < 1e-10, "mat4*mat4 identity mismatch at index {i}");
        }
    }

    #[test]
    fn mul_mat4_vec4_identity() {
        let mut h = Heap::new();
        let id = m4_identity();
        let ma = alloc_mat4(&mut h, id);
        let v = alloc_vec4(&mut h, 1.0, 2.0, 3.0, 4.0);
        let r = exec_mul(ma, v, &mut h, 0).unwrap();
        assert_eq!(get_vec4(&h, r), (1.0, 2.0, 3.0, 4.0));
    }

    #[test]
    fn mul_mat4_float() {
        let mut h = Heap::new();
        let id = m4_identity();
        let ma = alloc_mat4(&mut h, id);
        let r = exec_mul(ma, float(5.0), &mut h, 0).unwrap();
        let result = get_mat4(&h, r);
        assert_eq!(result[0], 5.0);
        assert_eq!(result[5], 5.0);
        assert_eq!(result[10], 5.0);
        assert_eq!(result[15], 5.0);
        assert_eq!(result[1], 0.0);
    }

    #[test]
    fn mul_float_mat4() {
        let mut h = Heap::new();
        let id = m4_identity();
        let ma = alloc_mat4(&mut h, id);
        let r = exec_mul(float(2.0), ma, &mut h, 0).unwrap();
        let result = get_mat4(&h, r);
        assert_eq!(result[0], 2.0);
        assert_eq!(result[5], 2.0);
    }

    #[test]
    fn mul_vec2_vec2_is_error() {
        let mut h = Heap::new();
        let a = alloc_vec2(&mut h, 1.0, 2.0);
        let b = alloc_vec2(&mut h, 3.0, 4.0);
        let e = exec_mul(a, b, &mut h, 0).unwrap_err();
        assert!(is_r011(&e));
    }

    // ── exec_div ─────────────────────────────────────────────────────────────

    #[test]
    fn div_float_float() {
        let mut h = Heap::new();
        let r = exec_div(float(10.0), float(4.0), &mut h, 0).unwrap();
        assert_eq!(expect_float(r), 2.5);
    }

    #[test]
    fn div_by_zero_is_r007() {
        let mut h = Heap::new();
        let e = exec_div(float(5.0), float(0.0), &mut h, 7).unwrap_err();
        assert!(is_r007(&e));
        assert_eq!(e.line, 7);
    }

    #[test]
    fn div_bool_coercion_by_zero_is_r007() {
        let mut h = Heap::new();
        let e = exec_div(bool_val(true), bool_val(false), &mut h, 2).unwrap_err();
        assert!(is_r007(&e));
    }

    #[test]
    fn div_vec2_float() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 6.0, 9.0);
        let r = exec_div(v, float(3.0), &mut h, 0).unwrap();
        assert_eq!(get_vec2(&h, r), (2.0, 3.0));
    }

    #[test]
    fn div_vec2_float_zero_is_r007() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 6.0, 9.0);
        let e = exec_div(v, float(0.0), &mut h, 1).unwrap_err();
        assert!(is_r007(&e));
    }

    #[test]
    fn div_vec3_float() {
        let mut h = Heap::new();
        let v = alloc_vec3(&mut h, 2.0, 4.0, 6.0);
        let r = exec_div(v, float(2.0), &mut h, 0).unwrap();
        assert_eq!(get_vec3(&h, r), (1.0, 2.0, 3.0));
    }

    #[test]
    fn div_vec4_float() {
        let mut h = Heap::new();
        let v = alloc_vec4(&mut h, 8.0, 4.0, 2.0, 1.0);
        let r = exec_div(v, float(2.0), &mut h, 0).unwrap();
        assert_eq!(get_vec4(&h, r), (4.0, 2.0, 1.0, 0.5));
    }

    #[test]
    fn div_float_vec2_is_error() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 2.0, 3.0);
        let e = exec_div(float(6.0), v, &mut h, 0).unwrap_err();
        assert!(is_r011(&e));
    }

    // ── exec_mod ─────────────────────────────────────────────────────────────

    #[test]
    fn mod_float_float() {
        let mut h = Heap::new();
        let r = exec_mod(float(10.0), float(3.0), &mut h, 0).unwrap();
        assert!((expect_float(r) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn mod_by_zero_is_r007() {
        let mut h = Heap::new();
        let e = exec_mod(float(5.0), float(0.0), &mut h, 4).unwrap_err();
        assert!(is_r007(&e));
        assert_eq!(e.line, 4);
    }

    #[test]
    fn mod_bool_coercion() {
        let mut h = Heap::new();
        // true % true = 1.0 % 1.0 = 0.0
        let r = exec_mod(bool_val(true), bool_val(true), &mut h, 0).unwrap();
        assert_eq!(expect_float(r), 0.0);
    }

    #[test]
    fn mod_bool_coercion_by_zero_is_r007() {
        let mut h = Heap::new();
        // true % false = 1.0 % 0.0 → error
        let e = exec_mod(bool_val(true), bool_val(false), &mut h, 0).unwrap_err();
        assert!(is_r007(&e));
    }

    #[test]
    fn mod_str_str_is_error() {
        let mut h = Heap::new();
        let a = alloc_str(&mut h, "hello");
        let b = alloc_str(&mut h, "world");
        let e = exec_mod(a, b, &mut h, 0).unwrap_err();
        assert!(is_r011(&e));
    }

    // ── exec_neg ─────────────────────────────────────────────────────────────

    #[test]
    fn neg_float() {
        let mut h = Heap::new();
        let r = exec_neg(float(3.14), &mut h, 0).unwrap();
        assert!((expect_float(r) - (-3.14)).abs() < 1e-10);
    }

    #[test]
    fn neg_vec2() {
        let mut h = Heap::new();
        let v = alloc_vec2(&mut h, 1.0, -2.0);
        let r = exec_neg(v, &mut h, 0).unwrap();
        assert_eq!(get_vec2(&h, r), (-1.0, 2.0));
    }

    #[test]
    fn neg_vec3() {
        let mut h = Heap::new();
        let v = alloc_vec3(&mut h, 1.0, 2.0, -3.0);
        let r = exec_neg(v, &mut h, 0).unwrap();
        assert_eq!(get_vec3(&h, r), (-1.0, -2.0, 3.0));
    }

    #[test]
    fn neg_vec4() {
        let mut h = Heap::new();
        let v = alloc_vec4(&mut h, 1.0, -2.0, 3.0, -4.0);
        let r = exec_neg(v, &mut h, 0).unwrap();
        assert_eq!(get_vec4(&h, r), (-1.0, 2.0, -3.0, 4.0));
    }

    #[test]
    fn neg_bool_is_error() {
        let mut h = Heap::new();
        let e = exec_neg(bool_val(true), &mut h, 0).unwrap_err();
        assert!(is_r011(&e));
    }

    #[test]
    fn neg_str_is_error() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "hi");
        let e = exec_neg(s, &mut h, 0).unwrap_err();
        assert!(is_r011(&e));
    }

    // ── exec_gt ──────────────────────────────────────────────────────────────

    #[test]
    fn gt_float_float_true() {
        let h = Heap::new();
        let r = exec_gt(float(5.0), float(3.0), &h, 0).unwrap();
        assert!(expect_bool(r));
    }

    #[test]
    fn gt_float_float_false() {
        let h = Heap::new();
        let r = exec_gt(float(3.0), float(5.0), &h, 0).unwrap();
        assert!(!expect_bool(r));
    }

    #[test]
    fn gt_float_float_equal_is_false() {
        let h = Heap::new();
        let r = exec_gt(float(3.0), float(3.0), &h, 0).unwrap();
        assert!(!expect_bool(r));
    }

    #[test]
    fn gt_str_str_lexicographic() {
        let mut h = Heap::new();
        let a = alloc_str(&mut h, "b");
        let b = alloc_str(&mut h, "a");
        let r = exec_gt(a, b, &h, 0).unwrap();
        assert!(expect_bool(r));
    }

    #[test]
    fn gt_str_str_abc_abd_false() {
        let mut h = Heap::new();
        let a = alloc_str(&mut h, "abc");
        let b = alloc_str(&mut h, "abd");
        let r = exec_gt(a, b, &h, 0).unwrap();
        assert!(!expect_bool(r));
    }

    #[test]
    fn gt_float_str_is_error() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "hi");
        let e = exec_gt(float(1.0), s, &h, 0).unwrap_err();
        assert!(is_r011(&e));
    }

    // ── exec_lt ──────────────────────────────────────────────────────────────

    #[test]
    fn lt_float_float_true() {
        let h = Heap::new();
        let r = exec_lt(float(2.0), float(5.0), &h, 0).unwrap();
        assert!(expect_bool(r));
    }

    #[test]
    fn lt_str_str_a_less_than_b() {
        let mut h = Heap::new();
        let a = alloc_str(&mut h, "a");
        let b = alloc_str(&mut h, "b");
        let r = exec_lt(a, b, &h, 0).unwrap();
        assert!(expect_bool(r));
    }

    #[test]
    fn lt_str_str_abc_less_than_abd() {
        let mut h = Heap::new();
        let a = alloc_str(&mut h, "abc");
        let b = alloc_str(&mut h, "abd");
        let r = exec_lt(a, b, &h, 0).unwrap();
        assert!(expect_bool(r));
    }

    #[test]
    fn lt_float_str_is_error() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "hi");
        let e = exec_lt(s, float(1.0), &h, 0).unwrap_err();
        assert!(is_r011(&e));
    }

    // ── exec_gte ─────────────────────────────────────────────────────────────

    #[test]
    fn gte_float_float_equal_is_true() {
        let h = Heap::new();
        let r = exec_gte(float(3.0), float(3.0), &h, 0).unwrap();
        assert!(expect_bool(r));
    }

    #[test]
    fn gte_float_float_greater_is_true() {
        let h = Heap::new();
        let r = exec_gte(float(4.0), float(3.0), &h, 0).unwrap();
        assert!(expect_bool(r));
    }

    #[test]
    fn gte_float_float_less_is_false() {
        let h = Heap::new();
        let r = exec_gte(float(2.0), float(3.0), &h, 0).unwrap();
        assert!(!expect_bool(r));
    }

    #[test]
    fn gte_str_str() {
        let mut h = Heap::new();
        let a = alloc_str(&mut h, "b");
        let b = alloc_str(&mut h, "b");
        let r = exec_gte(a, b, &h, 0).unwrap();
        assert!(expect_bool(r));
    }

    // ── exec_lte ─────────────────────────────────────────────────────────────

    #[test]
    fn lte_float_float_equal_is_true() {
        let h = Heap::new();
        let r = exec_lte(float(3.0), float(3.0), &h, 0).unwrap();
        assert!(expect_bool(r));
    }

    #[test]
    fn lte_float_float_less_is_true() {
        let h = Heap::new();
        let r = exec_lte(float(2.0), float(3.0), &h, 0).unwrap();
        assert!(expect_bool(r));
    }

    #[test]
    fn lte_float_float_greater_is_false() {
        let h = Heap::new();
        let r = exec_lte(float(4.0), float(3.0), &h, 0).unwrap();
        assert!(!expect_bool(r));
    }

    #[test]
    fn lte_str_str() {
        let mut h = Heap::new();
        let a = alloc_str(&mut h, "a");
        let b = alloc_str(&mut h, "b");
        let r = exec_lte(a, b, &h, 0).unwrap();
        assert!(expect_bool(r));
    }

    #[test]
    fn lte_str_str_greater_is_false() {
        let mut h = Heap::new();
        let a = alloc_str(&mut h, "b");
        let b = alloc_str(&mut h, "a");
        let r = exec_lte(a, b, &h, 0).unwrap();
        assert!(!expect_bool(r));
    }
}
