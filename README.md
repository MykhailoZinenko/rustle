# Rustle

A statically-typed scripting language for creative graphics programming. Write scripts, see shapes rendered live on a canvas alongside the editor.

**[Documentation →](https://mykhailozinenko.github.io/rustle/)**

---

## Table of Contents

1. [Overview](#overview)
2. [Script Structure](#script-structure)
3. [Types](#types)
4. [Variables](#variables)
5. [Operators](#operators)
6. [Control Flow](#control-flow)
7. [Functions](#functions)
8. [Imports and Namespaces](#imports-and-namespaces)
9. [Coordinate System](#coordinate-system)
10. [Shapes](#shapes)
11. [Rendering](#rendering)
12. [Transforms](#transforms)
13. [Animation and State](#animation-and-state)
14. [Error Handling](#error-handling)
15. [Built-in Reference](#built-in-reference)
16. [Examples](#examples)

---

## Overview

Rustle is designed around explicit control. Nothing is drawn unless you push it to the output stream. No coordinate system is assumed — you declare the one you want. Types are checked at compile time; errors are shown inline before any code runs.

The draw pipeline:

```
source → compile → Runtime::new → Runtime::tick → Vec<DrawCommand> → wgpu renderer
```

Everything in the draw buffer is rendered once per frame, bottom-to-top in push order.

---

## Script Structure

A script has four optional sections, in this order:

```
imports          — bring namespace members into scope
state {}         — declare persistent inter-frame variables
fn on_init(s) -> s  — one-time setup (optional)
fn on_update(s, input) -> s  — per-frame logic (optional)
fn on_exit(s) -> s  — runs when app stops (optional)
top-level stmts  — run every frame if no update, or once if update present
```

**Static script** — no `update`, top-level code runs every frame:

```
import shapes { rect, circle }
import coords { resolution, origin, top_left }

resolution(800, 600)
origin(top_left)

out << rect(vec2(400, 300), vec2(800, 600))
out << circle(vec2(400, 300), 100)
```

**Animated script** — `update` runs every frame:

```
import shapes { circle }
import coords { resolution }

resolution(800, 600)

state {
    let t: float = 0.0
}

fn on_update(s: State, input: Input) -> State {
    s.t = s.t + input.dt
    out << circle(vec2(sin(s.t) * 200.0 + 400.0, 300.0), 50)
    return s
}
```

---

## Types

### Primitives

| Type | Description |
|------|-------------|
| `float` | The only numeric type. All numbers are floats. No integer type. |
| `bool` | `true` or `false` |
| `string` | Text. Fields: `len: float` (read-only) |

### Vectors

| Type | Fields | Constructors |
|------|--------|--------------|
| `vec2` | `x`, `y` | `vec2(x, y)` |
| `vec3` | `x`, `y`, `z` | `vec3(x, y, z)` |
| `vec4` | `x`, `y`, `z`, `w` | `vec4(x, y, z, w)` |

All vector fields are read-write. Arithmetic operators (`+`, `-`, `*`, `/`) work component-wise between same-type vectors, and between vector and `float` (scalar broadcast).

### Matrices

| Type | Constructor | Purpose |
|------|-------------|---------|
| `mat3` | `mat3()` | Identity 3×3 (2D transforms in homogeneous coords) |
| `mat4` | `mat4()` | Identity 4×4 (3D transforms) |

Direct constructors:

```
mat3_translate(dx, dy)        → mat3
mat3_rotate(degrees)          → mat3
mat3_scale(sx, sy)            → mat3

mat4_translate(x, y, z)       → mat4
mat4_scale(x, y, z)           → mat4
mat4_rotate_x(degrees)        → mat4
mat4_rotate_y(degrees)        → mat4
mat4_rotate_z(degrees)        → mat4
```

### Color

`color` is a distinct type, not an alias for `vec4`. Fields: `r`, `g`, `b`, `a` — all read-write floats in 0.0–1.0.

```
color(r, g, b)          // alpha defaults to 1.0
color(r, g, b, a)       // explicit alpha
```

Named constants: `red`, `green`, `blue`, `white`, `black`, `transparent`.

### Collections

| Type | Description |
|------|-------------|
| `list[T]` | Dynamic, growable. Reference type — copies share the same data. |
| `array[T, N]` | Fixed-size — `array[float, 4]` |

```
let points: list[float] = []
let fixed: array[float, 3] = [1.0, 2.0, 3.0]

points.push(1.0)         // mutates in-place, void
let v = points.pop()     // removes and returns last element
let n = points.len       // field: float
let n = points.len()     // method: same
let first = points[0]    // index access
```

Lists are **reference types** — all copies of a list variable point to the same data. Mutations through any copy are immediately visible everywhere.

### Result Type

`res<T>` — a value that is either a success or an error.

| Field | Type | Description |
|-------|------|-------------|
| `.ok` | `bool` | `true` if success |
| `.value` | `T` | The result — only valid when `.ok` is `true` |
| `.error` | `string` | Error message — only valid when `.ok` is `false` |

```
let r: res<float> = divide(10.0, 0.0)
if r.ok {
    out << circle(vec2(r.value, 0.0), 0.1)
}
```

### Shape

`shape` is the drawable type. All primitives from the `shapes` namespace return a `shape`.

Methods:
- `.in(dx, dy) -> vec2` — returns a point in the shape's local coordinate space. `(0, 0)` is the anchor (typically the center). `dx` and `dy` are offsets from it.

### Transform

`transform` is a chainable transformation value. Create with `transform()`, chain methods, apply with `@`.

### State and Input

`State` — the script's persistent state object, passed into and returned from `update` and `init`.

`Input` — per-frame input data.

| Field | Type |
|-------|------|
| `input.dt` | `float` — seconds since last frame |

---

## Variables

```
let x = 0.5              // inferred float
let x: float = 0.5       // explicit type
const PI = 3.14159       // immutable — reassignment is a compile error
```

Reassignment:

```
x = 1.0
s.speed = s.speed + 1.0   // state field assignment
v.x = 5.0                 // vector component assignment
```

---

## Operators

### Arithmetic

```
x + y    x - y    x * y    x / y    x % y
```

Work on `float`. Also work on `vec2`, `vec3`, `vec4`, `color`, `mat3`, `mat4` (component-wise between same types). Scalar `*` and `/` broadcast a `float` across a vector.

### Comparison

```
x == y    x != y    x < y    x <= y    x > y    x >= y
```

Work on `float` and `bool`.

### Logical

```
a and b    a or b    not a
```

### Ternary

```
let v = condition ? then_value : else_value
```

### Cast

```
let x = some_expr as float
```

### List Index

```
let v = list[0]
```

Indices are truncated to whole numbers at runtime.

### Transform Application

```
let s2 = s@t              // apply one transform
let s2 = s@(t1, t2, t3)  // apply multiple — left to right
```

### Output Push

```
out << shape1
out << bg << s1 << s2     // chained — rendered bottom to top
```

---

## Control Flow

```
// if / else
if x > 0.0 {
    out << circle(vec2(x, 0.0), 0.1)
} else {
    out << rect(vec2(x, 0.0), vec2(0.2, 0.2))
}

// while
let i = 0.0
while i < 10.0 {
    i = i + 1.0
}

// for — loop variable declared with let inside the header
for let i = 0.0; i < 10.0; i = i + 1.0 {
    out << circle(vec2(i * 0.1, 0.0), 0.05)
}

// foreach — iterate over a list
let values: list[float] = [0.1, 0.2, 0.3]
foreach v in values {
    out << circle(vec2(v, 0.0), 0.05)
}

// foreach with explicit element type annotation
foreach v: float in values {
    out << circle(vec2(v, 0.0), 0.05)
}
```

---

## Functions

```
fn add(a: float, b: float) -> float {
    return a + b
}

fn draw_dot(x: float, y: float) {
    out << circle(vec2(x, y), 0.05)
}
```

`return` is always explicit. There are no implicit last-expression returns.

### First-class Functions

```
fn f = add                                    // alias

fn g = (a: float, b: float) -> float {       // lambda
    return a + b
}

fn apply(f: fn(float) -> float, x: float) -> float {
    return f(x)
}
```

### Named Arguments

Any namespace function or shape constructor supports named arguments:

```
rect(vec2(400, 300), vec2(200, 100), origin: top_left)
circle(vec2(400, 300), 100, render: sdf)
```

---

## Imports and Namespaces

```
import shapes { circle, rect }     // import specific members
import render { sdf, fill }
import coords { resolution, origin, top_left }

import render                      // import whole namespace
render.fill                        // access via dot
```

### Available Namespaces

| Namespace | What it provides |
|-----------|-----------------|
| `shapes` | Shape constructors |
| `render` | Render mode constants and functions |
| `coords` | Canvas configuration |

Built-in functions (always available, no import needed): math, vector/matrix constructors, `color`, `transform`, `ok`, `error`.

---

## Coordinate System

Default: NDC (normalized device coordinates), origin at center, -1.0 to 1.0 on both axes.

### Configuring the Canvas

```
import coords { resolution, origin, top_left }

resolution(800, 600)    // set canvas dimensions in pixels — enables px-based layout
origin(top_left)        // move origin to top-left corner
```

Origin constants: `center`, `top_left`, `top_right`, `bottom_left`, `bottom_right`, `top`, `bottom`, `left`, `right`.

These calls set the coordinate context for all shapes created after them. They persist across frames once set.

### Where to Call Them

- At **top-level** — applied once, persists
- In **`init(s)`** — applied once at startup, persists
- In **`update(s, input)`** — re-applied every frame (same result if values don't change)

---

## Shapes

All shapes are from the `shapes` namespace. Each returns a `shape`.

```
import shapes { circle, rect, line, polygon }

circle(vec2(x, y), radius)
circle(vec2(x, y), radius, render: sdf)

rect(vec2(x, y), vec2(width, height))
rect(vec2(x, y), vec2(width, height), origin: top_left)

line(vec2(x1, y1), vec2(x2, y2))

polygon([vec2(0.0, 0.0), vec2(0.5, 1.0), vec2(1.0, 0.0)])
```

Default origin for `rect` is `center`.

### Custom Shapes

```
import shapes { shape }

let s = shape([
    vec2(0.0, 0.0),
    vec2(0.5, 1.0),
    vec2(1.0, 0.0),
])
```

### Local Coordinates

`.in(dx, dy)` returns a point in the shape's local space. The origin is the shape's anchor (center for most shapes). Offsets are in the same units the shape was defined with.

```
let r = rect(vec2(400, 300), vec2(200, 100))

circle(r.in(0.0, 0.0), 10)    // center of r
circle(r.in(50.0, 0.0), 10)   // 50 units right of center
circle(r.in(-50.0, 30.0), 10) // left and above center
```

---

## Rendering

Render mode is set per-shape via the `render:` named argument.

```
import render { sdf, fill, outline, stroke }

circle(pos, r, render: sdf)
circle(pos, r, render: fill)
circle(pos, r, render: outline)
circle(pos, r, render: stroke(2.0))   // stroke with pixel width
```

| Mode | Description |
|------|-------------|
| `sdf` | Signed-distance field — anti-aliased, smooth edges |
| `fill` | Solid fill |
| `outline` | Outline only |
| `stroke(width)` | Stroked with given pixel width |

Default render mode if omitted is `fill`.

### Draw Order

Shapes render in push order — first pushed is at the bottom, last on top.

```
out << background
out << circle1
out << circle2    // on top
```

Or chained:

```
out << background << circle1 << circle2
```

---

## Transforms

`transform` is a chainable value. Methods return a new transform — original is unchanged.

```
let t = transform().move(50.0, 0.0).scale(1.5).rotate(45.0)
```

| Method | Description |
|--------|-------------|
| `.move(dx, dy)` | Translate by dx, dy |
| `.translate(dx, dy)` | Same as `.move` |
| `.scale(s)` | Uniform scale |
| `.rotate(degrees)` | Rotate by degrees |

Apply with `@`:

```
let s2 = s@t             // apply transform t to shape s
s = s@t                  // reassign — updates s in place
```

Apply multiple transforms left-to-right:

```
let s2 = s@(t1, t2, t3)   // t1 applied first, t3 last
```

Transforms are first-class values — define once, reuse:

```
let bounce = transform().move(0.0, 20.0).scale(1.1)

out << circle@bounce
out << rect@bounce
```

---

## Animation and State

### The `state {}` Block

Fields declared here persist between frames. Initializers are plain expressions.

```
state {
    let t: float = 0.0
    let active = true          // type inferred
    let points: list[float] = []
}
```

### The `fn on_update` Function

Called every frame. Receives current state and input, returns new state.

```
fn on_update(s: State, input: Input) -> State {
    s.t = s.t + input.dt
    out << circle(vec2(sin(s.t) * 200.0 + 400.0, 300.0), 50)
    return s
}
```

### The `fn on_init` Function

Called once at startup, after state field initializers. Use it for setup logic that requires loops, conditionals, or `push` — things that can't be written as a plain expression initializer.

```
fn on_init(s: State) -> State {
    resolution(800, 600)
    origin(top_left)
    for let i = 1.0; i <= 10.0; i = i + 1.0 {
        s.points.push(i)
    }
    return s
}
```

`resolution()` and `origin()` called in `init` persist for all subsequent frames.

`return s` is required — the state is passed through.

---

## Error Handling

### Compile-time Errors

Type mismatches, undefined variables, wrong argument counts — caught before execution, shown inline.

### Runtime Errors

Division by zero, pop on empty list, out-of-bounds — stop execution at the failure point. What rendered before the error remains visible.

### `res<T>` — Explicit Error Handling

```
fn divide(a: float, b: float) -> res<float> {
    if b == 0.0 {
        return error("division by zero")
    }
    return ok(a / b)
}

let r = divide(10.0, 0.0)

if r.ok {
    out << circle(vec2(r.value, 0.0), 0.1)
} else {
    out << circle(vec2(0.0, 0.0), 0.1)
}
```

`ok(value)` and `error(message)` are always available without importing.

### `try` Expression

Wraps an expression that might fail at runtime into a `res<T>`:

```
let r: res<float> = try (1.0 / 0.0)

if r.ok {
    let v = r.value
}
```

---

## Built-in Reference

### Math (always available)

| Function | Signature | Description |
|----------|-----------|-------------|
| `sin` | `(float) -> float` | |
| `cos` | `(float) -> float` | |
| `tan` | `(float) -> float` | |
| `asin` | `(float) -> float` | |
| `acos` | `(float) -> float` | |
| `atan` | `(float) -> float` | |
| `atan2` | `(float, float) -> float` | `atan2(y, x)` |
| `sqrt` | `(float) -> float` | |
| `pow` | `(float, float) -> float` | |
| `abs` | `(float) -> float` | |
| `floor` | `(float) -> float` | |
| `ceil` | `(float) -> float` | |
| `round` | `(float) -> float` | |
| `sign` | `(float) -> float` | Returns -1, 0, or 1 |
| `fract` | `(float) -> float` | Fractional part |
| `min` | `(float, float) -> float` | |
| `max` | `(float, float) -> float` | |
| `clamp` | `(float, float, float) -> float` | `clamp(x, lo, hi)` |
| `lerp` | `(float, float, float) -> float` | `lerp(a, b, t)` |

Constants: `PI`, `TAU`

### Constructors (always available)

```
vec2(x, y)
vec3(x, y, z)
vec4(x, y, z, w)
color(r, g, b)
color(r, g, b, a)
transform()
mat3()
mat4()
mat3_translate(dx, dy)
mat3_rotate(degrees)
mat3_scale(sx, sy)
mat4_translate(x, y, z)
mat4_scale(x, y, z)
mat4_rotate_x(degrees)
mat4_rotate_y(degrees)
mat4_rotate_z(degrees)
ok(value)
error(message)
```

### vec2 Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `.length()` | `float` | Euclidean length |
| `.normalize()` | `vec2` | Unit vector. Runtime error on zero vector. |
| `.dot(vec2)` | `float` | Dot product |
| `.distance(vec2)` | `float` | Distance to another point |
| `.lerp(vec2, t)` | `vec2` | Linear interpolation |
| `.abs()` | `vec2` | Component-wise absolute value |
| `.floor()` | `vec2` | Component-wise floor |
| `.ceil()` | `vec2` | Component-wise ceil |
| `.min(vec2)` | `vec2` | Component-wise min |
| `.max(vec2)` | `vec2` | Component-wise max |
| `.perp()` | `vec2` | Perpendicular vector `(-y, x)` |
| `.angle()` | `float` | Angle in radians via `atan2(y, x)` |

### vec3 Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `.length()` | `float` | |
| `.normalize()` | `vec3` | |
| `.dot(vec3)` | `float` | |
| `.cross(vec3)` | `vec3` | Cross product |
| `.distance(vec3)` | `float` | |
| `.lerp(vec3, t)` | `vec3` | |
| `.abs()` | `vec3` | |
| `.floor()` | `vec3` | |
| `.ceil()` | `vec3` | |
| `.min(vec3)` | `vec3` | |
| `.max(vec3)` | `vec3` | |

### vec4 Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `.length()` | `float` | |
| `.normalize()` | `vec4` | |
| `.dot(vec4)` | `float` | |
| `.lerp(vec4, t)` | `vec4` | |
| `.abs()` | `vec4` | |
| `.min(vec4)` | `vec4` | |
| `.max(vec4)` | `vec4` | |

### color Methods and Fields

Fields: `r`, `g`, `b`, `a` (read-write `float`)

| Method | Returns | Description |
|--------|---------|-------------|
| `.lerp(color, t)` | `color` | Interpolate between two colors |
| `.with_alpha(a)` | `color` | Return copy with new alpha |
| `.to_vec4()` | `vec4` | Convert to `vec4(r, g, b, a)` |

Color constants: `red`, `green`, `blue`, `white`, `black`, `transparent`

### mat3 Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `.transpose()` | `mat3` | |
| `.det()` | `float` | Determinant |
| `.inverse()` | `mat3` | Runtime error if not invertible |
| `.mul_vec(vec3)` | `vec3` | Matrix-vector multiply |
| `.scale(s)` | `mat3` | Scale all elements by s |

### mat4 Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `.transpose()` | `mat4` | |
| `.det()` | `float` | Determinant |
| `.inverse()` | `mat4` | Runtime error if not invertible |
| `.mul_vec(vec4)` | `vec4` | Matrix-vector multiply |
| `.scale(s)` | `mat4` | Scale all elements by s |

### list[T] Fields and Methods

| Field/Method | Returns | Description |
|--------------|---------|-------------|
| `.len` | `float` | Number of elements (field) |
| `.len()` | `float` | Number of elements (method) |
| `.push(T)` | void | Append element — mutates in-place |
| `.pop()` | `T` | Remove and return last element — mutates in-place. Runtime error if empty. |

### shape Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `.in(dx, dy)` | `vec2` | Point offset from shape's anchor by dx, dy |

### transform Methods

All methods return a new `transform` (original unchanged).

| Method | Description |
|--------|-------------|
| `.move(dx, dy)` | Translate |
| `.translate(dx, dy)` | Same as `.move` |
| `.scale(s)` | Uniform scale |
| `.rotate(degrees)` | Rotation |

---

## Examples

### Static: grid of circles

```
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

### Animated: bouncing dot

```
import shapes { circle }
import coords { resolution }

resolution(800, 600)

state {
    let t: float = 0.0
}

fn on_update(s: State, input: Input) -> State {
    s.t = s.t + input.dt
    let x = sin(s.t) * 300.0 + 400.0
    let y = abs(sin(s.t * 1.3)) * 400.0 + 100.0
    out << circle(vec2(x, y), 30.0)
    return s
}
```

### Init: populate state with loop

```
import shapes { rect }
import coords { resolution, origin, top_left }

resolution(800, 600)

state {
    let widths: list[float] = []
}

fn on_init(s: State) -> State {
    resolution(800, 600)
    origin(top_left)
    for let i = 1.0; i <= 8.0; i = i + 1.0 {
        s.widths.push(i * 20.0)
    }
    return s
}

fn on_update(s: State, input: Input) -> State {
    foreach w in s.widths {
        out << rect(vec2(400.0, 300.0), vec2(w, w))
    }
    return s
}
```

### Error handling

```
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
    out << circle(vec2(400.0, 300.0), 5.0)
}
```

### Transforms

```
import shapes { circle, rect }
import coords { resolution }

resolution(800, 600)

let spin = transform().rotate(45.0).scale(1.5)
let shift = transform().move(100.0, 0.0)

out << rect(vec2(400.0, 300.0), vec2(100.0, 100.0))@spin
out << circle(vec2(400.0, 300.0), 50.0)@shift
out << circle(vec2(400.0, 300.0), 50.0)@(spin, shift)
```
