//! Extensive semantic/resolver tests.
//!
//! Covers all error codes (S001–S012) and success paths through the full
//! resolve pipeline: Collector → TypeResolver → Validator.

#[cfg(test)]
mod tests {
    use crate::error::{Error, ErrorCode};
    use crate::syntax::lexer::Lexer;
    use crate::namespaces::NamespaceRegistry;
    use crate::syntax::parser::Parser;
    use crate::analysis::{self, symbols::SymbolKind};
    use crate::analysis::checker::type_name;

    // ─── Helpers ─────────────────────────────────────────────────────────────

    fn parse(src: &str) -> crate::syntax::ast::Program {
        let tokens = Lexer::new(src).tokenize().expect("lex failed");
        Parser::new(tokens).parse().expect("parse failed")
    }

    fn resolve(src: &str) -> Result<analysis::ResolveResult, Vec<Error>> {
        let program = parse(src);
        analysis::resolve(&program, &NamespaceRegistry::standard())
    }

    fn resolve_ok(src: &str) -> analysis::ResolveResult {
        resolve(src).expect("expected resolve to succeed")
    }

    fn resolve_err(src: &str) -> Vec<Error> {
        match resolve(src) {
            Ok(_) => panic!("expected resolve to fail"),
            Err(e) => e,
        }
    }

    fn has_code(errors: &[Error], code: ErrorCode) -> bool {
        errors.iter().any(|e| e.code == code)
    }

    fn has_message(errors: &[Error], substr: &str) -> bool {
        errors.iter().any(|e| e.message.contains(substr))
    }

    // ─── S001: undefined symbol ──────────────────────────────────────────────

    #[test]
    fn s001_undefined_in_return() {
        let errs = resolve_err(r#"
            fn add(a: float, b: float) -> float {
                return a + c
            }
        "#);
        assert!(has_code(&errs, ErrorCode::S001));
        assert!(has_message(&errs, "undefined"));
        assert!(has_message(&errs, "c"));
    }

    #[test]
    fn s001_undefined_in_expr_stmt() {
        let errs = resolve_err("let x = foo + 1.0");
        assert!(has_code(&errs, ErrorCode::S001));
        assert!(has_message(&errs, "foo"));
    }

    #[test]
    fn s001_undefined_in_condition() {
        let errs = resolve_err(r#"
            if bad_var { let x = 1.0 }
        "#);
        assert!(has_code(&errs, ErrorCode::S001));
    }

    #[test]
    fn s001_undefined_in_assign_rhs() {
        let errs = resolve_err(r#"
            let x = 0.0
            x = nonexistent
        "#);
        assert!(has_code(&errs, ErrorCode::S001));
    }

    #[test]
    fn s001_undefined_in_call() {
        let errs = resolve_err("let x = unknown_fn(1.0, 2.0)");
        assert!(has_code(&errs, ErrorCode::S001));
    }

    #[test]
    fn s001_undefined_in_field_access() {
        let errs = resolve_err(r#"
            let x = 1.0
            let y = x.nonexistent
        "#);
        assert!(has_code(&errs, ErrorCode::S009)); // field not found, not undefined
    }

    #[test]
    fn s001_undefined_nested_in_binop() {
        let errs = resolve_err(r#"
            fn f(a: float) -> float {
                return a * b + c
            }
        "#);
        assert!(has_code(&errs, ErrorCode::S001));
    }

    #[test]
    fn s001_undefined_in_foreach_body() {
        let errs = resolve_err(r#"
            let xs: list[float] = [1.0, 2.0]
            foreach v in xs {
                let z = v + missing
            }
        "#);
        assert!(has_code(&errs, ErrorCode::S001));
    }

    // ─── S002: type mismatch ─────────────────────────────────────────────────

    #[test]
    fn s002_if_condition_not_bool() {
        let errs = resolve_err(r#"
            let x = 3.14
            if x { let y = 1.0 }
        "#);
        assert!(has_code(&errs, ErrorCode::S002));
        assert!(has_message(&errs, "bool"));
    }

    #[test]
    fn s002_while_condition_not_bool() {
        let errs = resolve_err(r#"
            let i = 0.0
            while i { i = i + 1.0 }
        "#);
        assert!(has_code(&errs, ErrorCode::S002));
    }

    #[test]
    fn s002_for_condition_not_bool() {
        let errs = resolve_err(r#"
            for let i = 0.0; 42.0; i = i + 1.0 { }
        "#);
        assert!(has_code(&errs, ErrorCode::S002));
    }

    #[test]
    fn s002_return_type_mismatch() {
        let errs = resolve_err(r#"
            fn f() -> float {
                return true
            }
        "#);
        assert!(has_code(&errs, ErrorCode::S002));
    }

    #[test]
    fn s002_return_value_from_void() {
        let errs = resolve_err(r#"
            fn f() {
                return 1.0
            }
        "#);
        assert!(has_code(&errs, ErrorCode::S002));
        assert!(has_message(&errs, "void"));
    }

    #[test]
    fn s002_bare_return_when_value_expected() {
        let errs = resolve_err(r#"
            fn f() -> float {
                return
            }
        "#);
        assert!(has_code(&errs, ErrorCode::S002));
    }

    #[test]
    fn s002_var_decl_init_type_mismatch() {
        let errs = resolve_err("let x: float = true");
        assert!(has_code(&errs, ErrorCode::S002));
    }

    #[test]
    fn s002_assign_type_mismatch() {
        let errs = resolve_err(r#"
            let x: float = 0.0
            x = true
        "#);
        assert!(has_code(&errs, ErrorCode::S002));
    }

    #[test]
    fn s002_out_expects_shape() {
        let errs = resolve_err(r#"
            import shapes { circle }
            let x = 3.14
            out << x
        "#);
        assert!(has_code(&errs, ErrorCode::S002));
        assert!(has_message(&errs, "shape"));
    }

    #[test]
    fn s002_arithmetic_requires_float() {
        let errs = resolve_err(r#"
            let a = true
            let b = a + 1.0
        "#);
        assert!(has_code(&errs, ErrorCode::S008));
    }

    #[test]
    fn s002_comparison_requires_float() {
        let errs = resolve_err(r#"
            let a = true
            let b = a < 1.0
        "#);
        assert!(has_code(&errs, ErrorCode::S008));
    }

    #[test]
    fn s002_logical_requires_bool() {
        let errs = resolve_err(r#"
            let a = 1.0
            let b = a and true
        "#);
        assert!(has_code(&errs, ErrorCode::S008));
    }

    #[test]
    fn s002_ternary_branches_different_types() {
        let errs = resolve_err(r#"
            let x = true ? 1.0 : false
        "#);
        assert!(has_code(&errs, ErrorCode::S002));
    }

    #[test]
    fn s002_transform_expects_shape_left() {
        let errs = resolve_err(r#"
            import shapes { circle }
            let t = transform()
            let x = 3.14
            out << x@t
        "#);
        assert!(has_code(&errs, ErrorCode::S002));
    }

    #[test]
    fn s002_list_elements_mixed_types() {
        let errs = resolve_err("let xs = [1.0, 2.0, true]");
        assert!(has_code(&errs, ErrorCode::S002));
    }

    #[test]
    fn s002_fn_var_requires_function() {
        let errs = resolve_err("fn f = 3.14");
        assert!(has_code(&errs, ErrorCode::S002));
    }

    #[test]
    fn s002_foreach_iterable_not_list_or_array() {
        let errs = resolve_err(r#"
            let x = 3.14
            foreach v in x { }
        "#);
        assert!(has_code(&errs, ErrorCode::S002));
    }

    // ─── S003: redeclaration ─────────────────────────────────────────────────

    #[test]
    fn s003_duplicate_var() {
        let errs = resolve_err(r#"
            let x = 1.0
            let x = 2.0
        "#);
        assert!(has_code(&errs, ErrorCode::S003));
    }

    #[test]
    fn s003_duplicate_fn() {
        let errs = resolve_err(r#"
            fn f() -> float { return 1.0 }
            fn f() -> float { return 2.0 }
        "#);
        assert!(has_code(&errs, ErrorCode::S003));
    }

    #[test]
    fn s003_import_redeclares_core() {
        let errs = resolve_err(r#"
            import shapes { circle }
            import shapes { circle }
        "#);
        assert!(has_code(&errs, ErrorCode::S003));
    }

    #[test]
    fn s003_var_same_name_as_fn() {
        let errs = resolve_err(r#"
            fn add(a: float, b: float) -> float { return a + b }
            let add = 3.14
        "#);
        assert!(has_code(&errs, ErrorCode::S003));
    }

    #[test]
    fn s003_local_var_shadows_ok() {
        // Redeclaration in inner scope is allowed (different from top-level)
        let result = resolve_ok(r#"
            let x = 1.0
            fn f() -> float {
                let x = 2.0
                return x
            }
        "#);
        assert!(!result.symbol_table.global_symbols().is_empty());
    }

    // ─── S004: const reassignment ───────────────────────────────────────────

    #[test]
    fn s004_reassign_const() {
        let errs = resolve_err(r#"
            const TAU = 6.28318
            TAU = 3.0
        "#);
        assert!(has_code(&errs, ErrorCode::S004));
        assert!(has_message(&errs, "TAU"));
    }

    #[test]
    fn s004_reassign_const_in_if() {
        let errs = resolve_err(r#"
            const X = 1.0
            if true {
                X = 2.0
            }
        "#);
        assert!(has_code(&errs, ErrorCode::S004));
    }

    #[test]
    fn s004_reassign_const_in_while() {
        let errs = resolve_err(r#"
            const N = 10.0
            let i = 0.0
            while i < N {
                N = 5.0
                i = i + 1.0
            }
        "#);
        assert!(has_code(&errs, ErrorCode::S004));
    }

    #[test]
    fn s004_reassign_const_in_for() {
        let errs = resolve_err(r#"
            const LIMIT = 5.0
            for let i = 0.0; i < LIMIT; i = i + 1.0 {
                LIMIT = 10.0
            }
        "#);
        assert!(has_code(&errs, ErrorCode::S004));
    }

    // ─── S005: unknown namespace ─────────────────────────────────────────────

    #[test]
    fn s005_unknown_namespace() {
        let errs = resolve_err("import nonexistent { foo }");
        assert!(has_code(&errs, ErrorCode::S005));
        assert!(has_message(&errs, "nonexistent"));
    }

    #[test]
    fn s005_unknown_namespace_whole_import() {
        let errs = resolve_err("import fake_ns");
        assert!(has_code(&errs, ErrorCode::S005));
    }

    // ─── S006: member not exported ──────────────────────────────────────────

    #[test]
    fn s006_member_not_exported() {
        let errs = resolve_err("import shapes { circle, not_a_shape }");
        assert!(has_code(&errs, ErrorCode::S006));
        assert!(has_message(&errs, "not_a_shape"));
    }

    #[test]
    fn s006_wrong_namespace_member() {
        let errs = resolve_err("import coords { circle }");
        assert!(has_code(&errs, ErrorCode::S006));
    }

    #[test]
    fn s006_render_unknown_member() {
        let errs = resolve_err("import render { sdf, nonexistent_mode }");
        assert!(has_code(&errs, ErrorCode::S006));
    }

    // ─── S007: wrong argument count ──────────────────────────────────────────

    #[test]
    fn s007_too_few_args() {
        let errs = resolve_err(r#"
            import shapes { circle }
            let c = circle(vec2(0.0, 0.0))
        "#);
        assert!(has_code(&errs, ErrorCode::S007));
    }

    #[test]
    fn s007_too_many_args() {
        let errs = resolve_err(r#"
            import shapes { circle }
            let c = circle(vec2(0.0, 0.0), 0.2, 0.3)
        "#);
        assert!(has_code(&errs, ErrorCode::S007));
    }

    #[test]
    fn s007_vec2_wrong_count() {
        let errs = resolve_err("let v = vec2(1.0)");
        assert!(has_code(&errs, ErrorCode::S007));
    }

    #[test]
    fn s007_vec3_wrong_count_in_nested_call() {
        // vec3(10.0, 20.0) has 2 args but vec3 needs 3 — error must propagate from nested call
        let errs = resolve_err(r#"
            import shapes { circle }
            let s = circle(vec3(10.0, 20.0), 50.0)
        "#);
        assert!(has_code(&errs, ErrorCode::S007));
    }

    #[test]
    fn s002_circle_expects_vec2_not_vec3() {
        let errs = resolve_err(r#"
            import shapes { circle }
            let s = circle(vec3(10.0, 20.0, 0.0), 50.0)
        "#);
        assert!(has_code(&errs, ErrorCode::S002));
        assert!(has_message(&errs, "vec2"));
    }

    #[test]
    fn s007_color_wrong_count() {
        let errs = resolve_err("let c = color(1.0, 0.0)");
        assert!(has_code(&errs, ErrorCode::S007));
    }

    // ─── S008: operator not applicable ─────────────────────────────────────────

    #[test]
    fn s008_index_non_collection() {
        let errs = resolve_err(r#"
            let x = 3.14
            let y = x[0]
        "#);
        assert!(has_code(&errs, ErrorCode::S008));
    }

    #[test]
    fn s008_index_float() {
        let errs = resolve_err(r#"
            let x: float = 1.0
            let y = x[0]
        "#);
        assert!(has_code(&errs, ErrorCode::S008));
    }

    #[test]
    fn s008_compare_incompatible_types() {
        let errs = resolve_err(r#"
            let a = vec2(1.0, 0.0)
            let b = vec2(0.0, 1.0)
            let c = a == 1.0
        "#);
        assert!(has_code(&errs, ErrorCode::S008));
    }

    // ─── S009: field or method not found ─────────────────────────────────────

    #[test]
    fn s009_field_not_on_type() {
        let errs = resolve_err(r#"
            let x = 3.14
            let y = x.foo
        "#);
        assert!(has_code(&errs, ErrorCode::S009));
    }

    #[test]
    fn s009_method_not_on_type() {
        let errs = resolve_err(r#"
            let x = 3.14
            let y = x.move(1.0, 0.0)
        "#);
        assert!(has_code(&errs, ErrorCode::S009));
    }

    #[test]
    fn s009_vec2_invalid_field() {
        let errs = resolve_err(r#"
            let v = vec2(1.0, 0.0)
            let z = v.z
        "#);
        assert!(has_code(&errs, ErrorCode::S009));
    }

    #[test]
    fn s009_res_invalid_field() {
        let errs = resolve_err(r#"
            let r: res<float> = ok(1.0)
            let x = r.bad_field
        "#);
        assert!(has_code(&errs, ErrorCode::S009));
    }

    #[test]
    fn s009_shape_invalid_method() {
        let errs = resolve_err(r#"
            import shapes { circle }
            let s = circle(vec2(0.0, 0.0), 0.2)
            let x = s.bad_method()
        "#);
        assert!(has_code(&errs, ErrorCode::S009));
    }

    // ─── S010: not callable ──────────────────────────────────────────────────

    #[test]
    fn s010_call_non_function() {
        let errs = resolve_err(r#"
            let x = 3.14
            let y = x(1.0)
        "#);
        assert!(has_code(&errs, ErrorCode::S010));
    }

    #[test]
    fn s010_call_float() {
        let errs = resolve_err(r#"
            let f: float = 1.0
            let x = f()
        "#);
        assert!(has_code(&errs, ErrorCode::S010));
    }

    #[test]
    fn s010_call_color() {
        let errs = resolve_err(r#"
            let c = color(1.0, 0.0, 0.0)
            let x = c(1.0)
        "#);
        assert!(has_code(&errs, ErrorCode::S010));
    }

    // ─── S012: invalid update signature ─────────────────────────────────────

    #[test]
    fn s012_update_wrong_param_count() {
        let errs = resolve_err(r#"
            state { let t: float = 0.0 }
            fn update(s: State) -> State {
                return s
            }
        "#);
        assert!(has_code(&errs, ErrorCode::S012));
    }

    #[test]
    fn s012_update_wrong_first_param_type() {
        let errs = resolve_err(r#"
            state { let t: float = 0.0 }
            fn update(s: float, input: Input) -> State {
                return s
            }
        "#);
        assert!(has_code(&errs, ErrorCode::S012));
    }

    #[test]
    fn s012_update_wrong_return_type() {
        let errs = resolve_err(r#"
            state { let t: float = 0.0 }
            fn update(s: State, input: Input) -> float {
                return 1.0
            }
        "#);
        assert!(has_code(&errs, ErrorCode::S012));
    }

    #[test]
    fn s012_update_returns_state_ok() {
        let result = resolve_ok(r#"
            state { let t: float = 0.0 }
            fn update(s: State, input: Input) -> State {
                s.t = s.t + input.dt
                return s
            }
        "#);
        let syms = result.symbol_table.global_symbols();
        let update = syms.iter().find(|s| s.name == "update");
        assert!(update.is_some());
    }

    // ─── Success cases ───────────────────────────────────────────────────────

    #[test]
    fn success_simple_program() {
        let result = resolve_ok(r#"
            import shapes { circle }
            let x: float = 3.14
            let flag = true
            fn add(a: float, b: float) -> float {
                return a + b
            }
        "#);
        assert!(result.warnings.is_empty());
        let syms = result.symbol_table.global_symbols();
        let add = syms.iter().find(|s| s.name == "add").unwrap();
        assert_eq!(add.kind, SymbolKind::Function);
        assert_eq!(type_name(add.ty.as_ref().unwrap()), "fn(float, float) -> float");
    }

    #[test]
    fn success_static_render() {
        let result = resolve_ok(r#"
            import shapes { circle, rect }
            import render { sdf, fill }
            let bg = rect(vec2(0.0, 0.0), vec2(2.0, 2.0), render: fill)
            let c = circle(vec2(0.0, 0.0), 0.5, render: sdf)
            out << bg << c
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_animated_program() {
        let result = resolve_ok(r#"
            import shapes { circle }
            state {
                let t: float = 0.0
                let speed = 1.0
            }
            fn update(s: State, input: Input) -> State {
                s.t = s.t + input.dt * s.speed
                let c = circle(vec2(sin(s.t), 0.0), 0.3)
                out << c
                return s
            }
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_field_access_vec2() {
        let result = resolve_ok(r#"
            let v = vec2(1.0, 2.0)
            let x = v.x
            let y = v.y
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_field_access_res() {
        let result = resolve_ok(r#"
            let r: res<float> = ok(1.0)
            let ok_flag = r.ok
            let val = r.value
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_method_call_transform() {
        let result = resolve_ok(r#"
            let t = transform().move(0.5, 0.5).scale(2.0)
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_transform_operator() {
        let result = resolve_ok(r#"
            import shapes { circle }
            let t = transform().scale(2.0)
            let s = circle(vec2(0.0, 0.0), 0.2)
            out << s@t
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_list_and_foreach() {
        let result = resolve_ok(r#"
            let xs: list[float] = [1.0, 2.0, 3.0]
            foreach v in xs {
                let doubled = v * 2.0
            }
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_higher_order_fn() {
        let result = resolve_ok(r#"
            fn apply(f: fn(float) -> float, x: float) -> float {
                return f(x)
            }
            fn double(a: float) -> float {
                return a * 2.0
            }
            let y = apply(double, 5.0)
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_fn_var_lambda() {
        let result = resolve_ok(r#"
            fn add = (a: float, b: float) -> float { return a + b }
            let x = add(1.0, 2.0)
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_res_from_function() {
        let result = resolve_ok(r#"
            fn safe_div(a: float, b: float) -> res<float> {
                if b == 0.0 {
                    return error("division by zero")
                }
                return ok(a / b)
            }
            let r: res<float> = safe_div(10.0, 2.0)
            if r.ok {
                let val = r.value
            }
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_try_on_division() {
        let result = resolve_ok(r#"
            let r: res<float> = try 10.0 / 2.0
            if r.ok {
                let val = r.value
            }
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_ternary() {
        let result = resolve_ok(r#"
            let x = 5.0
            let y = x > 0.0 ? x : 0.0
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_index_list() {
        let result = resolve_ok(r#"
            let xs: list[float] = [1.0, 2.0, 3.0]
            let first = xs[0]
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_transform_chained_methods() {
        let result = resolve_ok(r#"
            let t = transform().move(0.5, 0.5).scale(2.0).rotate(45.0)
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_multiple_imports() {
        let result = resolve_ok(r#"
            import shapes { circle, rect }
            import render { sdf, fill }
            import coords { resolution, px }
            let c = circle(vec2(0.0, 0.0), 0.2, render: sdf)
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_whole_namespace_dot_notation() {
        let result = resolve_ok(r#"
            import shapes
            out << shapes.circle(vec2(10.0, 20.0), 20.0)
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_whole_namespace_field_access() {
        let result = resolve_ok(r#"
            import render
            let mode = render.fill
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_const_usage() {
        let result = resolve_ok(r#"
            const TAU = 6.28318
            fn circumference(r: float) -> float {
                return TAU * r
            }
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_nested_control_flow() {
        let result = resolve_ok(r#"
            let i = 0.0
            while i < 10.0 {
                if i > 5.0 {
                    let x = i * 2.0
                }
                i = i + 1.0
            }
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_for_loop() {
        let result = resolve_ok(r#"
            for let i = 0.0; i < 5.0; i = i + 1.0 {
                let x = i * 2.0
            }
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn success_state_inferred_type() {
        let result = resolve_ok(r#"
            state {
                let t = 0.0
                let active = true
            }
            fn update(s: State, input: Input) -> State {
                return s
            }
        "#);
        assert!(result.warnings.is_empty());
    }

    // ─── Complex / edge cases ────────────────────────────────────────────────

    #[test]
    fn complex_multiple_errors_collected() {
        let errs = resolve_err(r#"
            import shapes { circle }
            import fake { x }
            let c = circle(vec2(0.0, 0.0))
            out << undefined_var
            fn f() -> float { return bad_ref }
        "#);
        assert!(errs.len() >= 2);
        assert!(has_code(&errs, ErrorCode::S005));
        assert!(has_code(&errs, ErrorCode::S007) || has_code(&errs, ErrorCode::S001));
    }

    #[test]
    fn complex_strict_ordering_inside_function() {
        // Variable used before declaration inside function body
        let errs = resolve_err(r#"
            fn f() -> float {
                let x = y + 1.0
                let y = 2.0
                return x
            }
        "#);
        assert!(has_code(&errs, ErrorCode::S001));
    }

    #[test]
    fn complex_fn_visible_before_decl() {
        // Functions are always visible (no strict ordering)
        let result = resolve_ok(r#"
            let x = add(1.0, 2.0)
            fn add(a: float, b: float) -> float {
                return a + b
            }
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn complex_cast_expr() {
        let result = resolve_ok(r#"
            let x = 5.0
            let y = x as float
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn complex_color_named_constants() {
        let result = resolve_ok(r#"
            let c = red
            let c2 = white
        "#);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn complex_empty_list_typed() {
        let result = resolve_ok(r#"
            let xs: list[float] = []
        "#);
        assert!(result.warnings.is_empty());
    }
}
