# Phase 2a: Structs — Design Spec

## Overview

Add user-defined struct types to Rustle — reference-type objects with fields, public/private methods, and deep field access. Built on a trait-object foundation (`RustleObject`) that will later support enums and inheritance.

---

## Syntax

### Declaration

```
struct Point {
    let x: float = 0.0
    let y: float

    +fn distance_to(o: Point) -> float {
        return sqrt((this.x - o.x) * (this.x - o.x) + (this.y - o.y) * (this.y - o.y))
    }

    #fn internal_helper(v: float) -> float {
        return v * 2.0
    }
}
```

- Fields use `let`, optional type annotation (inferred from default), optional default value
- Fields without a default are required at construction
- `+fn` = public method, `#fn` = private method
- `this` refers to the instance inside methods, available implicitly (not a parameter)
- Methods follow normal function rules: `-> T` return type required when returning a value

### Construction

```
let p: Point = Point { x: 5.0, y: 10.0 }
let q: Point = Point { y: 3.0 }           // x defaults to 0.0
```

- Named fields, order doesn't matter
- Fields with defaults can be omitted
- Fields without defaults are required — compile error if missing

### Usage

```
p.x = 7.0                          // field mutation (reference type)
let d: float = p.distance_to(q)    // public method call
p.internal_helper(5.0)             // compile error: private method
```

---

## Nested Structs & Deep Field Access

### Nesting

```
struct Bounds {
    let min: Point
    let max: Point
}

let b: Bounds = Bounds { min: Point { x: 0.0, y: 0.0 }, max: Point { x: 10.0, y: 10.0 } }
```

### Deep field access and mutation

```
let v: float = b.min.x     // read through chain of Rc<RefCell<HashMap>>
b.min.x = 5.0              // mutate in-place — follow Rc chain, no rebuild needed
b.min.distance_to(b.max)   // method call on nested struct
```

Each level is a reference type (Rc), so deep mutation works by borrowing each Rc in sequence.

---

## Clone & Reference Semantics

Assignment shares the reference:
```
let a: Point = Point { x: 20.0, y: 10.0 }
let b: Point = a          // b and a point to same data
b.x = 30.0                // a.x is also 30.0
```

Independent copy via `.clone()`:
```
let b: Point = a.clone()  // deep copy — new Rc, new HashMap, fields cloned recursively
b.x = 30.0                // a.x is still 20.0
```

**`.clone()` rules:**
- Built-in method on all struct types (not user-defined)
- Deep clone — nested structs and lists are cloned recursively
- Return type is the same struct type
- Also add `.clone()` to `list[T]` for consistency

---

## Architecture: Value::Object & RustleObject Trait

### New Value variant

```rust
Value::Object(Rc<RefCell<dyn RustleObject>>)
```

### The trait

```rust
pub trait RustleObject: std::fmt::Debug {
    fn type_name(&self) -> &str;
    fn get_field(&self, name: &str) -> Option<Value>;
    fn set_field(&mut self, name: &str, val: Value) -> bool;
    fn call_method(&mut self, name: &str, args: &[Value], line: usize) -> Option<Result<Value, RuntimeError>>;
    fn clone_deep(&self) -> Box<dyn RustleObject>;
    fn field_names(&self) -> Vec<&str>;
    fn display(&self) -> String;
}
```

### StructInstance

```rust
pub struct StructInstance {
    pub type_name: String,
    pub fields: HashMap<String, Value>,
    pub methods: Rc<StructMethods>,     // shared across all instances of same type
}
```

Methods stored in a shared `Rc` — all instances of the same struct type reference the same method table. Only field data is per-instance. (`Rc`, not `Arc` — Rustle is single-threaded throughout; the codebase uses `Rc`/`RefCell` everywhere.)

### Interpreter dispatch & borrow safety

Field access uses **short-lived borrows only** — never hold a `borrow()` or `borrow_mut()` across a function call:

- `eval_field`: `obj.borrow().get_field(name)` — borrow released before the value is used further
- `exec_assign` for field writes: `obj.borrow_mut().set_field(name, val)` — borrow released immediately
- `exec_assign` for deep paths (`b.min.x = 5.0`): resolve each level with a short borrow, get the inner Rc, release the outer borrow, then borrow the inner Rc
- `console << obj` → `obj.borrow().display()` — short-lived borrow

**Method dispatch** requires special care to avoid RefCell panics. A method body may pass `this` to external functions that access its fields. If we held `borrow_mut()` for the entire method body, any re-access would panic.

**Solution:** Method dispatch does NOT hold a borrow during execution. Instead:
1. Clone the `Rc` (cheap — just increments refcount) to create the `this` binding
2. Pass `this` as a `Value::Object(rc_clone)` local variable in the method's scope
3. The method body accesses fields through `this.field` which does short-lived borrows as usual
4. No long-lived borrow is ever held — re-entrant access is safe

This matches how `State` already works — the interpreter passes `Value::State(rc.clone())` into lifecycle functions and field access uses short-lived borrows.

### Future extensibility

- Enums implement `RustleObject`
- Inheritance: `StructInstance` gains a `parent` field, `call_method` walks the chain
- New object types slot in without changing the Value enum

### Architectural decisions & trade-offs

**1. Object safety:** `RustleObject` is object-safe — no generic methods, no `Self: Sized` requirement. `clone_deep` returns `Box<dyn RustleObject>` which is valid for trait objects. Verified: all methods use `&self` or `&mut self`.

**2. Value enum size:** `Rc<RefCell<dyn RustleObject>>` is a fat pointer (16 bytes — pointer + vtable). Current largest variants are `Vec4` and `Color` at 32 bytes. The new variant does not increase the enum's overall size. No `large_enum_variant` concern.

**3. RefCell borrow panics:** Addressed above — short-lived borrows only, Rc-clone for `this` binding. Method dispatch never holds a borrow across user code execution.

**4. Rc, not Arc:** `StructMethods` uses `Rc`, not `Arc`. Rustle is single-threaded; atomic reference counting would add unnecessary overhead. Consistent with `Rc<RefCell<...>>` used throughout the codebase for `List` and `State`.

**5. Dynamic dispatch cost:** Every field access goes through vtable indirection (`dyn RustleObject`) plus a `HashMap::get`. The vtable adds one pointer chase (~1ns), negligible next to the HashMap lookup. This is the same cost class as existing `State` field access. Trade-off is justified: enums and inheritance require heterogeneous storage in `Value::Object`, which demands `dyn Trait`. "Static where you can, dynamic where you must" — we must here.

**6. Debug and Clone on Value:** `Value` derives `Debug` and `Clone`. For `Value::Object(Rc<RefCell<dyn RustleObject>>)`:
- `Clone`: `Rc::clone` is a shallow refcount bump — correct for reference semantics. Deep copy only via explicit `.clone()` method in Rustle.
- `Debug`: requires `RustleObject: Debug` (already specified in the trait bound). `StructInstance` can `#[derive(Debug)]` since all its fields (`String`, `HashMap<String, Value>`, `Rc<StructMethods>`) implement `Debug`.

---

## Compilation Pipeline

### Lexer

- New keyword: `struct`
- `+fn` / `#fn`: no new tokens needed — parser sees `Plus`+`Fn` or `Hash`+`Fn` inside struct bodies

### Parser — New AST nodes

```rust
pub struct StructDef {
    pub name: String,
    pub fields: Vec<StructField>,
    pub methods: Vec<StructMethod>,
    pub span: Span,
}

pub struct StructField {
    pub name: String,
    pub ty: Option<Type>,           // None = infer from default
    pub default: Option<Expr>,
    pub span: Span,
}

pub enum Visibility { Public, Private }

pub struct StructMethod {
    pub visibility: Visibility,
    pub def: FnDef,
    pub span: Span,
}
```

New top-level item: `Item::Struct(StructDef)`.

### Pass 1 — Collector

- Register struct name as a type in the symbol table
- Register field names/types and method signatures for lookup

### Pass 2 — Checker

- Validate field types exist
- Default expressions match declared types (or infer type from default)
- Method bodies type-check with `this: Named("StructName")` injected in scope
- Construction sites: all required fields provided, field types match, no unknown fields

### Pass 3 — Validator

- Duplicate field names → error
- Duplicate method names → error
- Private method access from outside → error

---

## Type System Rules

- `Type::Named(String)` resolves struct types — already exists in AST
- Struct definitions stored in `Program` as `HashMap<String, StructDef>` alongside `fn_table`
- `Point` assignable to `Point` — exact match only (no implicit conversion until inheritance)
- Struct types valid in all positions: function params, return types, `list[Point]`, `Point?`, state fields
- `LookupContext` gains new tier: check `Program.structs` for `Named(name)` types (after namespace, before TypeRegistry)
- `this` is a reserved identifier of type `Named("StructName")` inside struct methods; compile error outside

---

## Error Codes

| Code | Situation | Example message |
|------|-----------|-----------------|
| S016 | Private method access from outside | `'do_internal' is private on 'Point'` |
| S017 | Missing required field in construction | `missing required field 'y' in 'Point'` |
| S018 | Unknown field in construction | `'z' is not a field of 'Point'` (+ hint) |
| S019 | Duplicate field in struct definition | `duplicate field 'x' in 'Point'` |
| S020 | Duplicate method in struct definition | `duplicate method 'distance_to' in 'Point'` |
| S021 | `this` used outside struct method | `'this' is only valid inside struct methods` |
| R013 | Runtime field not found (safety net) | `field 'z' not found on 'Point'` |

All support hint/suggestion via existing `suggest_similar`.

---

## Testing Strategy

### Resolver tests (compile-time)

- Struct declaration resolves without error
- Required field missing at construction → S017
- Unknown field at construction → S018
- Private method call from outside → S016
- `this` outside struct → S021
- Nested struct field type resolution (`b.min.x` → `float`)
- Struct type in function params/return/list/optional positions
- Type inference from field defaults

### Runtime tests (integration)

- Construction with all fields
- Construction with defaults omitted
- Field read/write
- Deep field access and mutation (`b.min.x = 5.0`)
- Method call, `this` binds correctly
- Private method callable from within, not from outside
- Reference semantics: `let b = a` → shared mutation
- `.clone()` → independent deep copy
- `.clone()` on nested structs
- Struct in list: `list[Point]`, push/pop
- Struct as state field
- `console <<` prints struct representation
- Method calling another method on `this`

### Edge cases

- Struct with no methods (pure data)
- Struct with no fields (marker type)
- Self-referential: field of own type (`let next: Node?`)
- `.clone()` on self-referential (stops at `none`)
