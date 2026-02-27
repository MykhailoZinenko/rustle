---
layout: default
title: Syntax
nav_order: 4
---

# Syntax
{: .no_toc }

<details open markdown="block">
  <summary>Contents</summary>
  {: .text-delta }
1. TOC
{:toc}
</details>

---

## Comments

```rust
// single-line comment
/* block comment */
/* can span
   multiple lines */
```

---

## Variables

```rust
let x = 0.5              // type inferred from initializer
let x: float = 0.5       // explicit type annotation
const SPEED = 1.5         // immutable — reassignment is a compile error
```

Reassignment:

```rust
x = 1.0
s.field = value           // state field assignment
v.x = 5.0                 // vector component assignment
xs[i] = value             // list index assignment
```

Compound assignment (`+=`, `-=`, `*=`, `/=`) works on variables, state fields, and list indices:

```rust
x += 5.0                  // same as x = x + 5.0
s.speed *= 2.0            // state field
xs[i] += 1.0              // list index
```

Variables must be declared before use (within the same scope). Functions are visible anywhere in the file regardless of declaration order.

---

## Literals

```rust
1.0          // float — all numbers are floats, no integer type
true  false  // bool
"hello"      // string
#FF6633      // color literal (hex)
#FF6633FF    // color with alpha
```

---

## Operators

### Arithmetic

| Operator | Types | Notes |
|----------|-------|-------|
| `+` | float, vec2, vec3, vec4, color | |
| `-` | float, vec2, vec3, vec4, color | |
| `*` | float, vec2, vec3, vec4, color, mat3, mat4 | also `vec * float` (scalar broadcast) |
| `/` | float, vec2 / float | |
| `%` | float | modulo |
| `-x` | float | unary negation |
| `++x`, `x++` | float | increment (prefix returns new value, postfix returns old) |
| `--x`, `x--` | float | decrement (prefix returns new value, postfix returns old) |

Operands for `++` and `--` must be assignable (variable, state field, or list index).

```rust
let v = vec2(1.0, 2.0) + vec2(3.0, 4.0)   // (4.0, 6.0)
let w = vec2(1.0, 2.0) * 3.0              // (3.0, 6.0)
```

### Comparison

```rust
x == y    x != y    x < y    x <= y    x > y    x >= y
```

Work on `float`. `==` and `!=` also work on `bool`, `vec2`, `vec3`, `vec4`.

### Logical

```rust
a and b    a or b    not a
```

Only work on `bool`.

### Ternary

```rust
let v = condition ? then_value : else_value
```

Both branches must have the same type.

### Cast

```rust
let x = expr as float
```

### Index

```rust
let v = list[0]           // read
list[i] = value           // write
list[i] += 1.0            // compound assignment
```

Indices are float — truncated to whole number at runtime.

### Transform application

```rust
let s2 = shape@transform             // apply one transform
let s2 = shape@(t1, t2, t3)         // apply multiple — left to right
```

### Output push

```rust
out << shape
out << bg << s1 << s2                // chained, rendered bottom to top
```

---

## Control flow

### if / else

```rust
if x > 0.0 {
    // taken when condition is true
} else {
    // taken otherwise
}
```

The condition must be `bool`. `else if` chains are supported:

```rust
if x > 1.0 { out << a } else if x > 0.0 { out << b } else { out << c }
```

### while

```rust
let i = 0.0
while i < 10.0 {
    i += 1.0
}
```

### for

```rust
for let i = 0.0; i < 10.0; i += 1.0 {
    out << circle(vec2(i * 0.1, 0.0), 0.05)
}
```

The loop variable is declared with `let` inside the header. It is scoped to the loop body.

### foreach

```rust
let values: list[float] = [0.1, 0.2, 0.3]

foreach v in values {
    out << circle(vec2(v, 0.0), 0.05)
}

// With explicit element type annotation:
foreach v: float in values {
    out << circle(vec2(v, 0.0), 0.05)
}
```

---

## Functions

```rust
fn add(a: float, b: float) -> float {
    return a + b
}

fn draw_dot(x: float, y: float) {
    out << circle(vec2(x, y), 0.05)
}
```

`return` is always explicit — there are no implicit last-expression returns. A void function does not need `return`.

### First-class functions

```rust
// Alias
fn f = add

// Lambda
fn g = (a: float, b: float) -> float {
    return a + b
}

// Inside a function body
fn process(xs: list[float]) {
    fn double = (x: float) -> float { return x * 2.0 }
    foreach v in xs {
        out << circle(vec2(double(v), 0.0), 0.05)
    }
}
```

### Function types

```rust
fn apply(f: fn(float) -> float, x: float) -> float {
    return f(x)
}
```

### Named arguments

Namespace functions and shape constructors accept named arguments. Named arguments come after positional ones:

```rust
rect(vec2(400, 300), vec2(200, 100), origin: top_left)
circle(vec2(400, 300), 100, render: sdf)
```

---

## Imports

```rust
import shapes { circle, rect }        // import specific members
import render { sdf, fill }
import coords { resolution, origin, top_left }

import render                         // import whole namespace
render.fill                           // access via dot notation
```

Multiple imports of the same name are a compile error (S003).

---

## Error handling

```rust
// Construct a result value
let r: res<float> = ok(42.0)
let r: res<float> = error("something failed")

// Check
if r.ok {
    let v = r.value
} else {
    let msg = r.error
}

// try — wraps a runtime-fallible expression
let r: res<float> = try (1.0 / 0.0)
```

See [Types — Result](types#result) for full details.
