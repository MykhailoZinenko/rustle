//! Runtime behavior tests.
//!
//! Tests the full stack: compile → Runtime::new → tick.
//! State values are inspected after init/tick to verify correctness.
//! Draw commands are inspected for shape emission.

use rustle_lang::{compile, Runtime, Input, Value, DrawCommand};
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
    run_err(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 1.0 / 0.0
            return s
        }
    "#);
}

#[test]
fn float_mod_by_zero_runtime_error() {
    run_err(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 5.0 % 0.0
            return s
        }
    "#);
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
    assert!((f(&rt, "x") - 3.14).abs() < 1e-10);
}

#[test]
fn var_inferred_bool() {
    let rt = run("state { let x = true }");
    assert!(b(&rt, "x"));
}

#[test]
fn var_reassign_in_init() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 10.0
            s.x = s.x + 5.0
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 15.0);
}

#[test]
fn var_local_scope_in_init() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let local = 42.0
            s.x = local
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 42.0);
}

// ─── Control flow ─────────────────────────────────────────────────────────────

#[test]
fn if_true_branch_taken() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            if true { s.x = 1.0 }
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 1.0);
}

#[test]
fn if_false_branch_skipped() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            if false { s.x = 1.0 }
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 0.0);
}

#[test]
fn if_else_false_takes_else() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            if false { s.x = 1.0 } else { s.x = 2.0 }
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 2.0);
}

#[test]
fn if_condition_expression() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let v = 5.0
            if v > 3.0 { s.x = 1.0 } else { s.x = -1.0 }
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 1.0);
}

#[test]
fn compound_assignment() {
    let rt = run(r#"
        state { let x: float = 10.0 }
        fn on_init(s: State) -> State {
            s.x += 5.0
            s.x *= 2.0
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 30.0);
}

#[test]
fn match_float_with_else() {
    let rt = run(r#"
        state { let x: float = 2.0 }
        fn on_init(s: State) -> State {
            match s.x {
                1.0 => { s.x = 10.0 }
                2.0 => { s.x = 20.0 }
                else => { s.x = 99.0 }
            }
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 20.0);
}

#[test]
fn match_no_match_no_else() {
    let rt = run(r#"
        state { let x: float = 99.0 }
        fn on_init(s: State) -> State {
            match s.x {
                1.0 => { s.x = 10.0 }
                2.0 => { s.x = 20.0 }
            }
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 99.0);
}

#[test]
fn match_multi_value_arm() {
    let rt = run(r#"
        state { let x: float = 3.0 }
        fn on_init(s: State) -> State {
            match s.x {
                1.0, 2.0 => { s.x = 12.0 }
                3.0, 4.0 => { s.x = 34.0 }
                else => { s.x = 0.0 }
            }
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 34.0);
}

#[test]
fn inc_dec_prefix_postfix() {
    let rt = run(r#"
        state { let x: float = 5.0 }
        fn on_init(s: State) -> State {
            let a = s.x++   // a = 5, s.x = 6
            let b = ++s.x   // b = 7, s.x = 7
            s.x--           // s.x = 6
            let c = --s.x   // c = 5, s.x = 5
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 5.0);
}

#[test]
fn inc_dec_on_list_index() {
    let rt = run(r#"
        state { let xs: list[float] = [10.0, 20.0] }
        fn on_init(s: State) -> State {
            s.xs[0]++
            s.xs[1]--
            return s
        }
    "#);
    assert_eq!(list_floats(&rt, "xs"), [11.0, 19.0]);
}

#[test]
fn else_if_branches() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let v = 2.0
            if v < 1.0 { s.x = 1.0 } else if v < 3.0 { s.x = 2.0 } else { s.x = 3.0 }
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 2.0);
}

#[test]
fn while_runs_correct_iterations() {
    let rt = run(r#"
        state { let count: float = 0.0 }
        fn on_init(s: State) -> State {
            let i = 0.0
            while i < 5.0 {
                s.count = s.count + 1.0
                i = i + 1.0
            }
            return s
        }
    "#);
    assert_eq!(f(&rt, "count"), 5.0);
}

#[test]
fn while_false_condition_never_runs() {
    let rt = run(r#"
        state { let count: float = 0.0 }
        fn on_init(s: State) -> State {
            while false { s.count = s.count + 1.0 }
            return s
        }
    "#);
    assert_eq!(f(&rt, "count"), 0.0);
}

#[test]
fn for_loop_runs_n_times() {
    let rt = run(r#"
        state { let sum: float = 0.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 5.0; i = i + 1.0 {
                s.sum = s.sum + i
            }
            return s
        }
    "#);
    assert_eq!(f(&rt, "sum"), 10.0); // 0+1+2+3+4
}

#[test]
fn foreach_iterates_all_elements() {
    let rt = run(r#"
        state { let sum: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = [1.0, 2.0, 3.0, 4.0]
            foreach v in xs { s.sum = s.sum + v }
            return s
        }
    "#);
    assert_eq!(f(&rt, "sum"), 10.0);
}

#[test]
fn nested_if_in_for() {
    let rt = run(r#"
        state { let evens: float = 0.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 6.0; i = i + 1.0 {
                if i % 2.0 == 0.0 { s.evens = s.evens + 1.0 }
            }
            return s
        }
    "#);
    assert_eq!(f(&rt, "evens"), 3.0); // 0, 2, 4
}

#[test]
fn nested_for_loops() {
    let rt = run(r#"
        state { let count: float = 0.0 }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 3.0; i = i + 1.0 {
                for let j = 0.0; j < 3.0; j = j + 1.0 {
                    s.count = s.count + 1.0
                }
            }
            return s
        }
    "#);
    assert_eq!(f(&rt, "count"), 9.0);
}

#[test]
fn ternary_true_branch() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 1.0 > 0.0 ? 10.0 : 20.0
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 10.0);
}

#[test]
fn ternary_false_branch() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 1.0 < 0.0 ? 10.0 : 20.0
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 20.0);
}

// ─── Functions ────────────────────────────────────────────────────────────────

#[test]
fn fn_basic_call() {
    let rt = run(r#"
        fn add(a: float, b: float) -> float { return a + b }
        state { let x: float = add(3.0, 4.0) }
    "#);
    assert_eq!(f(&rt, "x"), 7.0);
}

#[test]
fn fn_called_in_init() {
    let rt = run(r#"
        fn square(x: float) -> float { return x * x }
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = square(5.0)
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 25.0);
}

#[test]
fn fn_visible_before_declaration() {
    let rt = run(r#"
        state { let x: float = add(1.0, 2.0) }
        fn add(a: float, b: float) -> float { return a + b }
    "#);
    assert_eq!(f(&rt, "x"), 3.0);
}

#[test]
fn fn_higher_order() {
    let rt = run(r#"
        fn apply(f: fn(float) -> float, x: float) -> float { return f(x) }
        fn double(x: float) -> float { return x * 2.0 }
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = apply(double, 5.0)
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 10.0);
}

#[test]
fn fn_lambda() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            fn triple = (x: float) -> float { return x * 3.0 }
            s.x = triple(4.0)
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 12.0);
}

// ─── Vec2 ─────────────────────────────────────────────────────────────────────

#[test]
fn vec2_construction_and_fields() {
    let rt = run(r#"
        state { let v: vec2 = vec2(3.0, 4.0) }
    "#);
    let (x, y) = v2(&rt, "v");
    assert_eq!(x, 3.0);
    assert_eq!(y, 4.0);
}

#[test]
fn vec2_add() {
    let rt = run(r#"
        state { let v: vec2 = vec2(1.0, 2.0) + vec2(3.0, 4.0) }
    "#);
    assert_eq!(v2(&rt, "v"), (4.0, 6.0));
}

#[test]
fn vec2_sub() {
    let rt = run(r#"
        state { let v: vec2 = vec2(5.0, 5.0) - vec2(1.0, 2.0) }
    "#);
    assert_eq!(v2(&rt, "v"), (4.0, 3.0));
}

#[test]
fn vec2_scalar_mul() {
    let rt = run(r#"
        state { let v: vec2 = vec2(1.0, 2.0) * 3.0 }
    "#);
    assert_eq!(v2(&rt, "v"), (3.0, 6.0));
}

#[test]
fn vec2_scalar_div() {
    let rt = run(r#"
        state { let v: vec2 = vec2(4.0, 6.0) / 2.0 }
    "#);
    assert_eq!(v2(&rt, "v"), (2.0, 3.0));
}

#[test]
fn vec2_length() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = vec2(3.0, 4.0).length()
            return s
        }
    "#);
    assert!((f(&rt, "x") - 5.0).abs() < 1e-10);
}

#[test]
fn vec2_dot_product() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = vec2(1.0, 0.0).dot(vec2(0.0, 1.0))
            return s
        }
    "#);
    assert!((f(&rt, "x") - 0.0).abs() < 1e-10);
}

#[test]
fn vec2_normalize() {
    let rt = run(r#"
        state { let v: vec2 = vec2(0.0, 0.0) }
        fn on_init(s: State) -> State {
            s.v = vec2(3.0, 0.0).normalize()
            return s
        }
    "#);
    let (x, y) = v2(&rt, "v");
    assert!((x - 1.0).abs() < 1e-10);
    assert!(y.abs() < 1e-10);
}

#[test]
fn vec2_normalize_zero_vector_error() {
    run_err(r#"
        state { let v: vec2 = vec2(0.0, 0.0) }
        fn on_init(s: State) -> State {
            s.v = vec2(0.0, 0.0).normalize()
            return s
        }
    "#);
}

#[test]
fn vec2_eq() {
    let rt = run(r#"
        state { let r: bool = vec2(1.0, 2.0) == vec2(1.0, 2.0) }
    "#);
    assert!(b(&rt, "r"));
}

#[test]
fn vec2_neq() {
    let rt = run(r#"
        state { let r: bool = vec2(1.0, 2.0) != vec2(3.0, 4.0) }
    "#);
    assert!(b(&rt, "r"));
}

// ─── Lists ────────────────────────────────────────────────────────────────────

#[test]
fn list_push_increases_len() {
    let rt = run(r#"
        state { let xs: list[float] = [] }
        fn on_init(s: State) -> State {
            s.xs.push(1.0)
            s.xs.push(2.0)
            s.xs.push(3.0)
            return s
        }
    "#);
    assert_eq!(list_floats(&rt, "xs"), vec![1.0, 2.0, 3.0]);
}

#[test]
fn list_pop_removes_last() {
    let rt = run(r#"
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
    "#);
    assert_eq!(f(&rt, "last"), 30.0);
    assert_eq!(list_floats(&rt, "xs"), vec![10.0, 20.0]);
}

#[test]
fn list_pop_empty_runtime_error() {
    run_err(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let empty: list[float] = []
            s.x = empty.pop()
            return s
        }
    "#);
}

#[test]
fn list_index_assignment() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = [10.0, 20.0, 30.0]
            xs[1] = 99.0
            s.x = xs[1]
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 99.0);
}

#[test]
fn list_index_compound_assignment() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = [10.0, 20.0, 30.0]
            xs[1] += 5.0
            s.x = xs[1]
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 25.0);
}

#[test]
fn list_index_access() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = [10.0, 20.0, 30.0]
            s.x = xs[1]
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 20.0);
}

#[test]
fn list_len_field() {
    let rt = run(r#"
        state { let n: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = [1.0, 2.0, 3.0]
            s.n = xs.len
            return s
        }
    "#);
    assert_eq!(f(&rt, "n"), 3.0);
}

#[test]
fn list_len_method() {
    let rt = run(r#"
        state { let n: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = [1.0, 2.0]
            s.n = xs.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "n"), 2.0);
}

#[test]
fn list_foreach_sum() {
    let rt = run(r#"
        state { let sum: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = [1.0, 2.0, 3.0, 4.0, 5.0]
            foreach v in xs { s.sum = s.sum + v }
            return s
        }
    "#);
    assert_eq!(f(&rt, "sum"), 15.0);
}

#[test]
fn list_mutation_is_shared() {
    // Pushing to a list stored in state mutates in-place.
    let rt = run(r#"
        state { let xs: list[float] = [] }
        fn on_init(s: State) -> State {
            for let i = 1.0; i <= 5.0; i = i + 1.0 {
                s.xs.push(i)
            }
            return s
        }
    "#);
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
    let rt = run(r#"
        state {
            let a: float = 2.0 + 3.0
            let b: bool  = 10.0 > 5.0
        }
    "#);
    assert_eq!(f(&rt, "a"), 5.0);
    assert!(b(&rt, "b"));
}

#[test]
fn init_runs_before_first_tick() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 99.0
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 99.0);
}

#[test]
fn update_accumulates_over_ticks() {
    let mut rt = run(r#"
        state { let t: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.t = s.t + 1.0
            return s
        }
    "#);
    tick(&mut rt);
    tick(&mut rt);
    tick(&mut rt);
    assert_eq!(f(&rt, "t"), 3.0);
}

#[test]
fn update_uses_input_dt() {
    let mut rt = run(r#"
        state { let t: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.t = s.t + input.dt
            return s
        }
    "#);
    tick(&mut rt);
    // dt is 0.016 per tick
    assert!((f(&rt, "t") - 0.016).abs() < 1e-10);
}

#[test]
fn init_and_update_together() {
    let mut rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = 10.0
            return s
        }
        fn on_update(s: State, input: Input) -> State {
            s.x = s.x + 1.0
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 10.0);
    tick(&mut rt);
    assert_eq!(f(&rt, "x"), 11.0);
    tick(&mut rt);
    assert_eq!(f(&rt, "x"), 12.0);
}

#[test]
fn on_exit_runs_when_exit_called() {
    let mut rt = run(r#"
        state { let x: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.x = s.x + 1.0
            return s
        }
        fn on_exit(s: State) -> State {
            s.x = 999.0
            return s
        }
    "#);
    tick(&mut rt);
    tick(&mut rt);
    assert_eq!(f(&rt, "x"), 2.0);
    rt.exit().unwrap();
    assert_eq!(f(&rt, "x"), 999.0);
}

// ─── Result type ──────────────────────────────────────────────────────────────

#[test]
fn res_ok_fields() {
    let rt = run(r#"
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
    "#);
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
    let rt = run(r#"
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
    "#);
    assert!(b(&rt, "flag"));
    assert_eq!(f(&rt, "val"), 5.0);
}

#[test]
fn try_catches_div_by_zero() {
    let rt = run(r#"
        state { let flag: bool = true }
        fn on_init(s: State) -> State {
            let r: res<float> = try 1.0 / 0.0
            s.flag = r.ok
            return s
        }
    "#);
    assert!(!b(&rt, "flag"));
}

// ─── Draw output ──────────────────────────────────────────────────────────────

#[test]
fn draw_static_emits_circle() {
    let mut rt = run(r#"
        import shapes { circle }
        out << circle(vec2(0.0, 0.0), 0.5)
    "#);
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 1);
    let DrawCommand::DrawShape(data) = &cmds[0];
    assert!(matches!(data.desc, ShapeDesc::Circle { .. }));
}

#[test]
fn draw_static_emits_rect() {
    let mut rt = run(r#"
        import shapes { rect }
        out << rect(vec2(0.0, 0.0), vec2(1.0, 1.0))
    "#);
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 1);
    let DrawCommand::DrawShape(data) = &cmds[0];
    assert!(matches!(data.desc, ShapeDesc::Rect { .. }));
}

#[test]
fn draw_static_multiple_shapes() {
    let mut rt = run(r#"
        import shapes { circle, rect }
        out << rect(vec2(0.0, 0.0), vec2(2.0, 2.0))
        out << circle(vec2(0.0, 0.0), 0.3)
    "#);
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 2);
}

#[test]
fn draw_static_chained_out() {
    let mut rt = run(r#"
        import shapes { circle, rect }
        let bg = rect(vec2(0.0, 0.0), vec2(2.0, 2.0))
        let c  = circle(vec2(0.0, 0.0), 0.3)
        out << bg << c
    "#);
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 2);
}

#[test]
fn draw_update_emits_each_tick() {
    let mut rt = run(r#"
        import shapes { circle }
        state { let t: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.t = s.t + input.dt
            out << circle(vec2(sin(s.t) * 0.5, 0.0), 0.2)
            return s
        }
    "#);
    let c1 = tick(&mut rt);
    let c2 = tick(&mut rt);
    assert_eq!(c1.len(), 1);
    assert_eq!(c2.len(), 1);
}

#[test]
fn draw_foreach_emits_one_per_element() {
    let mut rt = run(r#"
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
    "#);
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 5);
}

#[test]
fn draw_transform_attached_to_shape() {
    let mut rt = run(r#"
        import shapes { circle }
        let t = transform().scale(2.0)
        let s = circle(vec2(0.0, 0.0), 0.2)
        out << s@t
    "#);
    let cmds = tick(&mut rt);
    assert_eq!(cmds.len(), 1);
    let DrawCommand::DrawShape(data) = &cmds[0];
    assert_eq!(data.transforms.len(), 1);
    assert_eq!(data.transforms[0].sx, 2.0);
    assert_eq!(data.transforms[0].sy, 2.0);
}

#[test]
fn draw_multiple_transforms_accumulated() {
    let mut rt = run(r#"
        import shapes { circle }
        let t1 = transform().scale(2.0)
        let t2 = transform().move(0.5, 0.0)
        let s  = circle(vec2(0.0, 0.0), 0.2)
        out << s@(t1, t2)
    "#);
    let cmds = tick(&mut rt);
    let DrawCommand::DrawShape(data) = &cmds[0];
    assert_eq!(data.transforms.len(), 2);
}

// ─── Coordinate config ────────────────────────────────────────────────────────

#[test]
fn resolution_sets_coord_meta() {
    let mut rt = run(r#"
        import shapes { circle }
        import coords { resolution }
        resolution(800.0, 600.0)
        out << circle(vec2(400.0, 300.0), 50.0)
    "#);
    let cmds = tick(&mut rt);
    let DrawCommand::DrawShape(data) = &cmds[0];
    assert_eq!(data.coord_meta.px_width,  800.0);
    assert_eq!(data.coord_meta.px_height, 600.0);
}

#[test]
fn resolution_in_init_persists_to_tick() {
    let mut rt = run(r#"
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
    "#);
    let cmds = tick(&mut rt);
    let DrawCommand::DrawShape(data) = &cmds[0];
    assert_eq!(data.coord_meta.px_width,  1024.0);
    assert_eq!(data.coord_meta.px_height, 768.0);
}

// ─── Complex / edge cases ─────────────────────────────────────────────────────

#[test]
fn complex_recursive_fn() {
    let rt = run(r#"
        fn factorial(n: float) -> float {
            if n <= 1.0 { return 1.0 }
            return n * factorial(n - 1.0)
        }
        state { let x: float = factorial(5.0) }
    "#);
    assert_eq!(f(&rt, "x"), 120.0);
}

#[test]
fn complex_fibonacci() {
    let rt = run(r#"
        fn fib(n: float) -> float {
            if n <= 1.0 { return n }
            return fib(n - 1.0) + fib(n - 2.0)
        }
        state { let x: float = fib(7.0) }
    "#);
    assert_eq!(f(&rt, "x"), 13.0);
}

#[test]
fn complex_nested_fn_calls_in_expr() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = sqrt(pow(3.0, 2.0) + pow(4.0, 2.0))
            return s
        }
    "#);
    assert!((f(&rt, "x") - 5.0).abs() < 1e-10);
}

#[test]
fn complex_math_expression_chain() {
    let rt = run(r#"
        state { let x: float = 0.0 }
        fn on_init(s: State) -> State {
            s.x = clamp(abs(-15.0), 0.0, 10.0)
            return s
        }
    "#);
    assert_eq!(f(&rt, "x"), 10.0);
}

#[test]
fn complex_list_built_in_loop_then_sum() {
    let rt = run(r#"
        state { let sum: float = 0.0 }
        fn on_init(s: State) -> State {
            let xs: list[float] = []
            for let i = 1.0; i <= 10.0; i = i + 1.0 { xs.push(i) }
            foreach v in xs { s.sum = s.sum + v }
            return s
        }
    "#);
    assert_eq!(f(&rt, "sum"), 55.0); // 1+2+...+10
}

#[test]
fn complex_conditional_accumulation() {
    let rt = run(r#"
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
    "#);
    assert_eq!(f(&rt, "pos"), 10.0);
    assert_eq!(f(&rt, "neg"), -6.0);
}

#[test]
fn complex_state_persists_across_many_ticks() {
    let mut rt = run(r#"
        state { let count: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.count = s.count + 1.0
            return s
        }
    "#);
    for _ in 0..100 {
        tick(&mut rt);
    }
    assert_eq!(f(&rt, "count"), 100.0);
}
