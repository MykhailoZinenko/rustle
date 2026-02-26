---
layout: default
title: Built-ins
nav_order: 6
---

# Built-ins
{: .no_toc }

<details open markdown="block">
  <summary>Contents</summary>
  {: .text-delta }
1. TOC
{:toc}
</details>

---

## Always available (no import needed)

### Math functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `sin` | `(float) -> float` | Sine (radians) |
| `cos` | `(float) -> float` | Cosine (radians) |
| `tan` | `(float) -> float` | Tangent (radians) |
| `asin` | `(float) -> float` | Arc sine |
| `acos` | `(float) -> float` | Arc cosine |
| `atan` | `(float) -> float` | Arc tangent |
| `atan2` | `(float, float) -> float` | `atan2(y, x)` — full-quadrant angle |
| `sqrt` | `(float) -> float` | Square root |
| `pow` | `(float, float) -> float` | `pow(base, exp)` |
| `abs` | `(float) -> float` | Absolute value |
| `floor` | `(float) -> float` | Round down |
| `ceil` | `(float) -> float` | Round up |
| `round` | `(float) -> float` | Round to nearest |
| `sign` | `(float) -> float` | Returns -1.0, 0.0, or 1.0 |
| `fract` | `(float) -> float` | Fractional part (x - floor(x)) |
| `min` | `(float, float) -> float` | Minimum |
| `max` | `(float, float) -> float` | Maximum |
| `clamp` | `(float, float, float) -> float` | `clamp(x, lo, hi)` |
| `lerp` | `(float, float, float) -> float` | `lerp(a, b, t)` |

### Constants

| Name | Value |
|------|-------|
| `PI` | 3.14159265... |
| `TAU` | 6.28318530... (2π) |

### Constructors

```rust
vec2(x, y)
vec3(x, y, z)
vec4(x, y, z, w)

color(r, g, b)           // alpha defaults to 1.0
color(r, g, b, a)

transform()

mat3()                   // identity
mat4()                   // identity
mat3_translate(dx, dy)
mat3_rotate(degrees)
mat3_scale(sx, sy)
mat4_translate(x, y, z)
mat4_scale(x, y, z)
mat4_rotate_x(degrees)
mat4_rotate_y(degrees)
mat4_rotate_z(degrees)

ok(value)                // res<T> success
error(message)           // res<T> failure
```

### Color constants

```rust
red   green   blue   white   black   transparent
```

---

## `shapes` namespace

```rust
import shapes { circle, rect, line, polygon }
```

### circle

```rust
circle(center: vec2, radius: float) -> circle
circle(center: vec2, radius: float, render: RenderMode) -> circle
```

`center` is in canvas coordinates. `radius` is in the same units. Returns a `circle` with `.center` and `.radius` fields.

### rect

```rust
rect(center: vec2, size: vec2) -> rect
rect(center: vec2, size: vec2, render: RenderMode) -> rect
rect(center: vec2, size: vec2, origin: OriginMode) -> rect
```

Default origin is `center`. With `origin: top_left`, the position becomes the top-left corner. Returns a `rect` with `.center` and `.size` fields.

### line

```rust
line(from: vec2, to: vec2) -> line
```

Returns a `line` with `.from` and `.to` fields.

### polygon

```rust
polygon(points: list[vec2]) -> polygon
```

Closed polygon through all points in order.

---

## `render` namespace

```rust
import render { sdf, fill, outline, stroke }
```

Render modes control how a shape is drawn. Pass as a named argument `render:`.

| Mode | Description |
|------|-------------|
| `fill` | Solid fill (default if omitted) |
| `sdf` | Signed-distance field — anti-aliased, smooth edges |
| `outline` | Outline only, no fill |
| `stroke(width)` | Stroked outline with given pixel width |

```rust
circle(vec2(0.0, 0.0), 0.3, render: fill)
circle(vec2(0.0, 0.0), 0.3, render: sdf)
circle(vec2(0.0, 0.0), 0.3, render: outline)
circle(vec2(0.0, 0.0), 0.3, render: stroke(2.0))
```

---

## `coords` namespace

```rust
import coords { resolution, origin, top_left, top_right, bottom_left, bottom_right,
                center, top, bottom, left, right }
```

### resolution

```rust
resolution(width: float, height: float)
```

Sets the canvas dimensions in pixels. Enables pixel-based layout. After calling this, all coordinates you give to shapes are interpreted as pixel values (0 to `width` / 0 to `height`, depending on origin).

Without `resolution()`, the canvas uses NDC: -1.0 to 1.0 on both axes.

### origin

```rust
origin(mode)
```

Sets where (0, 0) is on the canvas. The `mode` argument is one of the origin constants:

| Constant | Position |
|----------|----------|
| `center` | Center of the canvas (default) |
| `top_left` | Top-left corner |
| `top_right` | Top-right corner |
| `bottom_left` | Bottom-left corner |
| `bottom_right` | Bottom-right corner |
| `top` | Top edge, horizontally centered |
| `bottom` | Bottom edge, horizontally centered |
| `left` | Left edge, vertically centered |
| `right` | Right edge, vertically centered |

```rust
import coords { resolution, origin, top_left }

resolution(800, 600)
origin(top_left)
// (0, 0) is now the top-left corner
// x goes right 0 → 800, y goes down 0 → 600
```

### Where to call them

`resolution()` and `origin()` can be called:
- At **top level** — applies every frame (fine for static scripts)
- In **`fn init`** — applies once, persists for all subsequent frames (preferred for animated scripts)
- In **`fn update`** — re-applies every frame (same result if values don't change)
