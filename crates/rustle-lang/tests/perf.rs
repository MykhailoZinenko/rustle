use std::time::{Duration, Instant};
use rustle_lang::{compile, Runtime, Input};

fn bench_script(label: &str, src: &str, warmup: u32, iterations: u32) -> Duration {
    let program = compile(src).expect(&format!("{label}: compile failed"));
    let mut rt = Runtime::new(program).expect(&format!("{label}: init failed"));
    let input = Input { dt: 1.0 / 60.0, ..Default::default() };

    for _ in 0..warmup {
        let _ = rt.tick(&input);
    }

    let start = Instant::now();
    for _ in 0..iterations {
        let _ = rt.tick(&input);
    }
    start.elapsed() / iterations
}

fn report(label: &str, per_frame: Duration) {
    let fps = 1.0 / per_frame.as_secs_f64();
    eprintln!("  {label:<40} {per_frame:>10.1?}  ({fps:>7.0} FPS)");
}

// ─── Shape count scaling ─────────────────────────────────────────────────────

fn circles_script(n: usize) -> String {
    format!(r#"
import shapes {{ circle }}
import render {{ fill }}
import coords {{ resolution }}
resolution(900, 700)

state {{ let pts: list[vec2] = [] }}

fn on_init(s: State) -> State {{
    for let i: float = 0.0; i < {n}; i++ {{
        s.pts.push(vec2(100 + i % 900, 100 + floor(i / 900) * 10))
    }}
    return s
}}

fn on_update(s: State, input: Input) -> State {{
    let len: float = s.pts.len
    for let i: float = 0.0; i < len; i++ {{
        out << circle(s.pts[i], 4.0, render: fill, color: color(i / len, 0.5, 0.8))
    }}
    return s
}}
"#)
}

#[test]
fn shape_count_scaling() {
    eprintln!("\n=== Shape Count Scaling (circles) ===");
    for n in [100, 500, 1000, 2000, 5000, 10000, 20000] {
        let per_frame = bench_script(
            &format!("{n} circles"),
            &circles_script(n),
            2, 20,
        );
        report(&format!("{n:>6} circles"), per_frame);
    }
}

// ─── Mixed shape types ──────────────────────────────────────────────────────

#[test]
fn mixed_shapes_scaling() {
    eprintln!("\n=== Mixed Shapes (circle + rect + line) ===");
    for n in [100, 500, 1000, 5000, 10000] {
        let src = format!(r#"
import shapes {{ circle, rect, line }}
import render {{ fill, outline, stroke }}
import coords {{ resolution }}
resolution(900, 700)

state {{ let t: float = 0 }}

fn on_init(s: State) -> State {{ return s }}

fn on_update(s: State, input: Input) -> State {{
    s.t = s.t + input.dt
    for let i: float = 0.0; i < {n}; i++ {{
        let x: float = (i % 90) * 10
        let y: float = floor(i / 90) * 10
        let kind: float = i % 3
        if kind == 0 {{
            out << circle(vec2(x, y), 4.0, render: fill, color: color(i / {n}.0, 0.5, 0.8))
        }}
        if kind == 1 {{
            out << rect(vec2(x, y), vec2(8, 8), render: fill, color: color(0.5, i / {n}.0, 0.8))
        }}
        if kind == 2 {{
            out << line(vec2(x, y), vec2(x + 8, y + 8), render: stroke(1.0), color: color(0.8, 0.5, i / {n}.0))
        }}
    }}
    return s
}}
"#);
        let per_frame = bench_script(&format!("{n} mixed"), &src, 2, 20);
        report(&format!("{n:>6} mixed shapes"), per_frame);
    }
}

// ─── Function call overhead ─────────────────────────────────────────────────

#[test]
fn function_call_overhead() {
    eprintln!("\n=== Function Call Overhead ===");

    let inline_src = r#"
import shapes { circle }
import render { fill }
import coords { resolution }
resolution(900, 700)
state { let x: float = 0 }
fn on_init(s: State) -> State { return s }
fn on_update(s: State, input: Input) -> State {
    for let i: float = 0.0; i < 1000; i++ {
        out << circle(vec2(i, i * 0.5), 4.0, render: fill, color: color(i / 1000.0, 0.5, 0.8))
    }
    return s
}
"#;

    let fn_call_src = r#"
import shapes { circle }
import render { fill }
import coords { resolution }
resolution(900, 700)
state { let x: float = 0 }

fn make_color(i: float, n: float) -> color {
    return color(i / n, 0.5, 0.8)
}

fn emit_circle(x: float, y: float, c: color) {
    out << circle(vec2(x, y), 4.0, render: fill, color: c)
}

fn on_init(s: State) -> State { return s }
fn on_update(s: State, input: Input) -> State {
    for let i: float = 0.0; i < 1000; i++ {
        let c: color = make_color(i, 1000.0)
        emit_circle(i, i * 0.5, c)
    }
    return s
}
"#;

    let inline = bench_script("1000 inline", inline_src, 2, 30);
    let fn_call = bench_script("1000 fn_call", fn_call_src, 2, 30);
    report("1000 shapes inline", inline);
    report("1000 shapes via fn calls", fn_call);
    let overhead = fn_call.as_secs_f64() / inline.as_secs_f64();
    eprintln!("  fn call overhead: {overhead:.2}x");
}

// ─── List access patterns ───────────────────────────────────────────────────

#[test]
fn list_access_patterns() {
    eprintln!("\n=== List Access Patterns (5000 elements) ===");

    let index_src = r#"
import shapes { circle }
import render { fill }
import coords { resolution }
resolution(900, 700)
state { let pts: list[vec2] = [] let sum: float = 0 }
fn on_init(s: State) -> State {
    for let i: float = 0.0; i < 5000; i++ {
        s.pts.push(vec2(i, i))
    }
    return s
}
fn on_update(s: State, input: Input) -> State {
    let len: float = s.pts.len
    let sum: float = 0.0
    for let i: float = 0.0; i < len; i++ {
        let p: vec2 = s.pts[i]
        sum = sum + p.x
    }
    s.sum = sum
    return s
}
"#;

    let foreach_src = r#"
import shapes { circle }
import render { fill }
import coords { resolution }
resolution(900, 700)
state { let pts: list[vec2] = [] let sum: float = 0 }
fn on_init(s: State) -> State {
    for let i: float = 0.0; i < 5000; i++ {
        s.pts.push(vec2(i, i))
    }
    return s
}
fn on_update(s: State, input: Input) -> State {
    let sum: float = 0.0
    foreach p in s.pts {
        sum = sum + p.x
    }
    s.sum = sum
    return s
}
"#;

    let idx = bench_script("index", index_src, 2, 30);
    let foreach = bench_script("foreach", foreach_src, 2, 30);
    report("5000 indexed access", idx);
    report("5000 foreach access", foreach);
}

// ─── HSV function (hot path in showcase) ────────────────────────────────────

#[test]
fn hsv_function_cost() {
    eprintln!("\n=== HSV Color Function Cost ===");

    let plain_src = r#"
import shapes { circle }
import render { fill }
import coords { resolution }
resolution(900, 700)
state { let x: float = 0 }
fn on_init(s: State) -> State { return s }
fn on_update(s: State, input: Input) -> State {
    for let i: float = 0.0; i < 2000; i++ {
        out << circle(vec2(i % 900, floor(i / 900) * 10), 4.0, render: fill, color: color(0.8, 0.5, 0.3))
    }
    return s
}
"#;

    let hsv_src = r#"
import shapes { circle }
import render { fill }
import coords { resolution }
resolution(900, 700)
state { let x: float = 0 }

fn hsv(h: float, s: float, v: float) -> color {
    let hh: float = h - floor(h)
    let ii: float = floor(hh * 6.0)
    let f: float = hh * 6.0 - ii
    let p: float = v * (1.0 - s)
    let q: float = v * (1.0 - s * f)
    let tt: float = v * (1.0 - s * (1.0 - f))
    let r: float = v
    let g: float = v
    let b: float = v
    match ii % 6.0 {
        0.0 => { r = v   g = tt  b = p }
        1.0 => { r = q   g = v   b = p }
        2.0 => { r = p   g = v   b = tt }
        3.0 => { r = p   g = q   b = v }
        4.0 => { r = tt  g = p   b = v }
        5.0 => { r = v   g = p   b = q }
        else => {}
    }
    return color(r, g, b)
}

fn on_init(s: State) -> State { return s }
fn on_update(s: State, input: Input) -> State {
    for let i: float = 0.0; i < 2000; i++ {
        let c: color = hsv(i / 2000.0, 0.8, 0.95)
        out << circle(vec2(i % 900, floor(i / 900) * 10), 4.0, render: fill, color: c)
    }
    return s
}
"#;

    let plain = bench_script("plain color", plain_src, 2, 20);
    let hsv = bench_script("hsv function", hsv_src, 2, 20);
    report("2000 circles, plain color", plain);
    report("2000 circles, hsv() per shape", hsv);
    let overhead = hsv.as_secs_f64() / plain.as_secs_f64();
    eprintln!("  hsv() overhead: {overhead:.2}x");
}

// ─── Interpreter overhead breakdown ─────────────────────────────────────────

#[test]
fn interpreter_overhead_breakdown() {
    eprintln!("\n=== Interpreter Overhead Breakdown ===");

    let empty_src = r#"
import coords { resolution }
resolution(900, 700)
state { let x: float = 0 }
fn on_init(s: State) -> State { return s }
fn on_update(s: State, input: Input) -> State { return s }
"#;

    let loop_only_src = r#"
import coords { resolution }
resolution(900, 700)
state { let x: float = 0 }
fn on_init(s: State) -> State { return s }
fn on_update(s: State, input: Input) -> State {
    let sum: float = 0.0
    for let i: float = 0.0; i < 10000; i++ {
        sum = sum + i
    }
    s.x = sum
    return s
}
"#;

    let math_src = r#"
import coords { resolution }
resolution(900, 700)
state { let x: float = 0 }
fn on_init(s: State) -> State { return s }
fn on_update(s: State, input: Input) -> State {
    let sum: float = 0.0
    for let i: float = 0.0; i < 10000; i++ {
        sum = sum + sin(i * 0.01) * cos(i * 0.02) + sqrt(i)
    }
    s.x = sum
    return s
}
"#;

    let empty = bench_script("empty", empty_src, 5, 100);
    let loop_only = bench_script("loop 10k", loop_only_src, 2, 30);
    let math = bench_script("loop 10k + math", math_src, 2, 30);
    report("empty tick", empty);
    report("10k loop iterations (add only)", loop_only);
    report("10k iterations + sin/cos/sqrt", math);
}

#[test]
fn showcase_compiles() {
    let src = include_str!("../../../examples/showcase.rustle");
    let program = rustle_lang::compile(src);
    assert!(program.is_ok(), "showcase errors: {:?}", program.err());
    let mut rt = rustle_lang::Runtime::new(program.unwrap()).unwrap();
    let input = rustle_lang::Input { dt: 1.0/60.0, mouse_x: 100.0, mouse_y: 300.0, mouse_pressed: true, ..Default::default() };
    let result = rt.tick(&input);
    assert!(result.is_ok(), "showcase tick error: {:?}", result.err());
}

#[test]
fn hsv_breakdown() {
    eprintln!("\n=== HSV Breakdown: What Costs What ===");

    // Baseline: just emit circles with literal color
    let baseline = bench_script("baseline", r#"
import shapes { circle }
import render { fill }
import coords { resolution }
resolution(900, 700)
state { let x: float = 0 }
fn on_init(s: State) -> State { return s }
fn on_update(s: State, input: Input) -> State {
    for let i: float = 0.0; i < 2000; i++ {
        out << circle(vec2(i, i), 4.0, render: fill, color: color(0.8, 0.5, 0.3))
    }
    return s
}
"#, 2, 30);

    // Just a function call + return (no work)
    let empty_fn = bench_script("empty fn", r#"
import shapes { circle }
import render { fill }
import coords { resolution }
resolution(900, 700)
state { let x: float = 0 }
fn noop(h: float) -> color { return color(h, 0.5, 0.8) }
fn on_init(s: State) -> State { return s }
fn on_update(s: State, input: Input) -> State {
    for let i: float = 0.0; i < 2000; i++ {
        let c: color = noop(i / 2000.0)
        out << circle(vec2(i, i), 4.0, render: fill, color: c)
    }
    return s
}
"#, 2, 30);

    // fn with 6 let declarations (no match)
    let lets_only = bench_script("6 lets", r#"
import shapes { circle }
import render { fill }
import coords { resolution }
resolution(900, 700)
state { let x: float = 0 }
fn work(h: float, s: float, v: float) -> color {
    let hh: float = h - floor(h)
    let ii: float = floor(hh * 6.0)
    let f: float = hh * 6.0 - ii
    let p: float = v * (1.0 - s)
    let q: float = v * (1.0 - s * f)
    let tt: float = v * (1.0 - s * (1.0 - f))
    return color(p, q, tt)
}
fn on_init(s: State) -> State { return s }
fn on_update(s: State, input: Input) -> State {
    for let i: float = 0.0; i < 2000; i++ {
        let c: color = work(i / 2000.0, 0.8, 0.95)
        out << circle(vec2(i, i), 4.0, render: fill, color: c)
    }
    return s
}
"#, 2, 30);

    // fn with match only (no extra lets)
    let match_only = bench_script("match", r#"
import shapes { circle }
import render { fill }
import coords { resolution }
resolution(900, 700)
state { let x: float = 0 }
fn work(h: float) -> color {
    let ii: float = floor(h * 6.0)
    let r: float = 0.5
    let g: float = 0.5
    let b: float = 0.5
    match ii % 6.0 {
        0.0 => { r = 1.0  g = 0.5  b = 0.0 }
        1.0 => { r = 0.5  g = 1.0  b = 0.0 }
        2.0 => { r = 0.0  g = 1.0  b = 0.5 }
        3.0 => { r = 0.0  g = 0.5  b = 1.0 }
        4.0 => { r = 0.5  g = 0.0  b = 1.0 }
        5.0 => { r = 1.0  g = 0.0  b = 0.5 }
        else => {}
    }
    return color(r, g, b)
}
fn on_init(s: State) -> State { return s }
fn on_update(s: State, input: Input) -> State {
    for let i: float = 0.0; i < 2000; i++ {
        let c: color = work(i / 2000.0)
        out << circle(vec2(i, i), 4.0, render: fill, color: c)
    }
    return s
}
"#, 2, 30);

    // Full HSV
    let full_hsv = bench_script("full hsv", r#"
import shapes { circle }
import render { fill }
import coords { resolution }
resolution(900, 700)
state { let x: float = 0 }
fn hsv(h: float, s: float, v: float) -> color {
    let hh: float = h - floor(h)
    let ii: float = floor(hh * 6.0)
    let f: float = hh * 6.0 - ii
    let p: float = v * (1.0 - s)
    let q: float = v * (1.0 - s * f)
    let tt: float = v * (1.0 - s * (1.0 - f))
    let r: float = v
    let g: float = v
    let b: float = v
    match ii % 6.0 {
        0.0 => { r = v   g = tt  b = p }
        1.0 => { r = q   g = v   b = p }
        2.0 => { r = p   g = v   b = tt }
        3.0 => { r = p   g = q   b = v }
        4.0 => { r = tt  g = p   b = v }
        5.0 => { r = v   g = p   b = q }
        else => {}
    }
    return color(r, g, b)
}
fn on_init(s: State) -> State { return s }
fn on_update(s: State, input: Input) -> State {
    for let i: float = 0.0; i < 2000; i++ {
        let c: color = hsv(i / 2000.0, 0.8, 0.95)
        out << circle(vec2(i, i), 4.0, render: fill, color: c)
    }
    return s
}
"#, 2, 30);

    report("baseline (literal color)", baseline);
    report("+ empty fn call", empty_fn);
    report("+ 6 let bindings + math", lets_only);
    report("+ match 6 arms (3 assigns each)", match_only);
    report("full hsv (lets + match)", full_hsv);

    let base = baseline.as_secs_f64();
    eprintln!("\n  Cost breakdown (per frame, 2000 calls):");
    eprintln!("    fn call:      {:>6.1}µs  ({:.0}%)", (empty_fn.as_secs_f64() - base) * 1e6, (empty_fn.as_secs_f64() - base) / (full_hsv.as_secs_f64() - base) * 100.0);
    eprintln!("    6 lets+math:  {:>6.1}µs  ({:.0}%)", (lets_only.as_secs_f64() - empty_fn.as_secs_f64()) * 1e6, (lets_only.as_secs_f64() - empty_fn.as_secs_f64()) / (full_hsv.as_secs_f64() - base) * 100.0);
    eprintln!("    match 6 arms: {:>6.1}µs  ({:.0}%)", (full_hsv.as_secs_f64() - lets_only.as_secs_f64()) * 1e6, (full_hsv.as_secs_f64() - lets_only.as_secs_f64()) / (full_hsv.as_secs_f64() - base) * 100.0);
}
