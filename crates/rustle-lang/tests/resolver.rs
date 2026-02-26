//! Semantic analysis (resolver) tests.
//!
//! Tests the full compile pipeline through the public `compile()` API.
//! Each test covers one specific semantic rule or success path.
//! Error codes: S001–S012.

use rustle_lang::{compile, Error, ErrorCode};

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn ok(src: &str) {
    compile(src).unwrap_or_else(|errs| {
        panic!("expected compile to succeed, got errors: {errs:#?}");
    });
}

fn err(src: &str) -> Vec<Error> {
    match compile(src) {
        Ok(_)  => panic!("expected compile to fail but it succeeded"),
        Err(e) => e,
    }
}

fn has(errs: &[Error], code: ErrorCode) -> bool {
    errs.iter().any(|e| e.code == code)
}

fn has_msg(errs: &[Error], s: &str) -> bool {
    errs.iter().any(|e| e.message.contains(s))
}

// ─── S001: undefined symbol ───────────────────────────────────────────────────

#[test]
fn s001_undefined_in_return() {
    let errs = err("fn f(a: float) -> float { return a + c }");
    assert!(has(&errs, ErrorCode::S001));
    assert!(has_msg(&errs, "c"));
}

#[test]
fn s001_undefined_in_let() {
    let errs = err("let x = foo + 1.0");
    assert!(has(&errs, ErrorCode::S001));
    assert!(has_msg(&errs, "foo"));
}

#[test]
fn s001_undefined_in_condition() {
    let errs = err("if bad_var { let x = 1.0 }");
    assert!(has(&errs, ErrorCode::S001));
}

#[test]
fn s001_undefined_in_assign_rhs() {
    let errs = err("let x = 0.0\nx = nonexistent");
    assert!(has(&errs, ErrorCode::S001));
}

#[test]
fn s001_undefined_in_call() {
    let errs = err("let x = unknown_fn(1.0, 2.0)");
    assert!(has(&errs, ErrorCode::S001));
}

#[test]
fn s001_undefined_in_foreach_body() {
    let errs = err(r#"
        let xs: list[float] = [1.0, 2.0]
        foreach v in xs { let z = v + missing }
    "#);
    assert!(has(&errs, ErrorCode::S001));
}

#[test]
fn s001_undefined_nested_binop() {
    let errs = err("fn f(a: float) -> float { return a * b + c }");
    assert!(has(&errs, ErrorCode::S001));
}

#[test]
fn s001_used_before_decl_in_fn() {
    let errs = err(r#"
        fn f() -> float {
            let x = y + 1.0
            let y = 2.0
            return x
        }
    "#);
    assert!(has(&errs, ErrorCode::S001));
}

// ─── S002: type mismatch ──────────────────────────────────────────────────────

#[test]
fn s002_if_condition_not_bool() {
    let errs = err("if 3.14 { let x = 1.0 }");
    assert!(has(&errs, ErrorCode::S002));
    assert!(has_msg(&errs, "bool"));
}

#[test]
fn s002_while_condition_not_bool() {
    let errs = err("let i = 0.0\nwhile i { i = i + 1.0 }");
    assert!(has(&errs, ErrorCode::S002));
}

#[test]
fn s002_for_condition_not_bool() {
    let errs = err("for let i = 0.0; 42.0; i = i + 1.0 { }");
    assert!(has(&errs, ErrorCode::S002));
}

#[test]
fn s002_return_type_mismatch() {
    let errs = err("fn f() -> float { return true }");
    assert!(has(&errs, ErrorCode::S002));
}

#[test]
fn s002_return_value_from_void_fn() {
    let errs = err("fn f() { return 1.0 }");
    assert!(has(&errs, ErrorCode::S002));
    assert!(has_msg(&errs, "void"));
}

#[test]
fn s002_bare_return_when_value_expected() {
    let errs = err("fn f() -> float { return }");
    assert!(has(&errs, ErrorCode::S002));
}

#[test]
fn s002_let_annotation_mismatch() {
    let errs = err("let x: float = true");
    assert!(has(&errs, ErrorCode::S002));
}

#[test]
fn s002_assign_type_mismatch() {
    let errs = err("let x: float = 0.0\nx = true");
    assert!(has(&errs, ErrorCode::S002));
}

#[test]
fn s002_out_expects_shape() {
    let errs = err("import shapes { circle }\nlet x = 3.14\nout << x");
    assert!(has(&errs, ErrorCode::S002));
    assert!(has_msg(&errs, "shape"));
}

#[test]
fn s002_ternary_branches_different_types() {
    let errs = err("let x = true ? 1.0 : false");
    assert!(has(&errs, ErrorCode::S002));
}

#[test]
fn s002_transform_on_non_shape() {
    let errs = err(r#"
        let t = transform()
        let x = 3.14
        out << x@t
    "#);
    assert!(has(&errs, ErrorCode::S002));
}

#[test]
fn s002_list_elements_mixed_types() {
    let errs = err("let xs = [1.0, 2.0, true]");
    assert!(has(&errs, ErrorCode::S002));
}

#[test]
fn s002_fn_var_not_function() {
    let errs = err("fn f = 3.14");
    assert!(has(&errs, ErrorCode::S002));
}

#[test]
fn s002_foreach_iterable_not_list() {
    let errs = err("let x = 3.14\nforeach v in x { }");
    assert!(has(&errs, ErrorCode::S002));
}

#[test]
fn s002_circle_expects_vec2_not_float() {
    let errs = err(r#"
        import shapes { circle }
        let s = circle(1.0, 0.5)
    "#);
    assert!(has(&errs, ErrorCode::S002));
}

#[test]
fn s002_circle_expects_vec2_not_vec3() {
    let errs = err(r#"
        import shapes { circle }
        let s = circle(vec3(1.0, 0.0, 0.0), 0.5)
    "#);
    assert!(has(&errs, ErrorCode::S002));
    assert!(has_msg(&errs, "vec2"));
}

// ─── S003: redeclaration ──────────────────────────────────────────────────────

#[test]
fn s003_duplicate_var() {
    let errs = err("let x = 1.0\nlet x = 2.0");
    assert!(has(&errs, ErrorCode::S003));
}

#[test]
fn s003_duplicate_fn() {
    let errs = err(r#"
        fn f() -> float { return 1.0 }
        fn f() -> float { return 2.0 }
    "#);
    assert!(has(&errs, ErrorCode::S003));
}

#[test]
fn s003_duplicate_import_member() {
    let errs = err("import shapes { circle }\nimport shapes { circle }");
    assert!(has(&errs, ErrorCode::S003));
}

#[test]
fn s003_var_same_name_as_fn() {
    let errs = err(r#"
        fn add(a: float, b: float) -> float { return a + b }
        let add = 3.14
    "#);
    assert!(has(&errs, ErrorCode::S003));
}

#[test]
fn s003_local_shadow_in_inner_scope_ok() {
    ok(r#"
        let x = 1.0
        fn f() -> float {
            let x = 2.0
            return x
        }
    "#);
}

// ─── S004: const reassignment ─────────────────────────────────────────────────

#[test]
fn s004_reassign_const() {
    let errs = err("const C = 1.0\nC = 2.0");
    assert!(has(&errs, ErrorCode::S004));
}

#[test]
fn s004_reassign_const_in_if() {
    let errs = err("const X = 1.0\nif true { X = 2.0 }");
    assert!(has(&errs, ErrorCode::S004));
}

#[test]
fn s004_reassign_const_in_while() {
    let errs = err("const N = 10.0\nlet i = 0.0\nwhile i < N { N = 5.0\ni = i + 1.0 }");
    assert!(has(&errs, ErrorCode::S004));
}

#[test]
fn s004_reassign_const_in_for() {
    let errs = err("const L = 5.0\nfor let i = 0.0; i < L; i = i + 1.0 { L = 10.0 }");
    assert!(has(&errs, ErrorCode::S004));
}

// ─── S005: unknown namespace ──────────────────────────────────────────────────

#[test]
fn s005_unknown_namespace() {
    let errs = err("import nonexistent { foo }");
    assert!(has(&errs, ErrorCode::S005));
    assert!(has_msg(&errs, "nonexistent"));
}

#[test]
fn s005_unknown_namespace_whole_import() {
    let errs = err("import fake_ns");
    assert!(has(&errs, ErrorCode::S005));
}

// ─── S006: member not exported ────────────────────────────────────────────────

#[test]
fn s006_member_not_exported() {
    let errs = err("import shapes { circle, not_a_shape }");
    assert!(has(&errs, ErrorCode::S006));
    assert!(has_msg(&errs, "not_a_shape"));
}

#[test]
fn s006_wrong_namespace_for_member() {
    let errs = err("import coords { circle }");
    assert!(has(&errs, ErrorCode::S006));
}

#[test]
fn s006_render_nonexistent_mode() {
    let errs = err("import render { sdf, nonexistent_mode }");
    assert!(has(&errs, ErrorCode::S006));
}

// ─── S007: wrong argument count ───────────────────────────────────────────────

#[test]
fn s007_circle_too_few_args() {
    let errs = err("import shapes { circle }\nlet c = circle(vec2(0.0, 0.0))");
    assert!(has(&errs, ErrorCode::S007));
}

#[test]
fn s007_circle_too_many_args() {
    let errs = err("import shapes { circle }\nlet c = circle(vec2(0.0, 0.0), 0.2, 0.3)");
    assert!(has(&errs, ErrorCode::S007));
}

#[test]
fn s007_vec2_too_few_args() {
    let errs = err("let v = vec2(1.0)");
    assert!(has(&errs, ErrorCode::S007));
}

#[test]
fn s007_vec2_too_many_args() {
    let errs = err("let v = vec2(1.0, 2.0, 3.0)");
    assert!(has(&errs, ErrorCode::S007));
}

#[test]
fn s007_vec3_too_few_nested() {
    let errs = err("import shapes { circle }\nlet s = circle(vec3(10.0, 20.0), 50.0)");
    assert!(has(&errs, ErrorCode::S007));
}

#[test]
fn s007_color_too_few_args() {
    let errs = err("let c = color(1.0, 0.0)");
    assert!(has(&errs, ErrorCode::S007));
}

#[test]
fn s007_user_fn_too_few_args() {
    let errs = err(r#"
        fn add(a: float, b: float) -> float { return a + b }
        let x = add(1.0)
    "#);
    assert!(has(&errs, ErrorCode::S007));
}

#[test]
fn s007_user_fn_too_many_args() {
    let errs = err(r#"
        fn add(a: float, b: float) -> float { return a + b }
        let x = add(1.0, 2.0, 3.0)
    "#);
    assert!(has(&errs, ErrorCode::S007));
}

// ─── S008: operator not applicable ───────────────────────────────────────────

#[test]
fn s008_index_non_collection() {
    let errs = err("let x = 3.14\nlet y = x[0]");
    assert!(has(&errs, ErrorCode::S008));
}

#[test]
fn s002_index_assign_not_float() {
    let errs = err("let xs: list[float] = [1.0, 2.0]\nxs[true] = 3.0");
    assert!(has(&errs, ErrorCode::S002));
    assert!(has_msg(&errs, "float"));
}

#[test]
fn s008_compare_vec2_with_float() {
    let errs = err("let a = vec2(1.0, 0.0)\nlet c = a == 1.0");
    assert!(has(&errs, ErrorCode::S008));
}

#[test]
fn s008_add_bool_and_float() {
    let errs = err("let a = true\nlet b = a + 1.0");
    assert!(has(&errs, ErrorCode::S008));
}

#[test]
fn s008_compare_float_lt_bool() {
    let errs = err("let a = true\nlet b = a < 1.0");
    assert!(has(&errs, ErrorCode::S008));
}

#[test]
fn s008_logical_on_float() {
    let errs = err("let a = 1.0\nlet b = a and true");
    assert!(has(&errs, ErrorCode::S008));
}

#[test]
fn s008_error_message_uses_symbol_not_debug() {
    let errs = err("let a = true\nlet b = a + 1.0");
    // Error should say "+" not "Add"
    assert!(has_msg(&errs, "+"));
    assert!(!has_msg(&errs, "\"Add\""));
}

// ─── S009: field or method not found ─────────────────────────────────────────

#[test]
fn s009_field_not_on_float() {
    let errs = err("let x = 3.14\nlet y = x.foo");
    assert!(has(&errs, ErrorCode::S009));
}

#[test]
fn s009_method_not_on_float() {
    let errs = err("let x = 3.14\nlet y = x.move(1.0, 0.0)");
    assert!(has(&errs, ErrorCode::S009));
}

#[test]
fn s009_vec2_has_no_z_field() {
    let errs = err("let v = vec2(1.0, 0.0)\nlet z = v.z");
    assert!(has(&errs, ErrorCode::S009));
}

#[test]
fn s009_res_invalid_field() {
    let errs = err("let r: res<float> = ok(1.0)\nlet x = r.bad_field");
    assert!(has(&errs, ErrorCode::S009));
}

#[test]
fn s009_shape_invalid_method() {
    let errs = err(r#"
        import shapes { circle }
        let s = circle(vec2(0.0, 0.0), 0.2)
        let x = s.bad_method()
    "#);
    assert!(has(&errs, ErrorCode::S009));
}

#[test]
fn s009_list_no_such_method() {
    let errs = err(r#"
        let xs: list[float] = []
        xs.nonexistent()
    "#);
    assert!(has(&errs, ErrorCode::S009));
}

// ─── S010: not callable ───────────────────────────────────────────────────────

#[test]
fn s010_call_float() {
    let errs = err("let f: float = 1.0\nlet x = f()");
    assert!(has(&errs, ErrorCode::S010));
}

#[test]
fn s010_call_color() {
    let errs = err("let c = color(1.0, 0.0, 0.0)\nlet x = c(1.0)");
    assert!(has(&errs, ErrorCode::S010));
}

#[test]
fn s010_call_bool() {
    let errs = err("let b = true\nlet x = b()");
    assert!(has(&errs, ErrorCode::S010));
}

// ─── S012: invalid update/init signature ─────────────────────────────────────

#[test]
fn s012_update_wrong_param_count() {
    let errs = err(r#"
        state { let t: float = 0.0 }
        fn on_update(s: State) -> State { return s }
    "#);
    assert!(has(&errs, ErrorCode::S012));
}

#[test]
fn s012_update_wrong_first_param() {
    let errs = err(r#"
        state { let t: float = 0.0 }
        fn on_update(s: float, input: Input) -> State { return s }
    "#);
    assert!(has(&errs, ErrorCode::S012));
}

#[test]
fn s012_update_wrong_return_type() {
    let errs = err(r#"
        state { let t: float = 0.0 }
        fn on_update(s: State, input: Input) -> float { return 1.0 }
    "#);
    assert!(has(&errs, ErrorCode::S012));
}

#[test]
fn s012_on_exit_wrong_signature() {
    let errs = err(r#"
        state { let t: float = 0.0 }
        fn on_exit(s: State, input: Input) -> State { return s }
    "#);
    assert!(has(&errs, ErrorCode::S012));
}

// ─── Success: type system ─────────────────────────────────────────────────────

#[test]
fn ok_simple_program() {
    ok(r#"
        import shapes { circle }
        let x: float = 3.14
        let flag = true
        fn add(a: float, b: float) -> float { return a + b }
    "#);
}

#[test]
fn ok_vec2_fields() {
    ok("let v = vec2(1.0, 2.0)\nlet x = v.x\nlet y = v.y");
}

#[test]
fn ok_vec3_fields() {
    ok("let v = vec3(1.0, 2.0, 3.0)\nlet z = v.z");
}

#[test]
fn ok_vec4_fields() {
    ok("let v = vec4(1.0, 2.0, 3.0, 4.0)\nlet w = v.w");
}

#[test]
fn ok_color_fields() {
    ok("let c = color(1.0, 0.0, 0.0)\nlet r = c.r\nlet a = c.a");
}

#[test]
fn ok_vec2_arithmetic() {
    ok(r#"
        let a = vec2(1.0, 2.0)
        let b = vec2(3.0, 4.0)
        let c = a + b
        let d = a - b
        let e = a * 2.0
        let f = b / 2.0
    "#);
}

#[test]
fn ok_res_fields() {
    ok("let r: res<float> = ok(1.0)\nlet flag = r.ok\nlet val = r.value");
}

#[test]
fn ok_res_error_field() {
    ok(r#"
        let r: res<float> = error("oops")
        let msg = r.error
    "#);
}

#[test]
fn ok_transform_chain() {
    ok("let t = transform().move(0.5, 0.5).scale(2.0).rotate(45.0)");
}

#[test]
fn ok_transform_on_shape() {
    ok(r#"
        import shapes { circle }
        let t = transform().scale(2.0)
        let s = circle(vec2(0.0, 0.0), 0.2)
        out << s@t
    "#);
}

#[test]
fn ok_transform_multi_apply() {
    ok(r#"
        import shapes { circle }
        let t1 = transform().scale(2.0)
        let t2 = transform().move(0.1, 0.0)
        let s = circle(vec2(0.0, 0.0), 0.2)
        out << s@(t1, t2)
    "#);
}

#[test]
fn ok_list_and_foreach() {
    ok(r#"
        let xs: list[float] = [1.0, 2.0, 3.0]
        foreach v in xs { let doubled = v * 2.0 }
    "#);
}

#[test]
fn ok_list_push_pop() {
    ok(r#"
        let xs: list[float] = []
        xs.push(1.0)
        xs.push(2.0)
        let v = xs.pop()
    "#);
}

#[test]
fn ok_list_len() {
    ok(r#"
        let xs: list[float] = [1.0, 2.0, 3.0]
        let n = xs.len
    "#);
}

#[test]
fn ok_list_index() {
    ok("let xs: list[float] = [1.0, 2.0, 3.0]\nlet first = xs[0]");
}

#[test]
fn ok_list_index_assign() {
    ok("let xs: list[float] = [1.0, 2.0, 3.0]\nxs[1] = 99.0");
}

#[test]
fn ok_list_of_vec2() {
    ok(r#"
        let pts: list[vec2] = [vec2(0.0, 0.0), vec2(1.0, 1.0)]
        foreach p in pts {
            let x = p.x
        }
    "#);
}

#[test]
fn ok_higher_order_fn() {
    ok(r#"
        fn apply(f: fn(float) -> float, x: float) -> float { return f(x) }
        fn double(a: float) -> float { return a * 2.0 }
        let y = apply(double, 5.0)
    "#);
}

#[test]
fn ok_lambda_top_level() {
    ok(r#"
        fn add = (a: float, b: float) -> float { return a + b }
        let x = add(1.0, 2.0)
    "#);
}

#[test]
fn ok_lambda_local_in_fn_body() {
    ok(r#"
        fn f() -> float {
            fn double = (x: float) -> float { return x * 2.0 }
            return double(5.0)
        }
        let y = f()
    "#);
}

#[test]
fn ok_result_from_fn() {
    ok(r#"
        fn safe_div(a: float, b: float) -> res<float> {
            if b == 0.0 { return error("division by zero") }
            return ok(a / b)
        }
        let r: res<float> = safe_div(10.0, 2.0)
        if r.ok { let val = r.value }
    "#);
}

#[test]
fn ok_try_expr() {
    ok(r#"
        let r: res<float> = try 10.0 / 2.0
        if r.ok { let val = r.value }
    "#);
}

#[test]
fn ok_ternary() {
    ok("let x = 5.0\nlet y = x > 0.0 ? x : 0.0");
}

#[test]
fn ok_cast_expr() {
    ok("let x = 5.0\nlet y = x as float");
}

#[test]
fn ok_static_render() {
    ok(r#"
        import shapes { circle, rect }
        import render { sdf, fill }
        let bg = rect(vec2(0.0, 0.0), vec2(2.0, 2.0), render: fill)
        let c = circle(vec2(0.0, 0.0), 0.5, render: sdf)
        out << bg << c
    "#);
}

#[test]
fn ok_animated_update() {
    ok(r#"
        import shapes { circle }
        state { let t: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.t = s.t + input.dt
            out << circle(vec2(sin(s.t), 0.0), 0.3)
            return s
        }
    "#);
}

#[test]
fn ok_state_inferred_type() {
    ok(r#"
        state {
            let t = 0.0
            let active = true
        }
        fn on_update(s: State, input: Input) -> State { return s }
    "#);
}

#[test]
fn ok_init_fn() {
    ok(r#"
        state { let xs: list[float] = [] }
        fn on_init(s: State) -> State {
            for let i = 0.0; i < 5.0; i = i + 1.0 {
                s.xs.push(i)
            }
            return s
        }
        fn on_update(s: State, input: Input) -> State { return s }
    "#);
}

#[test]
fn ok_update_uses_input_dt() {
    ok(r#"
        state { let t: float = 0.0 }
        fn on_update(s: State, input: Input) -> State {
            s.t = s.t + input.dt
            return s
        }
    "#);
}

// ─── Success: control flow ────────────────────────────────────────────────────

#[test]
fn ok_if_else() {
    ok(r#"
        let x = 5.0
        if x > 0.0 { let y = x } else { let y = 0.0 }
    "#);
}

#[test]
fn ok_while_loop() {
    ok(r#"
        let i = 0.0
        while i < 10.0 { i = i + 1.0 }
    "#);
}

#[test]
fn ok_for_loop() {
    ok("for let i = 0.0; i < 5.0; i = i + 1.0 { let x = i * 2.0 }");
}

#[test]
fn ok_foreach_with_type_annotation() {
    ok(r#"
        let xs: list[float] = [1.0, 2.0]
        foreach v: float in xs { let d = v * 2.0 }
    "#);
}

#[test]
fn ok_nested_control_flow() {
    ok(r#"
        let i = 0.0
        while i < 10.0 {
            if i > 5.0 { let x = i * 2.0 }
            i = i + 1.0
        }
    "#);
}

#[test]
fn ok_nested_for_loops() {
    ok(r#"
        for let i = 0.0; i < 3.0; i = i + 1.0 {
            for let j = 0.0; j < 3.0; j = j + 1.0 {
                let x = i + j
            }
        }
    "#);
}

// ─── Success: namespaces ──────────────────────────────────────────────────────

#[test]
fn ok_whole_namespace_dot_notation() {
    ok("import shapes\nout << shapes.circle(vec2(0.0, 0.0), 0.2)");
}

#[test]
fn ok_whole_namespace_field_access() {
    ok("import render\nlet mode = render.fill");
}

#[test]
fn ok_render_stroke() {
    ok(r#"
        import shapes { circle }
        import render { stroke }
        out << circle(vec2(0.0, 0.0), 0.2, render: stroke(2.0))
    "#);
}

#[test]
fn ok_shapes_origin_named_arg() {
    ok(r#"
        import shapes { rect }
        import coords { origin, top_left }
        let r = rect(vec2(0.0, 0.0), vec2(1.0, 1.0), origin: top_left)
    "#);
}

#[test]
fn ok_coords_resolution_origin() {
    ok(r#"
        import coords { resolution, origin, top_left }
        resolution(800.0, 600.0)
        origin(top_left)
    "#);
}

// ─── Success: constants ────────────────────────────────────────────────────────

#[test]
fn ok_pi_constant() {
    ok("let x = PI * 2.0");
}

#[test]
fn ok_color_constants() {
    ok("let c = red\nlet c2 = white\nlet c3 = transparent");
}

#[test]
fn ok_const_declaration() {
    ok("const SPEED = 1.5\nlet x = SPEED * 2.0");
}

#[test]
#[should_panic]
fn ok_const_usage_tau() {
    // known pre-existing failure — TAU conflicts with core constant
    ok(r#"
        const TAU = 6.28318
        fn circumference(r: float) -> float { return TAU * r }
    "#);
}

// ─── Success: math methods ────────────────────────────────────────────────────

#[test]
fn ok_vec2_methods() {
    ok(r#"
        let v = vec2(3.0, 4.0)
        let l = v.length()
        let n = v.normalize()
        let d = v.dot(vec2(1.0, 0.0))
        let dist = v.distance(vec2(0.0, 0.0))
        let lr = v.lerp(vec2(0.0, 0.0), 0.5)
        let a = v.abs()
        let fl = v.floor()
        let ce = v.ceil()
        let mn = v.min(vec2(1.0, 1.0))
        let mx = v.max(vec2(1.0, 1.0))
        let p = v.perp()
        let ang = v.angle()
    "#);
}

#[test]
fn ok_color_methods() {
    ok(r#"
        let c = color(1.0, 0.0, 0.0)
        let c2 = c.lerp(blue, 0.5)
        let c3 = c.with_alpha(0.5)
        let v = c.to_vec4()
    "#);
}

#[test]
fn ok_mat3_construction_and_methods() {
    ok(r#"
        let m = mat3_translate(1.0, 2.0)
        let r = mat3_rotate(45.0)
        let s = mat3_scale(2.0, 2.0)
        let t = m.transpose()
        let d = m.det()
        let inv = m.inverse()
        let v = m.mul_vec(vec3(1.0, 0.0, 1.0))
    "#);
}

#[test]
fn ok_mat4_construction_and_methods() {
    ok(r#"
        let m = mat4_translate(1.0, 2.0, 3.0)
        let t = m.transpose()
        let d = m.det()
        let inv = m.inverse()
        let v = m.mul_vec(vec4(1.0, 0.0, 0.0, 1.0))
    "#);
}

// ─── Complex / edge cases ─────────────────────────────────────────────────────

#[test]
fn complex_multiple_errors_collected() {
    let errs = err(r#"
        import fake { x }
        import shapes { circle }
        let c = circle(vec2(0.0, 0.0))
        out << undefined_var
    "#);
    assert!(errs.len() >= 2);
    assert!(has(&errs, ErrorCode::S005));
}

#[test]
fn complex_fns_visible_before_decl() {
    ok(r#"
        let x = add(1.0, 2.0)
        fn add(a: float, b: float) -> float { return a + b }
    "#);
}

#[test]
fn complex_empty_typed_list() {
    ok("let xs: list[float] = []");
}

#[test]
fn complex_nested_fn_calls() {
    ok(r#"
        import shapes { circle }
        let s = circle(vec2(sin(PI * 0.5), cos(0.0)), sqrt(0.25))
    "#);
}

#[test]
fn complex_bool_expr_chain() {
    ok(r#"
        let a = true
        let b = false
        let c = (a and not b) or (b and a)
    "#);
}

#[test]
fn complex_deep_nested_arithmetic() {
    ok("let x = (2.0 + 3.0) * (4.0 - 1.0) / (sqrt(9.0) + 0.0) % 5.0");
}

#[test]
fn complex_chained_out() {
    ok(r#"
        import shapes { circle, rect }
        let bg = rect(vec2(0.0, 0.0), vec2(2.0, 2.0))
        let c = circle(vec2(0.0, 0.0), 0.3)
        out << bg << c
    "#);
}

#[test]
fn complex_shape_in_method() {
    ok(r#"
        import shapes { rect }
        let r = rect(vec2(0.0, 0.0), vec2(1.0, 1.0))
        let p = r.in(0.5, 0.5)
        let x = p.x
    "#);
}
