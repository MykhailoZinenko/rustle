---
layout: default
title: Examples
nav_order: 7
---

# Examples
{: .no_toc }

<details open markdown="block">
  <summary>Contents</summary>
  {: .text-delta }
1. TOC
{:toc}
</details>

---

## Minimal — single circle

```rust
import shapes { circle }

out << circle(vec2(0.0, 0.0), 0.3)
```

No coordinate setup. Uses NDC — canvas is -1.0 to 1.0, origin at center.

---

## Grid of circles

```rust
import shapes { circle }
import coords { resolution, origin, top_left }

resolution(800, 600)
origin(top_left)

for let row = 0.0; row < 5.0; row = row + 1.0 {
    for let col = 0.0; col < 8.0; col = col + 1.0 {
        out << circle(vec2(50.0 + col * 100.0, 60.0 + row * 120.0), 30.0)
    }
}
```

---

## Bouncing dot

```rust
import shapes { circle }
import coords { resolution }

resolution(800, 600)

state {
    let t: float = 0.0
}

fn update(s: State, input: Input) -> State {
    s.t = s.t + input.dt
    let x = sin(s.t) * 300.0 + 400.0
    let y = abs(sin(s.t * 1.3)) * 400.0 + 100.0
    out << circle(vec2(x, y), 30.0)
    return s
}
```

---

## Init: populate list in a loop

```rust
import shapes { rect }
import coords { resolution, origin, top_left }

state {
    let widths: list[float] = []
}

fn init(s: State) -> State {
    resolution(800, 600)
    origin(top_left)
    for let i = 1.0; i <= 8.0; i = i + 1.0 {
        s.widths.push(i * 20.0)
    }
    return s
}

fn update(s: State, input: Input) -> State {
    foreach w in s.widths {
        out << rect(vec2(400.0, 300.0), vec2(w, w))
    }
    return s
}
```

---

## Transforms: rotating rectangle

```rust
import shapes { rect }
import coords { resolution }

resolution(800, 600)

state {
    let angle: float = 0.0
}

fn update(s: State, input: Input) -> State {
    s.angle = s.angle + 60.0 * input.dt
    let spin = transform().rotate(s.angle).scale(1.5)
    out << rect(vec2(400.0, 300.0), vec2(150.0, 80.0))@spin
    return s
}
```

---

## Multiple transforms left to right

```rust
import shapes { circle, rect }
import coords { resolution }

resolution(800, 600)

let spin  = transform().rotate(45.0).scale(1.5)
let shift = transform().move(100.0, 0.0)

out << rect(vec2(400.0, 300.0), vec2(100.0, 100.0))@spin
out << circle(vec2(400.0, 300.0), 50.0)@shift
out << circle(vec2(400.0, 300.0), 50.0)@(spin, shift)   // spin first, then shift
```

---

## Render modes

```rust
import shapes { circle }
import render { sdf, fill, outline, stroke }
import coords { resolution, origin, top_left }

resolution(800, 600)
origin(top_left)

out << circle(vec2(100.0, 300.0), 60.0, render: fill)
out << circle(vec2(266.0, 300.0), 60.0, render: sdf)
out << circle(vec2(434.0, 300.0), 60.0, render: outline)
out << circle(vec2(600.0, 300.0), 60.0, render: stroke(3.0))
```

---

## Error handling with `res<T>`

```rust
import shapes { circle }
import coords { resolution }

resolution(800, 600)

fn safe_sqrt(x: float) -> res<float> {
    if x < 0.0 {
        return error("negative input")
    }
    return ok(sqrt(x))
}

let r = safe_sqrt(-4.0)

if r.ok {
    out << circle(vec2(r.value * 100.0 + 400.0, 300.0), 20.0)
} else {
    // r.error == "negative input"
    out << circle(vec2(400.0, 300.0), 5.0)
}
```

---

## try expression

```rust
import shapes { circle }
import coords { resolution }

resolution(800, 600)

state {
    let t: float = 0.0
}

fn update(s: State, input: Input) -> State {
    s.t = s.t + input.dt
    let r: res<float> = try (1.0 / sin(s.t))   // sin(t) passes through 0
    if r.ok {
        out << circle(vec2(clamp(r.value * 50.0 + 400.0, 0.0, 800.0), 300.0), 20.0)
    } else {
        out << circle(vec2(400.0, 300.0), 5.0)
    }
    return s
}
```

---

## Higher-order functions

```rust
import shapes { circle }
import coords { resolution }

resolution(800, 600)

fn apply_to_all(xs: list[float], f: fn(float) -> float) -> list[float] {
    let result: list[float] = []
    foreach v in xs { result.push(f(v)) }
    return result
}

fn wave(x: float) -> float {
    return sin(x * 0.05) * 200.0 + 300.0
}

state {
    let xs: list[float] = []
}

fn init(s: State) -> State {
    for let i = 0.0; i < 8.0; i = i + 1.0 {
        s.xs.push(100.0 + i * 90.0)
    }
    return s
}

fn update(s: State, input: Input) -> State {
    let ys = apply_to_all(s.xs, wave)
    for let i = 0.0; i < s.xs.len; i = i + 1.0 {
        out << circle(vec2(s.xs[i], ys[i]), 15.0)
    }
    return s
}
```

---

## Using `shape.in()` for relative positioning

```rust
import shapes { rect, circle }
import coords { resolution, origin, top_left }

resolution(800, 600)
origin(top_left)

let box = rect(vec2(400.0, 300.0), vec2(200.0, 100.0))

out << box
out << circle(box.in(0.0,   0.0),  8.0)    // center
out << circle(box.in(80.0,  0.0),  8.0)    // right edge area
out << circle(box.in(-80.0, 30.0), 8.0)    // bottom-left area
```
