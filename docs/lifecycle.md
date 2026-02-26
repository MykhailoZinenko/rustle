---
layout: default
title: Script Lifecycle
nav_order: 3
---

# Script Lifecycle
{: .no_toc }

<details open markdown="block">
  <summary>Contents</summary>
  {: .text-delta }
1. TOC
{:toc}
</details>

---

## Script sections

A Rustle script is made of up to four sections, always in this order:

```
imports              — bring namespace members into scope
state { }            — declare persistent inter-frame fields
fn init(s) -> s      — one-time startup logic (optional)
fn update(s, i) -> s — per-frame logic (optional)
top-level statements — run if no update, or for non-animated static setup
```

All sections are optional. The simplest valid script is a single `out <<` statement.

---

## Static scripts

No `update` function — top-level statements run every frame:

```rust
import shapes { circle }
import coords { resolution, origin, top_left }

resolution(800, 600)
origin(top_left)

out << circle(vec2(400.0, 300.0), 80.0)
```

This is suitable for non-animated visuals. Top-level `resolution()` and `origin()` calls re-run every frame but produce the same result.

---

## The `state {}` block

Fields declared in `state {}` persist between frames. Their initializers are evaluated once at startup.

```rust
state {
    let t: float = 0.0       // explicit type
    let active = true         // type inferred from initializer
    let points: list[float] = []
}
```

Fields are accessed as `s.field` inside `update` and `init`.

{: .note }
State field initializers must be plain expressions — no loops, conditionals, or function calls that require iteration. Use `fn init` for complex initialization.

---

## `fn init(s: State) -> State`

Called exactly once at startup, after state field initializers have run. Use it for setup logic that requires loops, conditionals, or list mutations.

```rust
fn init(s: State) -> State {
    resolution(800, 600)
    origin(top_left)
    for let i = 0.0; i < 10.0; i = i + 1.0 {
        s.points.push(i * 50.0)
    }
    return s
}
```

Rules:
- Signature must be exactly `fn init(s: State) -> State`
- `return s` is required — always return the (modified) state
- `resolution()` and `origin()` called here persist for all subsequent frames
- Called before the first `update`

---

## `fn update(s: State, input: Input) -> State`

Called every frame. Receives current state and per-frame input. Returns updated state.

```rust
fn update(s: State, input: Input) -> State {
    s.t = s.t + input.dt

    out << circle(vec2(sin(s.t) * 300.0 + 400.0, 300.0), 40.0)

    return s
}
```

Rules:
- Signature must be exactly `fn update(s: State, input: Input) -> State`
- `return s` is required
- `input.dt` is the time in seconds since the last frame
- All `out <<` calls inside `update` add to the current frame's output

### Input fields

| Field | Type | Description |
|-------|------|-------------|
| `input.dt` | `float` | Seconds elapsed since the previous frame |

---

## Execution order per frame

1. `update(s, input)` is called with current state and input
2. All `out <<` calls inside `update` collect shapes
3. Return value becomes the new state
4. Collected shapes are rendered

For static scripts (no `update`):

1. Top-level statements execute
2. All `out <<` calls collect shapes
3. Shapes are rendered

---

## Complete lifecycle diagram

```
startup:
  evaluate state{} initializers
  → call init(s) if defined
  → s is now ready

each frame:
  call update(s, input)   [or re-run top-level if no update]
  collect all out << shapes
  render frame
  s = returned state
```

---

## Common mistakes

**Forgetting `return s`**

```rust
fn update(s: State, input: Input) -> State {
    s.t = s.t + input.dt
    // ❌ missing return s — compiler error
}
```

**Calling `resolution()` in `update` without `init`**

```rust
fn update(s: State, input: Input) -> State {
    resolution(800, 600)   // ✅ works, but re-runs every frame (harmless)
    ...
}
```

Prefer calling `resolution()` in `init` — it runs once and persists.

**Trying to initialize state with a loop**

```rust
state {
    let xs: list[float] = [1.0, 2.0, 3.0]   // ✅ literal is fine
    // let ys: list[float] = populate()      // ❌ not supported — use init
}
```
