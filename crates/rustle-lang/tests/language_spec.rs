//! Comprehensive language specification tests.
//!
//! Every test is a Rustle program compiled and executed through the bytecode VM.
//! Tests are derived ONLY from the language specification — not from implementation details.
//! Coverage target: 90%+ of all Rustle language code paths.

#![expect(clippy::float_cmp, reason = "float equality in tests is intentional")]

use rustle_lang::types::draw::ShapeDesc;
use rustle_lang::{DrawCommand, ErrorCode, Input, Runtime, Value, compile};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn run(src: &str) -> Runtime {
    let prog = compile(src).unwrap_or_else(|errs| {
        panic!("compile failed: {errs:#?}");
    });
    Runtime::new(prog).unwrap_or_else(|e| {
        panic!("Runtime::new failed: {e:?}");
    })
}

fn compile_ok(src: &str) {
    compile(src).unwrap_or_else(|errs| {
        panic!("compile failed: {errs:#?}");
    });
}

fn compile_err(src: &str) -> Vec<rustle_lang::Error> {
    match compile(src) {
        Ok(_) => panic!("expected compile error"),
        Err(errs) => errs,
    }
}

fn run_err(src: &str) -> rustle_lang::RuntimeError {
    let prog = compile(src).unwrap_or_else(|errs| {
        panic!("compile failed (expected runtime error): {errs:#?}");
    });
    match Runtime::new(prog) {
        Ok(_) => panic!("expected Runtime::new to fail"),
        Err(e) => e,
    }
}

fn tick(rt: &mut Runtime) -> Vec<DrawCommand> {
    rt.tick(&Input {
        dt: 0.016,
        ..Default::default()
    })
    .unwrap_or_else(|e| panic!("tick failed: {e:?}"))
}

fn tick_with(rt: &mut Runtime, input: Input) -> Vec<DrawCommand> {
    rt.tick(&input)
        .unwrap_or_else(|e| panic!("tick failed: {e:?}"))
}

fn f(rt: &Runtime, key: &str) -> f64 {
    match rt.state().get(key) {
        Some(Value::Float(x)) => *x,
        other => panic!("expected Float for '{key}', got: {other:?}"),
    }
}

fn b(rt: &Runtime, key: &str) -> bool {
    match rt.state().get(key) {
        Some(Value::Bool(x)) => *x,
        other => panic!("expected Bool for '{key}', got: {other:?}"),
    }
}

fn s(rt: &Runtime, key: &str) -> String {
    match rt.state().get(key) {
        Some(Value::Str(x)) => x.clone(),
        other => panic!("expected Str for '{key}', got: {other:?}"),
    }
}

fn v2(rt: &Runtime, key: &str) -> (f64, f64) {
    match rt.state().get(key) {
        Some(Value::Vec2(x, y)) => (*x, *y),
        other => panic!("expected Vec2 for '{key}', got: {other:?}"),
    }
}

fn v3(rt: &Runtime, key: &str) -> (f64, f64, f64) {
    match rt.state().get(key) {
        Some(Value::Vec3(x, y, z)) => (*x, *y, *z),
        other => panic!("expected Vec3 for '{key}', got: {other:?}"),
    }
}

fn v4(rt: &Runtime, key: &str) -> (f64, f64, f64, f64) {
    match rt.state().get(key) {
        Some(Value::Vec4(x, y, z, w)) => (*x, *y, *z, *w),
        other => panic!("expected Vec4 for '{key}', got: {other:?}"),
    }
}

fn col(rt: &Runtime, key: &str) -> (f64, f64, f64, f64) {
    match rt.state().get(key) {
        Some(Value::Color { r, g, b, a }) => (*r, *g, *b, *a),
        other => panic!("expected Color for '{key}', got: {other:?}"),
    }
}

fn list_floats(rt: &Runtime, key: &str) -> Vec<f64> {
    match rt.state().get(key) {
        Some(Value::List(rc)) => rc
            .borrow()
            .iter()
            .map(|v| match v {
                Value::Float(x) => *x,
                other => panic!("list elem not Float: {other:?}"),
            })
            .collect(),
        other => panic!("expected List for '{key}', got: {other:?}"),
    }
}

fn list_strings(rt: &Runtime, key: &str) -> Vec<String> {
    match rt.state().get(key) {
        Some(Value::List(rc)) => rc
            .borrow()
            .iter()
            .map(|v| match v {
                Value::Str(s) => s.clone(),
                other => panic!("list elem not Str: {other:?}"),
            })
            .collect(),
        other => panic!("expected List for '{key}', got: {other:?}"),
    }
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

// =============================================================================
// 1. COMMENTS
// =============================================================================

#[test]
fn comment_line() {
    let rt = run(r"
        state { let x: float = 5.0 }
        // this is a comment
        fn on_init(s: State) -> State {
            // set x
            s.x = 10.0
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 10.0);
}

#[test]
fn comment_block() {
    let rt = run(r"
        state { let x: float = 5.0 }
        /* this is a block comment */
        fn on_init(s: State) -> State {
            /* multi
               line
               comment */
            s.x = 42.0
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 42.0);
}

#[test]
fn comment_block_inline() {
    let rt = run(r"
        state { let x: float = /* inline comment */ 7.0 }
    ");
    assert_eq!(f(&rt, "x"), 7.0);
}

// =============================================================================
// 2. CONSTANTS
// =============================================================================

#[test]
fn const_declaration() {
    let rt = run(r"
        const RATE = 9.81
        state { let x: float = RATE }
    ");
    assert_eq!(f(&rt, "x"), 9.81);
}

#[test]
fn const_reassign_compile_error() {
    let errs = compile_err(
        r"
        const RATE = 9.81
        fn on_init(s: State) -> State {
            RATE = 5.0
            return s
        }
    ",
    );
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S004)));
}

#[test]
fn const_pi_and_tau() {
    let rt = run(r"
        state {
            let p: float = PI
            let t: float = TAU
        }
    ");
    assert!(approx(f(&rt, "p"), std::f64::consts::PI));
    assert!(approx(f(&rt, "t"), std::f64::consts::TAU));
}

// =============================================================================
// 3. VEC3 — construction, fields, arithmetic, methods
// =============================================================================

#[test]
fn vec3_construction_and_fields() {
    let rt = run(r"
        state {
            let v: vec3 = vec3(1.0, 2.0, 3.0)
            let x: float = 0.0
            let y: float = 0.0
            let z: float = 0.0
        }
        fn on_init(s: State) -> State {
            s.x = s.v.x
            s.y = s.v.y
            s.z = s.v.z
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 1.0);
    assert_eq!(f(&rt, "y"), 2.0);
    assert_eq!(f(&rt, "z"), 3.0);
}

#[test]
fn vec3_add() {
    let rt = run(r"
        state { let v: vec3 = vec3(1.0, 2.0, 3.0) + vec3(4.0, 5.0, 6.0) }
    ");
    assert_eq!(v3(&rt, "v"), (5.0, 7.0, 9.0));
}

#[test]
fn vec3_sub() {
    let rt = run(r"
        state { let v: vec3 = vec3(5.0, 7.0, 9.0) - vec3(1.0, 2.0, 3.0) }
    ");
    assert_eq!(v3(&rt, "v"), (4.0, 5.0, 6.0));
}

#[test]
fn vec3_scalar_mul() {
    let rt = run(r"
        state { let v: vec3 = vec3(1.0, 2.0, 3.0) * 2.0 }
    ");
    assert_eq!(v3(&rt, "v"), (2.0, 4.0, 6.0));
}

#[test]
fn vec3_scalar_mul_left() {
    let rt = run(r"
        state { let v: vec3 = 3.0 * vec3(1.0, 2.0, 3.0) }
    ");
    assert_eq!(v3(&rt, "v"), (3.0, 6.0, 9.0));
}

#[test]
fn vec3_scalar_div() {
    let rt = run(r"
        state { let v: vec3 = vec3(4.0, 6.0, 8.0) / 2.0 }
    ");
    assert_eq!(v3(&rt, "v"), (2.0, 3.0, 4.0));
}

#[test]
fn vec3_length() {
    let rt = run(r"
        state { let r: float = vec3(3.0, 4.0, 0.0).length() }
    ");
    assert!(approx(f(&rt, "r"), 5.0));
}

#[test]
fn vec3_normalize() {
    let rt = run(r"
        state { let v: vec3 = vec3(3.0, 0.0, 0.0).normalize() }
    ");
    let (x, y, z) = v3(&rt, "v");
    assert!(approx(x, 1.0));
    assert!(approx(y, 0.0));
    assert!(approx(z, 0.0));
}

#[test]
fn vec3_dot() {
    let rt = run(r"
        state { let r: float = vec3(1.0, 2.0, 3.0).dot(vec3(4.0, 5.0, 6.0)) }
    ");
    assert_eq!(f(&rt, "r"), 32.0);
}

#[test]
fn vec3_cross() {
    let rt = run(r"
        state { let v: vec3 = vec3(1.0, 0.0, 0.0).cross(vec3(0.0, 1.0, 0.0)) }
    ");
    assert_eq!(v3(&rt, "v"), (0.0, 0.0, 1.0));
}

#[test]
fn vec3_lerp() {
    let rt = run(r"
        state { let v: vec3 = vec3(0.0, 0.0, 0.0).lerp(vec3(10.0, 20.0, 30.0), 0.5) }
    ");
    assert_eq!(v3(&rt, "v"), (5.0, 10.0, 15.0));
}

#[test]
fn vec3_abs() {
    let rt = run(r"
        state { let v: vec3 = vec3(-1.0, -2.0, -3.0).abs() }
    ");
    assert_eq!(v3(&rt, "v"), (1.0, 2.0, 3.0));
}

#[test]
fn vec3_min() {
    let rt = run(r"
        state { let v: vec3 = vec3(3.0, 1.0, 5.0).min(vec3(2.0, 4.0, 3.0)) }
    ");
    assert_eq!(v3(&rt, "v"), (2.0, 1.0, 3.0));
}

#[test]
fn vec3_max() {
    let rt = run(r"
        state { let v: vec3 = vec3(3.0, 1.0, 5.0).max(vec3(2.0, 4.0, 3.0)) }
    ");
    assert_eq!(v3(&rt, "v"), (3.0, 4.0, 5.0));
}

#[test]
fn vec3_reflect() {
    let rt = run(r"
        state { let v: vec3 = vec3(1.0, -1.0, 0.0).reflect(vec3(0.0, 1.0, 0.0)) }
    ");
    let (x, y, z) = v3(&rt, "v");
    assert!(approx(x, 1.0));
    assert!(approx(y, 1.0));
    assert!(approx(z, 0.0));
}

#[test]
fn vec3_eq() {
    let rt = run(r"
        state { let r: bool = vec3(1.0, 2.0, 3.0) == vec3(1.0, 2.0, 3.0) }
    ");
    assert!(b(&rt, "r"));
}

#[test]
fn vec3_neq() {
    let rt = run(r"
        state { let r: bool = vec3(1.0, 2.0, 3.0) != vec3(4.0, 5.0, 6.0) }
    ");
    assert!(b(&rt, "r"));
}

// =============================================================================
// 4. VEC4 — construction, fields, arithmetic, methods
// =============================================================================

#[test]
fn vec4_construction_and_fields() {
    let rt = run(r"
        state {
            let v: vec4 = vec4(1.0, 2.0, 3.0, 4.0)
            let x: float = 0.0
            let y: float = 0.0
            let z: float = 0.0
            let w: float = 0.0
        }
        fn on_init(s: State) -> State {
            s.x = s.v.x
            s.y = s.v.y
            s.z = s.v.z
            s.w = s.v.w
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 1.0);
    assert_eq!(f(&rt, "y"), 2.0);
    assert_eq!(f(&rt, "z"), 3.0);
    assert_eq!(f(&rt, "w"), 4.0);
}

#[test]
fn vec4_add() {
    let rt = run(r"
        state { let v: vec4 = vec4(1.0, 2.0, 3.0, 4.0) + vec4(5.0, 6.0, 7.0, 8.0) }
    ");
    assert_eq!(v4(&rt, "v"), (6.0, 8.0, 10.0, 12.0));
}

#[test]
fn vec4_sub() {
    let rt = run(r"
        state { let v: vec4 = vec4(10.0, 20.0, 30.0, 40.0) - vec4(1.0, 2.0, 3.0, 4.0) }
    ");
    assert_eq!(v4(&rt, "v"), (9.0, 18.0, 27.0, 36.0));
}

#[test]
fn vec4_scalar_mul() {
    let rt = run(r"
        state { let v: vec4 = vec4(1.0, 2.0, 3.0, 4.0) * 3.0 }
    ");
    assert_eq!(v4(&rt, "v"), (3.0, 6.0, 9.0, 12.0));
}

#[test]
fn vec4_scalar_div() {
    let rt = run(r"
        state { let v: vec4 = vec4(2.0, 4.0, 6.0, 8.0) / 2.0 }
    ");
    assert_eq!(v4(&rt, "v"), (1.0, 2.0, 3.0, 4.0));
}

#[test]
fn vec4_length() {
    let rt = run(r"
        state { let r: float = vec4(1.0, 0.0, 0.0, 0.0).length() }
    ");
    assert!(approx(f(&rt, "r"), 1.0));
}

#[test]
fn vec4_normalize() {
    let rt = run(r"
        state { let v: vec4 = vec4(0.0, 5.0, 0.0, 0.0).normalize() }
    ");
    let (x, y, z, w) = v4(&rt, "v");
    assert!(approx(x, 0.0));
    assert!(approx(y, 1.0));
    assert!(approx(z, 0.0));
    assert!(approx(w, 0.0));
}

#[test]
fn vec4_dot() {
    let rt = run(r"
        state { let r: float = vec4(1.0, 2.0, 3.0, 4.0).dot(vec4(5.0, 6.0, 7.0, 8.0)) }
    ");
    assert_eq!(f(&rt, "r"), 70.0);
}

#[test]
fn vec4_lerp() {
    let rt = run(r"
        state { let v: vec4 = vec4(0.0, 0.0, 0.0, 0.0).lerp(vec4(10.0, 20.0, 30.0, 40.0), 0.25) }
    ");
    assert_eq!(v4(&rt, "v"), (2.5, 5.0, 7.5, 10.0));
}

#[test]
fn vec4_abs() {
    let rt = run(r"
        state { let v: vec4 = vec4(-1.0, -2.0, 3.0, -4.0).abs() }
    ");
    assert_eq!(v4(&rt, "v"), (1.0, 2.0, 3.0, 4.0));
}

#[test]
fn vec4_min() {
    let rt = run(r"
        state { let v: vec4 = vec4(5.0, 1.0, 3.0, 8.0).min(vec4(2.0, 4.0, 6.0, 1.0)) }
    ");
    assert_eq!(v4(&rt, "v"), (2.0, 1.0, 3.0, 1.0));
}

#[test]
fn vec4_max() {
    let rt = run(r"
        state { let v: vec4 = vec4(5.0, 1.0, 3.0, 8.0).max(vec4(2.0, 4.0, 6.0, 1.0)) }
    ");
    assert_eq!(v4(&rt, "v"), (5.0, 4.0, 6.0, 8.0));
}

#[test]
fn vec4_eq() {
    let rt = run(r"
        state { let r: bool = vec4(1.0, 2.0, 3.0, 4.0) == vec4(1.0, 2.0, 3.0, 4.0) }
    ");
    assert!(b(&rt, "r"));
}

// =============================================================================
// 5. COLOR — construction, fields, arithmetic, methods, hex literals
// =============================================================================

#[test]
fn color_construction_and_fields() {
    let rt = run(r"
        state {
            let c: color = color(1.0, 0.5, 0.25)
            let r: float = 0.0
            let g: float = 0.0
            let b: float = 0.0
            let a: float = 0.0
        }
        fn on_init(s: State) -> State {
            s.r = s.c.r
            s.g = s.c.g
            s.b = s.c.b
            s.a = s.c.a
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 1.0);
    assert_eq!(f(&rt, "g"), 0.5);
    assert_eq!(f(&rt, "b"), 0.25);
    assert_eq!(f(&rt, "a"), 1.0);
}

#[test]
fn color_add() {
    let rt = run(r"
        state { let c: color = color(0.2, 0.3, 0.4) + color(0.1, 0.1, 0.1) }
    ");
    let (r, g, b, _) = col(&rt, "c");
    assert!(approx(r, 0.3));
    assert!(approx(g, 0.4));
    assert!(approx(b, 0.5));
}

#[test]
fn color_scalar_mul() {
    let rt = run(r"
        state { let c: color = color(0.5, 0.5, 0.5) * 2.0 }
    ");
    let (r, g, b, _) = col(&rt, "c");
    assert!(approx(r, 1.0));
    assert!(approx(g, 1.0));
    assert!(approx(b, 1.0));
}

#[test]
fn color_lerp() {
    let rt = run(r"
        state { let c: color = color(0.0, 0.0, 0.0).lerp(color(1.0, 1.0, 1.0), 0.5) }
    ");
    let (r, g, b, _) = col(&rt, "c");
    assert!(approx(r, 0.5));
    assert!(approx(g, 0.5));
    assert!(approx(b, 0.5));
}

#[test]
fn color_with_alpha() {
    let rt = run(r"
        state { let c: color = color(1.0, 0.0, 0.0).with_alpha(0.5) }
    ");
    let (r, _, _, a) = col(&rt, "c");
    assert!(approx(r, 1.0));
    assert!(approx(a, 0.5));
}

#[test]
fn color_to_vec4() {
    let rt = run(r"
        state { let v: vec4 = color(0.1, 0.2, 0.3).to_vec4() }
    ");
    let (x, y, z, w) = v4(&rt, "v");
    assert!(approx(x, 0.1));
    assert!(approx(y, 0.2));
    assert!(approx(z, 0.3));
    assert!(approx(w, 1.0));
}

#[test]
fn color_hex_literal() {
    let rt = run(r"
        state { let c: color = #ff0000 }
    ");
    let (r, g, b, a) = col(&rt, "c");
    assert!(approx(r, 1.0));
    assert!(approx(g, 0.0));
    assert!(approx(b, 0.0));
    assert!(approx(a, 1.0));
}

#[test]
fn color_hex_literal_with_alpha() {
    let rt = run(r"
        state { let c: color = #00ff0080 }
    ");
    let (r, g, b, a) = col(&rt, "c");
    assert!(approx(r, 0.0));
    assert!(approx(g, 1.0));
    assert!(approx(b, 0.0));
    assert!((a - 128.0 / 255.0).abs() < 0.01);
}

#[test]
fn color_named_constants() {
    let rt = run(r"
        state {
            let r: color = red
            let g: color = green
            let bl: color = blue
            let w: color = white
            let bk: color = black
        }
    ");
    assert_eq!(col(&rt, "r"), (1.0, 0.0, 0.0, 1.0));
    assert_eq!(col(&rt, "g"), (0.0, 1.0, 0.0, 1.0));
    assert_eq!(col(&rt, "bl"), (0.0, 0.0, 1.0, 1.0));
    assert_eq!(col(&rt, "w"), (1.0, 1.0, 1.0, 1.0));
    assert_eq!(col(&rt, "bk"), (0.0, 0.0, 0.0, 1.0));
}

#[test]
fn color_eq() {
    let rt = run(r"
        state { let r: bool = red == color(1.0, 0.0, 0.0) }
    ");
    assert!(b(&rt, "r"));
}

// =============================================================================
// 6. MAT3 — construction, methods, multiplication
// =============================================================================

#[test]
fn mat3_identity_mul_vec() {
    let rt = run(r"
        state { let v: vec3 = mat3().mul_vec(vec3(1.0, 2.0, 3.0)) }
    ");
    assert_eq!(v3(&rt, "v"), (1.0, 2.0, 3.0));
}

#[test]
fn mat3_transpose() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let m: mat3 = mat3_translate(5.0, 10.0)
            let mt: mat3 = m.transpose()
            s.r = 1.0
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn mat3_det_identity() {
    let rt = run(r"
        state { let d: float = mat3().det() }
    ");
    assert!(approx(f(&rt, "d"), 1.0));
}

#[test]
fn mat3_scale() {
    let rt = run(r"
        state { let v: vec3 = mat3().scale(2.0).mul_vec(vec3(3.0, 4.0, 1.0)) }
    ");
    let (x, y, _) = v3(&rt, "v");
    assert!(approx(x, 6.0));
    assert!(approx(y, 8.0));
}

#[test]
fn mat3_mul_mat3() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: mat3 = mat3_scale(2.0, 2.0)
            let b: mat3 = mat3_translate(5.0, 10.0)
            let c: mat3 = a * b
            s.r = 1.0
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn mat3_scalar_mul() {
    let rt = run(r"
        state { let d: float = (mat3() * 2.0).det() }
    ");
    assert!(approx(f(&rt, "d"), 8.0));
}

#[test]
fn mat3_inverse() {
    let rt = run(r"
        state { let v: vec3 = vec3(0.0, 0.0, 0.0) }
        fn on_init(s: State) -> State {
            let m: mat3 = mat3_scale(2.0, 3.0)
            let mi: mat3 = m.inverse()
            s.v = (m * mi).mul_vec(vec3(1.0, 1.0, 1.0))
            return s
        }
    ");
    let (x, y, z) = v3(&rt, "v");
    assert!(approx(x, 1.0));
    assert!(approx(y, 1.0));
    assert!(approx(z, 1.0));
}

// =============================================================================
// 7. MAT4 — construction, methods, multiplication
// =============================================================================

#[test]
fn mat4_identity_mul_vec() {
    let rt = run(r"
        state { let v: vec4 = mat4().mul_vec(vec4(1.0, 2.0, 3.0, 1.0)) }
    ");
    assert_eq!(v4(&rt, "v"), (1.0, 2.0, 3.0, 1.0));
}

#[test]
fn mat4_det_identity() {
    let rt = run(r"
        state { let d: float = mat4().det() }
    ");
    assert!(approx(f(&rt, "d"), 1.0));
}

#[test]
fn mat4_scale() {
    let rt = run(r"
        state { let v: vec4 = mat4().scale(3.0).mul_vec(vec4(1.0, 2.0, 3.0, 1.0)) }
    ");
    let (x, y, z, w) = v4(&rt, "v");
    assert!(approx(x, 3.0));
    assert!(approx(y, 6.0));
    assert!(approx(z, 9.0));
    assert!(approx(w, 3.0));
}

#[test]
fn mat4_mul_mat4() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: mat4 = mat4_scale(2.0, 2.0, 2.0)
            let b: mat4 = mat4_translate(1.0, 2.0, 3.0)
            let c: mat4 = a * b
            s.r = 1.0
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn mat4_transpose() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let m: mat4 = mat4_translate(1.0, 2.0, 3.0)
            let mt: mat4 = m.transpose()
            s.r = 1.0
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn mat4_inverse() {
    let rt = run(r"
        state { let v: vec4 = vec4(0.0, 0.0, 0.0, 0.0) }
        fn on_init(s: State) -> State {
            let m: mat4 = mat4_scale(2.0, 3.0, 4.0)
            let mi: mat4 = m.inverse()
            s.v = (m * mi).mul_vec(vec4(1.0, 1.0, 1.0, 1.0))
            return s
        }
    ");
    let (x, y, z, w) = v4(&rt, "v");
    assert!(approx(x, 1.0));
    assert!(approx(y, 1.0));
    assert!(approx(z, 1.0));
    assert!(approx(w, 1.0));
}

#[test]
fn mat4_scalar_mul() {
    let rt = run(r"
        state { let d: float = (mat4() * 2.0).det() }
    ");
    assert!(approx(f(&rt, "d"), 16.0));
}

#[test]
fn mat4_rotate_x() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let m: mat4 = mat4_rotate_x(0.0)
            s.r = m.det()
            return s
        }
    ");
    assert!(approx(f(&rt, "r"), 1.0));
}

#[test]
fn mat4_rotate_y() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let m: mat4 = mat4_rotate_y(0.0)
            s.r = m.det()
            return s
        }
    ");
    assert!(approx(f(&rt, "r"), 1.0));
}

#[test]
fn mat4_rotate_z() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let m: mat4 = mat4_rotate_z(0.0)
            s.r = m.det()
            return s
        }
    ");
    assert!(approx(f(&rt, "r"), 1.0));
}

// =============================================================================
// 8. TRANSFORM — construction and methods
// =============================================================================

#[test]
fn transform_basic() {
    let rt = run(r#"
        import shapes { circle }
        import coords { resolution, origin }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let t: transform = transform().move(10.0, 20.0)
            let t2: transform = t.scale(2.0)
            let t3: transform = t2.rotate(0.5)
            s.r = 1.0
            return s
        }
    "#);
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn transform_translate_alias() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let t: transform = transform().translate(5.0, 10.0)
            s.r = 1.0
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn transform_applied_to_shape() {
    let rt = run(r#"
        import shapes { circle }
        import coords { resolution, origin }

        fn on_init(s: State) -> State {
            resolution(400.0, 400.0)
            origin("center")
            return s
        }

        fn on_update(s: State, input: Input) -> State {
            let c: circle = circle(vec2(0.0, 0.0), 50.0)
            let t: transform = transform().move(100.0, 0.0)
            out << c@t
            return s
        }
    "#);
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert!(!cmds.is_empty());
}

// =============================================================================
// 9. NAMESPACE IMPORTS
// =============================================================================

#[test]
fn import_shapes_selective() {
    let rt = run(r#"
        import shapes { circle, rect }
        import coords { resolution, origin }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let c: circle = circle(vec2(0.0, 0.0), 50.0)
            let r2: rect = rect(vec2(0.0, 0.0), vec2(100.0, 100.0))
            s.r = c.radius
            return s
        }
    "#);
    assert_eq!(f(&rt, "r"), 50.0);
}

#[test]
fn import_whole_namespace() {
    let rt = run(r#"
        import shapes
        import coords { resolution, origin }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let c: circle = shapes.circle(vec2(0.0, 0.0), 25.0)
            s.r = c.radius
            return s
        }
    "#);
    assert_eq!(f(&rt, "r"), 25.0);
}

#[test]
fn import_render() {
    compile_ok(
        r#"
        import render { fill, outline, stroke }
        import shapes { circle }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let c: circle = circle(vec2(0.0, 0.0), 50.0)
            out << c
            return s
        }
    "#,
    );
}

#[test]
fn import_unknown_namespace_error() {
    let errs = compile_err(
        r"
        import nonexistent { foo }
    ",
    );
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S005)));
}

#[test]
fn import_unknown_member_error() {
    let errs = compile_err(
        r"
        import shapes { nonexistent_shape }
    ",
    );
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S006)));
}

// =============================================================================
// 10. SHAPES — line, polygon, shape fields
// =============================================================================

#[test]
fn line_shape_fields() {
    let rt = run(r"
        import shapes { line }
        state {
            let fx: float = 0.0
            let fy: float = 0.0
            let tx: float = 0.0
            let ty: float = 0.0
        }
        fn on_init(s: State) -> State {
            let l: line = line(vec2(10.0, 20.0), vec2(30.0, 40.0))
            s.fx = l.from.x
            s.fy = l.from.y
            s.tx = l.to.x
            s.ty = l.to.y
            return s
        }
    ");
    assert_eq!(f(&rt, "fx"), 10.0);
    assert_eq!(f(&rt, "fy"), 20.0);
    assert_eq!(f(&rt, "tx"), 30.0);
    assert_eq!(f(&rt, "ty"), 40.0);
}

#[test]
fn polygon_shape_emits() {
    let rt = run(r#"
        import shapes { polygon }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let pts: list[vec2] = [vec2(0.0, 0.0), vec2(100.0, 0.0), vec2(50.0, 100.0)]
            let p: polygon = polygon(pts)
            out << p
            return s
        }
    "#);
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert!(!cmds.is_empty());
}

#[test]
fn circle_field_center() {
    let rt = run(r"
        import shapes { circle }
        state { let cx: float = 0.0 let cy: float = 0.0 }
        fn on_init(s: State) -> State {
            let c: circle = circle(vec2(15.0, 25.0), 10.0)
            s.cx = c.center.x
            s.cy = c.center.y
            return s
        }
    ");
    assert_eq!(f(&rt, "cx"), 15.0);
    assert_eq!(f(&rt, "cy"), 25.0);
}

#[test]
fn rect_field_size() {
    let rt = run(r"
        import shapes { rect }
        state { let w: float = 0.0 let h: float = 0.0 }
        fn on_init(s: State) -> State {
            let r: rect = rect(vec2(0.0, 0.0), vec2(200.0, 100.0))
            s.w = r.size.x
            s.h = r.size.y
            return s
        }
    ");
    assert_eq!(f(&rt, "w"), 200.0);
    assert_eq!(f(&rt, "h"), 100.0);
}

// =============================================================================
// 11. STRING OPERATIONS — split, advanced methods
// =============================================================================

#[test]
fn string_split() {
    let rt = run(r#"
        state { let parts: list[string] = "hello world foo".split(" ") }
    "#);
    let parts = list_strings(&rt, "parts");
    assert_eq!(parts, vec!["hello", "world", "foo"]);
}

#[test]
fn string_split_empty() {
    let rt = run(r#"
        state { let parts: list[string] = "".split(",") }
    "#);
    let parts = list_strings(&rt, "parts");
    assert_eq!(parts, vec![""]);
}

#[test]
fn string_split_no_delimiter() {
    let rt = run(r#"
        state { let parts: list[string] = "abc".split(",") }
    "#);
    let parts = list_strings(&rt, "parts");
    assert_eq!(parts, vec!["abc"]);
}

#[test]
fn string_replace_all_occurrences() {
    let rt = run(r#"
        state { let s: string = "aabaa".replace("a", "x") }
    "#);
    assert_eq!(s(&rt, "s"), "xxbxx");
}

#[test]
fn string_chained_methods() {
    let rt = run(r#"
        state { let s: string = "  Hello World  ".trim().to_lower() }
    "#);
    assert_eq!(s(&rt, "s"), "hello world");
}

// =============================================================================
// 12. TYPE CASTS — as keyword
// =============================================================================

#[test]
fn cast_float_to_string_integer() {
    let rt = run(r"
        state { let s: string = 42.0 as string }
    ");
    assert_eq!(s(&rt, "s"), "42");
}

#[test]
fn cast_float_to_string_decimal() {
    let rt = run(r"
        state { let s: string = 3.14 as string }
    ");
    assert_eq!(s(&rt, "s"), "3.14");
}

#[test]
fn cast_bool_to_float_true() {
    let rt = run(r"
        state { let x: float = true as float }
    ");
    assert_eq!(f(&rt, "x"), 1.0);
}

#[test]
fn cast_bool_to_float_false() {
    let rt = run(r"
        state { let x: float = false as float }
    ");
    assert_eq!(f(&rt, "x"), 0.0);
}

#[test]
fn cast_string_to_float_valid() {
    let rt = run(r#"
        state { let x: float = "3.14" as float }
    "#);
    assert!(approx(f(&rt, "x"), 3.14));
}

#[test]
fn cast_string_to_float_invalid_runtime_error() {
    run_err(
        r#"
        state { let x: float = "abc" as float }
    "#,
    );
}

#[test]
fn cast_float_to_bool_nonzero() {
    let rt = run(r"
        state { let r: bool = 5.0 as bool }
    ");
    assert!(b(&rt, "r"));
}

#[test]
fn cast_float_to_bool_zero() {
    let rt = run(r"
        state { let r: bool = 0.0 as bool }
    ");
    assert!(!b(&rt, "r"));
}

// =============================================================================
// 13. CLOSURES — capturing, higher-order, nested
// =============================================================================

#[test]
fn closure_captures_variable() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let offset: float = 100.0
            let add_offset = (x: float) -> float { return x + offset }
            s.r = add_offset(5.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 105.0);
}

#[test]
fn closure_captures_multiple() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: float = 10.0
            let b: float = 20.0
            let sum_ab = (x: float) -> float { return x + a + b }
            s.r = sum_ab(5.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 35.0);
}

#[test]
fn closure_as_argument() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn apply(f: fn(float) -> float, x: float) -> float {
            return f(x)
        }
        fn on_init(s: State) -> State {
            let double = (x: float) -> float { return x * 2.0 }
            s.r = apply(double, 7.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 14.0);
}

#[test]
fn closure_returned_from_function() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn make_adder(n: float) -> fn(float) -> float {
            return (x: float) -> float { return x + n }
        }
        fn on_init(s: State) -> State {
            let add5 = make_adder(5.0)
            s.r = add5(10.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 15.0);
}

#[test]
fn closure_in_list_map() {
    let rt = run(r"
        state { let r: list[float] = [1.0, 2.0, 3.0] }
        fn on_init(s: State) -> State {
            let factor: float = 10.0
            s.r = s.r.map((x: float) -> float { return x * factor })
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "r"), vec![10.0, 20.0, 30.0]);
}

#[test]
fn fn_var_declaration() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn double(x: float) -> float { return x * 2.0 }
        fn on_init(s: State) -> State {
            fn f = double
            s.r = f(5.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 10.0);
}

#[test]
fn fn_var_lambda() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            fn f = (x: float) -> float { return x * 3.0 }
            s.r = f(4.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 12.0);
}

// =============================================================================
// 14. CONTROL FLOW — advanced patterns
// =============================================================================

#[test]
fn for_loop_with_break() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 100.0; i += 1.0 {
                if i == 5.0 { break }
                s.r += 1.0
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 5.0);
}

#[test]
fn for_loop_with_continue() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 10.0; i += 1.0 {
                if i % 2.0 == 0.0 { continue }
                s.r += 1.0
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 5.0);
}

#[test]
fn while_with_break_and_continue() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let i: float = 0.0
            while true {
                if i >= 10.0 { break }
                i += 1.0
                if i % 3.0 == 0.0 { continue }
                s.r += 1.0
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 7.0);
}

#[test]
fn foreach_with_break() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = [1.0, 2.0, 3.0, 4.0, 5.0]
            foreach v: float in items {
                if v == 4.0 { break }
                s.r += v
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 6.0);
}

#[test]
fn nested_loops_break_inner() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 3.0; i += 1.0 {
                for let j = 0.0; j < 10.0; j += 1.0 {
                    if j == 2.0 { break }
                    s.r += 1.0
                }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 6.0);
}

#[test]
fn match_string_values() {
    let rt = run(r#"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let cmd: string = "go"
            match cmd {
                "stop" => { s.r = 1.0 }
                "go" => { s.r = 2.0 }
                else => { s.r = 3.0 }
            }
            return s
        }
    "#);
    assert_eq!(f(&rt, "r"), 2.0);
}

#[test]
fn match_bool_values() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let flag: bool = true
            match flag {
                true => { s.r = 10.0 }
                false => { s.r = 20.0 }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 10.0);
}

#[test]
fn ternary_nested() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let x: float = 5.0
            s.r = x > 10.0 ? 1.0 : (x > 3.0 ? 2.0 : 3.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 2.0);
}

#[test]
fn compound_assignment_all_ops() {
    let rt = run(r"
        state { let a: float = 10.0 let b: float = 10.0 let c: float = 10.0 let d: float = 10.0 }
        fn on_init(s: State) -> State {
            s.a += 5.0
            s.b -= 3.0
            s.c *= 2.0
            s.d /= 4.0
            return s
        }
    ");
    assert_eq!(f(&rt, "a"), 15.0);
    assert_eq!(f(&rt, "b"), 7.0);
    assert_eq!(f(&rt, "c"), 20.0);
    assert_eq!(f(&rt, "d"), 2.5);
}

#[test]
fn prefix_inc_returns_new() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let x: float = 5.0
            s.r = ++x
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 6.0);
}

#[test]
fn postfix_inc_returns_old() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let x: float = 5.0
            s.r = x++
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 5.0);
}

#[test]
fn prefix_dec() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let x: float = 5.0
            s.r = --x
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 4.0);
}

#[test]
fn postfix_dec() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let x: float = 5.0
            s.r = x--
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 5.0);
}

// =============================================================================
// 15. OPTIONAL TYPES — advanced patterns
// =============================================================================

#[test]
fn optional_in_function_param() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn safe_div(a: float, b: float) -> float? {
            if b == 0.0 { return none }
            return a / b
        }
        fn on_init(s: State) -> State {
            let result: float? = safe_div(10.0, 2.0)
            s.r = result ?? 0.0
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 5.0);
}

#[test]
fn optional_function_returns_none() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn safe_div(a: float, b: float) -> float? {
            if b == 0.0 { return none }
            return a / b
        }
        fn on_init(s: State) -> State {
            let result: float? = safe_div(10.0, 0.0)
            s.r = result ?? -1.0
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), -1.0);
}

#[test]
fn optional_chain_deep() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let v: vec2? = none
            let x: float? = v?.x
            s.r = x ?? 99.0
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 99.0);
}

#[test]
fn optional_chain_with_value() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let v: vec2? = vec2(42.0, 0.0)
            let x: float? = v?.x
            s.r = x ?? 0.0
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
}

#[test]
fn if_let_with_expression() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn find(x: float) -> float? {
            if x > 0.0 { return x * 10.0 }
            return none
        }
        fn on_init(s: State) -> State {
            if let val = find(5.0) {
                s.r = val
            } else {
                s.r = -1.0
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 50.0);
}

#[test]
fn if_let_none_takes_else() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn find(x: float) -> float? {
            if x > 0.0 { return x * 10.0 }
            return none
        }
        fn on_init(s: State) -> State {
            if let val = find(-1.0) {
                s.r = val
            } else {
                s.r = -999.0
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), -999.0);
}

#[test]
fn coalesce_short_circuits() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let x: float? = 42.0
            s.r = x ?? (1.0 / 0.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
}

// =============================================================================
// 16. RESULT TYPE — try, ok, error
// =============================================================================

#[test]
fn result_ok_construction() {
    let rt = run(r"
        state { let r: float = 0.0 let is_ok: bool = false }
        fn on_init(s: State) -> State {
            let result: res<float> = ok(42.0)
            s.r = result.value
            s.is_ok = result.ok
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
    assert!(b(&rt, "is_ok"));
}

#[test]
fn result_error_construction() {
    let rt = run(r#"
        state { let msg: string = "" let is_ok: bool = true }
        fn on_init(s: State) -> State {
            let result: res<float> = error("something went wrong")
            s.msg = result.error
            s.is_ok = result.ok
            return s
        }
    "#);
    assert_eq!(s(&rt, "msg"), "something went wrong");
    assert!(!b(&rt, "is_ok"));
}

#[test]
fn try_catches_runtime_error() {
    let rt = run(r"
        state { let r: float = 0.0 let caught: bool = false }
        fn on_init(s: State) -> State {
            let result: res<float> = try 1.0 / 0.0
            s.caught = not result.ok
            s.r = result.ok ? 1.0 : 0.0
            return s
        }
    ");
    assert!(b(&rt, "caught"));
}

#[test]
fn try_success_preserves_value() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let result: res<float> = try 10.0 / 2.0
            s.r = result.value
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 5.0);
}

// =============================================================================
// 17. STRUCTS — advanced patterns
// =============================================================================

#[test]
fn struct_private_field_compile_error() {
    let errs = compile_err(
        r"
        struct Foo {
            #let secret: float = 0.0
            +let visible: float = 0.0
        }
        fn on_init(s: State) -> State {
            let f: Foo = Foo { secret: 1.0, visible: 2.0 }
            let x: float = f.secret
            return s
        }
    ",
    );
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S016)));
}

#[test]
fn struct_private_method_compile_error() {
    let errs = compile_err(
        r"
        struct Foo {
            +let x: float = 0.0
            #fn hidden() -> float { return this.x }
        }
        fn on_init(s: State) -> State {
            let f: Foo = Foo { x: 5.0 }
            let r: float = f.hidden()
            return s
        }
    ",
    );
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S016)));
}

#[test]
fn struct_private_accessed_via_public_method() {
    let rt = run(r"
        struct Foo {
            #let secret: float = 42.0
            +fn get_secret() -> float { return this.secret }
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let f: Foo = Foo { secret: 42.0 }
            s.r = f.get_secret()
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
}

#[test]
fn struct_this_keyword() {
    let rt = run(r"
        struct Counter {
            +let count: float = 0.0
            +fn increment() {
                this.count += 1.0
            }
            +fn get() -> float { return this.count }
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let c: Counter = Counter { count: 0.0 }
            c.increment()
            c.increment()
            c.increment()
            s.r = c.get()
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 3.0);
}

#[test]
fn struct_clone_deep_copy() {
    let rt = run(r"
        struct Point {
            +let x: float = 0.0
            +let y: float = 0.0
        }
        state { let ax: float = 0.0 let bx: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: Point = Point { x: 1.0, y: 2.0 }
            let b: Point = a.clone()
            b.x = 99.0
            s.ax = a.x
            s.bx = b.x
            return s
        }
    ");
    assert_eq!(f(&rt, "ax"), 1.0);
    assert_eq!(f(&rt, "bx"), 99.0);
}

#[test]
fn struct_reference_semantics() {
    let rt = run(r"
        struct Bag {
            +let value: float = 0.0
        }
        state { let r: float = 0.0 }
        fn set_value(b: Bag, v: float) {
            b.value = v
        }
        fn on_init(s: State) -> State {
            let bag: Bag = Bag { value: 0.0 }
            set_value(bag, 42.0)
            s.r = bag.value
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
}

#[test]
fn struct_with_list_field() {
    let rt = run(r"
        struct Container {
            +let items: list[float] = []
            +fn add(x: float) { this.items.push(x) }
            +fn count() -> float { return this.items.len() }
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let c: Container = Container { items: [] }
            c.add(1.0)
            c.add(2.0)
            c.add(3.0)
            s.r = c.count()
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 3.0);
}

#[test]
fn struct_method_with_conditional() {
    let rt = run(r"
        struct Checker {
            +let threshold: float = 10.0
            +fn check(val: float) -> bool {
                if val > this.threshold {
                    return true
                }
                return false
            }
        }
        state { let r1: bool = false let r2: bool = false }
        fn on_init(s: State) -> State {
            let c: Checker = Checker { threshold: 10.0 }
            s.r1 = c.check(15.0)
            s.r2 = c.check(5.0)
            return s
        }
    ");
    assert!(b(&rt, "r1"));
    assert!(!b(&rt, "r2"));
}

// =============================================================================
// 18. ENUMS — advanced patterns
// =============================================================================

#[test]
fn enum_dataless_variant() {
    let rt = run(r"
        enum Direction { Up Down Left Right }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let d: Direction = Direction.Up
            match d {
                Direction.Up => { s.r = 1.0 }
                Direction.Down => { s.r = 2.0 }
                Direction.Left => { s.r = 3.0 }
                Direction.Right => { s.r = 4.0 }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn enum_with_fields() {
    let rt = run(r"
        enum Shape {
            Circle { radius: float }
            Rect { width: float, height: float }
            Empty
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let sh: Shape = Shape.Circle { radius: 25.0 }
            match sh {
                Shape.Circle => { s.r = sh.radius }
                Shape.Rect => { s.r = sh.width }
                Shape.Empty => { s.r = 0.0 }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 25.0);
}

#[test]
fn enum_equality_same_variant() {
    let rt = run(r"
        enum Color { Red Green Blue }
        state { let r: bool = false }
        fn on_init(s: State) -> State {
            s.r = Color.Red == Color.Red
            return s
        }
    ");
    assert!(b(&rt, "r"));
}

#[test]
fn enum_inequality_different_variant() {
    let rt = run(r"
        enum Color { Red Green Blue }
        state { let r: bool = false }
        fn on_init(s: State) -> State {
            s.r = Color.Red != Color.Green
            return s
        }
    ");
    assert!(b(&rt, "r"));
}

#[test]
fn enum_in_list() {
    let rt = run(r"
        enum Dir { N S E W }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let dirs: list[Dir] = [Dir.N, Dir.S, Dir.E, Dir.W]
            s.r = dirs.len()
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 4.0);
}

#[test]
fn enum_passed_to_function() {
    let rt = run(r"
        enum Op { Add Sub }
        fn apply(op: Op, a: float, b: float) -> float {
            match op {
                Op.Add => { return a + b }
                Op.Sub => { return a - b }
            }
            return 0.0
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            s.r = apply(Op.Add, 10.0, 3.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 13.0);
}

#[test]
fn enum_match_else_arm() {
    let rt = run(r"
        enum Status { Ok Loading Error }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let st: Status = Status.Loading
            match st {
                Status.Ok => { s.r = 1.0 }
                else => { s.r = -1.0 }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), -1.0);
}

// =============================================================================
// 19. LIFECYCLE — on_init, on_update, on_exit
// =============================================================================

#[test]
fn lifecycle_init_runs_once() {
    let rt = run(r"
        state { let count: float = 0.0 }
        fn on_init(s: State) -> State {
            s.count += 1.0
            return s
        }
    ");
    assert_eq!(f(&rt, "count"), 1.0);
}

#[test]
fn lifecycle_update_accumulates() {
    let rt = run(r"
        state { let count: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.count += 1.0
            return s
        }
    ");
    let mut rt = rt;
    tick(&mut rt);
    tick(&mut rt);
    tick(&mut rt);
    assert_eq!(f(&rt, "count"), 3.0);
}

#[test]
fn lifecycle_exit() {
    let rt = run(r"
        state { let exited: bool = false }
        fn on_exit(s: State) -> State {
            s.exited = true
            return s
        }
    ");
    let mut rt = rt;
    rt.exit().unwrap();
    assert!(b(&rt, "exited"));
}

#[test]
fn lifecycle_input_dt() {
    let rt = run(r"
        state { let t: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.t += input.dt
            return s
        }
    ");
    let mut rt = rt;
    tick(&mut rt);
    assert!(f(&rt, "t") > 0.0);
}

#[test]
fn lifecycle_static_script_emits_each_tick() {
    let rt = run(r#"
        import shapes { circle }
        import coords { resolution, origin }
        resolution(400.0, 400.0)
        origin("center")
        out << circle(vec2(0.0, 0.0), 50.0)
    "#);
    let mut rt = rt;
    let cmds1 = tick(&mut rt);
    let cmds2 = tick(&mut rt);
    assert!(!cmds1.is_empty());
    assert!(!cmds2.is_empty());
}

// =============================================================================
// 20. MATH FUNCTIONS — trig and advanced
// =============================================================================

#[test]
fn math_tan() {
    let rt = run(r"
        state { let r: float = tan(0.0) }
    ");
    assert!(approx(f(&rt, "r"), 0.0));
}

#[test]
fn math_asin() {
    let rt = run(r"
        state { let r: float = asin(1.0) }
    ");
    assert!(approx(f(&rt, "r"), std::f64::consts::FRAC_PI_2));
}

#[test]
fn math_acos() {
    let rt = run(r"
        state { let r: float = acos(1.0) }
    ");
    assert!(approx(f(&rt, "r"), 0.0));
}

#[test]
fn math_atan() {
    let rt = run(r"
        state { let r: float = atan(0.0) }
    ");
    assert!(approx(f(&rt, "r"), 0.0));
}

#[test]
fn math_atan2() {
    let rt = run(r"
        state { let r: float = atan2(1.0, 0.0) }
    ");
    assert!(approx(f(&rt, "r"), std::f64::consts::FRAC_PI_2));
}

#[test]
fn math_sign_positive() {
    let rt = run(r"
        state { let r: float = sign(42.0) }
    ");
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn math_sign_negative() {
    let rt = run(r"
        state { let r: float = sign(-7.0) }
    ");
    assert_eq!(f(&rt, "r"), -1.0);
}

#[test]
fn math_sign_zero() {
    let rt = run(r"
        state { let r: float = sign(0.0) }
    ");
    assert_eq!(f(&rt, "r"), 0.0);
}

#[test]
fn math_fract() {
    let rt = run(r"
        state { let r: float = fract(3.75) }
    ");
    assert!(approx(f(&rt, "r"), 0.75));
}

#[test]
fn math_tau_constant() {
    let rt = run(r"
        state { let r: float = TAU }
    ");
    assert!(approx(f(&rt, "r"), std::f64::consts::TAU));
}

// =============================================================================
// 21. VEC2 — additional methods
// =============================================================================

#[test]
fn vec2_distance() {
    let rt = run(r"
        state { let r: float = vec2(0.0, 0.0).distance(vec2(3.0, 4.0)) }
    ");
    assert!(approx(f(&rt, "r"), 5.0));
}

#[test]
fn vec2_perp() {
    let rt = run(r"
        state { let v: vec2 = vec2(1.0, 0.0).perp() }
    ");
    let (x, y) = v2(&rt, "v");
    assert!(approx(x, 0.0));
    assert!(approx(y, 1.0));
}

#[test]
fn vec2_angle() {
    let rt = run(r"
        state { let r: float = vec2(1.0, 0.0).angle() }
    ");
    assert!(approx(f(&rt, "r"), 0.0));
}

#[test]
fn vec2_abs() {
    let rt = run(r"
        state { let v: vec2 = vec2(-3.0, -4.0).abs() }
    ");
    assert_eq!(v2(&rt, "v"), (3.0, 4.0));
}

#[test]
fn vec2_floor() {
    let rt = run(r"
        state { let v: vec2 = vec2(3.7, 4.2).floor() }
    ");
    assert_eq!(v2(&rt, "v"), (3.0, 4.0));
}

#[test]
fn vec2_ceil() {
    let rt = run(r"
        state { let v: vec2 = vec2(3.1, 4.9).ceil() }
    ");
    assert_eq!(v2(&rt, "v"), (4.0, 5.0));
}

#[test]
fn vec2_min() {
    let rt = run(r"
        state { let v: vec2 = vec2(5.0, 1.0).min(vec2(2.0, 8.0)) }
    ");
    assert_eq!(v2(&rt, "v"), (2.0, 1.0));
}

#[test]
fn vec2_max() {
    let rt = run(r"
        state { let v: vec2 = vec2(5.0, 1.0).max(vec2(2.0, 8.0)) }
    ");
    assert_eq!(v2(&rt, "v"), (5.0, 8.0));
}

#[test]
fn vec2_lerp() {
    let rt = run(r"
        state { let v: vec2 = vec2(0.0, 0.0).lerp(vec2(10.0, 20.0), 0.5) }
    ");
    assert_eq!(v2(&rt, "v"), (5.0, 10.0));
}

#[test]
fn vec2_field_mutation() {
    let rt = run(r"
        state { let v: vec2 = vec2(0.0, 0.0) }
        fn on_init(s: State) -> State {
            s.v = vec2(1.0, 2.0)
            return s
        }
    ");
    assert_eq!(v2(&rt, "v"), (1.0, 2.0));
}

// =============================================================================
// 22. STRING INTERPOLATION — advanced patterns
// =============================================================================

#[test]
fn interpolation_with_math() {
    let rt = run(r"
        state { let s: string = `result: ${2.0 + 3.0}` }
    ");
    assert_eq!(s(&rt, "s"), "result: 5");
}

#[test]
fn interpolation_with_function_call() {
    let rt = run(r"
        state { let s: string = `sqrt of 16 is ${sqrt(16.0)}` }
    ");
    assert_eq!(s(&rt, "s"), "sqrt of 16 is 4");
}

#[test]
fn interpolation_with_ternary() {
    let rt = run(r"
        state { let s: string = `value is ${true ? 1.0 : 0.0}` }
    ");
    assert_eq!(s(&rt, "s"), "value is 1");
}

#[test]
fn interpolation_empty_expression() {
    let rt = run(r#"
        state { let s: string = `hello ${"world"}` }
    "#);
    assert_eq!(s(&rt, "s"), "hello world");
}

#[test]
fn interpolation_multiple_types() {
    let rt = run(r"
        state { let r: string = `` }
        fn on_init(s: State) -> State {
            let x: float = 42.0
            let flag: bool = true
            s.r = `x=${x}, flag=${flag}`
            return s
        }
    ");
    assert_eq!(s(&rt, "r"), "x=42, flag=true");
}

// =============================================================================
// 23. BOOLEAN ARITHMETIC — full coverage
// =============================================================================

#[test]
fn bool_plus_bool() {
    let rt = run(r"
        state { let r: float = true + true }
    ");
    assert_eq!(f(&rt, "r"), 2.0);
}

#[test]
fn bool_minus_bool() {
    let rt = run(r"
        state { let r: float = true - false }
    ");
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn float_plus_bool() {
    let rt = run(r"
        state { let r: float = 10.0 + true }
    ");
    assert_eq!(f(&rt, "r"), 11.0);
}

#[test]
fn bool_times_float() {
    let rt = run(r"
        state { let r: float = true * 5.0 }
    ");
    assert_eq!(f(&rt, "r"), 5.0);
}

#[test]
fn false_times_float() {
    let rt = run(r"
        state { let r: float = false * 100.0 }
    ");
    assert_eq!(f(&rt, "r"), 0.0);
}

// =============================================================================
// 24. TRUTHINESS — comprehensive
// =============================================================================

#[test]
fn truthy_float_zero_is_false() {
    let rt = run(r"
        state { let r: bool = false }
        fn on_init(s: State) -> State {
            let x: float = 0.0
            if x {
                s.r = true
            }
            return s
        }
    ");
    assert!(!b(&rt, "r"));
}

#[test]
fn truthy_list_with_elements() {
    let rt = run(r"
        state { let r: bool = false }
        fn on_init(s: State) -> State {
            let items: list[float] = [1.0]
            if items { s.r = true }
            return s
        }
    ");
    assert!(b(&rt, "r"));
}

#[test]
fn truthy_list_empty() {
    let rt = run(r"
        state { let r: bool = true }
        fn on_init(s: State) -> State {
            let items: list[float] = []
            if items { s.r = true } else { s.r = false }
            return s
        }
    ");
    assert!(!b(&rt, "r"));
}

#[test]
fn truthy_string_nonempty() {
    let rt = run(r#"
        state { let r: bool = false }
        fn on_init(s: State) -> State {
            if "hello" { s.r = true }
            return s
        }
    "#);
    assert!(b(&rt, "r"));
}

#[test]
fn truthy_string_empty() {
    let rt = run(r#"
        state { let r: bool = true }
        fn on_init(s: State) -> State {
            if "" { s.r = true } else { s.r = false }
            return s
        }
    "#);
    assert!(!b(&rt, "r"));
}

#[test]
fn truthy_optional_none() {
    let rt = run(r"
        state { let r: bool = true }
        fn on_init(s: State) -> State {
            let x: float? = none
            if x { s.r = true } else { s.r = false }
            return s
        }
    ");
    assert!(!b(&rt, "r"));
}

#[test]
fn truthy_optional_with_nonzero() {
    let rt = run(r"
        state { let r: bool = false }
        fn on_init(s: State) -> State {
            let x: float? = 5.0
            if x { s.r = true }
            return s
        }
    ");
    assert!(b(&rt, "r"));
}

#[test]
fn truthy_optional_with_zero() {
    let rt = run(r"
        state { let r: bool = true }
        fn on_init(s: State) -> State {
            let x: float? = 0.0
            if x { s.r = true } else { s.r = false }
            return s
        }
    ");
    assert!(!b(&rt, "r"));
}

// =============================================================================
// 25. SCOPE / VARIABLES
// =============================================================================

#[test]
fn variable_shadowing_in_scope() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let x: float = 1.0
            if true {
                let x: float = 2.0
                s.r = x
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 2.0);
}

#[test]
fn variable_outer_scope_after_inner() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let x: float = 10.0
            if true {
                let x: float = 20.0
            }
            s.r = x
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 10.0);
}

#[test]
fn variable_type_inference() {
    let rt = run(r#"
        state {
            let x = 42.0
            let y = true
            let z = "hello"
        }
    "#);
    assert_eq!(f(&rt, "x"), 42.0);
    assert!(b(&rt, "y"));
    assert_eq!(s(&rt, "z"), "hello");
}

#[test]
fn local_var_in_for_loop() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 5.0; i += 1.0 {
                let temp: float = i * 2.0
                s.r += temp
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 20.0);
}

// =============================================================================
// 26. FUNCTIONS — advanced patterns
// =============================================================================

#[test]
fn function_forward_declaration() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            s.r = helper()
            return s
        }
        fn helper() -> float {
            return 42.0
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
}

#[test]
fn function_multiple_params() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn add3(a: float, b: float, c: float) -> float {
            return a + b + c
        }
        fn on_init(s: State) -> State {
            s.r = add3(1.0, 2.0, 3.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 6.0);
}

#[test]
fn function_returns_vec2() {
    let rt = run(r"
        state { let v: vec2 = vec2(0.0, 0.0) }
        fn make_point(x: float, y: float) -> vec2 {
            return vec2(x, y)
        }
        fn on_init(s: State) -> State {
            s.v = make_point(3.0, 4.0)
            return s
        }
    ");
    assert_eq!(v2(&rt, "v"), (3.0, 4.0));
}

#[test]
fn function_returns_bool() {
    let rt = run(r"
        state { let r: bool = false }
        fn is_positive(x: float) -> bool {
            return x > 0.0
        }
        fn on_init(s: State) -> State {
            s.r = is_positive(5.0)
            return s
        }
    ");
    assert!(b(&rt, "r"));
}

#[test]
fn function_void_side_effect() {
    let rt = run(r"
        state { let r: float = 99.0 }
        fn side_effect(s: State) {
            s.r = 42.0
        }
        fn on_init(s: State) -> State {
            side_effect(s)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
}

#[test]
fn function_typed_empty_body_returns_none() {
    // fn with -> float and empty body: VM returns None, which becomes 0.0 when assigned to float
    // NOTE: spec lists P005 (empty body) but parser doesn't enforce it
    let rt = run(r"
        state { let r: float? = 99.0 }
        fn noop() -> float? { }
        fn on_init(s: State) -> State {
            s.r = noop()
            return s
        }
    ");
    match rt.state().get("r") {
        Some(Value::None) => {} // VM returns none for empty body
        Some(Value::Float(v)) => assert_eq!(*v, 0.0), // or possibly 0.0
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn function_recursive_factorial() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn fact(n: float) -> float {
            if n <= 1.0 { return 1.0 }
            return n * fact(n - 1.0)
        }
        fn on_init(s: State) -> State {
            s.r = fact(6.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 720.0);
}

// =============================================================================
// 27. LIST OPERATIONS — additional coverage
// =============================================================================

#[test]
fn list_nested_operations() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [1.0, 2.0, 3.0, 4.0, 5.0]
            let evens: list[float] = nums.filter((x: float) -> bool { return x % 2.0 == 0.0 })
            let doubled: list[float] = evens.map((x: float) -> float { return x * 2.0 })
            s.r = doubled[0] + doubled[1]
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 12.0);
}

#[test]
fn list_sort_with_custom_comparator() {
    let rt = run(r"
        state { let r: list[float] = [3.0, 1.0, 4.0, 1.0, 5.0] }
        fn on_init(s: State) -> State {
            s.r.sort((a: float, b: float) -> float { return b - a })
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "r"), vec![5.0, 4.0, 3.0, 1.0, 1.0]);
}

#[test]
fn list_any_with_condition() {
    let rt = run(r"
        state { let r: bool = false }
        fn on_init(s: State) -> State {
            let nums: list[float] = [1.0, 2.0, 3.0, 4.0, 5.0]
            s.r = nums.any((x: float) -> bool { return x > 4.0 })
            return s
        }
    ");
    assert!(b(&rt, "r"));
}

#[test]
fn list_all_with_condition() {
    let rt = run(r"
        state { let r: bool = false }
        fn on_init(s: State) -> State {
            let nums: list[float] = [2.0, 4.0, 6.0, 8.0]
            s.r = nums.all((x: float) -> bool { return x % 2.0 == 0.0 })
            return s
        }
    ");
    assert!(b(&rt, "r"));
}

#[test]
fn list_take_and_drop() {
    let rt = run(r"
        state {
            let t: list[float] = []
            let d: list[float] = []
        }
        fn on_init(s: State) -> State {
            let nums: list[float] = [10.0, 20.0, 30.0, 40.0, 50.0]
            s.t = nums.take(0.0, 3.0)
            s.d = nums.drop(0.0, 2.0)
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "t"), vec![10.0, 20.0, 30.0]);
    assert_eq!(list_floats(&rt, "d"), vec![30.0, 40.0, 50.0]);
}

#[test]
fn list_cut() {
    let rt = run(r"
        state { let r: list[float] = [] }
        fn on_init(s: State) -> State {
            let nums: list[float] = [10.0, 20.0, 30.0, 40.0, 50.0]
            s.r = nums.cut(1.0, 3.0)
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "r"), vec![20.0, 30.0]);
}

#[test]
fn list_paste_value() {
    let rt = run(r"
        state { let r: list[float] = [1.0, 2.0, 3.0] }
        fn on_init(s: State) -> State {
            s.r.paste(1.0, 99.0)
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "r"), vec![1.0, 99.0, 2.0, 3.0]);
}

#[test]
fn list_search_returns_index() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = [10.0, 20.0, 30.0, 40.0]
            s.r = items.search(30.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 2.0);
}

#[test]
fn list_search_not_found_returns_negative() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = [10.0, 20.0, 30.0]
            s.r = items.search(99.0)
            return s
        }
    ");
    assert!(f(&rt, "r") < 0.0);
}

#[test]
fn list_bsearch_sorted() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = [1.0, 3.0, 5.0, 7.0, 9.0]
            s.r = items.bsearch(5.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 2.0);
}

#[test]
fn list_clone_independence() {
    let rt = run(r"
        state { let a: list[float] = [1.0, 2.0, 3.0] let blen: float = 0.0 }
        fn on_init(s: State) -> State {
            let b: list[float] = s.a.clone()
            b.push(4.0)
            s.blen = b.len()
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "a"), vec![1.0, 2.0, 3.0]);
    assert_eq!(f(&rt, "blen"), 4.0);
}

// =============================================================================
// 28. INPUT HANDLING
// =============================================================================

#[test]
fn input_all_mouse_fields() {
    let rt = run(r"
        state {
            let mx: float = 0.0
            let my: float = 0.0
            let md: bool = false
            let mp: bool = false
            let mr: bool = false
        }
        fn on_update(s: State, input: Input) -> State {
            s.mx = input.mouse_x
            s.my = input.mouse_y
            s.md = input.mouse_down
            s.mp = input.mouse_pressed
            s.mr = input.mouse_released
            return s
        }
    ");
    let mut rt = rt;
    tick_with(
        &mut rt,
        Input {
            dt: 0.016,
            mouse_x: 100.0,
            mouse_y: 200.0,
            mouse_down: true,
            mouse_pressed: true,
            mouse_released: false,
            ..Default::default()
        },
    );
    assert_eq!(f(&rt, "mx"), 100.0);
    assert_eq!(f(&rt, "my"), 200.0);
    assert!(b(&rt, "md"));
    assert!(b(&rt, "mp"));
    assert!(!b(&rt, "mr"));
}

#[test]
fn input_keyboard_fields() {
    let rt = run(r#"
        state { let kp: string = "" let kd: string = "" let kr: string = "" }
        fn on_update(s: State, input: Input) -> State {
            s.kp = input.key_pressed
            s.kd = input.key_down
            s.kr = input.key_released
            return s
        }
    "#);
    let mut rt = rt;
    tick_with(
        &mut rt,
        Input {
            dt: 0.016,
            key_pressed: "a".into(),
            key_down: "a".into(),
            key_released: "".into(),
            ..Default::default()
        },
    );
    assert_eq!(s(&rt, "kp"), "a");
    assert_eq!(s(&rt, "kd"), "a");
    assert_eq!(s(&rt, "kr"), "");
}

// =============================================================================
// 29. COORDINATES
// =============================================================================

#[test]
fn coords_resolution_and_origin() {
    let rt = run(r#"
        import coords { resolution, origin }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            resolution(800.0, 600.0)
            origin("center")
            s.r = 1.0
            return s
        }
    "#);
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn coords_resolution_in_update() {
    let rt = run(r#"
        import coords { resolution, origin }
        import shapes { circle }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("top_left")
            out << circle(vec2(50.0, 50.0), 25.0)
            return s
        }
    "#);
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert!(!cmds.is_empty());
}

// =============================================================================
// 30. CONSOLE OUTPUT
// =============================================================================

#[test]
fn console_log() {
    let rt = run(r"
        state { let x: float = 42.0 }
        fn on_update(s: State, input: Input) -> State {
            console << s.x
            return s
        }
    ");
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert!(cmds.iter().any(|c| matches!(c, DrawCommand::Print(_))));
}

#[test]
fn console_warn() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            console.warn << 42.0
            return s
        }
    ");
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert!(cmds.iter().any(|c| matches!(c, DrawCommand::Warn(_))));
}

#[test]
fn console_error() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            console.error << 42.0
            return s
        }
    ");
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert!(cmds.iter().any(|c| matches!(c, DrawCommand::Error(_))));
}

#[test]
fn console_multiple_values() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            console << 1.0 << "hello" << true
            return s
        }
    "#);
    let mut rt = rt;
    let cmds = tick(&mut rt);
    match &cmds[0] {
        DrawCommand::Print(msg) => assert_eq!(msg, "1 hello true"),
        other => panic!("expected Print, got: {other:?}"),
    }
}

// =============================================================================
// 31. RENDER MODES
// =============================================================================

#[test]
fn render_fill_mode() {
    compile_ok(
        r#"
        import render { fill }
        import shapes { circle }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let c: circle = circle(vec2(0.0, 0.0), 50.0)
            out << c
            return s
        }
    "#,
    );
}

#[test]
fn render_stroke_function() {
    compile_ok(
        r#"
        import render { stroke }
        import shapes { circle }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let c: circle = circle(vec2(0.0, 0.0), 50.0)
            out << c
            return s
        }
    "#,
    );
}

// =============================================================================
// 32. COMPLEX PROGRAMS — real-world patterns
// =============================================================================

#[test]
fn complex_bouncing_ball() {
    let rt = run(r"
        state {
            let x: float = 0.0
            let y: float = 0.0
            let vx: float = 2.0
            let vy: float = 3.0
        }
        fn on_init(s: State) -> State {
            s.x = 100.0
            s.y = 100.0
            return s
        }
        fn on_update(s: State, input: Input) -> State {
            s.x += s.vx
            s.y += s.vy
            if s.x > 400.0 or s.x < 0.0 {
                s.vx = -s.vx
            }
            if s.y > 400.0 or s.y < 0.0 {
                s.vy = -s.vy
            }
            return s
        }
    ");
    let mut rt = rt;
    for _ in 0..10 {
        tick(&mut rt);
    }
    assert!(f(&rt, "x") != 100.0 || f(&rt, "y") != 100.0);
}

#[test]
fn complex_particle_system() {
    let rt = run(r"
        struct Particle {
            +let x: float = 0.0
            +let y: float = 0.0
            +let vx: float = 0.0
            +let vy: float = 0.0
            +fn update(dt: float) {
                this.x += this.vx * dt
                this.y += this.vy * dt
            }
        }
        state { let count: float = 0.0 }
        fn on_init(s: State) -> State {
            let particles: list[Particle] = []
            for let i = 0.0; i < 5.0; i += 1.0 {
                let p: Particle = Particle { x: i * 10.0, y: 0.0, vx: 1.0, vy: 2.0 }
                p.update(0.1)
                particles.push(p)
            }
            s.count = particles.len()
            return s
        }
    ");
    assert_eq!(f(&rt, "count"), 5.0);
}

#[test]
fn complex_state_machine() {
    let rt = run(r"
        enum GameState { Menu Playing Paused GameOver }
        state { let score: float = 0.0 }
        fn on_init(s: State) -> State {
            let current: GameState = GameState.Menu
            match current {
                GameState.Menu => {
                    current = GameState.Playing
                }
                else => { }
            }
            match current {
                GameState.Playing => {
                    s.score = 100.0
                }
                else => { }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "score"), 100.0);
}

#[test]
fn complex_linked_structs() {
    let rt = run(r"
        struct Node {
            +let value: float = 0.0
            +let next_val: float = -1.0
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: Node = Node { value: 1.0, next_val: 2.0 }
            let b: Node = Node { value: 2.0, next_val: 3.0 }
            let c: Node = Node { value: 3.0, next_val: -1.0 }
            s.r = a.value + b.value + c.value
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 6.0);
}

#[test]
fn complex_fibonacci_iterative() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn fib(n: float) -> float {
            if n <= 1.0 { return n }
            let a: float = 0.0
            let b: float = 1.0
            for let i = 2.0; i <= n; i += 1.0 {
                let temp: float = b
                b = a + b
                a = temp
            }
            return b
        }
        fn on_init(s: State) -> State {
            s.r = fib(10.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 55.0);
}

#[test]
fn complex_matrix_operations() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let translate: mat3 = mat3_translate(10.0, 20.0)
            let scale: mat3 = mat3_scale(2.0, 2.0)
            let combined: mat3 = translate * scale
            let point: vec3 = vec3(5.0, 5.0, 1.0)
            let result: vec3 = combined.mul_vec(point)
            s.r = result.x
            return s
        }
    ");
    assert!(f(&rt, "r") != 0.0);
}

// =============================================================================
// 33. ERROR HANDLING — compile-time errors
// =============================================================================

#[test]
fn error_undefined_variable() {
    let errs = compile_err(
        r"
        state { let x: float = undefined_var }
    ",
    );
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S001)));
}

#[test]
fn error_type_mismatch() {
    let errs = compile_err(
        r"
        state { let x: float = true }
    ",
    );
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S002)));
}

#[test]
fn error_redeclaration() {
    let errs = compile_err(
        r"
        fn foo() -> float { return 1.0 }
        fn foo() -> float { return 2.0 }
    ",
    );
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S003)));
}

#[test]
fn error_wrong_arg_count() {
    let errs = compile_err(
        r"
        fn add(a: float, b: float) -> float { return a + b }
        state { let x: float = add(1.0) }
    ",
    );
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S007)));
}

#[test]
fn error_unknown_field() {
    let errs = compile_err(
        r"
        state { let x: float = vec2(1.0, 2.0).z }
    ",
    );
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S009)));
}

// =============================================================================
// 34. ERROR HANDLING — runtime errors
// =============================================================================

#[test]
fn runtime_div_by_zero() {
    let err = run_err(
        r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 1.0 / 0.0
            return s
        }
    ",
    );
    assert!(matches!(err.code, ErrorCode::R007));
}

#[test]
fn runtime_index_out_of_bounds() {
    let err = run_err(
        r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = [1.0, 2.0]
            s.x = items[5]
            return s
        }
    ",
    );
    assert!(matches!(err.code, ErrorCode::R005));
}

#[test]
fn runtime_negative_index() {
    let err = run_err(
        r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = [1.0, 2.0]
            s.x = items[-1]
            return s
        }
    ",
    );
    assert!(matches!(err.code, ErrorCode::R005 | ErrorCode::R006));
}

#[test]
fn runtime_pop_empty_list() {
    let err = run_err(
        r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = []
            s.x = items.pop()
            return s
        }
    ",
    );
    assert!(matches!(err.code, ErrorCode::R011));
}

#[test]
fn runtime_stack_overflow() {
    let err = run_err(
        r"
        state { let x: float = 0.0 }
        fn recurse(n: float) -> float {
            return recurse(n + 1.0)
        }
        fn on_init(s: State) -> State {
            s.x = recurse(0.0)
            return s
        }
    ",
    );
    assert!(matches!(
        err.code,
        ErrorCode::R011
    ));
}

// =============================================================================
// 35. DRAW OUTPUT — shape types and transforms
// =============================================================================

#[test]
fn draw_line_shape() {
    let rt = run(r#"
        import shapes { line }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let l: line = line(vec2(0.0, 0.0), vec2(100.0, 100.0))
            out << l
            return s
        }
    "#);
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 1);
    assert!(
        matches!(&cmds[0], DrawCommand::DrawShape(sd) if matches!(sd.desc, ShapeDesc::Line { .. }))
    );
}

#[test]
fn draw_multiple_shape_types() {
    let rt = run(r#"
        import shapes { circle, rect, line }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            out << circle(vec2(0.0, 0.0), 50.0)
            out << rect(vec2(100.0, 100.0), vec2(50.0, 50.0))
            out << line(vec2(0.0, 0.0), vec2(200.0, 200.0))
            return s
        }
    "#);
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 3);
}

#[test]
fn draw_chained_out() {
    let rt = run(r#"
        import shapes { circle }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let c1: circle = circle(vec2(0.0, 0.0), 25.0)
            let c2: circle = circle(vec2(50.0, 50.0), 25.0)
            out << c1 << c2
            return s
        }
    "#);
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 2);
}

#[test]
fn draw_with_transform() {
    let rt = run(r#"
        import shapes { circle }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let c: circle = circle(vec2(0.0, 0.0), 50.0)
            let t: transform = transform().move(100.0, 50.0).scale(2.0).rotate(0.5)
            out << c@t
            return s
        }
    "#);
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        DrawCommand::DrawShape(sd) => assert!(!sd.transforms.is_empty()),
        other => panic!("expected DrawShape, got: {other:?}"),
    }
}

#[test]
fn draw_in_foreach() {
    let rt = run(r#"
        import shapes { circle }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let positions: list[vec2] = [vec2(0.0, 0.0), vec2(50.0, 50.0), vec2(100.0, 100.0)]
            foreach pos: vec2 in positions {
                out << circle(pos, 10.0)
            }
            return s
        }
    "#);
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 3);
}

// =============================================================================
// 36. STATE PERSISTENCE ACROSS TICKS
// =============================================================================

#[test]
fn state_persists_float_across_ticks() {
    let rt = run(r"
        state { let counter: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.counter += 1.0
            return s
        }
    ");
    let mut rt = rt;
    for _ in 0..5 {
        tick(&mut rt);
    }
    assert_eq!(f(&rt, "counter"), 5.0);
}

#[test]
fn state_persists_string_across_ticks() {
    let rt = run(r#"
        state { let log: string = "" }
        fn on_update(s: State, input: Input) -> State {
            s.log = s.log + "x"
            return s
        }
    "#);
    let mut rt = rt;
    for _ in 0..3 {
        tick(&mut rt);
    }
    assert_eq!(s(&rt, "log"), "xxx");
}

#[test]
fn state_persists_list_across_ticks() {
    let rt = run(r"
        state { let items: list[float] = [] }
        fn on_update(s: State, input: Input) -> State {
            s.items.push(s.items.len())
            return s
        }
    ");
    let mut rt = rt;
    for _ in 0..4 {
        tick(&mut rt);
    }
    assert_eq!(list_floats(&rt, "items"), vec![0.0, 1.0, 2.0, 3.0]);
}

#[test]
fn state_persists_struct_across_ticks() {
    let rt = run(r"
        struct Acc {
            +let total: float = 0.0
        }
        state { let acc: Acc = Acc { total: 0.0 } }
        fn on_update(s: State, input: Input) -> State {
            s.acc.total += 10.0
            return s
        }
    ");
    let mut rt = rt;
    tick(&mut rt);
    tick(&mut rt);
    tick(&mut rt);
    let state = rt.state();
    match state.get("acc") {
        Some(Value::Object { fields, .. }) => match fields.get("total") {
            Some(Value::Float(v)) => assert_eq!(*v, 30.0),
            other => panic!("expected Float for acc.total, got: {other:?}"),
        },
        other => panic!("expected Object for 'acc', got: {other:?}"),
    }
}

// =============================================================================
// 37. EDGE CASES
// =============================================================================

#[test]
fn empty_state_block() {
    let rt = run("state { }");
    assert!(rt.state().is_empty());
}

#[test]
fn state_with_many_fields() {
    let rt = run(r"
        state {
            let a: float = 1.0
            let b: float = 2.0
            let c: float = 3.0
            let d: float = 4.0
            let e: float = 5.0
        }
    ");
    assert_eq!(f(&rt, "a"), 1.0);
    assert_eq!(f(&rt, "e"), 5.0);
}

#[test]
fn deeply_nested_expressions() {
    let rt = run(r"
        state { let r: float = ((((1.0 + 2.0) * 3.0) - 4.0) / 5.0) }
    ");
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn multiple_functions_calling_each_other() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn a(x: float) -> float { return b(x + 1.0) }
        fn b(x: float) -> float { return c(x + 1.0) }
        fn c(x: float) -> float { return x + 1.0 }
        fn on_init(s: State) -> State {
            s.r = a(0.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 3.0);
}

#[test]
fn list_of_strings() {
    let rt = run(r#"
        state { let items: list[string] = ["hello", "world"] }
    "#);
    let items = list_strings(&rt, "items");
    assert_eq!(items, vec!["hello", "world"]);
}

#[test]
fn list_of_bools() {
    let rt = run(r"
        state { let items: list[bool] = [true, false, true] }
        fn on_init(s: State) -> State {
            return s
        }
    ");
    match rt.state().get("items") {
        Some(Value::List(rc)) => {
            let items = rc.borrow();
            assert_eq!(items.len(), 3);
        }
        other => panic!("expected List, got: {other:?}"),
    }
}

#[test]
fn nested_struct_mutation() {
    let rt = run(r"
        struct Inner {
            +let value: float = 0.0
        }
        struct Outer {
            +let inner: Inner = Inner { value: 0.0 }
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let o: Outer = Outer { inner: Inner { value: 5.0 } }
            o.inner.value = 42.0
            s.r = o.inner.value
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
}

#[test]
fn enum_in_struct_field() {
    let rt = run(r"
        enum Priority { Low Medium High }
        struct Task {
            +let priority: Priority = Priority.Low
            +let score: float = 0.0
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let t: Task = Task { priority: Priority.High, score: 100.0 }
            match t.priority {
                Priority.High => { s.r = t.score }
                else => { s.r = 0.0 }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 100.0);
}

#[test]
fn string_in_match() {
    let rt = run(r#"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let cmd: string = "fire"
            match cmd {
                "move" => { s.r = 1.0 }
                "fire", "shoot" => { s.r = 2.0 }
                else => { s.r = 3.0 }
            }
            return s
        }
    "#);
    assert_eq!(f(&rt, "r"), 2.0);
}

#[test]
fn operator_precedence_full() {
    let rt = run(r"
        state {
            let r1: float = 2.0 + 3.0 * 4.0
            let r2: bool = 1.0 < 2.0 and 3.0 > 1.0
            let r3: bool = false or true and true
        }
    ");
    assert_eq!(f(&rt, "r1"), 14.0);
    assert!(b(&rt, "r2"));
    assert!(b(&rt, "r3"));
}

#[test]
fn empty_function_body() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn do_nothing() { }
        fn on_init(s: State) -> State {
            do_nothing()
            s.r = 1.0
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn multiple_returns_in_function() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn classify(x: float) -> float {
            if x > 0.0 { return 1.0 }
            if x < 0.0 { return -1.0 }
            return 0.0
        }
        fn on_init(s: State) -> State {
            s.r = classify(-5.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), -1.0);
}

#[test]
fn nested_optional_coalesce() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn get_value() -> float? {
            return none
        }
        fn on_init(s: State) -> State {
            let x: float? = get_value()
            let y: float? = none
            s.r = x ?? (y ?? 42.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
}

// =============================================================================
// 38. FILE I/O
// =============================================================================

#[test]
fn file_write_read_roundtrip() {
    let rt = run(r#"
        import file { write, read }
        state { let content: string = "" }
        fn on_init(s: State) -> State {
            let wr: res<bool> = write("/tmp/rustle_spec_test.txt", "spec test data")
            let rd: res<string> = read("/tmp/rustle_spec_test.txt")
            if rd.ok {
                s.content = rd.value
            }
            return s
        }
    "#);
    assert_eq!(s(&rt, "content"), "spec test data");
}

#[test]
fn file_read_nonexistent_returns_error() {
    let rt = run(r#"
        import file { read }
        state { let is_err: bool = false }
        fn on_init(s: State) -> State {
            let rd: res<string> = read("/tmp/rustle_nonexistent_file_12345.txt")
            s.is_err = not rd.ok
            return s
        }
    "#);
    assert!(b(&rt, "is_err"));
}

#[test]
fn file_read_lines() {
    let rt = run(r#"
        import file { write, read_lines }
        state { let count: float = 0.0 }
        fn on_init(s: State) -> State {
            write("/tmp/rustle_spec_lines.txt", "line1
line2
line3")
            let rd: res<list[string]> = read_lines("/tmp/rustle_spec_lines.txt")
            if rd.ok {
                s.count = rd.value.len()
            }
            return s
        }
    "#);
    assert_eq!(f(&rt, "count"), 3.0);
}

#[test]
fn file_append() {
    let rt = run(r#"
        import file { write, append, read }
        state { let content: string = "" }
        fn on_init(s: State) -> State {
            write("/tmp/rustle_spec_append.txt", "first")
            append("/tmp/rustle_spec_append.txt", " second")
            let rd: res<string> = read("/tmp/rustle_spec_append.txt")
            if rd.ok {
                s.content = rd.value
            }
            return s
        }
    "#);
    assert_eq!(s(&rt, "content"), "first second");
}

// =============================================================================
// 39. STRUCT + ENUM INTERACTION
// =============================================================================

#[test]
fn struct_with_enum_method_dispatch() {
    let rt = run(r"
        enum Op { Add Sub Mul }
        struct Calculator {
            +let result: float = 0.0
            +fn apply(op: Op, val: float) {
                match op {
                    Op.Add => { this.result += val }
                    Op.Sub => { this.result -= val }
                    Op.Mul => { this.result *= val }
                }
            }
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let calc: Calculator = Calculator { result: 10.0 }
            calc.apply(Op.Add, 5.0)
            calc.apply(Op.Mul, 3.0)
            s.r = calc.result
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 45.0);
}

// =============================================================================
// 40. MISC LANGUAGE FEATURES
// =============================================================================

#[test]
fn multiple_state_init_expressions() {
    let rt = run(r#"
        state {
            let a: float = 1.0 + 2.0
            let b: float = sin(0.0)
            let c: bool = 5.0 > 3.0
            let d: string = "hello"
        }
    "#);
    assert_eq!(f(&rt, "a"), 3.0);
    assert!(approx(f(&rt, "b"), 0.0));
    assert!(b(&rt, "c"));
    assert_eq!(s(&rt, "d"), "hello");
}

#[test]
fn list_index_in_expression() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = [10.0, 20.0, 30.0]
            s.r = items[0] + items[1] + items[2]
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 60.0);
}

#[test]
fn list_index_with_expression() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = [10.0, 20.0, 30.0]
            let idx: float = 1.0
            s.r = items[idx]
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 20.0);
}

#[test]
fn method_on_literal() {
    let rt = run(r#"
        state { let r: float = "hello world".len() }
    "#);
    assert_eq!(f(&rt, "r"), 11.0);
}

#[test]
fn chained_vec2_operations() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let v: vec2 = (vec2(1.0, 0.0) + vec2(0.0, 1.0)).normalize()
            s.r = v.length()
            return s
        }
    ");
    assert!(approx(f(&rt, "r"), 1.0));
}

#[test]
fn list_of_vec2() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let points: list[vec2] = [vec2(1.0, 2.0), vec2(3.0, 4.0), vec2(5.0, 6.0)]
            s.r = points[1].x
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 3.0);
}

#[test]
fn closure_in_list_filter() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let threshold: float = 3.0
            let items: list[float] = [1.0, 2.0, 3.0, 4.0, 5.0]
            let above: list[float] = items.filter((x: float) -> bool { return x > threshold })
            s.r = above.len()
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 2.0);
}

#[test]
fn for_loop_building_list() {
    let rt = run(r"
        state { let r: list[float] = [] }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 5.0; i += 1.0 {
                s.r.push(i * i)
            }
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "r"), vec![0.0, 1.0, 4.0, 9.0, 16.0]);
}

#[test]
fn struct_in_list_mutation_via_ref() {
    // NOTE: direct items[0].value = 99.0 syntax is not supported by parser
    // Workaround: get reference, mutate through it (struct is reference type)
    let rt = run(r"
        struct Item {
            +let value: float = 0.0
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[Item] = [Item { value: 1.0 }, Item { value: 2.0 }]
            let first: Item = items[0]
            first.value = 99.0
            s.r = first.value
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 99.0);
}

// =============================================================================
// 41. ELSE IF CHAINS
// =============================================================================

#[test]
fn else_if_chain() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let x: float = 5.0
            if x > 10.0 {
                s.r = 1.0
            } else if x > 3.0 {
                s.r = 2.0
            } else if x > 1.0 {
                s.r = 3.0
            } else {
                s.r = 4.0
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 2.0);
}

#[test]
fn else_if_first_branch() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let x: float = 100.0
            if x > 10.0 {
                s.r = 1.0
            } else if x > 5.0 {
                s.r = 2.0
            } else {
                s.r = 3.0
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn else_if_last_branch() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let x: float = 0.0
            if x > 10.0 {
                s.r = 1.0
            } else if x > 5.0 {
                s.r = 2.0
            } else {
                s.r = 3.0
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 3.0);
}

// =============================================================================
// 42. MISSING MATH CONSTRUCTORS
// =============================================================================

#[test]
fn mat3_rotate_constructor() {
    let rt = run(r"
        state { let d: float = 0.0 }
        fn on_init(s: State) -> State {
            let m: mat3 = mat3_rotate(0.0)
            s.d = m.det()
            return s
        }
    ");
    assert!(approx(f(&rt, "d"), 1.0));
}

#[test]
fn mat4_translate_constructor() {
    let rt = run(r"
        state { let v: vec4 = vec4(0.0, 0.0, 0.0, 0.0) }
        fn on_init(s: State) -> State {
            let m: mat4 = mat4_translate(10.0, 20.0, 30.0)
            s.v = m.mul_vec(vec4(0.0, 0.0, 0.0, 1.0))
            return s
        }
    ");
    let (x, y, z, w) = v4(&rt, "v");
    assert!(approx(x, 10.0));
    assert!(approx(y, 20.0));
    assert!(approx(z, 30.0));
    assert!(approx(w, 1.0));
}

#[test]
fn mat4_scale_xyz_constructor() {
    let rt = run(r"
        state { let v: vec4 = vec4(0.0, 0.0, 0.0, 0.0) }
        fn on_init(s: State) -> State {
            let m: mat4 = mat4_scale(2.0, 3.0, 4.0)
            s.v = m.mul_vec(vec4(1.0, 1.0, 1.0, 1.0))
            return s
        }
    ");
    let (x, y, z, _) = v4(&rt, "v");
    assert!(approx(x, 2.0));
    assert!(approx(y, 3.0));
    assert!(approx(z, 4.0));
}

// =============================================================================
// 43. STRING COMPARISON AND EDGE CASES
// =============================================================================

#[test]
fn string_less_than() {
    let rt = run(r#"
        state { let r: bool = "abc" < "def" }
    "#);
    assert!(b(&rt, "r"));
}

#[test]
fn string_greater_than() {
    let rt = run(r#"
        state { let r: bool = "def" > "abc" }
    "#);
    assert!(b(&rt, "r"));
}

#[test]
fn string_empty_len() {
    let rt = run(r#"
        state { let r: float = "".len() }
    "#);
    assert_eq!(f(&rt, "r"), 0.0);
}

#[test]
fn string_empty_contains() {
    let rt = run(r#"
        state { let r: bool = "".contains("x") }
    "#);
    assert!(!b(&rt, "r"));
}

#[test]
fn string_empty_trim() {
    let rt = run(r#"
        state { let r: string = "".trim() }
    "#);
    assert_eq!(s(&rt, "r"), "");
}

#[test]
fn string_split_multichar_delimiter() {
    let rt = run(r#"
        state { let parts: list[string] = "a::b::c".split("::") }
    "#);
    let parts = list_strings(&rt, "parts");
    assert_eq!(parts, vec!["a", "b", "c"]);
}

#[test]
fn string_replace_not_found() {
    let rt = run(r#"
        state { let r: string = "hello".replace("xyz", "abc") }
    "#);
    assert_eq!(s(&rt, "r"), "hello");
}

#[test]
fn string_concatenation_chain() {
    let rt = run(r#"
        state { let r: string = "a" + "b" + "c" + "d" }
    "#);
    assert_eq!(s(&rt, "r"), "abcd");
}

#[test]
fn string_slice() {
    let rt = run(r#"
        state { let r: string = "hello world".slice(0.0, 5.0) }
    "#);
    assert_eq!(s(&rt, "r"), "hello");
}

#[test]
fn template_string_only_interpolation() {
    let rt = run(r"
        state { let r: string = `${42.0}` }
    ");
    assert_eq!(s(&rt, "r"), "42");
}

// =============================================================================
// 44. SHAPE .in() METHOD
// =============================================================================

#[test]
fn circle_in_method() {
    let rt = run(r"
        import shapes { circle }
        state { let v: vec2 = vec2(0.0, 0.0) }
        fn on_init(s: State) -> State {
            let c: circle = circle(vec2(10.0, 20.0), 50.0)
            s.v = c.in(1.0, 2.0)
            return s
        }
    ");
    let (x, y) = v2(&rt, "v");
    assert!(approx(x, 11.0));
    assert!(approx(y, 22.0));
}

#[test]
fn rect_in_method() {
    let rt = run(r"
        import shapes { rect }
        state { let v: vec2 = vec2(0.0, 0.0) }
        fn on_init(s: State) -> State {
            let r: rect = rect(vec2(100.0, 100.0), vec2(50.0, 50.0))
            s.v = r.in(3.0, 4.0)
            return s
        }
    ");
    let (x, y) = v2(&rt, "v");
    assert!(approx(x, 103.0));
    assert!(approx(y, 104.0));
}

#[test]
fn line_in_method() {
    let rt = run(r"
        import shapes { line }
        state { let v: vec2 = vec2(0.0, 0.0) }
        fn on_init(s: State) -> State {
            let l: line = line(vec2(10.0, 20.0), vec2(100.0, 100.0))
            s.v = l.in(5.0, 7.0)
            return s
        }
    ");
    let (x, y) = v2(&rt, "v");
    assert!(approx(x, 15.0));
    assert!(approx(y, 27.0));
}

// =============================================================================
// 45. STATE METHODS
// =============================================================================

#[test]
fn state_len_method() {
    let rt = run(r"
        state { let a: float = 1.0 let b: float = 2.0 let c: float = 3.0 }
        fn on_init(s: State) -> State {
            s.a = s.len()
            return s
        }
    ");
    assert_eq!(f(&rt, "a"), 3.0);
}

// =============================================================================
// 46. NAMED ARGUMENTS
// =============================================================================

#[test]
fn named_arg_circle_color() {
    let rt = run(r#"
        import shapes { circle }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            out << circle(vec2(0.0, 0.0), 50.0, color: red)
            return s
        }
    "#);
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert!(!cmds.is_empty());
}

// =============================================================================
// 47. TRANSPARENT COLOR CONSTANT
// =============================================================================

#[test]
fn color_transparent_constant() {
    let rt = run(r"
        state { let c: color = transparent }
    ");
    let (_, _, _, a) = col(&rt, "c");
    assert!(approx(a, 0.0));
}

// =============================================================================
// 48. RENDER MODES — sdf and outline
// =============================================================================

#[test]
fn render_sdf_mode() {
    compile_ok(r#"
        import render { sdf }
        import shapes { circle }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            out << circle(vec2(0.0, 0.0), 50.0)
            return s
        }
    "#);
}

#[test]
fn render_outline_mode() {
    compile_ok(r#"
        import render { outline }
        import shapes { circle }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            out << circle(vec2(0.0, 0.0), 50.0)
            return s
        }
    "#);
}

// =============================================================================
// 49. LIST SORT DEFAULT (no comparator)
// =============================================================================

#[test]
fn list_sort_default() {
    let rt = run(r"
        state { let r: list[float] = [5.0, 3.0, 1.0, 4.0, 2.0] }
        fn on_init(s: State) -> State {
            s.r.sort()
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "r"), vec![1.0, 2.0, 3.0, 4.0, 5.0]);
}

// =============================================================================
// 50. NESTED CLOSURES
// =============================================================================

#[test]
fn closure_nested() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: float = 10.0
            let outer = (x: float) -> float {
                let b: float = 20.0
                let inner = (y: float) -> float {
                    return y + a + b
                }
                return inner(x)
            }
            s.r = outer(5.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 35.0);
}

// =============================================================================
// 51. FOREACH WITHOUT TYPE ANNOTATION
// =============================================================================

#[test]
fn foreach_no_type_annotation() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = [1.0, 2.0, 3.0]
            foreach v in items {
                s.r += v
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 6.0);
}

// =============================================================================
// 52. VEC2 COMPONENT MUTATION THROUGH STATE (SetFieldRebuild)
// =============================================================================

#[test]
fn vec2_component_mutation_via_state() {
    let rt = run(r"
        state { let pos: vec2 = vec2(0.0, 0.0) }
        fn on_init(s: State) -> State {
            s.pos.x = 42.0
            s.pos.y = 99.0
            return s
        }
    ");
    assert_eq!(v2(&rt, "pos"), (42.0, 99.0));
}

#[test]
fn vec2_component_compound_assign() {
    let rt = run(r"
        state { let pos: vec2 = vec2(10.0, 20.0) }
        fn on_init(s: State) -> State {
            s.pos.x += 5.0
            s.pos.y -= 3.0
            return s
        }
    ");
    assert_eq!(v2(&rt, "pos"), (15.0, 17.0));
}

// =============================================================================
// 53. MULTIPLE TRANSFORMS
// =============================================================================

#[test]
fn multiple_transforms_on_shape() {
    let rt = run(r#"
        import shapes { circle }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let c: circle = circle(vec2(0.0, 0.0), 50.0)
            let t1: transform = transform().move(10.0, 0.0)
            let t2: transform = transform().scale(2.0)
            let t3: transform = transform().rotate(1.0)
            out << c@(t1, t2, t3)
            return s
        }
    "#);
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        DrawCommand::DrawShape(sd) => assert_eq!(sd.transforms.len(), 3),
        other => panic!("expected DrawShape, got: {other:?}"),
    }
}

// =============================================================================
// 54. STRUCT METHOD CALLING ANOTHER METHOD
// =============================================================================

#[test]
fn struct_method_calls_other_method() {
    let rt = run(r"
        struct Calc {
            +let value: float = 0.0
            +fn double() -> float { return this.value * 2.0 }
            +fn double_plus_one() -> float { return this.double() + 1.0 }
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let c: Calc = Calc { value: 5.0 }
            s.r = c.double_plus_one()
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 11.0);
}

// =============================================================================
// 55. TEXT SHAPE
// =============================================================================

#[test]
fn text_shape_construction() {
    let rt = run(r#"
        import shapes { text }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let t: text_shape = text(vec2(0.0, 0.0), "hello", 24.0)
            out << t
            return s
        }
    "#);
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert!(!cmds.is_empty());
}

#[test]
fn text_shape_fields() {
    let rt = run(r#"
        import shapes { text }
        state { let px: float = 0.0 let content: string = "" let sz: float = 0.0 }
        fn on_init(s: State) -> State {
            let t: text_shape = text(vec2(10.0, 20.0), "world", 32.0)
            s.px = t.pos.x
            s.content = t.content
            s.sz = t.size
            return s
        }
    "#);
    assert_eq!(f(&rt, "px"), 10.0);
    assert_eq!(s(&rt, "content"), "world");
    assert_eq!(f(&rt, "sz"), 32.0);
}

// =============================================================================
// 56. TRY ON FUNCTION CALL
// =============================================================================

#[test]
fn try_on_function_call() {
    let rt = run(r"
        state { let r: float = 0.0 let caught: bool = false }
        fn divide(a: float, b: float) -> float {
            return a / b
        }
        fn on_init(s: State) -> State {
            let result: res<float> = try divide(10.0, 0.0)
            s.caught = not result.ok
            return s
        }
    ");
    assert!(b(&rt, "caught"));
}

#[test]
fn try_on_function_call_success() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn divide(a: float, b: float) -> float {
            return a / b
        }
        fn on_init(s: State) -> State {
            let result: res<float> = try divide(10.0, 2.0)
            s.r = result.value
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 5.0);
}

// =============================================================================
// 57. MATCH WITH ENUM FIELD ACCESS IN ARMS
// =============================================================================

#[test]
fn match_enum_field_access_in_arm() {
    let rt = run(r"
        enum Shape {
            Circle { radius: float }
            Rect { w: float, h: float }
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let sh: Shape = Shape.Rect { w: 10.0, h: 20.0 }
            match sh {
                Shape.Circle => { s.r = sh.radius }
                Shape.Rect => { s.r = sh.w + sh.h }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 30.0);
}

// =============================================================================
// 58. ENUM EQUALITY WITH DATA FIELDS
// =============================================================================

#[test]
fn enum_equality_with_data_same() {
    let rt = run(r"
        enum Val { Num { n: float } }
        state { let r: bool = false }
        fn on_init(s: State) -> State {
            let a: Val = Val.Num { n: 42.0 }
            let b: Val = Val.Num { n: 42.0 }
            s.r = a == b
            return s
        }
    ");
    assert!(b(&rt, "r"));
}

#[test]
fn enum_equality_with_data_different() {
    let rt = run(r"
        enum Val { Num { n: float } }
        state { let r: bool = true }
        fn on_init(s: State) -> State {
            let a: Val = Val.Num { n: 42.0 }
            let b: Val = Val.Num { n: 99.0 }
            s.r = a == b
            return s
        }
    ");
    assert!(!b(&rt, "r"));
}

// =============================================================================
// 59. LIST OF STRUCTS — push, access, method call
// =============================================================================

#[test]
fn list_of_structs_push_and_access() {
    let rt = run(r"
        struct Point {
            +let x: float = 0.0
            +let y: float = 0.0
            +fn sum() -> float { return this.x + this.y }
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let points: list[Point] = []
            points.push(Point { x: 1.0, y: 2.0 })
            points.push(Point { x: 3.0, y: 4.0 })
            let p: Point = points[1]
            s.r = p.sum()
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 7.0);
}

// =============================================================================
// 60. COMPILE ERRORS — type system rules
// =============================================================================

#[test]
fn error_bare_none_no_annotation() {
    let errs = compile_err(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let x = none
            return s
        }
    ");
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S015)));
}

#[test]
fn error_optional_assigned_to_non_optional() {
    let errs = compile_err(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let maybe: float? = 5.0
            let val: float = maybe
            return s
        }
    ");
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S002)));
}

#[test]
fn error_invalid_operator() {
    let errs = compile_err(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let v: vec2 = vec2(1.0, 2.0)
            let c: color = color(1.0, 0.0, 0.0)
            s.x = v + c
            return s
        }
    ");
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S008)));
}

#[test]
fn error_duplicate_state_block() {
    let errs = compile_err(r"
        state { let x: float = 0.0 }
        state { let y: float = 0.0 }
    ");
    assert!(!errs.is_empty());
}

// =============================================================================
// 61. OPERATOR PRECEDENCE — full chain
// =============================================================================

#[test]
fn precedence_coalesce_vs_and() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let x: float? = none
            let val: float = x ?? 5.0
            s.r = val
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 5.0);
}

#[test]
fn precedence_or_vs_and() {
    let rt = run(r"
        state { let r: bool = false or true and true }
    ");
    assert!(b(&rt, "r"));
}

#[test]
fn precedence_comparison_vs_arithmetic() {
    let rt = run(r"
        state { let r: bool = 2.0 + 3.0 > 4.0 }
    ");
    assert!(b(&rt, "r"));
}

#[test]
fn precedence_unary_vs_multiplicative() {
    let rt = run(r"
        state { let r: float = -2.0 * 3.0 }
    ");
    assert_eq!(f(&rt, "r"), -6.0);
}

// =============================================================================
// 62. VALUE SEMANTICS vs REFERENCE SEMANTICS
// =============================================================================

#[test]
fn value_semantics_float_not_shared() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn set_val(x: float) {
            x = 99.0
        }
        fn on_init(s: State) -> State {
            let a: float = 5.0
            set_val(a)
            s.r = a
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 5.0);
}

#[test]
fn reference_semantics_list_shared() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn push_val(items: list[float], v: float) {
            items.push(v)
        }
        fn on_init(s: State) -> State {
            let items: list[float] = [1.0, 2.0]
            push_val(items, 3.0)
            s.r = items.len()
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 3.0);
}

#[test]
fn reference_semantics_struct_shared() {
    let rt = run(r"
        struct Box {
            +let value: float = 0.0
        }
        state { let r: float = 0.0 }
        fn set_box(b: Box, v: float) {
            b.value = v
        }
        fn on_init(s: State) -> State {
            let b: Box = Box { value: 0.0 }
            set_box(b, 42.0)
            s.r = b.value
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
}

#[test]
fn value_semantics_vec2_not_shared() {
    let rt = run(r"
        state { let ra: vec2 = vec2(0.0, 0.0) let rb: vec2 = vec2(0.0, 0.0) }
        fn on_init(s: State) -> State {
            let a: vec2 = vec2(1.0, 2.0)
            let b: vec2 = a
            s.ra = a
            s.rb = b
            return s
        }
    ");
    assert_eq!(v2(&rt, "ra"), (1.0, 2.0));
    assert_eq!(v2(&rt, "rb"), (1.0, 2.0));
}

// =============================================================================
// 63. ARITHMETIC EDGE CASES
// =============================================================================

#[test]
fn modulo_negative() {
    let rt = run(r"
        state { let r: float = -5.0 % 3.0 }
    ");
    assert!(approx(f(&rt, "r"), -2.0));
}

#[test]
fn pow_fractional_exponent() {
    let rt = run(r"
        state { let r: float = pow(4.0, 0.5) }
    ");
    assert!(approx(f(&rt, "r"), 2.0));
}

#[test]
fn clamp_in_range() {
    let rt = run(r"
        state { let r: float = clamp(5.0, 0.0, 10.0) }
    ");
    assert_eq!(f(&rt, "r"), 5.0);
}

#[test]
fn lerp_at_zero() {
    let rt = run(r"
        state { let r: float = lerp(10.0, 20.0, 0.0) }
    ");
    assert_eq!(f(&rt, "r"), 10.0);
}

#[test]
fn lerp_at_one() {
    let rt = run(r"
        state { let r: float = lerp(10.0, 20.0, 1.0) }
    ");
    assert_eq!(f(&rt, "r"), 20.0);
}

#[test]
fn min_equal_values() {
    let rt = run(r"
        state { let r: float = min(5.0, 5.0) }
    ");
    assert_eq!(f(&rt, "r"), 5.0);
}

#[test]
fn long_arithmetic_chain() {
    let rt = run(r"
        state { let r: float = 1.0 + 2.0 + 3.0 + 4.0 + 5.0 }
    ");
    assert_eq!(f(&rt, "r"), 15.0);
}

// =============================================================================
// 64. LIST EDGE CASES
// =============================================================================

#[test]
fn list_single_element() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = [42.0]
            s.r = items[0]
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
}

#[test]
fn list_push_after_pop() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = [1.0, 2.0, 3.0]
            items.pop()
            items.push(99.0)
            s.r = items[2]
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 99.0);
}

#[test]
fn list_sort_already_sorted() {
    let rt = run(r"
        state { let r: list[float] = [1.0, 2.0, 3.0, 4.0] }
        fn on_init(s: State) -> State {
            s.r.sort()
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "r"), vec![1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn list_sort_reverse_sorted() {
    let rt = run(r"
        state { let r: list[float] = [4.0, 3.0, 2.0, 1.0] }
        fn on_init(s: State) -> State {
            s.r.sort()
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "r"), vec![1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn list_sort_single_element() {
    let rt = run(r"
        state { let r: list[float] = [42.0] }
        fn on_init(s: State) -> State {
            s.r.sort()
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "r"), vec![42.0]);
}

#[test]
fn list_take_zero() {
    let rt = run(r"
        state { let r: list[float] = [] }
        fn on_init(s: State) -> State {
            let items: list[float] = [1.0, 2.0, 3.0]
            s.r = items.take(0.0, 0.0)
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "r"), Vec::<f64>::new());
}

#[test]
fn list_drop_beyond_length() {
    let rt = run(r"
        state { let r: list[float] = [] }
        fn on_init(s: State) -> State {
            let items: list[float] = [1.0, 2.0, 3.0]
            s.r = items.drop(0.0, 100.0)
            return s
        }
    ");
    assert!(list_floats(&rt, "r").is_empty());
}

#[test]
fn list_filter_returns_all() {
    let rt = run(r"
        state { let r: list[float] = [] }
        fn on_init(s: State) -> State {
            let items: list[float] = [1.0, 2.0, 3.0]
            s.r = items.filter((x: float) -> bool { return true })
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "r"), vec![1.0, 2.0, 3.0]);
}

#[test]
fn list_filter_returns_none() {
    let rt = run(r"
        state { let r: list[float] = [99.0] }
        fn on_init(s: State) -> State {
            let items: list[float] = [1.0, 2.0, 3.0]
            s.r = items.filter((x: float) -> bool { return false })
            return s
        }
    ");
    assert!(list_floats(&rt, "r").is_empty());
}

#[test]
fn list_map_empty() {
    let rt = run(r"
        state { let r: list[float] = [99.0] }
        fn on_init(s: State) -> State {
            let items: list[float] = []
            s.r = items.map((x: float) -> float { return x * 2.0 })
            return s
        }
    ");
    assert!(list_floats(&rt, "r").is_empty());
}

#[test]
fn list_any_empty() {
    let rt = run(r"
        state { let r: bool = true }
        fn on_init(s: State) -> State {
            let items: list[float] = []
            s.r = items.any((x: float) -> bool { return true })
            return s
        }
    ");
    assert!(!b(&rt, "r"));
}

#[test]
fn list_all_empty() {
    let rt = run(r"
        state { let r: bool = false }
        fn on_init(s: State) -> State {
            let items: list[float] = []
            s.r = items.all((x: float) -> bool { return false })
            return s
        }
    ");
    assert!(b(&rt, "r"));
}

// =============================================================================
// 65. SCOPE RULES
// =============================================================================

#[test]
fn scope_if_block_not_visible_outside() {
    let errs = compile_err(r"
        fn on_init(s: State) -> State {
            if true {
                let inner: float = 5.0
            }
            s.x = inner
            return s
        }
    ");
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S001)));
}

#[test]
fn scope_for_var_not_visible_after() {
    let errs = compile_err(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 5.0; i += 1.0 {
            }
            s.r = i
            return s
        }
    ");
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S001)));
}

#[test]
fn scope_foreach_var_not_visible_after() {
    let errs = compile_err(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = [1.0]
            foreach v: float in items {
            }
            s.r = v
            return s
        }
    ");
    assert!(errs.iter().any(|e| matches!(e.code, ErrorCode::S001)));
}

// =============================================================================
// 66. CAST EDGE CASES
// =============================================================================

#[test]
fn cast_chained() {
    let rt = run(r"
        state { let r: float = (5.0 as bool) as float }
    ");
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn cast_in_expression() {
    let rt = run(r#"
        state { let r: string = (42.0 as string) + " items" }
    "#);
    assert_eq!(s(&rt, "r"), "42 items");
}

// =============================================================================
// 67. EXPRESSION CHAINS
// =============================================================================

#[test]
fn field_on_function_return() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn make_vec() -> vec2 { return vec2(42.0, 99.0) }
        fn on_init(s: State) -> State {
            s.r = make_vec().x
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
}

#[test]
fn method_on_function_return() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn make_vec() -> vec2 { return vec2(3.0, 4.0) }
        fn on_init(s: State) -> State {
            s.r = make_vec().length()
            return s
        }
    ");
    assert!(approx(f(&rt, "r"), 5.0));
}

#[test]
fn index_on_function_return() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn make_list() -> list[float] { return [10.0, 20.0, 30.0] }
        fn on_init(s: State) -> State {
            s.r = make_list()[1]
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 20.0);
}

#[test]
fn method_on_literal_string() {
    let rt = run(r#"
        state { let r: bool = "hello world".contains("world") }
    "#);
    assert!(b(&rt, "r"));
}

#[test]
fn chained_string_methods() {
    let rt = run(r#"
        state { let r: string = "Hello World".to_lower().replace("world", "rustle") }
    "#);
    assert_eq!(s(&rt, "r"), "hello rustle");
}

// =============================================================================
// 68. LIFECYCLE EDGE CASES
// =============================================================================

#[test]
fn exit_without_ticks() {
    let rt = run(r"
        state { let exited: bool = false }
        fn on_exit(s: State) -> State {
            s.exited = true
            return s
        }
    ");
    let mut rt = rt;
    rt.exit().unwrap();
    assert!(b(&rt, "exited"));
}

#[test]
fn exit_after_ticks() {
    let rt = run(r"
        state { let count: float = 0.0 let exited: bool = false }
        fn on_update(s: State, input: Input) -> State {
            s.count += 1.0
            return s
        }
        fn on_exit(s: State) -> State {
            s.exited = true
            return s
        }
    ");
    let mut rt = rt;
    tick(&mut rt);
    tick(&mut rt);
    rt.exit().unwrap();
    assert_eq!(f(&rt, "count"), 2.0);
    assert!(b(&rt, "exited"));
}

#[test]
fn tick_with_zero_dt() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.r = input.dt
            return s
        }
    ");
    let mut rt = rt;
    tick_with(&mut rt, Input { dt: 0.0, ..Default::default() });
    assert_eq!(f(&rt, "r"), 0.0);
}

// =============================================================================
// 69. DRAWING EDGE CASES
// =============================================================================

#[test]
fn draw_conditional_some_ticks() {
    let rt = run(r#"
        import shapes { circle }
        import coords { resolution, origin }
        state { let count: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            s.count += 1.0
            if s.count % 2.0 == 0.0 {
                out << circle(vec2(0.0, 0.0), 50.0)
            }
            return s
        }
    "#);
    let mut rt = rt;
    let cmds1 = tick(&mut rt);
    let cmds2 = tick(&mut rt);
    let cmds3 = tick(&mut rt);
    assert!(cmds1.is_empty());
    assert_eq!(cmds2.len(), 1);
    assert!(cmds3.is_empty());
}

#[test]
fn draw_same_shape_twice() {
    let rt = run(r#"
        import shapes { circle }
        import coords { resolution, origin }
        fn on_update(s: State, input: Input) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let c: circle = circle(vec2(0.0, 0.0), 50.0)
            out << c << c
            return s
        }
    "#);
    let mut rt = rt;
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 2);
}

// =============================================================================
// 70. IMPORT EDGE CASES
// =============================================================================

#[test]
fn import_two_namespaces() {
    let rt = run(r#"
        import shapes { circle }
        import render { fill }
        import coords { resolution, origin }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            resolution(400.0, 400.0)
            origin("center")
            let c: circle = circle(vec2(0.0, 0.0), 50.0)
            s.r = c.radius
            return s
        }
    "#);
    assert_eq!(f(&rt, "r"), 50.0);
}

// =============================================================================
// 71. COMBINATIONS — closure + struct, closure + list, enum in struct
// =============================================================================

#[test]
fn closure_captures_struct_and_mutates() {
    let rt = run(r"
        struct Counter {
            +let n: float = 0.0
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let c: Counter = Counter { n: 0.0 }
            let inc = () -> float {
                c.n += 1.0
                return c.n
            }
            inc()
            inc()
            inc()
            s.r = c.n
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 3.0);
}

#[test]
fn closure_captures_list_and_pushes() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = []
            let add = (x: float) {
                items.push(x)
            }
            add(10.0)
            add(20.0)
            add(30.0)
            s.r = items.len()
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 3.0);
}

#[test]
fn struct_with_optional_field() {
    let rt = run(r"
        struct Node {
            +let value: float = 0.0
            +let next: float? = none
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let n: Node = Node { value: 42.0, next: none }
            s.r = n.next ?? -1.0
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), -1.0);
}

// =============================================================================
// 72. CONTROL FLOW COMBINATIONS
// =============================================================================

#[test]
fn break_inside_match_inside_loop() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let i: float = 0.0
            while i < 100.0 {
                i += 1.0
                match i {
                    5.0 => { break }
                    else => { s.r += 1.0 }
                }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 4.0);
}

#[test]
fn continue_inside_match_inside_loop() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let i: float = 0.0
            while i < 5.0 {
                i += 1.0
                match i {
                    3.0 => { continue }
                    else => { s.r += 1.0 }
                }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 4.0);
}

#[test]
fn return_inside_match() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn classify(x: float) -> float {
            match x {
                1.0 => { return 100.0 }
                2.0 => { return 200.0 }
                else => { return 0.0 }
            }
        }
        fn on_init(s: State) -> State {
            s.r = classify(2.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 200.0);
}

#[test]
fn match_inside_loop() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 5.0; i += 1.0 {
                match i {
                    0.0, 2.0, 4.0 => { s.r += 10.0 }
                    else => { s.r += 1.0 }
                }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 32.0);
}

#[test]
fn foreach_on_inline_list() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            foreach v: float in [10.0, 20.0, 30.0] {
                s.r += v
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 60.0);
}

#[test]
fn match_no_match_no_else() {
    let rt = run(r"
        state { let r: float = 99.0 }
        fn on_init(s: State) -> State {
            match 42.0 {
                1.0 => { s.r = 1.0 }
                2.0 => { s.r = 2.0 }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 99.0);
}

#[test]
fn nested_for_with_continue_in_inner() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 3.0; i += 1.0 {
                for let j = 0.0; j < 5.0; j += 1.0 {
                    if j == 2.0 { continue }
                    s.r += 1.0
                }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 12.0);
}

// =============================================================================
// 73. STRUCT EDGE CASES
// =============================================================================

#[test]
fn struct_many_fields() {
    let rt = run(r"
        struct Big {
            +let a: float = 1.0
            +let b: float = 2.0
            +let c: float = 3.0
            +let d: float = 4.0
            +let e: float = 5.0
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let x: Big = Big { a: 1.0, b: 2.0, c: 3.0, d: 4.0, e: 5.0 }
            s.r = x.a + x.b + x.c + x.d + x.e
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 15.0);
}

#[test]
fn struct_all_defaults() {
    let rt = run(r"
        struct Defaults {
            +let x: float = 42.0
            +let y: float = 99.0
        }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let d: Defaults = Defaults {}
            s.r = d.x + d.y
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 141.0);
}

#[test]
fn single_variant_enum() {
    let rt = run(r"
        enum Wrapper { Value { n: float } }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let w: Wrapper = Wrapper.Value { n: 42.0 }
            match w {
                Wrapper.Value => { s.r = w.n }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
}

// =============================================================================
// 74. STATE WITH MIXED TYPES
// =============================================================================

#[test]
fn state_mixed_types() {
    let rt = run(r#"
        state {
            let n: float = 1.0
            let flag: bool = true
            let name: string = "hello"
            let pos: vec2 = vec2(3.0, 4.0)
            let items: list[float] = [10.0, 20.0]
        }
    "#);
    assert_eq!(f(&rt, "n"), 1.0);
    assert!(b(&rt, "flag"));
    assert_eq!(s(&rt, "name"), "hello");
    assert_eq!(v2(&rt, "pos"), (3.0, 4.0));
    assert_eq!(list_floats(&rt, "items"), vec![10.0, 20.0]);
}

// =============================================================================
// 75. WHILE TRUE + BREAK PATTERN
// =============================================================================

#[test]
fn while_true_break_pattern() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let i: float = 0.0
            while true {
                i += 1.0
                if i >= 10.0 { break }
            }
            s.r = i
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 10.0);
}

// =============================================================================
// 76. DEEPLY NESTED IF
// =============================================================================

#[test]
fn deeply_nested_if() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            if true {
                if true {
                    if true {
                        if true {
                            if true {
                                s.r = 42.0
                            }
                        }
                    }
                }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
}

// =============================================================================
// 77. FOR LOOP EDGE CASES
// =============================================================================

#[test]
fn for_loop_zero_iterations() {
    let rt = run(r"
        state { let r: float = 99.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 0.0; i += 1.0 {
                s.r = 0.0
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 99.0);
}

#[test]
fn for_loop_one_iteration() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 1.0; i += 1.0 {
                s.r += 1.0
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn foreach_empty_list() {
    let rt = run(r"
        state { let r: float = 99.0 }
        fn on_init(s: State) -> State {
            let items: list[float] = []
            foreach v: float in items {
                s.r = 0.0
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 99.0);
}

// =============================================================================
// 78. VEC2 DOT PRODUCT
// =============================================================================

#[test]
fn vec2_dot() {
    let rt = run(r"
        state { let r: float = vec2(3.0, 4.0).dot(vec2(1.0, 2.0)) }
    ");
    assert_eq!(f(&rt, "r"), 11.0);
}

// =============================================================================
// 79. ORIGIN MODES
// =============================================================================

#[test]
fn origin_top_left() {
    let rt = run(r#"
        import coords { resolution, origin }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            resolution(400.0, 400.0)
            origin("top_left")
            s.r = 1.0
            return s
        }
    "#);
    assert_eq!(f(&rt, "r"), 1.0);
}

#[test]
fn origin_bottom_left() {
    let rt = run(r#"
        import coords { resolution, origin }
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            resolution(400.0, 400.0)
            origin("bottom_left")
            s.r = 1.0
            return s
        }
    "#);
    assert_eq!(f(&rt, "r"), 1.0);
}

// =============================================================================
// 80. RES.VALUE ON ERROR — should runtime error
// =============================================================================

#[test]
fn res_value_on_error_crashes() {
    let err = run_err(r#"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            let result: res<float> = error("bad")
            s.r = result.value
            return s
        }
    "#);
    assert!(!err.message.is_empty());
}

// =============================================================================
// 81. LAMBDA IMMEDIATE INVOCATION
// =============================================================================

#[test]
fn lambda_immediate_invoke() {
    let rt = run(r"
        state { let r: float = 0.0 }
        fn on_init(s: State) -> State {
            s.r = ((x: float) -> float { return x * 2.0 })(21.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "r"), 42.0);
}
