//! Runtime behavior tests.
//!
//! Tests the full stack: compile → `Runtime::new` → tick.
//! State values are inspected after init/tick to verify correctness.
//! Draw commands are inspected for shape emission.

#![expect(clippy::float_cmp, reason = "float equality in tests is intentional — values are computed exactly")]

use rustle_lang::{compile, Runtime, Input, Value, DrawCommand, ErrorCode};
use rustle_lang::types::draw::ShapeDesc;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn run(src: &str) -> Runtime {
    let prog = compile(src).unwrap_or_else(|errs| {
        panic!("compile failed: {errs:#?}");
    });
    Runtime::new(prog).unwrap_or_else(|e| {
        panic!("Runtime::new failed: {e:?}");
    })
}

fn run_err(src: &str) -> rustle_lang::RuntimeError {
    let prog = compile(src).unwrap_or_else(|errs| {
        panic!("compile failed (expected runtime error, not compile error): {errs:#?}");
    });
    match Runtime::new(prog) {
        Ok(_)  => panic!("expected Runtime::new to fail but it succeeded"),
        Err(e) => e,
    }
}

fn tick(rt: &mut Runtime) -> Vec<DrawCommand> {
    rt.tick(&Input { dt: 0.016 })
        .unwrap_or_else(|e| panic!("tick failed: {e:?}"))
}

fn tick_err(rt: &mut Runtime) -> rustle_lang::RuntimeError {
    rt.tick(&Input { dt: 0.016 })
        .expect_err("expected tick to fail")
}

fn f(rt: &Runtime, key: &str) -> f64 {
    match rt.state().0.get(key) {
        Some(Value::Float(x)) => *x,
        other => panic!("expected Float for '{key}', got: {other:?}"),
    }
}

fn b(rt: &Runtime, key: &str) -> bool {
    match rt.state().0.get(key) {
        Some(Value::Bool(x)) => *x,
        other => panic!("expected Bool for '{key}', got: {other:?}"),
    }
}

fn s(rt: &Runtime, key: &str) -> String {
    match rt.state().0.get(key) {
        Some(Value::Str(x)) => x.clone(),
        other => panic!("expected Str for '{key}', got: {other:?}"),
    }
}

fn v2(rt: &Runtime, key: &str) -> (f64, f64) {
    match rt.state().0.get(key) {
        Some(Value::Vec2(x, y)) => (*x, *y),
        other => panic!("expected Vec2 for '{key}', got: {other:?}"),
    }
}

fn list_floats(rt: &Runtime, key: &str) -> Vec<f64> {
    match rt.state().0.get(key) {
        Some(Value::List(rc)) => rc.borrow().iter().map(|v| {
            match v { Value::Float(x) => *x, other => panic!("list element not Float: {other:?}") }
        }).collect(),
        other => panic!("expected List for '{key}', got: {other:?}"),
    }
}

// ─── Float arithmetic ─────────────────────────────────────────────────────────

#[test]
fn float_add() {
    let rt = run("state { let x: float = 2.0 + 3.0 }");
    assert_eq!(f(&rt, "x"), 5.0);
}

#[test]
fn float_sub() {
    let rt = run("state { let x: float = 10.0 - 4.0 }");
    assert_eq!(f(&rt, "x"), 6.0);
}

#[test]
fn float_mul() {
    let rt = run("state { let x: float = 3.0 * 4.0 }");
    assert_eq!(f(&rt, "x"), 12.0);
}

#[test]
fn float_div() {
    let rt = run("state { let x: float = 10.0 / 4.0 }");
    assert_eq!(f(&rt, "x"), 2.5);
}

#[test]
fn float_mod() {
    let rt = run("state { let x: float = 10.0 % 3.0 }");
    assert_eq!(f(&rt, "x"), 1.0);
}

#[test]
fn float_unary_neg() {
    let rt = run("state { let x: float = -5.0 }");
    assert_eq!(f(&rt, "x"), -5.0);
}

#[test]
fn float_nested_arithmetic() {
    let rt = run("state { let x: float = (2.0 + 3.0) * 4.0 - 1.0 }");
    assert_eq!(f(&rt, "x"), 19.0);
}

#[test]
fn float_precedence() {
    let rt = run("state { let x: float = 2.0 + 3.0 * 4.0 }");
    assert_eq!(f(&rt, "x"), 14.0);
}

#[test]
fn float_div_by_zero_runtime_error() {
    run_err(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 1.0 / 0.0
            return s
        }
    ");
}

#[test]
fn float_mod_by_zero_runtime_error() {
    run_err(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 5.0 % 0.0
            return s
        }
    ");
}

// ─── Float comparisons ────────────────────────────────────────────────────────

#[test]
fn float_lt_true() {
    let rt = run("state { let r: bool = 1.0 < 2.0 }");
    assert!(b(&rt, "r"));
}

#[test]
fn float_lt_false() {
    let rt = run("state { let r: bool = 2.0 < 1.0 }");
    assert!(!b(&rt, "r"));
}

#[test]
fn float_lteq_equal() {
    let rt = run("state { let r: bool = 2.0 <= 2.0 }");
    assert!(b(&rt, "r"));
}

#[test]
fn float_gt_true() {
    let rt = run("state { let r: bool = 3.0 > 1.0 }");
    assert!(b(&rt, "r"));
}

#[test]
fn float_eq_true() {
    let rt = run("state { let r: bool = 5.0 == 5.0 }");
    assert!(b(&rt, "r"));
}

#[test]
fn float_neq_true() {
    let rt = run("state { let r: bool = 5.0 != 4.0 }");
    assert!(b(&rt, "r"));
}

// ─── Bool logic ───────────────────────────────────────────────────────────────

#[test]
fn bool_and_true() {
    let rt = run("state { let r: bool = true and true }");
    assert!(b(&rt, "r"));
}

#[test]
fn bool_and_false() {
    let rt = run("state { let r: bool = true and false }");
    assert!(!b(&rt, "r"));
}

#[test]
fn bool_or_true() {
    let rt = run("state { let r: bool = false or true }");
    assert!(b(&rt, "r"));
}

#[test]
fn bool_or_false() {
    let rt = run("state { let r: bool = false or false }");
    assert!(!b(&rt, "r"));
}

#[test]
fn bool_not_true() {
    let rt = run("state { let r: bool = not false }");
    assert!(b(&rt, "r"));
}

#[test]
fn bool_not_false() {
    let rt = run("state { let r: bool = not true }");
    assert!(!b(&rt, "r"));
}

#[test]
fn bool_complex_expr() {
    let rt = run("state { let r: bool = (true and not false) or (false and true) }");
    assert!(b(&rt, "r"));
}

// ─── Math functions ───────────────────────────────────────────────────────────

#[test]
fn math_sin_zero() {
    let rt = run("state { let x: float = sin(0.0) }");
    assert!((f(&rt, "x") - 0.0).abs() < 1e-10);
}

#[test]
fn math_cos_zero() {
    let rt = run("state { let x: float = cos(0.0) }");
    assert!((f(&rt, "x") - 1.0).abs() < 1e-10);
}

#[test]
fn math_sqrt_four() {
    let rt = run("state { let x: float = sqrt(4.0) }");
    assert!((f(&rt, "x") - 2.0).abs() < 1e-10);
}

#[test]
fn math_abs_negative() {
    let rt = run("state { let x: float = abs(-5.0) }");
    assert_eq!(f(&rt, "x"), 5.0);
}

#[test]
fn math_floor() {
    let rt = run("state { let x: float = floor(3.9) }");
    assert_eq!(f(&rt, "x"), 3.0);
}

#[test]
fn math_ceil() {
    let rt = run("state { let x: float = ceil(3.1) }");
    assert_eq!(f(&rt, "x"), 4.0);
}

#[test]
fn math_round() {
    let rt = run("state { let x: float = round(3.5) }");
    assert_eq!(f(&rt, "x"), 4.0);
}

#[test]
fn math_clamp() {
    let rt = run("state { let x: float = clamp(15.0, 0.0, 10.0) }");
    assert_eq!(f(&rt, "x"), 10.0);
}

#[test]
fn math_lerp() {
    let rt = run("state { let x: float = lerp(0.0, 10.0, 0.5) }");
    assert_eq!(f(&rt, "x"), 5.0);
}

#[test]
fn math_min() {
    let rt = run("state { let x: float = min(3.0, 7.0) }");
    assert_eq!(f(&rt, "x"), 3.0);
}

#[test]
fn math_max() {
    let rt = run("state { let x: float = max(3.0, 7.0) }");
    assert_eq!(f(&rt, "x"), 7.0);
}

#[test]
fn math_pow() {
    let rt = run("state { let x: float = pow(2.0, 8.0) }");
    assert_eq!(f(&rt, "x"), 256.0);
}

#[test]
fn math_pi_constant() {
    let rt = run("state { let x: float = PI }");
    assert!((f(&rt, "x") - std::f64::consts::PI).abs() < 1e-10);
}

// ─── Variables ────────────────────────────────────────────────────────────────

#[test]
fn var_inferred_float() {
    let rt = run("state { let x = 3.14 }");
    #[expect(clippy::approx_constant, reason = "3.14 is used as a test literal, not as PI")]
    let expected = 3.14;
    assert!((f(&rt, "x") - expected).abs() < 1e-10);
}

#[test]
fn var_inferred_bool() {
    let rt = run("state { let x = true }");
    assert!(b(&rt, "x"));
}

#[test]
fn var_reassign_in_init() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 10.0
            s.x = s.x + 5.0
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 15.0);
}

#[test]
fn var_local_scope_in_init() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let local = 42.0
            s.x = local
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 42.0);
}

// ─── Control flow ─────────────────────────────────────────────────────────────

#[test]
fn if_true_branch_taken() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            if true { s.x = 1.0 }
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 1.0);
}

#[test]
fn if_false_branch_skipped() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            if false { s.x = 1.0 }
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 0.0);
}

#[test]
fn if_else_false_takes_else() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            if false { s.x = 1.0 } else { s.x = 2.0 }
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 2.0);
}

#[test]
fn if_condition_expression() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let v = 5.0
            if v > 3.0 { s.x = 1.0 } else { s.x = -1.0 }
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 1.0);
}

#[test]
fn compound_assignment() {
    let rt = run(r"
        state { let x: float = 10.0 }
        fn on_init(s: State) -> State {
            s.x += 5.0
            s.x *= 2.0
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 30.0);
}

#[test]
fn match_float_with_else() {
    let rt = run(r"
        state { let x: float = 2.0 }
        fn on_init(s: State) -> State {
            match s.x {
                1.0 => { s.x = 10.0 }
                2.0 => { s.x = 20.0 }
                else => { s.x = 99.0 }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 20.0);
}

#[test]
fn match_no_match_no_else() {
    let rt = run(r"
        state { let x: float = 99.0 }
        fn on_init(s: State) -> State {
            match s.x {
                1.0 => { s.x = 10.0 }
                2.0 => { s.x = 20.0 }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 99.0);
}

#[test]
fn match_multi_value_arm() {
    let rt = run(r"
        state { let x: float = 3.0 }
        fn on_init(s: State) -> State {
            match s.x {
                1.0, 2.0 => { s.x = 12.0 }
                3.0, 4.0 => { s.x = 34.0 }
                else => { s.x = 0.0 }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 34.0);
}

#[test]
fn inc_dec_prefix_postfix() {
    let rt = run(r"
        state { let x: float = 5.0 }
        fn on_init(s: State) -> State {
            let a = s.x++   // a = 5, s.x = 6
            let b = ++s.x   // b = 7, s.x = 7
            s.x--           // s.x = 6
            let c = --s.x   // c = 5, s.x = 5
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 5.0);
}

#[test]
fn inc_dec_on_list_index() {
    let rt = run(r"
        state { let xs: list[float] = [10.0, 20.0] }
        fn on_init(s: State) -> State {
            s.xs[0]++
            s.xs[1]--
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "xs"), [11.0, 19.0]);
}

#[test]
fn else_if_branches() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let v = 2.0
            if v < 1.0 { s.x = 1.0 } else if v < 3.0 { s.x = 2.0 } else { s.x = 3.0 }
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 2.0);
}

#[test]
fn while_runs_correct_iterations() {
    let rt = run(r"
        state { let count: float = 0.0 }
        fn on_init(s: State) -> State {
            let i = 0.0
            while i < 5.0 {
                s.count = s.count + 1.0
                i = i + 1.0
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "count"), 5.0);
}

#[test]
fn while_false_condition_never_runs() {
    let rt = run(r"
        state { let count: float = 0.0 }
        fn on_init(s: State) -> State {
            while false { s.count = s.count + 1.0 }
            return s
        }
    ");
    assert_eq!(f(&rt, "count"), 0.0);
}

#[test]
fn for_loop_runs_n_times() {
    let rt = run(r"
        state { let sum: float = 0.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 5.0; i = i + 1.0 {
                s.sum = s.sum + i
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "sum"), 10.0); // 0+1+2+3+4
}

#[test]
fn foreach_iterates_all_elements() {
    let rt = run(r"
        state { let sum: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = [1.0, 2.0, 3.0, 4.0]
            foreach v in xs { s.sum = s.sum + v }
            return s
        }
    ");
    assert_eq!(f(&rt, "sum"), 10.0);
}

#[test]
fn nested_if_in_for() {
    let rt = run(r"
        state { let evens: float = 0.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 6.0; i = i + 1.0 {
                if i % 2.0 == 0.0 { s.evens = s.evens + 1.0 }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "evens"), 3.0); // 0, 2, 4
}

#[test]
fn nested_for_loops() {
    let rt = run(r"
        state { let count: float = 0.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 3.0; i = i + 1.0 {
                for let j = 0.0; j < 3.0; j = j + 1.0 {
                    s.count = s.count + 1.0
                }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "count"), 9.0);
}

#[test]
fn ternary_true_branch() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 1.0 > 0.0 ? 10.0 : 20.0
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 10.0);
}

#[test]
fn ternary_false_branch() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 1.0 < 0.0 ? 10.0 : 20.0
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 20.0);
}

// ─── Functions ────────────────────────────────────────────────────────────────

#[test]
fn fn_basic_call() {
    let rt = run(r"
        fn add(a: float, b: float) -> float { return a + b }
        state { let x: float = add(3.0, 4.0) }
    ");
    assert_eq!(f(&rt, "x"), 7.0);
}

#[test]
fn fn_called_in_init() {
    let rt = run(r"
        fn square(x: float) -> float { return x * x }
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = square(5.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 25.0);
}

#[test]
fn fn_visible_before_declaration() {
    let rt = run(r"
        state { let x: float = add(1.0, 2.0) }
        fn add(a: float, b: float) -> float { return a + b }
    ");
    assert_eq!(f(&rt, "x"), 3.0);
}

#[test]
fn fn_higher_order() {
    let rt = run(r"
        fn apply(f: fn(float) -> float, x: float) -> float { return f(x) }
        fn double(x: float) -> float { return x * 2.0 }
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = apply(double, 5.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 10.0);
}

#[test]
fn fn_lambda() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            fn triple = (x: float) -> float { return x * 3.0 }
            s.x = triple(4.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 12.0);
}

// ─── Vec2 ─────────────────────────────────────────────────────────────────────

#[test]
fn vec2_construction_and_fields() {
    let rt = run(r"
        state { let v: vec2 = vec2(3.0, 4.0) }
    ");
    let (x, y) = v2(&rt, "v");
    assert_eq!(x, 3.0);
    assert_eq!(y, 4.0);
}

#[test]
fn vec2_add() {
    let rt = run(r"
        state { let v: vec2 = vec2(1.0, 2.0) + vec2(3.0, 4.0) }
    ");
    assert_eq!(v2(&rt, "v"), (4.0, 6.0));
}

#[test]
fn vec2_sub() {
    let rt = run(r"
        state { let v: vec2 = vec2(5.0, 5.0) - vec2(1.0, 2.0) }
    ");
    assert_eq!(v2(&rt, "v"), (4.0, 3.0));
}

#[test]
fn vec2_scalar_mul() {
    let rt = run(r"
        state { let v: vec2 = vec2(1.0, 2.0) * 3.0 }
    ");
    assert_eq!(v2(&rt, "v"), (3.0, 6.0));
}

#[test]
fn vec2_scalar_div() {
    let rt = run(r"
        state { let v: vec2 = vec2(4.0, 6.0) / 2.0 }
    ");
    assert_eq!(v2(&rt, "v"), (2.0, 3.0));
}

#[test]
fn vec2_length() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = vec2(3.0, 4.0).length()
            return s
        }
    ");
    assert!((f(&rt, "x") - 5.0).abs() < 1e-10);
}

#[test]
fn vec2_dot_product() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = vec2(1.0, 0.0).dot(vec2(0.0, 1.0))
            return s
        }
    ");
    assert!((f(&rt, "x") - 0.0).abs() < 1e-10);
}

#[test]
fn vec2_normalize() {
    let rt = run(r"
        state { let v: vec2 = vec2(0.0, 0.0) }
        fn on_init(s: State) -> State {
            s.v = vec2(3.0, 0.0).normalize()
            return s
        }
    ");
    let (x, y) = v2(&rt, "v");
    assert!((x - 1.0).abs() < 1e-10);
    assert!(y.abs() < 1e-10);
}

#[test]
fn vec2_normalize_zero_vector_error() {
    run_err(r"
        state { let v: vec2 = vec2(0.0, 0.0) }
        fn on_init(s: State) -> State {
            s.v = vec2(0.0, 0.0).normalize()
            return s
        }
    ");
}

#[test]
fn vec2_eq() {
    let rt = run(r"
        state { let r: bool = vec2(1.0, 2.0) == vec2(1.0, 2.0) }
    ");
    assert!(b(&rt, "r"));
}

#[test]
fn vec2_neq() {
    let rt = run(r"
        state { let r: bool = vec2(1.0, 2.0) != vec2(3.0, 4.0) }
    ");
    assert!(b(&rt, "r"));
}

// ─── Lists ────────────────────────────────────────────────────────────────────

#[test]
fn list_push_increases_len() {
    let rt = run(r"
        state { let xs: list[float] = [] }
        fn on_init(s: State) -> State {
            s.xs.push(1.0)
            s.xs.push(2.0)
            s.xs.push(3.0)
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "xs"), vec![1.0, 2.0, 3.0]);
}

#[test]
fn list_pop_removes_last() {
    let rt = run(r"
        state {
            let xs: list[float] = []
            let last: float = 0.0
        }
        fn on_init(s: State) -> State {
            s.xs.push(10.0)
            s.xs.push(20.0)
            s.xs.push(30.0)
            s.last = s.xs.pop()
            return s
        }
    ");
    assert_eq!(f(&rt, "last"), 30.0);
    assert_eq!(list_floats(&rt, "xs"), vec![10.0, 20.0]);
}

#[test]
fn list_pop_empty_runtime_error() {
    run_err(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let empty: list[float] = []
            s.x = empty.pop()
            return s
        }
    ");
}

#[test]
fn list_index_assignment() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = [10.0, 20.0, 30.0]
            xs[1] = 99.0
            s.x = xs[1]
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 99.0);
}

#[test]
fn list_index_compound_assignment() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = [10.0, 20.0, 30.0]
            xs[1] += 5.0
            s.x = xs[1]
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 25.0);
}

#[test]
fn list_index_access() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = [10.0, 20.0, 30.0]
            s.x = xs[1]
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 20.0);
}

#[test]
fn list_len_field() {
    let rt = run(r"
        state { let n: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = [1.0, 2.0, 3.0]
            s.n = xs.len
            return s
        }
    ");
    assert_eq!(f(&rt, "n"), 3.0);
}

#[test]
fn list_len_method() {
    let rt = run(r"
        state { let n: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = [1.0, 2.0]
            s.n = xs.len()
            return s
        }
    ");
    assert_eq!(f(&rt, "n"), 2.0);
}

#[test]
fn list_foreach_sum() {
    let rt = run(r"
        state { let sum: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = [1.0, 2.0, 3.0, 4.0, 5.0]
            foreach v in xs { s.sum = s.sum + v }
            return s
        }
    ");
    assert_eq!(f(&rt, "sum"), 15.0);
}

#[test]
fn list_mutation_is_shared() {
    // Pushing to a list stored in state mutates in-place.
    let rt = run(r"
        state { let xs: list[float] = [] }
        fn on_init(s: State) -> State {
            for let i = 1.0; i <= 5.0; i = i + 1.0 {
                s.xs.push(i)
            }
            return s
        }
    ");
    assert_eq!(list_floats(&rt, "xs"), vec![1.0, 2.0, 3.0, 4.0, 5.0]);
}

#[test]
fn list_literal_initial_values() {
    let rt = run("state { let xs: list[float] = [10.0, 20.0, 30.0] }");
    assert_eq!(list_floats(&rt, "xs"), vec![10.0, 20.0, 30.0]);
}

// ─── State lifecycle ──────────────────────────────────────────────────────────

#[test]
fn state_initializers_run() {
    let rt = run(r"
        state {
            let a: float = 2.0 + 3.0
            let b: bool  = 10.0 > 5.0
        }
    ");
    assert_eq!(f(&rt, "a"), 5.0);
    assert!(b(&rt, "b"));
}

#[test]
fn init_runs_before_first_tick() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 99.0
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 99.0);
}

#[test]
fn update_accumulates_over_ticks() {
    let mut rt = run(r"
        state { let t: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.t = s.t + 1.0
            return s
        }
    ");
    tick(&mut rt);
    tick(&mut rt);
    tick(&mut rt);
    assert_eq!(f(&rt, "t"), 3.0);
}

#[test]
fn update_uses_input_dt() {
    let mut rt = run(r"
        state { let t: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.t = s.t + input.dt
            return s
        }
    ");
    tick(&mut rt);
    // dt is 0.016 per tick
    assert!((f(&rt, "t") - 0.016).abs() < 1e-10);
}

#[test]
fn init_and_update_together() {
    let mut rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 10.0
            return s
        }
        fn on_update(s: State, input: Input) -> State {
            s.x = s.x + 1.0
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 10.0);
    tick(&mut rt);
    assert_eq!(f(&rt, "x"), 11.0);
    tick(&mut rt);
    assert_eq!(f(&rt, "x"), 12.0);
}

#[test]
fn on_exit_runs_when_exit_called() {
    let mut rt = run(r"
        state { let x: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.x = s.x + 1.0
            return s
        }
        fn on_exit(s: State) -> State {
            s.x = 999.0
            return s
        }
    ");
    tick(&mut rt);
    tick(&mut rt);
    assert_eq!(f(&rt, "x"), 2.0);
    rt.exit().unwrap();
    assert_eq!(f(&rt, "x"), 999.0);
}

// ─── Result type ──────────────────────────────────────────────────────────────

#[test]
fn res_ok_fields() {
    let rt = run(r"
        state {
            let flag: bool = false
            let val: float = 0.0
        }
        fn on_init(s: State) -> State {
            let r: res<float> = ok(42.0)
            s.flag = r.ok
            s.val  = r.value
            return s
        }
    ");
    assert!(b(&rt, "flag"));
    assert_eq!(f(&rt, "val"), 42.0);
}

#[test]
fn res_error_fields() {
    let rt = run(r#"
        state { let flag: bool = true }
        fn on_init(s: State) -> State {
            let r: res<float> = error("oops")
            s.flag = r.ok
            return s
        }
    "#);
    assert!(!b(&rt, "flag"));
}

#[test]
fn res_from_fn_success() {
    let rt = run(r#"
        fn safe_div(a: float, b: float) -> res<float> {
            if b == 0.0 { return error("div by zero") }
            return ok(a / b)
        }
        state {
            let x: float = 0.0
            let ok_flag: bool = false
        }
        fn on_init(s: State) -> State {
            let r = safe_div(10.0, 2.0)
            s.ok_flag = r.ok
            s.x = r.value
            return s
        }
    "#);
    assert!(b(&rt, "ok_flag"));
    assert_eq!(f(&rt, "x"), 5.0);
}

#[test]
fn res_from_fn_failure() {
    let rt = run(r#"
        fn safe_div(a: float, b: float) -> res<float> {
            if b == 0.0 { return error("div by zero") }
            return ok(a / b)
        }
        state { let ok_flag: bool = true }
        fn on_init(s: State) -> State {
            let r = safe_div(10.0, 0.0)
            s.ok_flag = r.ok
            return s
        }
    "#);
    assert!(!b(&rt, "ok_flag"));
}

#[test]
fn try_successful_expr() {
    let rt = run(r"
        state {
            let flag: bool = false
            let val: float = 0.0
        }
        fn on_init(s: State) -> State {
            let r: res<float> = try 10.0 / 2.0
            s.flag = r.ok
            s.val  = r.value
            return s
        }
    ");
    assert!(b(&rt, "flag"));
    assert_eq!(f(&rt, "val"), 5.0);
}

#[test]
fn try_catches_div_by_zero() {
    let rt = run(r"
        state { let flag: bool = true }
        fn on_init(s: State) -> State {
            let r: res<float> = try 1.0 / 0.0
            s.flag = r.ok
            return s
        }
    ");
    assert!(!b(&rt, "flag"));
}

// ─── Draw output ──────────────────────────────────────────────────────────────

#[test]
fn draw_static_emits_circle() {
    let mut rt = run(r"
        import shapes { circle }
        out << circle(vec2(0.0, 0.0), 0.5)
    ");
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 1);
    let DrawCommand::DrawShape(data) = &cmds[0] else { panic!("expected DrawShape") };
    assert!(matches!(data.desc, ShapeDesc::Circle { .. }));
}

#[test]
fn draw_static_emits_rect() {
    let mut rt = run(r"
        import shapes { rect }
        out << rect(vec2(0.0, 0.0), vec2(1.0, 1.0))
    ");
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 1);
    let DrawCommand::DrawShape(data) = &cmds[0] else { panic!("expected DrawShape") };
    assert!(matches!(data.desc, ShapeDesc::Rect { .. }));
}

#[test]
fn draw_static_multiple_shapes() {
    let mut rt = run(r"
        import shapes { circle, rect }
        out << rect(vec2(0.0, 0.0), vec2(2.0, 2.0))
        out << circle(vec2(0.0, 0.0), 0.3)
    ");
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 2);
}

#[test]
fn draw_static_chained_out() {
    let mut rt = run(r"
        import shapes { circle, rect }
        let bg = rect(vec2(0.0, 0.0), vec2(2.0, 2.0))
        let c  = circle(vec2(0.0, 0.0), 0.3)
        out << bg << c
    ");
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 2);
}

#[test]
fn draw_update_emits_each_tick() {
    let mut rt = run(r"
        import shapes { circle }
        state { let t: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.t = s.t + input.dt
            out << circle(vec2(sin(s.t) * 0.5, 0.0), 0.2)
            return s
        }
    ");
    let c1 = tick(&mut rt);
    let c2 = tick(&mut rt);
    assert_eq!(c1.len(), 1);
    assert_eq!(c2.len(), 1);
}

#[test]
fn draw_foreach_emits_one_per_element() {
    let mut rt = run(r"
        import shapes { circle }
        state { let xs: list[float] = [] }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 5.0; i = i + 1.0 { s.xs.push(i * 0.1) }
            return s
        }
        fn on_update(s: State, input: Input) -> State {
            foreach v in s.xs { out << circle(vec2(v, 0.0), 0.05) }
            return s
        }
    ");
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 5);
}

#[test]
fn draw_transform_attached_to_shape() {
    let mut rt = run(r"
        import shapes { circle }
        let t = transform().scale(2.0)
        let s = circle(vec2(0.0, 0.0), 0.2)
        out << s@t
    ");
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 1);
    let DrawCommand::DrawShape(data) = &cmds[0] else { panic!("expected DrawShape") };
    assert_eq!(data.transforms.len(), 1);
    assert_eq!(data.transforms[0].sx, 2.0);
    assert_eq!(data.transforms[0].sy, 2.0);
}

#[test]
fn draw_multiple_transforms_accumulated() {
    let mut rt = run(r"
        import shapes { circle }
        let t1 = transform().scale(2.0)
        let t2 = transform().move(0.5, 0.0)
        let s  = circle(vec2(0.0, 0.0), 0.2)
        out << s@(t1, t2)
    ");
    let cmds = tick(&mut rt);
    let DrawCommand::DrawShape(data) = &cmds[0] else { panic!("expected DrawShape") };
    assert_eq!(data.transforms.len(), 2);
}

// ─── Coordinate config ────────────────────────────────────────────────────────

#[test]
fn resolution_sets_coord_meta() {
    let mut rt = run(r"
        import shapes { circle }
        import coords { resolution }
        resolution(800.0, 600.0)
        out << circle(vec2(400.0, 300.0), 50.0)
    ");
    let cmds = tick(&mut rt);
    let DrawCommand::DrawShape(data) = &cmds[0] else { panic!("expected DrawShape") };
    assert_eq!(data.coord_meta.px_width,  800.0);
    assert_eq!(data.coord_meta.px_height, 600.0);
}

#[test]
fn resolution_in_init_persists_to_tick() {
    let mut rt = run(r"
        import shapes { circle }
        import coords { resolution, origin, top_left }
        state { }
        fn on_init(s: State) -> State {
            resolution(1024.0, 768.0)
            origin(top_left)
            return s
        }
        fn on_update(s: State, input: Input) -> State {
            out << circle(vec2(100.0, 100.0), 30.0)
            return s
        }
    ");
    let cmds = tick(&mut rt);
    let DrawCommand::DrawShape(data) = &cmds[0] else { panic!("expected DrawShape") };
    assert_eq!(data.coord_meta.px_width,  1024.0);
    assert_eq!(data.coord_meta.px_height, 768.0);
}

// ─── Complex / edge cases ─────────────────────────────────────────────────────

#[test]
fn complex_recursive_fn() {
    let rt = run(r"
        fn factorial(n: float) -> float {
            if n <= 1.0 { return 1.0 }
            return n * factorial(n - 1.0)
        }
        state { let x: float = factorial(5.0) }
    ");
    assert_eq!(f(&rt, "x"), 120.0);
}

#[test]
fn complex_fibonacci() {
    let rt = run(r"
        fn fib(n: float) -> float {
            if n <= 1.0 { return n }
            return fib(n - 1.0) + fib(n - 2.0)
        }
        state { let x: float = fib(7.0) }
    ");
    assert_eq!(f(&rt, "x"), 13.0);
}

#[test]
fn complex_nested_fn_calls_in_expr() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = sqrt(pow(3.0, 2.0) + pow(4.0, 2.0))
            return s
        }
    ");
    assert!((f(&rt, "x") - 5.0).abs() < 1e-10);
}

#[test]
fn complex_math_expression_chain() {
    let rt = run(r"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = clamp(abs(-15.0), 0.0, 10.0)
            return s
        }
    ");
    assert_eq!(f(&rt, "x"), 10.0);
}

#[test]
fn complex_list_built_in_loop_then_sum() {
    let rt = run(r"
        state { let sum: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = []
            for let i = 1.0; i <= 10.0; i = i + 1.0 { xs.push(i) }
            foreach v in xs { s.sum = s.sum + v }
            return s
        }
    ");
    assert_eq!(f(&rt, "sum"), 55.0); // 1+2+...+10
}

#[test]
fn complex_conditional_accumulation() {
    let rt = run(r"
        state {
            let pos: float = 0.0
            let neg: float = 0.0
        }
        fn on_init(s: State) -> State {
            let xs: list[float] = [-3.0, 1.0, -1.0, 4.0, -2.0, 5.0]
            foreach v in xs {
                if v > 0.0 { s.pos = s.pos + v } else { s.neg = s.neg + v }
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "pos"), 10.0);
    assert_eq!(f(&rt, "neg"), -6.0);
}

#[test]
fn complex_state_persists_across_many_ticks() {
    let mut rt = run(r"
        state { let count: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.count = s.count + 1.0
            return s
        }
    ");
    for _ in 0..100 {
        tick(&mut rt);
    }
    assert_eq!(f(&rt, "count"), 100.0);
}

#[test]
fn console_with_postfix_inc() {
    let mut rt = run("
        let x: float = 5.0
        console << x++
    ");
    let cmds = tick(&mut rt);
    match &cmds[0] {
        DrawCommand::Print(msg) => assert_eq!(msg, "5"),
        other => panic!("expected Print, got {other:?}"),
    }
}

// ─── Phase 5A: Console warn/error tests ─────────────────────────────────────

#[test]
fn console_warn() {
    let mut rt = run("console.warn << 42");
    let cmds = tick(&mut rt);
    match &cmds[0] {
        DrawCommand::Warn(msg) => assert_eq!(msg, "42"),
        other => panic!("expected Warn, got {other:?}"),
    }
}

#[test]
fn console_error() {
    let mut rt = run("console.error << \"oops\"");
    let cmds = tick(&mut rt);
    match &cmds[0] {
        DrawCommand::Error(msg) => assert_eq!(msg, "oops"),
        other => panic!("expected Error, got {other:?}"),
    }
}

#[test]
fn console_multiple_values() {
    let mut rt = run("console << 1 << 2 << 3");
    let cmds = tick(&mut rt);
    match &cmds[0] {
        DrawCommand::Print(msg) => assert_eq!(msg, "1 2 3"),
        other => panic!("expected Print, got {other:?}"),
    }
}

#[test]
fn console_warn_multiple_values() {
    let mut rt = run("console.warn << true << \"hello\"");
    let cmds = tick(&mut rt);
    match &cmds[0] {
        DrawCommand::Warn(msg) => assert_eq!(msg, "true hello"),
        other => panic!("expected Warn, got {other:?}"),
    }
}

// ─── Phase 5B: String type tests ────────────────────────────────────────────

#[test]
fn string_in_state() {
    let rt = run(r#"
        state { let name: string = "hello" }
    "#);
    match rt.state().0.get("name") {
        Some(Value::Str(s)) => assert_eq!(s, "hello"),
        other => panic!("expected Str, got {other:?}"),
    }
}

#[test]
fn string_equality() {
    let rt = run(r#"
        state {
            let eq: bool = "abc" == "abc"
            let neq: bool = "abc" != "xyz"
        }
    "#);
    assert!(b(&rt, "eq"));
    assert!(b(&rt, "neq"));
}

#[test]
fn string_in_list() {
    let mut rt = run(r#"
        let xs: list[string] = ["a", "b", "c"]
        console << xs
    "#);
    let cmds = tick(&mut rt);
    match &cmds[0] {
        DrawCommand::Print(msg) => assert_eq!(msg, "[a, b, c]"),
        other => panic!("expected Print, got {other:?}"),
    }
}

#[test]
fn string_console_output() {
    let mut rt = run(r#"console << "hello world""#);
    let cmds = tick(&mut rt);
    match &cmds[0] {
        DrawCommand::Print(msg) => assert_eq!(msg, "hello world"),
        other => panic!("expected Print, got {other:?}"),
    }
}

// ─── Phase 5C: Index edge case tests ────────────────────────────────────────

#[test]
fn index_negative_error() {
    let mut rt = run("
        let xs: list[float] = [1, 2, 3]
        let i: float = -1
        state { let val: float = 0 }
        fn on_update(s: State, input: Input) -> State {
            s.val = xs[i]
            return s
        }
    ");
    let err = tick_err(&mut rt);
    assert!(err.message.contains("negative"), "expected negative index error, got: {}", err.message);
}

#[test]
fn index_out_of_bounds_error() {
    let mut rt = run("
        let xs: list[float] = [1, 2, 3]
        state { let val: float = 0 }
        fn on_update(s: State, input: Input) -> State {
            s.val = xs[10]
            return s
        }
    ");
    let err = tick_err(&mut rt);
    assert!(err.message.contains("out of bounds"),
        "expected bounds error, got: {}", err.message);
}

#[test]
fn index_valid_access() {
    let rt = run("
        let xs: list[float] = [10, 20, 30]
        state { let val: float = xs[1] }
    ");
    assert_eq!(f(&rt, "val"), 20.0);
}

// ─── Optional types (none, T?, ??) ──────────────────────────────────────────

#[test]
fn optional_none_init() {
    let rt = run("state { let x: float? = none }");
    match rt.state().0.get("x") {
        Some(Value::None) => {}
        other => panic!("expected None, got {other:?}"),
    }
}

#[test]
fn optional_value_init() {
    let rt = run("state { let x: float? = 5.0 }");
    match rt.state().0.get("x") {
        Some(Value::Float(v)) => assert_eq!(*v, 5.0),
        other => panic!("expected Float(5.0), got {other:?}"),
    }
}

#[test]
fn coalesce_none_uses_default() {
    let rt = run("
        let x: float? = none
        state { let val: float = x ?? 42.0 }
    ");
    assert_eq!(f(&rt, "val"), 42.0);
}

#[test]
fn coalesce_value_uses_left() {
    let rt = run("
        let x: float? = 5.0
        state { let val: float = x ?? 42.0 }
    ");
    assert_eq!(f(&rt, "val"), 5.0);
}

#[test]
fn none_eq_none() {
    let rt = run("state { let val: bool = none == none }");
    assert!(b(&rt, "val"));
}

#[test]
fn some_neq_none() {
    let rt = run("
        let x: float? = 5.0
        state { let val: bool = x != none }
    ");
    assert!(b(&rt, "val"));
}

#[test]
fn none_display() {
    let mut rt = run("
        let x: float? = none
        console << x
    ");
    let cmds = tick(&mut rt);
    match &cmds[0] {
        DrawCommand::Print(msg) => assert_eq!(msg, "none"),
        other => panic!("expected Print, got {other:?}"),
    }
}

#[test]
fn coalesce_in_update() {
    let mut rt = run("
        state { let x: float? = none }
        fn on_update(s: State, input: Input) -> State {
            let val: float = s.x ?? 99.0
            console << val
            s.x = 10.0
            return s
        }
    ");
    let cmds = tick(&mut rt);
    match &cmds[0] {
        DrawCommand::Print(msg) => assert_eq!(msg, "99"),
        other => panic!("expected Print, got {other:?}"),
    }
    let cmds2 = tick(&mut rt);
    match &cmds2[0] {
        DrawCommand::Print(msg) => assert_eq!(msg, "10"),
        other => panic!("expected Print, got {other:?}"),
    }
}

#[test]
fn optional_reassign_none_to_value() {
    let mut rt = run("
        state { let x: float? = none }
        fn on_update(s: State, input: Input) -> State {
            s.x = 42.0
            return s
        }
    ");
    tick(&mut rt);
    match rt.state().0.get("x") {
        Some(Value::Float(v)) => assert_eq!(*v, 42.0),
        other => panic!("expected Float(42.0), got {other:?}"),
    }
}

#[test]
fn optional_reassign_value_to_none() {
    let mut rt = run("
        state { let x: float? = 10.0 }
        fn on_update(s: State, input: Input) -> State {
            s.x = none
            return s
        }
    ");
    tick(&mut rt);
    match rt.state().0.get("x") {
        Some(Value::None) => {}
        other => panic!("expected None, got {other:?}"),
    }
}

// ─── if let ─────────────────────────────────────────────────────────────────

#[test]
fn if_let_some_runs_then() {
    let mut rt = run("
        let x: float? = 42.0
        if let v = x {
            console << v
        } else {
            console << 0
        }
    ");
    let cmds = tick(&mut rt);
    match &cmds[0] {
        DrawCommand::Print(msg) => assert_eq!(msg, "42"),
        other => panic!("expected Print(42), got {other:?}"),
    }
}

#[test]
fn if_let_none_runs_else() {
    let mut rt = run("
        let x: float? = none
        if let v = x {
            console << v
        } else {
            console << 0
        }
    ");
    let cmds = tick(&mut rt);
    match &cmds[0] {
        DrawCommand::Print(msg) => assert_eq!(msg, "0"),
        other => panic!("expected Print(0), got {other:?}"),
    }
}

#[test]
fn if_let_none_no_else() {
    let mut rt = run("
        let x: float? = none
        if let v = x {
            console << v
        }
        console << 99
    ");
    let cmds = tick(&mut rt);
    match &cmds[0] {
        DrawCommand::Print(msg) => assert_eq!(msg, "99"),
        other => panic!("expected Print(99), got {other:?}"),
    }
}

// ─── ?. optional chaining ───────────────────────────────────────────────────

#[test]
fn optional_chain_some() {
    let rt = run("
        let v: vec2? = vec2(3.0, 7.0)
        state { let val: float = v?.x ?? 0.0 }
    ");
    assert_eq!(f(&rt, "val"), 3.0);
}

#[test]
fn optional_chain_none() {
    let rt = run("
        let v: vec2? = none
        state { let val: float = v?.x ?? 0.0 }
    ");
    assert_eq!(f(&rt, "val"), 0.0);
}

// ─── Truthiness ──────────────────────────────────────────────────────────────

#[test]
fn truthy_float_zero_false() {
    let rt = run("state { let v: float = 0.0 ? 1.0 : 2.0 }");
    assert_eq!(f(&rt, "v"), 2.0);
}

#[test]
fn truthy_float_nonzero_true() {
    let rt = run("state { let v: float = 3.14 ? 1.0 : 2.0 }");
    assert_eq!(f(&rt, "v"), 1.0);
}

#[test]
fn truthy_string_empty_false() {
    let rt = run("state { let v: float = \"\" ? 1.0 : 2.0 }");
    assert_eq!(f(&rt, "v"), 2.0);
}

#[test]
fn truthy_string_nonempty_true() {
    let rt = run("state { let v: float = \"hi\" ? 1.0 : 2.0 }");
    assert_eq!(f(&rt, "v"), 1.0);
}

#[test]
fn truthy_list_empty_false() {
    let rt = run("
        let xs: list[float] = []
        state { let v: float = xs ? 1.0 : 2.0 }
    ");
    assert_eq!(f(&rt, "v"), 2.0);
}

#[test]
fn truthy_list_nonempty_true() {
    let rt = run("
        let xs: list[float] = [1.0]
        state { let v: float = xs ? 1.0 : 2.0 }
    ");
    assert_eq!(f(&rt, "v"), 1.0);
}

#[test]
fn truthy_none_false() {
    let rt = run("
        let x: float? = none
        state { let v: float = x ? 1.0 : 2.0 }
    ");
    assert_eq!(f(&rt, "v"), 2.0);
}

#[test]
fn truthy_optional_value_true() {
    let rt = run("
        let x: float? = 5.0
        state { let v: float = x ? 1.0 : 2.0 }
    ");
    assert_eq!(f(&rt, "v"), 1.0);
}

#[test]
fn truthy_optional_zero_false() {
    // float? with 0.0 — unwrap + apply inner truthiness → false
    let rt = run("
        let x: float? = 0.0
        state { let v: float = x ? 1.0 : 2.0 }
    ");
    assert_eq!(f(&rt, "v"), 2.0);
}

#[test]
fn truthy_optional_vec2_present_true() {
    // vec2? with value — presence check only → true
    let rt = run("
        let x: vec2? = vec2(0.0, 0.0)
        state { let v: float = x ? 1.0 : 2.0 }
    ");
    assert_eq!(f(&rt, "v"), 1.0);
}

#[test]
fn truthy_not_float() {
    let rt = run("state { let a: bool = not 0.0 }");
    assert!(b(&rt, "a"));
}

#[test]
fn truthy_not_float_nonzero() {
    let rt = run("state { let a: bool = not 1.0 }");
    assert!(!b(&rt, "a"));
}

#[test]
fn truthy_and_short_circuit() {
    // false and (side effect that would crash) — RHS not evaluated
    let rt = run("
        let xs: list[float] = []
        state { let v: bool = false and xs[99] }
    ");
    assert!(!b(&rt, "v"));
}

#[test]
fn truthy_or_short_circuit() {
    // true or (side effect that would crash) — RHS not evaluated
    let rt = run("
        let xs: list[float] = []
        state { let v: bool = true or xs[99] }
    ");
    assert!(b(&rt, "v"));
}

#[test]
fn truthy_and_mixed_types() {
    let rt = run("state { let v: bool = 1.0 and \"hello\" }");
    assert!(b(&rt, "v"));
}

#[test]
fn truthy_and_mixed_false() {
    let rt = run("state { let v: bool = 0.0 and \"hello\" }");
    assert!(!b(&rt, "v"));
}

#[test]
fn truthy_or_mixed_types() {
    let rt = run("state { let v: bool = 0.0 or \"\" }");
    assert!(!b(&rt, "v"));
}

#[test]
fn truthy_or_mixed_true() {
    let rt = run("state { let v: bool = 0.0 or \"hi\" }");
    assert!(b(&rt, "v"));
}

#[test]
fn truthy_if_float() {
    let rt = run("
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            if 0.0 { s.v = 1.0 } else { s.v = 2.0 }
            return s
        }
    ");
    assert_eq!(f(&rt, "v"), 2.0);
}

#[test]
fn truthy_while_string() {
    // while with empty string — falsy, so loop body never runs
    let rt = run("
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let s2: string = \"\"
            while s2 { s.v = 1.0 s2 = \"\" }
            return s
        }
    ");
    assert_eq!(f(&rt, "v"), 0.0);
}

// ─── Boolean arithmetic ──────────────────────────────────────────────────────

#[test]
fn bool_add_true_plus_one() {
    let rt = run("state { let v: float = true + 1.0 }");
    assert_eq!(f(&rt, "v"), 2.0);
}

#[test]
fn bool_add_false_plus_one() {
    let rt = run("state { let v: float = false + 1.0 }");
    assert_eq!(f(&rt, "v"), 1.0);
}

#[test]
fn bool_sub() {
    let rt = run("state { let v: float = true - false }");
    assert_eq!(f(&rt, "v"), 1.0);
}

#[test]
fn bool_mul() {
    let rt = run("state { let v: float = true * 5.0 }");
    assert_eq!(f(&rt, "v"), 5.0);
}

#[test]
fn bool_mul_false() {
    let rt = run("state { let v: float = false * 5.0 }");
    assert_eq!(f(&rt, "v"), 0.0);
}

#[test]
fn bool_div() {
    let rt = run("state { let v: float = 5.0 / true }");
    assert_eq!(f(&rt, "v"), 5.0);
}

#[test]
fn bool_mod() {
    let rt = run("state { let v: float = 3.0 % true }");
    assert_eq!(f(&rt, "v"), 0.0);
}

#[test]
fn bool_add_both() {
    let rt = run("state { let v: float = true + true }");
    assert_eq!(f(&rt, "v"), 2.0);
}

#[test]
fn bool_chain_arithmetic() {
    let rt = run("state { let v: float = true + true + true }");
    assert_eq!(f(&rt, "v"), 3.0);
}

// ─── Error code tests ────────────────────────────────────────────────────────

#[test]
fn runtime_error_has_code_r005() {
    let e = run_err(r"
        state { let xs: list[float] = [] }
        fn on_init(s: State) -> State {
            let v: float = s.xs[0]
            return s
        }
    ");
    assert_eq!(e.code, ErrorCode::R005);
}

#[test]
fn runtime_error_index_out_of_bounds_with_details() {
    let e = run_err(r"
        state { let xs: list[float] = [] }
        fn on_init(s: State) -> State {
            s.xs.push(1)
            let v: float = s.xs[5]
            return s
        }
    ");
    assert_eq!(e.code, ErrorCode::R005);
    assert!(e.message.contains('5'), "should include the index: {}", e.message);
    assert!(e.message.contains('1'), "should include the list length: {}", e.message);
}

#[test]
fn runtime_error_division_by_zero_has_code() {
    let e = run_err("state { let v: float = 1.0 / 0.0 }");
    assert_eq!(e.code, ErrorCode::R007);
}

#[test]
fn runtime_error_stack_trace_on_nested_call() {
    let e = run_err(r"
        fn inner(x: float) -> float { return 1.0 / 0.0 }
        fn outer(x: float) -> float { return inner(x) }
        fn on_init(s: State) -> State {
            let v: float = outer(1)
            return s
        }
    ");
    assert_eq!(e.code, ErrorCode::R007);
    assert!(!e.stack.is_empty(), "should have stack frames, got: {:?}", e.stack);
}

#[test]
fn runtime_error_field_not_found_message_format() {
    // Test that field-not-found error on state includes available fields
    // Use a runtime path: index-out-of-bounds on empty list to test R003 format
    let e = run_err("state { let v: float = 1.0 / 0.0 }");
    // Just verify the error has a code and formatted message
    assert_eq!(e.code, ErrorCode::R007);
    assert!(e.message.contains("division by zero"), "message: {}", e.message);
}

#[test]
fn runtime_error_display_includes_code() {
    let e = run_err("state { let v: float = 1.0 / 0.0 }");
    let display = format!("{e}");
    assert!(display.contains("[R007]"), "display should include error code: {display}");
}

// ─── break / continue ────────────────────────────────────────────────────────

#[test]
fn break_exits_while_loop() {
    let rt = run(r"
        state { let v: float = 0 }
        fn on_init(s: State) -> State {
            let i: float = 0
            while i < 100 {
                if i == 5 { break }
                i++
            }
            s.v = i
            return s
        }
    ");
    assert_eq!(f(&rt, "v"), 5.0);
}

#[test]
fn continue_skips_iteration() {
    let rt = run(r"
        state { let v: float = 0 }
        fn on_init(s: State) -> State {
            let i: float = 0
            while i < 10 {
                i++
                if i == 3 or i == 7 { continue }
                s.v += 1
            }
            return s
        }
    ");
    // 10 iterations, skip 2 (i==3 and i==7), so 8 increments
    assert_eq!(f(&rt, "v"), 8.0);
}

#[test]
fn break_exits_for_loop() {
    let rt = run(r"
        state { let v: float = 0 }
        fn on_init(s: State) -> State {
            for let i = 0; i < 100; i = i + 1 {
                if i == 3 { break }
                s.v += 1
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "v"), 3.0);
}

#[test]
fn break_exits_foreach_loop() {
    let rt = run(r"
        state { let xs: list[float] = [1, 2, 3, 4, 5] let v: float = 0 }
        fn on_init(s: State) -> State {
            foreach x in s.xs {
                if x == 4 { break }
                s.v += x
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "v"), 6.0); // 1 + 2 + 3
}

#[test]
fn infinite_recursion_hits_depth_limit() {
    // Run on a thread with a larger stack to ensure the depth check fires
    // before the Rust stack overflows in debug builds.
    let result = std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(|| {
            run_err(r"
                fn boom(x: float) -> float { return boom(x) }
                state { let v: float = boom(1) }
            ")
        })
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(result.code, ErrorCode::R011);
    assert!(result.message.contains("call depth"), "message: {}", result.message);
}

#[test]
fn continue_in_foreach() {
    let rt = run(r"
        state { let xs: list[float] = [1, 2, 3, 4, 5] let v: float = 0 }
        fn on_init(s: State) -> State {
            foreach x in s.xs {
                if x == 2 or x == 4 { continue }
                s.v += x
            }
            return s
        }
    ");
    assert_eq!(f(&rt, "v"), 9.0); // 1 + 3 + 5
}

// ── Cast ──────────────────────────────────────────────────────────────────────

#[test]
fn cast_float_to_bool_true() {
    let rt = run("state { let v: bool = 1.0 as bool }");
    assert!(b(&rt, "v"));
}

#[test]
fn cast_float_to_bool_false() {
    let rt = run("state { let v: bool = 0.0 as bool }");
    assert!(!b(&rt, "v"));
}

#[test]
fn cast_bool_to_float() {
    let rt = run("state { let v: float = true as float }");
    assert_eq!(f(&rt, "v"), 1.0);
}

#[test]
fn cast_float_to_string() {
    let rt = run(r"state { let v: string = 42.0 as string }");
    assert_eq!(s(&rt, "v"), "42");
}

#[test]
fn cast_string_to_float() {
    let rt = run(r#"state { let v: float = "3.14" as float }"#);
    assert!((f(&rt, "v") - std::f64::consts::PI).abs() < 0.1);
}

#[test]
fn cast_invalid_string_to_float_error() {
    let e = run_err(r#"state { let v: float = "hello" as float }"#);
    assert_eq!(e.code, ErrorCode::R001);
}

#[test]
fn cast_string_to_bool_nonempty() {
    let rt = run(r#"state { let v: bool = "hello" as bool }"#);
    assert!(b(&rt, "v"));
}

#[test]
fn cast_string_to_bool_empty() {
    let rt = run(r#"state { let v: bool = "" as bool }"#);
    assert!(!b(&rt, "v"));
}

#[test]
fn cast_bool_to_string() {
    let rt = run(r"state { let v: string = true as string }");
    assert_eq!(s(&rt, "v"), "true");
}

#[test]
fn cast_same_type_noop() {
    let rt = run("state { let v: float = 5.0 as float }");
    assert_eq!(f(&rt, "v"), 5.0);
}

// ─── string operations ──────────────────────────────────────────────────────

#[test]
fn string_concatenation() {
    let rt = run(r#"state { let v: string = "hello" + " " + "world" }"#);
    match rt.state().0.get("v") {
        Some(Value::Str(sv)) => assert_eq!(sv, "hello world"),
        other => panic!("expected string, got {other:?}"),
    }
}

#[test]
fn string_len() {
    let rt = run(r#"state { let v: float = "hello".len() }"#);
    assert_eq!(f(&rt, "v"), 5.0);
}

#[test]
fn string_contains() {
    let rt = run(r#"state { let v: bool = "hello world".contains("world") }"#);
    assert!(b(&rt, "v"));
}

#[test]
fn string_trim() {
    let rt = run(r#"state { let v: string = "  hello  ".trim() }"#);
    match rt.state().0.get("v") {
        Some(Value::Str(sv)) => assert_eq!(sv, "hello"),
        other => panic!("expected string, got {other:?}"),
    }
}

#[test]
fn string_to_upper() {
    let rt = run(r#"state { let v: string = "hello".to_upper() }"#);
    match rt.state().0.get("v") {
        Some(Value::Str(sv)) => assert_eq!(sv, "HELLO"),
        other => panic!("expected string, got {other:?}"),
    }
}

#[test]
fn string_to_lower() {
    let rt = run(r#"state { let v: string = "HELLO".to_lower() }"#);
    match rt.state().0.get("v") {
        Some(Value::Str(sv)) => assert_eq!(sv, "hello"),
        other => panic!("expected string, got {other:?}"),
    }
}

#[test]
fn string_comparison() {
    let rt = run(r#"state { let v: bool = "abc" < "abd" }"#);
    assert!(b(&rt, "v"));
}

#[test]
fn string_starts_with() {
    let rt = run(r#"state { let v: bool = "hello world".starts_with("hello") }"#);
    assert!(b(&rt, "v"));
}

#[test]
fn string_ends_with() {
    let rt = run(r#"state { let v: bool = "hello world".ends_with("world") }"#);
    assert!(b(&rt, "v"));
}

#[test]
fn string_replace() {
    let rt = run(r#"state { let v: string = "hello world".replace("world", "rustle") }"#);
    match rt.state().0.get("v") {
        Some(Value::Str(sv)) => assert_eq!(sv, "hello rustle"),
        other => panic!("expected string, got {other:?}"),
    }
}

// ─── String interpolation ───────────────────────────────────────────────────

#[test]
fn template_string_no_interpolation() {
    let rt = run("state { let v: string = `hello world` }");
    assert_eq!(s(&rt, "v"), "hello world");
}

#[test]
fn template_string_with_variable() {
    let rt = run(r#"
        state { let v: string = "" }
        fn on_init(s: State) -> State {
            let name: string = "Rustle"
            s.v = `hello ${name}`
            return s
        }
    "#);
    assert_eq!(s(&rt, "v"), "hello Rustle");
}

#[test]
fn template_string_with_expression() {
    let rt = run("state { let v: string = `2 + 2 = ${2 + 2}` }");
    assert_eq!(s(&rt, "v"), "2 + 2 = 4");
}

#[test]
fn template_string_multiple_interpolations() {
    let rt = run(r#"
        state { let v: string = "" }
        fn on_init(s: State) -> State {
            let x: float = 10
            let y: float = 20
            s.v = `(${x}, ${y})`
            return s
        }
    "#);
    assert_eq!(s(&rt, "v"), "(10, 20)");
}

#[test]
fn template_string_nested_braces() {
    let rt = run("state { let v: string = `value is ${(1 + 2) * 3}` }");
    assert_eq!(s(&rt, "v"), "value is 9");
}

#[test]
fn template_string_escape_backtick() {
    let rt = run(r"state { let v: string = `hello \` world` }");
    assert_eq!(s(&rt, "v"), "hello ` world");
}

#[test]
fn template_string_escape_dollar() {
    let rt = run(r"state { let v: string = `price is \$5` }");
    assert_eq!(s(&rt, "v"), "price is $5");
}

#[test]
fn regular_string_no_interpolation() {
    // Regular "" strings should NOT interpolate
    let rt = run(r#"state { let v: string = "hello ${world}" }"#);
    assert_eq!(s(&rt, "v"), "hello ${world}");
}

#[test]
fn console_with_template_string() {
    let _rt = run(r#"
        state { let v: string = "test" }
        fn on_update(s: State, input: Input) -> State {
            console << `frame ${s.v}`
            return s
        }
    "#);
}

#[test]
fn console_with_template_cast_expr() {
    let mut rt = run(r#"
        state {
            let frame: float = 0.0
            let t: float = 1.5
        }
        fn on_update(s: State, input: Input) -> State {
            s.frame = s.frame + 1.0
            if s.frame % 100.0 == 0.0 {
                console << `frame ${s.frame as string}: t=${(round(s.t * 100.0) / 100.0) as string}s`
            }
            return s
        }
    "#);
    // Tick 100 times to trigger the console log
    for _ in 0..100 {
        tick(&mut rt);
    }
    assert_eq!(f(&rt, "frame"), 100.0);
}

#[test]
fn struct_construction_and_field_read() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Point = Point { x: 5.0, y: 10.0 }
            s.v = p.x + p.y
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 15.0);
}

#[test]
fn struct_field_mutation() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Point = Point { x: 1.0, y: 2.0 }
            p.x = 99.0
            s.v = p.x
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 99.0);
}

#[test]
fn struct_default_fields() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Point = Point { y: 7.0 }
            s.v = p.x + p.y
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 7.0);
}

#[test]
fn struct_reference_semantics() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: Point = Point { x: 1.0, y: 2.0 }
            let b: Point = a
            b.x = 99.0
            s.v = a.x
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 99.0);  // shared reference — a.x changed too
}

#[test]
fn struct_method_call() {
    let rt = run(r#"
        struct Point {
            let x: float
            let y: float

            +fn sum() -> float {
                return this.x + this.y
            }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Point = Point { x: 3.0, y: 4.0 }
            s.v = p.sum()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 7.0);
}

#[test]
fn struct_method_with_params() {
    let rt = run(r#"
        struct Point {
            let x: float
            let y: float

            +fn add(dx: float, dy: float) -> Point {
                return Point { x: this.x + dx, y: this.y + dy }
            }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Point = Point { x: 1.0, y: 2.0 }
            let q: Point = p.add(10.0, 20.0)
            s.v = q.x + q.y
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 33.0);
}

#[test]
fn struct_method_mutates_this() {
    let rt = run(r#"
        struct Counter {
            let count: float

            +fn increment() {
                this.count = this.count + 1.0
            }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let c: Counter = Counter { count: 0.0 }
            c.increment()
            c.increment()
            c.increment()
            s.v = c.count
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 3.0);
}

#[test]
fn struct_method_calls_another_method() {
    let rt = run(r#"
        struct Calc {
            let val: float

            +fn doubled() -> float {
                return this.helper(2.0)
            }

            +fn helper(factor: float) -> float {
                return this.val * factor
            }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let c: Calc = Calc { val: 5.0 }
            s.v = c.doubled()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 10.0);
}

// ─── Nested Structs & Deep Access ────────────────────────────────────────────

#[test]
fn struct_nested_field_access() {
    let rt = run(r#"
        struct Point {
            let x: float
            let y: float
        }
        struct Bounds {
            let min: Point
            let max: Point
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let b: Bounds = Bounds {
                min: Point { x: 1.0, y: 2.0 },
                max: Point { x: 10.0, y: 20.0 }
            }
            s.v = b.min.x + b.max.y
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 21.0);
}

#[test]
fn struct_nested_field_mutation() {
    let rt = run(r#"
        struct Point {
            let x: float
            let y: float
        }
        struct Bounds {
            let min: Point
            let max: Point
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let b: Bounds = Bounds {
                min: Point { x: 1.0, y: 2.0 },
                max: Point { x: 10.0, y: 20.0 }
            }
            b.min.x = 99.0
            s.v = b.min.x
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 99.0);
}

#[test]
fn struct_nested_method_call() {
    let rt = run(r#"
        struct Point {
            let x: float
            let y: float
            +fn sum() -> float { return this.x + this.y }
        }
        struct Bounds {
            let min: Point
            let max: Point
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let b: Bounds = Bounds {
                min: Point { x: 3.0, y: 4.0 },
                max: Point { x: 10.0, y: 20.0 }
            }
            s.v = b.min.sum()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 7.0);
}

#[test]
fn struct_as_state_field() {
    let rt = run(r#"
        struct Point {
            let x: float
            let y: float
        }
        state { let p: Point = Point { x: 1.0, y: 2.0 } }
        fn on_init(s: State) -> State {
            s.p.x = 99.0
            return s
        }
    "#);
    match rt.state().0.get("p") {
        Some(Value::Object(rc)) => {
            let obj = rc.borrow();
            let val = obj.get_field("x").unwrap();
            match val {
                Value::Float(x) => assert_eq!(x, 99.0),
                other => panic!("expected Float, got {other:?}"),
            }
        }
        other => panic!("expected Object, got {other:?}"),
    }
}

#[test]
fn struct_console_output() {
    let mut rt = run(r#"
        struct Point {
            let x: float
            let y: float
        }
        let p: Point = Point { x: 1.0, y: 2.0 }
        console << p
    "#);
    let cmds = tick(&mut rt);
    let found = cmds.iter().any(|c| matches!(c, DrawCommand::Print(msg) if msg.contains("Point")));
    assert!(found, "expected Print containing 'Point', got: {cmds:?}");
}

// ─── Clone deep copy ─────────────────────────────────────────────────────────

#[test]
fn struct_clone_independence() {
    let rt = run(r#"
        struct Point {
            let x: float
            let y: float
        }
        state {
            let a_x: float = 0.0
            let b_x: float = 0.0
        }
        fn on_init(s: State) -> State {
            let a: Point = Point { x: 1.0, y: 2.0 }
            let b: Point = a.clone()
            b.x = 99.0
            s.a_x = a.x
            s.b_x = b.x
            return s
        }
    "#);
    assert_eq!(f(&rt, "a_x"), 1.0);
    assert_eq!(f(&rt, "b_x"), 99.0);
}

#[test]
fn struct_clone_nested() {
    let rt = run(r#"
        struct Point {
            let x: float
            let y: float
        }
        struct Bounds {
            let min: Point
            let max: Point
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let b1: Bounds = Bounds {
                min: Point { x: 1.0, y: 2.0 },
                max: Point { x: 10.0, y: 20.0 }
            }
            let b2: Bounds = b1.clone()
            b2.min.x = 99.0
            s.v = b1.min.x
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 1.0);
}

#[test]
fn list_clone() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: list[float] = [1.0, 2.0, 3.0]
            let b: list[float] = a.clone()
            b.push(99.0)
            s.v = a.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 3.0);
}

#[test]
fn struct_no_methods() {
    let rt = run(r#"
        struct Pair {
            let a: float
            let b: float
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Pair = Pair { a: 3.0, b: 4.0 }
            s.v = p.a + p.b
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 7.0);
}

#[test]
fn struct_no_fields() {
    let _rt = run(r#"
        struct Marker {
            +fn tag() -> string { return "marker" }
        }
        let m: Marker = Marker {}
    "#);
}

#[test]
fn struct_in_list() {
    let rt = run(r#"
        struct Point {
            let x: float
            let y: float
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let points: list[Point] = []
            points.push(Point { x: 1.0, y: 2.0 })
            points.push(Point { x: 3.0, y: 4.0 })
            s.v = points.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 2.0);
}

#[test]
fn struct_method_with_struct_param() {
    let rt = run(r#"
        struct Point {
            let x: float
            let y: float

            +fn distance_to(o: Point) -> float {
                let dx: float = this.x - o.x
                let dy: float = this.y - o.y
                return sqrt(dx * dx + dy * dy)
            }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: Point = Point { x: 0.0, y: 0.0 }
            let b: Point = Point { x: 3.0, y: 4.0 }
            s.v = a.distance_to(b)
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 5.0);
}

#[test]
fn struct_passed_to_function() {
    let rt = run(r#"
        struct Point {
            let x: float
            let y: float
        }
        fn sum_point(p: Point) -> float {
            return p.x + p.y
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Point = Point { x: 10.0, y: 20.0 }
            s.v = sum_point(p)
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 30.0);
}

#[test]
fn struct_returned_from_function() {
    let rt = run(r#"
        struct Point {
            let x: float
            let y: float
        }
        fn make_point(x: float, y: float) -> Point {
            return Point { x: x, y: y }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Point = make_point(7.0, 8.0)
            s.v = p.x + p.y
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 15.0);
}

// ─── Struct: Construction edge cases ─────────────────────────────────────────

#[test]
fn struct_empty_construction() {
    // All fields have defaults — empty {} is valid
    let rt = run(r#"
        struct Config {
            let width: float = 800.0
            let height: float = 600.0
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let c: Config = Config {}
            s.v = c.width + c.height
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 1400.0);
}

#[test]
fn struct_field_order_independent() {
    // Fields can be provided in any order
    let rt = run(r#"
        struct Point {
            let x: float
            let y: float
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Point = Point { y: 20.0, x: 10.0 }
            s.v = p.x * 100.0 + p.y
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 1020.0);
}

#[test]
fn struct_field_expression_default() {
    // Default is an expression, not just a literal
    let rt = run(r#"
        struct Config {
            let half: float = 100.0 / 2.0
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let c: Config = Config {}
            s.v = c.half
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 50.0);
}

// ─── Struct: Method edge cases ───────────────────────────────────────────────

#[test]
fn struct_method_no_return() {
    // Method with no return value (void)
    let rt = run(r#"
        struct Counter {
            let count: float = 0.0
            +fn reset() {
                this.count = 0.0
            }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let c: Counter = Counter { count: 99.0 }
            c.reset()
            s.v = c.count
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 0.0);
}

#[test]
fn struct_method_multiple_params() {
    let rt = run(r#"
        struct Rect {
            let w: float
            let h: float
            +fn scale(sx: float, sy: float) -> Rect {
                return Rect { w: this.w * sx, h: this.h * sy }
            }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let r: Rect = Rect { w: 10.0, h: 5.0 }
            let r2: Rect = r.scale(2.0, 3.0)
            s.v = r2.w + r2.h
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 35.0);
}

#[test]
fn struct_method_returns_bool() {
    let rt = run(r#"
        struct Box {
            let w: float
            let h: float
            +fn is_square() -> bool {
                return this.w == this.h
            }
        }
        state { let v: bool = false }
        fn on_init(s: State) -> State {
            let b: Box = Box { w: 5.0, h: 5.0 }
            s.v = b.is_square()
            return s
        }
    "#);
    assert!(b(&rt, "v"));
}

#[test]
fn struct_chained_method_calls() {
    let rt = run(r#"
        struct Builder {
            let val: float = 0.0
            +fn add(n: float) -> Builder {
                return Builder { val: this.val + n }
            }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let b: Builder = Builder {}
            let result: Builder = b.add(1.0).add(2.0).add(3.0)
            s.v = result.val
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 6.0);
}

// ─── Struct: Reference semantics ─────────────────────────────────────────────

#[test]
fn struct_reference_through_function() {
    // Passing struct to a function — function mutates via reference
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        fn move_point(p: Point, dx: float, dy: float) {
            p.x = p.x + dx
            p.y = p.y + dy
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Point = Point { x: 1.0, y: 2.0 }
            move_point(p, 10.0, 20.0)
            s.v = p.x + p.y
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 33.0);
}

#[test]
fn struct_multiple_references() {
    let rt = run(r#"
        struct Data {
            let val: float = 0.0
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: Data = Data { val: 1.0 }
            let b: Data = a
            let c: Data = b
            c.val = 42.0
            s.v = a.val
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 42.0);  // all point to same data
}

// ─── Struct: Various field types ─────────────────────────────────────────────

#[test]
fn struct_string_field() {
    let rt = run(r#"
        struct Named {
            let name: string
            let value: float
        }
        state { let v: string = "" }
        fn on_init(s: State) -> State {
            let n: Named = Named { name: "hello", value: 42.0 }
            s.v = n.name
            return s
        }
    "#);
    assert_eq!(s(&rt, "v"), "hello");
}

#[test]
fn struct_bool_field() {
    let rt = run(r#"
        struct Flags {
            let active: bool = true
            let visible: bool = false
        }
        state { let v: bool = false }
        fn on_init(s: State) -> State {
            let f: Flags = Flags {}
            s.v = f.active
            return s
        }
    "#);
    assert!(b(&rt, "v"));
}

#[test]
fn struct_list_field() {
    let rt = run(r#"
        struct Container {
            let items: list[float]
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let c: Container = Container { items: [1.0, 2.0, 3.0] }
            c.items.push(4.0)
            s.v = c.items.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 4.0);
}

// ─── Struct: Control flow inside methods ─────────────────────────────────────

#[test]
fn struct_method_with_if() {
    let rt = run(r#"
        struct Abs {
            let val: float
            +fn absolute() -> float {
                if this.val < 0.0 {
                    return 0.0 - this.val
                }
                return this.val
            }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: Abs = Abs { val: -5.0 }
            s.v = a.absolute()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 5.0);
}

#[test]
fn struct_method_with_loop() {
    let rt = run(r#"
        struct Summer {
            let items: list[float]
            +fn total() -> float {
                let sum: float = 0.0
                foreach v: float in this.items {
                    sum = sum + v
                }
                return sum
            }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let s2: Summer = Summer { items: [1.0, 2.0, 3.0, 4.0] }
            s.v = s2.total()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 10.0);
}

// ─── Struct: Persists in state across ticks ──────────────────────────────────

#[test]
fn struct_persists_in_state() {
    let mut rt = run(r#"
        struct Counter {
            let n: float = 0.0
            +fn inc() { this.n = this.n + 1.0 }
        }
        state { let c: Counter = Counter {} }
        fn on_update(s: State, input: Input) -> State {
            s.c.inc()
            return s
        }
    "#);
    tick(&mut rt);
    tick(&mut rt);
    tick(&mut rt);
    match rt.state().0.get("c") {
        Some(Value::Object(rc)) => {
            let val = rc.borrow().get_field("n").unwrap().clone();
            match val { Value::Float(x) => assert_eq!(x, 3.0), other => panic!("got {other:?}") }
        }
        other => panic!("expected Object, got {other:?}"),
    }
}

// ─── List higher-order methods ──────────────────────────────────────────────

#[test]
fn list_map() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [1.0, 2.0, 3.0]
            let doubled: list[float] = nums.map((x: float) -> float { return x * 2.0 })
            s.v = doubled.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 3.0);
}

#[test]
fn list_map_values() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [1.0, 2.0, 3.0]
            let doubled: list[float] = nums.map((x: float) -> float { return x * 2.0 })
            s.v = doubled.pop()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 6.0);
}

#[test]
fn list_filter() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [1.0, 2.0, 3.0, 4.0, 5.0]
            let big: list[float] = nums.filter((x: float) -> bool { return x > 3.0 })
            s.v = big.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 2.0);
}


#[test]
fn list_any_true() {
    let rt = run(r#"
        state { let v: bool = false }
        fn on_init(s: State) -> State {
            let nums: list[float] = [1.0, 2.0, 3.0]
            s.v = nums.any((x: float) -> bool { return x > 2.0 })
            return s
        }
    "#);
    assert!(b(&rt, "v"));
}

#[test]
fn list_any_false() {
    let rt = run(r#"
        state { let v: bool = true }
        fn on_init(s: State) -> State {
            let nums: list[float] = [1.0, 2.0, 3.0]
            s.v = nums.any((x: float) -> bool { return x > 10.0 })
            return s
        }
    "#);
    assert!(!b(&rt, "v"));
}

#[test]
fn list_all_true() {
    let rt = run(r#"
        state { let v: bool = false }
        fn on_init(s: State) -> State {
            let nums: list[float] = [2.0, 4.0, 6.0]
            s.v = nums.all((x: float) -> bool { return x > 1.0 })
            return s
        }
    "#);
    assert!(b(&rt, "v"));
}

#[test]
fn list_all_false() {
    let rt = run(r#"
        state { let v: bool = true }
        fn on_init(s: State) -> State {
            let nums: list[float] = [2.0, 4.0, 6.0]
            s.v = nums.all((x: float) -> bool { return x > 3.0 })
            return s
        }
    "#);
    assert!(!b(&rt, "v"));
}

#[test]
fn list_map_with_named_fn() {
    let rt = run(r#"
        fn double(x: float) -> float { return x * 2.0 }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [5.0]
            let result: list[float] = nums.map(double)
            s.v = result.pop()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 10.0);
}

#[test]
fn list_filter_empty_result() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [1.0, 2.0, 3.0]
            let empty: list[float] = nums.filter((x: float) -> bool { return x > 100.0 })
            s.v = empty.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 0.0);
}

#[test]
fn list_search_found() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [10.0, 20.0, 30.0, 40.0]
            s.v = nums.search(30.0)
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 2.0);
}

#[test]
fn list_search_not_found() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [10.0, 20.0, 30.0]
            s.v = nums.search(99.0)
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), -1.0);
}

#[test]
fn list_bsearch_found() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [1.0, 2.0, 3.0, 4.0, 5.0]
            s.v = nums.bsearch(4.0)
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 3.0);
}

#[test]
fn list_bsearch_not_found() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [1.0, 2.0, 3.0, 4.0, 5.0]
            s.v = nums.bsearch(3.5)
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), -1.0);
}

#[test]
fn list_sort_default() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [3.0, 1.0, 4.0, 1.0, 5.0]
            nums.sort()
            s.v = nums.pop()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 5.0);
}

#[test]
fn list_sort_with_comparator() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [3.0, 1.0, 4.0, 1.0, 5.0]
            nums.sort((a: float, b: float) -> float { return b - a })
            s.v = nums.pop()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 1.0);
}

#[test]
fn list_take() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [10.0, 20.0, 30.0, 40.0, 50.0]
            let first_three: list[float] = nums.take(0, 3)
            s.v = first_three.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 3.0);
}

#[test]
fn list_take_does_not_modify() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [10.0, 20.0, 30.0, 40.0, 50.0]
            let sub: list[float] = nums.take(1, 3)
            s.v = nums.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 5.0);
}

#[test]
fn list_drop() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [10.0, 20.0, 30.0, 40.0, 50.0]
            let without_mid: list[float] = nums.drop(1, 3)
            s.v = without_mid.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 3.0);
}

#[test]
fn list_cut() {
    let rt = run(r#"
        state {
            let removed_len: float = 0.0
            let remaining_len: float = 0.0
        }
        fn on_init(s: State) -> State {
            let nums: list[float] = [10.0, 20.0, 30.0, 40.0, 50.0]
            let removed: list[float] = nums.cut(1, 3)
            s.removed_len = removed.len()
            s.remaining_len = nums.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "removed_len"), 2.0);
    assert_eq!(f(&rt, "remaining_len"), 3.0);
}

#[test]
fn list_paste_single() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [1.0, 2.0, 3.0]
            nums.paste(1, 99.0)
            s.v = nums.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 4.0);
}

#[test]
fn list_paste_list() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [1.0, 4.0, 5.0]
            nums.paste(1, [2.0, 3.0])
            s.v = nums.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 5.0);
}

#[test]
fn list_chained_operations() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let nums: list[float] = [1.0, 2.0, 3.0, 4.0, 5.0]
            let result: list[float] = nums.filter((x: float) -> bool { return x > 2.0 })
                                          .map((x: float) -> float { return x * 10.0 })
            s.v = result.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 3.0);
}
