---
layout: default
title: Error Reference
nav_order: 8
---

# Error Reference
{: .no_toc }

All errors are caught at **compile time** (before execution) unless marked as runtime.

<details open markdown="block">
  <summary>Contents</summary>
  {: .text-delta }
1. TOC
{:toc}
</details>

---

## Compile-time errors

### S001 — Undefined symbol

A name is used but has not been declared.

```rust
let x = foo + 1.0   // ❌ S001: undefined `foo`
```

**Common causes:**
- Typo in variable or function name
- Variable used before it is declared (local variables must be declared before use)
- Missing `import` statement

**Note:** Functions defined at the top level are visible everywhere in the file regardless of order. Only local variables require declaration before use.

---

### S002 — Type mismatch

A value of the wrong type was used where a specific type was expected.

```rust
if 3.14 { }                          // ❌ S002: condition must be bool
fn f() -> float { return true }      // ❌ S002: bool is not float
let x: float = true                  // ❌ S002: annotation mismatch
out << 3.14                          // ❌ S002: out << expects shape
let v = true ? 1.0 : false           // ❌ S002: branches must have same type
```

---

### S003 — Redeclaration

A name is declared more than once in the same scope.

```rust
let x = 1.0
let x = 2.0                          // ❌ S003: `x` already declared

import shapes { circle }
import shapes { circle }             // ❌ S003: `circle` already declared
```

**Note:** Declaring the same name in an inner scope is allowed — it shadows the outer declaration.

---

### S004 — Const reassignment

A `const` is assigned a new value after declaration.

```rust
const SPEED = 1.5
SPEED = 2.0                          // ❌ S004: cannot reassign const `SPEED`
```

---

### S005 — Unknown namespace

An `import` refers to a namespace that doesn't exist.

```rust
import nonexistent { foo }           // ❌ S005: unknown namespace `nonexistent`
```

**Available namespaces:** `shapes`, `render`, `coords`

---

### S006 — Member not exported

An `import` names a member that the namespace doesn't provide.

```rust
import shapes { circle, not_a_shape }  // ❌ S006: `shapes` does not export `not_a_shape`
import coords { circle }               // ❌ S006: `coords` does not export `circle`
```

---

### S007 — Wrong argument count

A function or constructor is called with the wrong number of arguments.

```rust
vec2(1.0)                            // ❌ S007: vec2 expects 2 arguments
circle(vec2(0.0, 0.0))              // ❌ S007: circle expects 2 arguments (center + radius)
circle(vec2(0.0, 0.0), 0.5, 0.3)   // ❌ S007: too many arguments
```

---

### S008 — Operator not applicable

An operator is used on types it doesn't support.

```rust
let a = true
let b = a + 1.0      // ❌ S008: operator `+` not applicable to `bool` and `float`
let c = a < 1.0      // ❌ S008: operator `<` not applicable to `bool` and `float`
let d = 1.0 and true // ❌ S008: operator `and` not applicable to `float` and `bool`
let x = 3.14
let y = x[0]         // ❌ S008: cannot index `float`
```

---

### S009 — Field or method not found

A field access or method call on a type that doesn't have that field or method.

```rust
let x = 3.14
let y = x.foo         // ❌ S009: `float` has no field `foo`
let z = x.move(1.0)   // ❌ S009: `float` has no method `move`

let v = vec2(1.0, 0.0)
let w = v.z            // ❌ S009: `vec2` has no field `z` (use vec3 for z)
```

---

### S010 — Not callable

A call expression is applied to a value that isn't a function.

```rust
let f: float = 1.0
let x = f()           // ❌ S010: `float` is not callable
```

---

### S012 — Invalid lifecycle function signature

`on_update`, `on_init`, or `on_exit` has the wrong signature.

```rust
// ❌ S012 — on_update must accept (State, Input) and return State
fn on_update(s: State) -> State { ... }
fn on_update(s: float, input: Input) -> State { ... }
fn on_update(s: State, input: Input) -> float { ... }
```

Correct signatures:

```rust
fn on_update(s: State, input: Input) -> State { ... }
fn on_init(s: State) -> State { ... }
```

---

## Runtime errors

Runtime errors stop execution at the point of failure. Whatever was already pushed to `out <<` before the error remains visible.

| Situation | Error |
|-----------|-------|
| Division by zero (`1.0 / 0.0`) | `division by zero` |
| Modulo by zero (`x % 0.0`) | `modulo by zero` |
| `list.pop()` on empty list | `pop on empty list` |
| Index out of bounds (`xs[10]` when `xs.len == 3`) | `index out of bounds` |
| `vec.normalize()` on zero vector | `normalize: zero vector` |
| `mat.inverse()` on non-invertible matrix | `matrix is not invertible` |

### Handling runtime errors explicitly

Wrap a fallible expression with `try`:

```rust
let r: res<float> = try (1.0 / 0.0)
if r.ok { ... } else { ... }
```

Or return `res<T>` from a function:

```rust
fn safe_div(a: float, b: float) -> res<float> {
    if b == 0.0 { return error("division by zero") }
    return ok(a / b)
}
```
