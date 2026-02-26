---
layout: default
title: Types
nav_order: 5
---

# Types
{: .no_toc }

<details open markdown="block">
  <summary>Contents</summary>
  {: .text-delta }
1. TOC
{:toc}
</details>

---

## float

The only numeric type. All numbers in Rustle are floats — there is no integer type.

```rust
let x = 3.14
let y: float = 0.0
```

**Arithmetic:** `+`, `-`, `*`, `/`, `%`  
**Comparison:** `==`, `!=`, `<`, `<=`, `>`, `>=`  
**Unary negation:** `-x`

Division by zero and modulo by zero are **runtime errors**.

---

## bool

```rust
let flag = true
let other: bool = false
```

**Logical:** `and`, `or`, `not`  
**Comparison:** `==`, `!=`

---

## string

```rust
let s = "hello"
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `.len` | `float` | Number of characters (read-only) |

Strings are primarily used for error messages in `res<T>`.

---

## vec2

Two-component float vector.

```rust
let v = vec2(1.0, 2.0)
let x = v.x
v.y = 5.0
```

**Fields:** `x`, `y` — read-write `float`

**Arithmetic:** `+`, `-` between `vec2`s; `*`, `/` with a `float` scalar.

**Methods:**

| Method | Returns | Description |
|--------|---------|-------------|
| `.length()` | `float` | Euclidean length |
| `.normalize()` | `vec2` | Unit vector. **Runtime error** on zero vector. |
| `.dot(vec2)` | `float` | Dot product |
| `.distance(vec2)` | `float` | Distance to another point |
| `.lerp(vec2, t)` | `vec2` | Linear interpolation (`t` = 0 → self, `t` = 1 → other) |
| `.abs()` | `vec2` | Component-wise absolute value |
| `.floor()` | `vec2` | Component-wise floor |
| `.ceil()` | `vec2` | Component-wise ceil |
| `.min(vec2)` | `vec2` | Component-wise minimum |
| `.max(vec2)` | `vec2` | Component-wise maximum |
| `.perp()` | `vec2` | Perpendicular vector `(-y, x)` |
| `.angle()` | `float` | Angle in radians, via `atan2(y, x)` |

---

## vec3

Three-component float vector.

```rust
let v = vec3(1.0, 2.0, 3.0)
```

**Fields:** `x`, `y`, `z` — read-write `float`

**Methods:**

| Method | Returns | Description |
|--------|---------|-------------|
| `.length()` | `float` | |
| `.normalize()` | `vec3` | **Runtime error** on zero vector. |
| `.dot(vec3)` | `float` | |
| `.cross(vec3)` | `vec3` | Cross product |
| `.distance(vec3)` | `float` | |
| `.lerp(vec3, t)` | `vec3` | |
| `.abs()` | `vec3` | |
| `.floor()` | `vec3` | |
| `.ceil()` | `vec3` | |
| `.min(vec3)` | `vec3` | |
| `.max(vec3)` | `vec3` | |

---

## vec4

Four-component float vector.

```rust
let v = vec4(1.0, 2.0, 3.0, 1.0)
```

**Fields:** `x`, `y`, `z`, `w` — read-write `float`

**Methods:**

| Method | Returns | Description |
|--------|---------|-------------|
| `.length()` | `float` | |
| `.normalize()` | `vec4` | **Runtime error** on zero vector. |
| `.dot(vec4)` | `float` | |
| `.lerp(vec4, t)` | `vec4` | |
| `.abs()` | `vec4` | |
| `.min(vec4)` | `vec4` | |
| `.max(vec4)` | `vec4` | |

---

## color

A distinct type — not an alias for `vec4`. RGBA, all components 0.0–1.0.

```rust
let c = color(1.0, 0.0, 0.0)      // alpha defaults to 1.0
let c = color(1.0, 0.0, 0.0, 0.5) // explicit alpha
let c = #FF0000                    // hex literal
```

**Named constants:** `red`, `green`, `blue`, `white`, `black`, `transparent`

**Fields:** `r`, `g`, `b`, `a` — read-write `float`

**Methods:**

| Method | Returns | Description |
|--------|---------|-------------|
| `.lerp(color, t)` | `color` | Interpolate between two colors |
| `.with_alpha(a)` | `color` | Return copy with new alpha value |
| `.to_vec4()` | `vec4` | Convert to `vec4(r, g, b, a)` |

---

## mat3

3×3 matrix, intended for 2D homogeneous transforms.

```rust
let m = mat3()                  // identity
let m = mat3_translate(dx, dy)
let m = mat3_rotate(degrees)
let m = mat3_scale(sx, sy)
```

**Methods:**

| Method | Returns | Description |
|--------|---------|-------------|
| `.transpose()` | `mat3` | |
| `.det()` | `float` | Determinant |
| `.inverse()` | `mat3` | **Runtime error** if not invertible |
| `.mul_vec(vec3)` | `vec3` | Matrix-vector multiply |
| `.scale(s)` | `mat3` | Scale all elements by scalar |

---

## mat4

4×4 matrix, intended for 3D transforms.

```rust
let m = mat4()                       // identity
let m = mat4_translate(x, y, z)
let m = mat4_scale(x, y, z)
let m = mat4_rotate_x(degrees)
let m = mat4_rotate_y(degrees)
let m = mat4_rotate_z(degrees)
```

**Methods:**

| Method | Returns | Description |
|--------|---------|-------------|
| `.transpose()` | `mat4` | |
| `.det()` | `float` | Determinant |
| `.inverse()` | `mat4` | **Runtime error** if not invertible |
| `.mul_vec(vec4)` | `vec4` | Matrix-vector multiply |
| `.scale(s)` | `mat4` | Scale all elements by scalar |

---

## list[T]

A dynamic, growable sequence. **Reference type** — all copies of a list variable share the same underlying data. Mutations through any copy are immediately visible everywhere.

```rust
let xs: list[float] = []
let xs: list[float] = [1.0, 2.0, 3.0]
let xs: list[vec2]  = [vec2(0.0, 0.0), vec2(1.0, 1.0)]
```

**Fields and methods:**

| | Returns | Description |
|-|---------|-------------|
| `.len` | `float` | Number of elements (field) |
| `.len()` | `float` | Number of elements (method) |
| `.push(T)` | void | Append an element — mutates in-place |
| `.pop()` | `T` | Remove and return the last element — mutates in-place. **Runtime error** if empty. |
| `list[i]` | `T` | Index access. Index is a float, truncated to whole number. **Runtime error** if out of bounds. |

---

## res\<T\>

A result value — either success or an error message. Used for explicit error handling.

```rust
let r: res<float> = ok(42.0)
let r: res<float> = error("something went wrong")
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `.ok` | `bool` | `true` if success |
| `.value` | `T` | The success value. Only valid when `.ok` is `true`. |
| `.error` | `string` | The error message. Only valid when `.ok` is `false`. |

```rust
let r = safe_divide(10.0, 0.0)
if r.ok {
    let v = r.value
} else {
    // r.error contains the message
}
```

The `try` expression wraps any runtime-fallible operation into a `res<T>`:

```rust
let r: res<float> = try (1.0 / 0.0)
```

---

## Shape types

Shape constructors return concrete typed values — not a generic `shape`. Each type exposes its own geometry fields and can be inspected, stored in typed lists, or used in constraints. All shape types are accepted by `out <<` and `@`, and are assignable to the erased `shape` type.

### circle

```rust
let c = circle(vec2(400, 300), 50)
c.center   // vec2
c.radius   // float
```

| Field/Method | Returns | Description |
|--------------|---------|-------------|
| `.center` | `vec2` | Center position (read-only) |
| `.radius` | `float` | Radius (read-only) |
| `.in(dx, dy)` | `vec2` | Point offset from center by `dx`, `dy` |

### rect

```rust
let r = rect(vec2(400, 300), vec2(200, 100))
r.center   // vec2
r.size     // vec2 — (width, height)
```

| Field/Method | Returns | Description |
|--------------|---------|-------------|
| `.center` | `vec2` | Center position (read-only) |
| `.size` | `vec2` | Width and height (read-only) |
| `.in(dx, dy)` | `vec2` | Point offset from anchor by `dx`, `dy` |

### line

```rust
let l = line(vec2(0, 0), vec2(100, 100))
l.from   // vec2
l.to     // vec2
```

| Field/Method | Returns | Description |
|--------------|---------|-------------|
| `.from` | `vec2` | Start point (read-only) |
| `.to` | `vec2` | End point (read-only) |
| `.in(dx, dy)` | `vec2` | Point offset from start by `dx`, `dy` |

### polygon

```rust
let p = polygon([vec2(0, 0), vec2(50, 100), vec2(100, 0)])
```

| Method | Returns | Description |
|--------|---------|-------------|
| `.in(dx, dy)` | `vec2` | Point offset from first vertex by `dx`, `dy` |

### shape (erased)

The erased drawable type. Any concrete shape kind is assignable to `shape`. Used when you need a heterogeneous `list[shape]` or don't need field access:

```rust
let s: shape = circle(vec2(0, 0), 50)   // erase to shape — fields no longer accessible
let shapes: list[shape] = []
shapes.push(circle(vec2(0, 0), 30))
shapes.push(rect(vec2(100, 100), vec2(50, 50)))
```

| Method | Returns | Description |
|--------|---------|-------------|
| `.in(dx, dy)` | `vec2` | Point offset from the shape's anchor |

---

## transform

A chainable transformation value. All methods return a new transform — the original is unchanged.

```rust
let t = transform()
let t = transform().move(50.0, 0.0).scale(1.5).rotate(45.0)
```

**Methods:**

| Method | Description |
|--------|-------------|
| `.move(dx, dy)` | Translate |
| `.translate(dx, dy)` | Same as `.move` |
| `.scale(s)` | Uniform scale |
| `.rotate(degrees)` | Rotation |

Apply to a shape with `@`:

```rust
out << circle(vec2(0.0, 0.0), 0.3)@t
out << circle(vec2(0.0, 0.0), 0.3)@(t1, t2)   // multiple, left to right
```

---

## State and Input

`State` is the type of the `s` parameter in `update` and `init`. Its fields are whatever you declared in the `state {}` block. Accessed via `s.field`.

`Input` is the per-frame input object.

| Field | Type | Description |
|-------|------|-------------|
| `input.dt` | `float` | Seconds elapsed since the previous frame |
