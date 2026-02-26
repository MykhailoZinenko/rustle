---
layout: default
title: Quick Start
nav_order: 2
---

# Quick Start
{: .no_toc }

<details open markdown="block">
  <summary>Contents</summary>
  {: .text-delta }
1. TOC
{:toc}
</details>

---

## Your first script

Open the Rustle editor and paste this into the script panel:

```rust
import shapes { circle }

out << circle(vec2(0.0, 0.0), 0.3)
```

You should see a white circle in the center of the canvas. That's it — no setup, no main function.

---

## Adding a coordinate system

By default Rustle uses NDC (normalized device coordinates): the canvas goes from -1.0 to 1.0 on both axes, origin at center. For pixel-based layout, declare a resolution:

```rust
import shapes { circle }
import coords { resolution, origin, top_left }

resolution(800, 600)
origin(top_left)

out << circle(vec2(400.0, 300.0), 50.0)
```

Now all coordinates are in pixels, origin at the top-left corner.

---

## Drawing multiple shapes

Shapes render in push order — first pushed is at the bottom, last is on top.

```rust
import shapes { circle, rect }
import coords { resolution, origin, top_left }

resolution(800, 600)
origin(top_left)

out << rect(vec2(400.0, 300.0), vec2(800.0, 600.0))   // background
out << circle(vec2(400.0, 300.0), 80.0)               // on top
```

Or chain them:

```rust
out << rect(vec2(400.0, 300.0), vec2(800.0, 600.0)) << circle(vec2(400.0, 300.0), 80.0)
```

---

## Animation

Add a `state {}` block for values that persist between frames, and an `on_update` function that runs every frame:

```rust
import shapes { circle }
import coords { resolution }

resolution(800, 600)

state {
    let t: float = 0.0
}

fn on_update(s: State, input: Input) -> State {
    s.t += input.dt
    let x = sin(s.t) * 300.0 + 400.0
    out << circle(vec2(x, 300.0), 40.0)
    return s
}
```

`input.dt` is the time in seconds since the last frame. `return s` is required — always return the state at the end of `on_update`.

---

## One-time setup with `on_init`

If you need loops or conditionals to initialize state, use `fn on_init`:

```rust
import shapes { rect }
import coords { resolution, origin, top_left }

state {
    let sizes: list[float] = []
}

fn on_init(s: State) -> State {
    resolution(800, 600)
    origin(top_left)
    for let i = 1.0; i <= 6.0; i += 1.0 {
        s.sizes.push(i * 30.0)
    }
    return s
}

fn on_update(s: State, input: Input) -> State {
    foreach sz in s.sizes {
        out << rect(vec2(400.0, 300.0), vec2(sz, sz))
    }
    return s
}
```

`on_init` runs exactly once at startup. `resolution()` and `origin()` called inside `on_init` persist for all subsequent frames.

---

## What's next

- [Script Lifecycle](lifecycle) — understand on_init, on_update, and state in depth
- [Syntax](syntax) — full language syntax reference
- [Types](types) — all types with fields and methods
- [Built-ins](builtins) — shapes, math, rendering options
