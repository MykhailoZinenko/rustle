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
fn on_init(s) -> s   — one-time startup logic (optional)
fn on_update(s, i) -> s — per-frame logic (optional)
fn on_exit(s) -> s   — runs when app stops (optional)
top-level statements — run if no on_update, or for non-animated static setup
```

All sections are optional. The simplest valid script is a single `out <<` statement.

---

## Static scripts

No `on_update` function — top-level statements run every frame:

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

Fields are accessed as `s.field` inside `on_update` and `on_init`.

{: .note }
State field initializers must be plain expressions — no loops, conditionals, or function calls that require iteration. Use `fn on_init` for complex initialization.

---

## `fn on_init(s: State) -> State`

Called exactly once at startup, after state field initializers have run. Use it for setup logic that requires loops, conditionals, or list mutations.

```rust
fn on_init(s: State) -> State {
    resolution(800, 600)
    origin(top_left)
    for let i = 0.0; i < 10.0; i += 1.0 {
        s.points.push(i * 50.0)
    }
    return s
}
```

Rules:
- Signature must be exactly `fn on_init(s: State) -> State`
- `return s` is required — always return the (modified) state
- `resolution()` and `origin()` called here persist for all subsequent frames
- Called before the first `on_update`

---

## `fn on_update(s: State, input: Input) -> State`

Called every frame. Receives current state and per-frame input. Returns updated state.

```rust
fn on_update(s: State, input: Input) -> State {
    s.t += input.dt

    out << circle(vec2(sin(s.t) * 300.0 + 400.0, 300.0), 40.0)

    return s
}
```

Rules:
- Signature must be exactly `fn on_update(s: State, input: Input) -> State`
- `return s` is required
- `input.dt` is the time in seconds since the last frame
- All `out <<` calls inside `on_update` add to the current frame's output

### Input fields

| Field | Type | Description |
|-------|------|-------------|
| `input.dt` | `float` | Seconds elapsed since the previous frame |

---

## `fn on_exit(s: State) -> State`

Runs once when the app stops (user presses Stop in the editor). Use it to clean up or persist state.

```rust
fn on_exit(s: State) -> State {
    // e.g. save s to file, log final values
    return s
}
```

Rules:
- Signature must be exactly `fn on_exit(s: State) -> State`
- `return s` is required
- Called when the runtime's `exit()` method is invoked (e.g. when Stop is pressed)

---

## Execution order per frame

1. `on_update(s, input)` is called with current state and input
2. All `out <<` calls inside `on_update` collect shapes
3. Return value becomes the new state
4. Collected shapes are rendered

For static scripts (no `on_update`):

1. Top-level statements execute
2. All `out <<` calls collect shapes
3. Shapes are rendered

---

## Complete lifecycle diagram

```
startup:
  evaluate state{} initializers
  → call on_init(s) if defined
  → s is now ready

each frame:
  call on_update(s, input)   [or re-run top-level if no on_update]
  collect all out << shapes
  render frame
  s = returned state

on stop (Run/Stop → Stop pressed):
  call on_exit(s) if defined
```

---

## Common mistakes

**Forgetting `return s`**

```rust
fn on_update(s: State, input: Input) -> State {
    s.t += input.dt
    // ❌ missing return s — compiler error
}
```

**Calling `resolution()` in `on_update` without `on_init`**

```rust
fn on_update(s: State, input: Input) -> State {
    resolution(800, 600)   // ✅ works, but re-runs every frame (harmless)
    ...
}
```

Prefer calling `resolution()` in `on_init` — it runs once and persists.

**Trying to initialize state with a loop**

```rust
state {
    let xs: list[float] = [1.0, 2.0, 3.0]   // ✅ literal is fine
    // let ys: list[float] = populate()      // ❌ not supported — use on_init
}
```
