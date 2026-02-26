---
layout: home
title: Home
nav_order: 1
---

# Rustle

A statically-typed scripting language for creative graphics programming.

Write scripts, see shapes rendered live on a canvas alongside the editor. No boilerplate. No hidden state. Every shape is drawn explicitly.

---

## What makes Rustle different

- **Explicit output** — nothing is drawn unless you push it with `out <<`
- **No assumed coordinate system** — declare the one you need with `resolution()` and `origin()`
- **Types checked before execution** — errors appear inline before the script runs
- **Persistent state** — declare what survives between frames in `state {}`, everything else is gone after each tick

## The draw pipeline

```
source → compile → Runtime::new → Runtime::tick → Vec<DrawCommand> → renderer
```

Each `tick` runs the `on_update` function (or top-level code for static scripts) and returns all shapes pushed to `out <<` that frame.

---

## Quick example

```rust
import shapes { circle }
import coords { resolution, origin, top_left }

resolution(800, 600)
origin(top_left)

state {
    let t: float = 0.0
}

fn on_update(s: State, input: Input) -> State {
    s.t += input.dt
    out << circle(vec2(sin(s.t) * 300.0 + 400.0, 300.0), 40.0)
    return s
}
```

---

## Documentation sections

| Section | What it covers |
|---------|---------------|
| [Quick Start]({{ '/quickstart/' | relative_url }}) | Get a script running |
| [Script Lifecycle]({{ '/lifecycle/' | relative_url }}) | How on_init, on_update, and state work |
| [Syntax]({{ '/syntax/' | relative_url }}) | Variables, operators, control flow, functions |
| [Types]({{ '/types/' | relative_url }}) | All types — fields, methods, constructors |
| [Built-ins]({{ '/builtins/' | relative_url }}) | Math, shapes, rendering, coordinate config |
| [Examples]({{ '/examples/' | relative_url }}) | Annotated scripts for common patterns |
| [Error Reference]({{ '/errors/' | relative_url }}) | All error codes with explanations |
