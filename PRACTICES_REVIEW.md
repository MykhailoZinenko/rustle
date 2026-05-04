# Rustle — Rust Best Practices Review

**Date:** 2025-05-04
**Scope:** Full review of `crates/rustle-lang/` against Apollo GraphQL Rust Best Practices (Chapters 1-9)
**Build status:** 476/476 tests pass, 53 clippy warnings

**Rating system:**
- **Priority** (1-10): How urgently this violates best practices. 10 = performance/correctness risk, 1 = stylistic.
- **Complexity** (1-10): Effort to fix. 10 = architectural, 1 = one-line change.

Sorted by priority descending, then complexity descending.

---

## HIGH PRIORITY

### ✅ 1. No Clippy Lint Configuration (Priority: 9, Complexity: 2)

**Chapter:** 2 (Clippy and Linting)
**Files:** `Cargo.toml` (workspace), `crates/rustle-lang/Cargo.toml`

No `[lints.clippy]` section in either Cargo.toml. No `rustfmt.toml` or `clippy.toml`. There are currently **53 clippy warnings** that go unnoticed. Per Ch.2: "Add `cargo clippy --all-targets -- -D warnings` to your CI/workflow."

Key warnings found:
- 17x collapsible `if` statements
- 12x `assert_eq!` with literal bool (should be `assert!`)
- 11x immediately-dereferenced references
- 3x derivable `impl` blocks
- 2x simplifiable `map_or`
- 1x redundant closure
- 1x very complex type needing a type alias

**Fix:** Add `[workspace.lints.clippy]` to root `Cargo.toml`, add `[lints] workspace = true` to each crate, then fix all warnings.

---

### ✅ 2. `Env::get` Clones Every Value on Read (Priority: 9, Complexity: 5)

**Chapter:** 1.1 (Borrowing Over Cloning), 3.2 (Avoid Redundant Cloning)
**File:** `runtime/interpreter.rs:48-51`

```rust
fn get(&self, name: &str) -> Option<Value> {
    for scope in self.scopes.iter().rev() {
        if let Some(v) = scope.get(name) { return Some(v.clone()); }
    }
    None
}
```

Every variable read deep-clones the value. For `List(Rc<RefCell<Vec<Value>>>)`, `Closure { HashMap<String, Value> }`, `Mat4(Box<[f64; 16]>)`, `Shape(ShapeData)`, etc., this is expensive. Called hundreds of times per frame. This is exactly the "Clone trap" from Ch.1.1: "Cloning large data structures like `Vec<T>` or `HashMap<K, V>`."

**Fix:** Return `&Value` where possible. For cases that need ownership (passing to functions), let the caller clone explicitly.

---

### ✅ 3. Error Types Don't Implement `std::error::Error` (Priority: 8, Complexity: 2)

**Chapter:** 4.3 (thiserror for Crate-level Errors)
**File:** `error.rs`

Neither `Error` nor `RuntimeError` implement `std::error::Error`. They have manual `Display` implementations. The crate already depends on `thiserror` but doesn't use it for these types. Per Ch.4: "Use `thiserror` for library errors."

This prevents:
- `?` operator interoperability with other error types
- `anyhow` integration in downstream binaries
- Standard error chaining via `source()`

**Fix:** Derive `#[derive(Debug, thiserror::Error)]` on both `Error` and `RuntimeError`.

---

### ✅ 4. `unwrap()` in Production Code (Priority: 8, Complexity: 3)

**Chapter:** 4.2 (Avoid unwrap/expect in Production)
**Files:** `runtime/interpreter.rs:35,957`, `analysis/symbols.rs:94,100`, `analysis/checker.rs:161`

Multiple `unwrap()` calls in non-test code:
- `self.scopes.last_mut().unwrap()` — could panic if scopes is empty
- `indices.last().unwrap()` — could panic if indices is empty
- `sym.ty.clone().unwrap()` — could panic if type is None

Per Ch.4.2: "Never use `unwrap()`/`expect()` outside tests." These are currently safe because of invariants, but the invariants aren't machine-checked.

**Fix:** Use `let Some(x) = expr else { unreachable!("reason") }` pattern to document the invariant, or use `expect("scope stack must not be empty")` at minimum.

---

### ✅ 5. `Value` Enum Has Massive Variant Size Disparity (Priority: 7, Complexity: 4)

**Chapter:** 2.3 (`large_enum_variant` lint), 3.3 (Stack vs Heap)
**File:** `runtime/value.rs`

`Value` enum has variants ranging from 8 bytes (`Float(f64)`) to very large (e.g., `Closure { params: Arc<[Param]>, body: Arc<[Stmt]>, captured: HashMap<String, Value> }` — ~72+ bytes on the stack). Every `Value` instance pays the cost of the largest variant. Clippy lint `large_enum_variant` would flag this.

**Fix:** Box the large variants: `Closure(Box<ClosureData>)`, `Shape(Box<ShapeData>)`, `Mat4(Box<[f64; 16]>)` is already boxed — good. Review other large variants.

---

### ✅ 6. Massive Code Duplication in `run_init`/`run_update`/`run_on_exit` (Priority: 7, Complexity: 3)

**Chapter:** 1.6 (Breaking up long functions), 8.5 (Replace Comments with Code)
**File:** `runtime/interpreter.rs:189-281`

Three lifecycle methods are nearly identical (~30 lines each, differing only in function name lookup, parameter binding, and whether `run_top_level()` is called). This violates DRY and makes every change require triple edits (as we saw with break/continue).

**Fix:** Extract a shared `run_lifecycle(&mut self, name: &str, state: State, extra_params: Option<(&Input,)>) -> Result<State, RuntimeError>` method.

---

### ✅ 7. `Span` Should Derive `Copy` (Priority: 6, Complexity: 1)

**Chapter:** 1.2 (Copy trait for small types)
**File:** `syntax/ast.rs:1-6`

```rust
pub struct Span { pub line: usize, pub column: usize }
```

16 bytes, all fields are `Copy`, pure data. Per Ch.1.2: "All fields are `Copy`, struct is small (≤24 bytes), represents plain data." Currently derived as `Clone` only, causing unnecessary `.clone()` calls throughout the parser.

**Fix:** Add `Copy` to the derive: `#[derive(Debug, Clone, Copy, PartialEq)]`.

---

### ✅ 8. Functions Accept `String` Where `&str` Would Suffice (Priority: 6, Complexity: 4)

**Chapter:** 1.1 (Prefer `&str` over `String`, `&[T]` over `Vec<T>`)
**Files:** Throughout

Multiple functions take owned `String` when they only read:
- `Symbol::new(name: impl Into<String>, ...)` — forces allocation for every symbol
- `Env::declare(&mut self, name: &str, ...)` then does `name.to_string()` internally
- `Error::new(..., message: impl Into<String>)` — fine for the API, but internally allocates even for string literals

Per Ch.1.1: "Prefer `&str` instead of `String`" and "Clone a reference argument → the caller should have passed ownership instead."

**Fix:** Where the string is only used for lookup/comparison, accept `&str`. Where ownership is needed (HashMap keys), accept `impl Into<String>` or `String` explicitly.

---

### ✅ 9. No Doc Comments on Public API (Priority: 6, Complexity: 3)

**Chapter:** 8.7 (When to use doc comments), 8.9 (Checklist)
**Files:** `lib.rs`, `error.rs`, `runtime/value.rs`

The public API has minimal `///` documentation:
- `compile()` has a one-liner but no `# Examples`, `# Errors` sections
- `Runtime` struct lacks usage documentation
- `Value` enum has no doc comments on any variant
- `State`, `Input`, `Program` lack docs
- No `//!` crate-level documentation in `lib.rs`

Per Ch.8.9: All public functions, structs, traits, enums should have `///` docs with purpose, usage, and error behavior.

**Fix:** Add `///` docs to all `pub` items. Add `//!` crate doc at top of `lib.rs`. Consider `#![deny(missing_docs)]`.

---

### ✅ 10. `ErrorCode::as_str()` Could Be a Simple `Display` (Priority: 5, Complexity: 2)

**Chapter:** 4.3 (thiserror), 1.6 (Comments not Clutter)
**File:** `error.rs:52-99`

`ErrorCode::as_str()` is a 50-line match that maps every variant to its string name. This is boilerplate that `thiserror` or `strum` would eliminate. Additionally, `ErrorCode::is_error()` always returns `true` — dead abstraction.

**Fix:** Derive `Display` via `strum::Display` or `thiserror`, or use `format!("{:?}", self)` since the variant names already match the codes.

---

## MEDIUM PRIORITY

### ✅ 11. `Origin` Uses Manual `from_str` Instead of `FromStr` Trait (Priority: 5, Complexity: 1)

**Chapter:** Clippy warning: "method `from_str` can be confused for the standard trait"
**File:** `types/draw.rs:15`

```rust
impl Origin {
    pub fn from_str(s: &str) -> Option<Self> { ... }
}
```

This shadows the standard `std::str::FromStr` trait. Clippy flags this.

**Fix:** Implement `impl FromStr for Origin` with `type Err = ()` (or a proper error), then change callers to use `.parse()`.

---

### ✅ 12. `Default` Implementations Could Be Derived (Priority: 5, Complexity: 1)

**Chapter:** Clippy: "this impl can be derived"
**Files:** `types/draw.rs` (3 instances: `Origin::default()`, `CoordMeta::default()`, `TransformData::default()`)

Manual `impl Default` blocks where `#[derive(Default)]` would work. Clippy flags these.

**Fix:** Use `#[default]` attribute on the default variant for enums, `#[derive(Default)]` for structs where field defaults match.

---

### ✅ 13. Collapsible `if` Statements (Priority: 5, Complexity: 1)

**Chapter:** 2.3 (Clippy lints), style
**Files:** 17 occurrences across `checker.rs`, `interpreter.rs`, `collector.rs`

Pattern:
```rust
if let Type::Named(n) = obj_ty {
    if n == "State" { ... }
}
```
Could be:
```rust
if let Type::Named(n) = obj_ty && n == "State" { ... }
```

Clippy reports 17 of these.

**Fix:** Collapse into single `if let` with guards, or use `matches!()`.

---

### ✅ 14. Redundant Reference Creation (Priority: 5, Complexity: 1)

**Chapter:** 2.3 (`needless_borrow` lint)
**Files:** 11 occurrences across `interpreter.rs`, `checker.rs`

Clippy reports "this expression creates a reference which is immediately dereferenced by the compiler." Examples: passing `&self.binops` where `self.binops` is already `&BinopRegistry`.

**Fix:** Remove the extra `&`.

---

### ✅ 15. `named()` Helper Duplicated 5 Times (Priority: 5, Complexity: 2)

**Chapter:** 1.1, DRY principle
**Files:** `namespaces/core.rs`, `namespaces/shapes.rs`, `namespaces/render.rs`, `namespaces/coords.rs`, `types/registry.rs`

Five separate `fn named(s: &str) -> Type` helpers, each with their own match-to-variant mapping (added during fix #4). These should be a single shared function.

**Fix:** Add `Type::from_name(s: &str) -> Type` as a method on the `Type` enum, remove all `named()` helpers.

---

### ✅ 16. `read_number` Silently Swallows Parse Errors (Priority: 5, Complexity: 2)

**Chapter:** 4.1 (Prefer Result, avoid panic)
**File:** `syntax/lexer.rs:243`

```rust
s.parse().unwrap_or(0.0)
```

If the number is too large or malformed (shouldn't happen given the digit scanning, but still), it silently becomes `0.0` instead of producing an error. Per Ch.4: "If your function can fail, prefer to return a Result."

**Fix:** Return a proper `L005` error for unparseable numbers.

---

### ✅ 17. No `rustfmt.toml` Configuration (Priority: 4, Complexity: 1)

**Chapter:** 1.7 (Use Declarations — imports)
**File:** Project root

No `rustfmt.toml` exists. Imports are not consistently ordered per the standard grouping (std → external → workspace → crate). Per Ch.1.7, use:
```toml
reorder_imports = true
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
```

**Fix:** Add `rustfmt.toml` with the recommended settings. Run `cargo +nightly fmt`.

---

### ✅ 18. `interpreter.rs` is 1326 Lines (Priority: 4, Complexity: 5)

**Chapter:** 1.6 (Breaking up long functions), 8.5 (Replace Comments with Code)
**File:** `runtime/interpreter.rs`

The interpreter is a single 1326-line file containing the `Env`, `Interpreter`, all statement execution, all expression evaluation, free variable analysis, field access, assignment, transforms, binary ops, unary ops, and helper utilities. Per Ch.1.6: "break logic into named helper functions" for clarity and testability.

**Fix:** Extract into submodules: `runtime/env.rs`, `runtime/eval.rs`, `runtime/exec.rs`, `runtime/helpers.rs`.

---

### ✅ 19. `ExportKind` Should Derive `Copy` (Priority: 4, Complexity: 1)

**Chapter:** 1.2 (Copy for small types)
**File:** `namespaces/mod.rs:30`

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ExportKind { Function, Constant }
```

Fieldless enum, 1 byte. Perfect candidate for `Copy`. Currently requires `.clone()` for no reason.

**Fix:** Add `Copy` to derive.

---

### ✅ 20. `BinOp` and `UnOp` Should Derive `Copy` (Priority: 4, Complexity: 1)

**Chapter:** 1.2 (Copy for small types)
**File:** `syntax/ast.rs`

`BinOp` and `UnOp` are fieldless enums used extensively in pattern matching with `*op == BinOp::And` etc. They derive `Clone` but not `Copy`, causing unnecessary borrows.

**Fix:** Add `Copy` to derive for both.

---

### ✅ 21. Tests Use Generic Names (Priority: 4, Complexity: 2)

**Chapter:** 5.1 (Use descriptive names)
**Files:** `tests/resolver.rs`, `tests/runtime.rs`, `syntax/lexer.rs` (tests)

Many test names are terse identifiers rather than behavior descriptions:
- `empty()`, `float_literal()`, `keywords()`, `assignment()`
- `vec2_add`, `vec2_sub` — what behavior are they verifying?

Per Ch.5.1: "use a name which reads like a sentence, describing the desired behavior." Better: `vec2_add_returns_component_wise_sum`, `assignment_updates_existing_variable`.

**Fix:** Rename tests to describe the expected behavior, not just the feature being tested.

---

### ✅ 22. Tests Assert Multiple Behaviors (Priority: 4, Complexity: 2)

**Chapter:** 5.1 (Only test one behavior per function)
**Files:** `tests/runtime.rs`, `syntax/lexer.rs`

Several tests check multiple unrelated behaviors:
```rust
fn all_arithmetic_ops() {
    assert!(matches!(parse_expr_src("a + b"), ...));
    assert!(matches!(parse_expr_src("a - b"), ...));
    assert!(matches!(parse_expr_src("a * b"), ...));
    // ... 5 more
}
```

Per Ch.5.1: "To keep tests clear, they should describe one thing."

**Fix:** Split into per-operator tests or use `rstest` parameterization.

---

### ✅ 23. `ErrorCode` Enum Variants Are Opaque Names (Priority: 4, Complexity: 3)

**Chapter:** 4.3 (thiserror), 8.7 (Documentation)
**File:** `error.rs:2-50`

```rust
pub enum ErrorCode {
    L001, L002, L003, L004, L005,
    P001, P002, ...
}
```

Numeric codes without semantic names. A developer seeing `S009` in code must look up the comment to know it means "field or method not found." Per Ch.4.3, error variants should be self-documenting: `FieldNotFound`, `UndefinedSymbol`, etc.

**Fix:** Either rename variants to semantic names (`ErrorCode::UndefinedSymbol` instead of `S001`) or add doc comments to each variant.

---

## LOW PRIORITY

### ✅ 24. `PrintLevel` Could Derive `Copy` (Priority: 3, Complexity: 1)

**Chapter:** 1.2
**File:** `syntax/ast.rs:156-157`

`PrintLevel { Log, Warn, Error }` — fieldless enum, derives `PartialEq` but not `Copy`.

---

### ✅ 25. `ScopeKind` Could Derive `Copy` (Priority: 3, Complexity: 1)

**Chapter:** 1.2
**File:** `analysis/symbols.rs:36`

`ScopeKind { Global, Function, Block }` — fieldless enum.

---

### ✅ 26. `assert_eq!` with Literal Bool in Tests (Priority: 3, Complexity: 1)

**Chapter:** 5.4 (How to assert)
**Files:** 12 occurrences in `tests/runtime.rs`

Pattern: `assert_eq!(f(&rt, "flag"), true)` — should be `assert!(matches!(...))` or `assert!(f_bool(&rt, "flag"))`.

---

### ✅ 27. No `#[non_exhaustive]` on Public Enums (Priority: 3, Complexity: 1)

**Chapter:** 8.9 (Documentation Checklist)
**Files:** `error.rs:ErrorCode`, `runtime/value.rs:Value`, `types/draw.rs:DrawCommand`

Public enums that downstream crates might match on should use `#[non_exhaustive]` to allow adding variants without breaking changes.

**Fix:** Add `#[non_exhaustive]` to `ErrorCode`, `Value`, `DrawCommand`, `ShapeDesc`, `RenderMode`.

---

### ✅ 28. `NamespaceRegistry` Uses `Vec<Box<dyn NamespaceProvider>>` (Priority: 3, Complexity: 3)

**Chapter:** 6.5 (Trait Object Ergonomics), 6.6
**File:** `namespaces/mod.rs:72`

The registry uses `Vec<Box<dyn NamespaceProvider>>` for 4 fixed namespaces. These are always the same concrete types (`CoreNamespace`, `ShapesNamespace`, `RenderNamespace`, `CoordsNamespace`). Per Ch.6: "Prefer generics when you control the concrete types."

Dynamic dispatch is not needed here since the set of namespaces is fixed at compile time. However, the current design allows runtime extensibility which may be useful later.

**Fix:** Consider using a struct with named fields instead of a Vec of trait objects, or accept the trade-off for future plugin support.

---

### ✅ 29. `call_fn` and `call_closure` Are Nearly Identical (Priority: 3, Complexity: 3)

**Chapter:** 1.6 (Breaking up long functions)
**File:** `runtime/interpreter.rs:488-576`

`call_fn` and `call_closure` differ only in: closure has captured environment, closure checks arg count. The body iteration, depth tracking, scope management, error handling are identical.

**Fix:** Extract shared logic into `call_body(name, params, body, arg_vals, captured, call_line)`.

---

### ✅ 30. Intermediate Collections in Iterator Chains (Priority: 3, Complexity: 2)

**Chapter:** 3.4 (Avoid intermediate collections)
**Files:** `interpreter.rs:448-453`, `analysis/checker.rs:528-529`

```rust
let arg_vals: Vec<Value> = args.iter()
    .map(|a| self.eval_expr(a))
    .collect::<Result<_, _>>()?;
```

This is actually correct since the `Vec` is consumed by the callee. However, patterns like `self.lookup.field_names(&obj_ty)` → `Vec<String>` → `.iter().map(|s| s.as_str()).collect::<Vec<&str>>()` create an unnecessary intermediate `Vec`.

**Fix:** Return iterators instead of `Vec` where possible from `field_names`, `method_names`.

---

### ✅ 31. `TokenKind` Contains Owned `String` Data (Priority: 2, Complexity: 6)

**Chapter:** 1.1 (Borrowing), 3.2 (Redundant Cloning)
**File:** `syntax/token.rs:1`

`TokenKind::Ident(String)`, `TokenKind::StringLit(String)`, `TokenKind::HexColor(String)` store owned strings. Every `peek_kind()` call clones the entire token including its string payload. The lexer output is immutable after creation.

**Fix:** Store string data in a separate arena or intern pool, have `TokenKind` reference it by index. This is a large refactor and may not be worth it unless parsing becomes a bottleneck.

---

### ✅ 32. No Doc Tests (Priority: 2, Complexity: 2)

**Chapter:** 5.2 (Add Test Examples to Docs)
**File:** `lib.rs`

The public API has no doc tests. `compile()` and `Runtime` have no `/// # Examples` sections. Per Ch.5.2: "Rustdoc can turn examples into executable tests."

**Fix:** Add `/// # Examples` with runnable code to `compile()`, `Runtime::new()`, `Runtime::tick()`.

---

### ✅ 33. `Scope::symbols` Field Is `pub` via `pub struct` but Accessed Through Methods (Priority: 2, Complexity: 1)

**Chapter:** 8.7 (API documentation)
**File:** `analysis/symbols.rs:43-44`

`Scope` has `pub symbols: HashMap<String, Symbol>` but also provides `get()`, `get_mut()`, `declare()`. Direct field access bypasses the methods. Should be private with public methods.

**Fix:** Make `symbols` private (`symbols: HashMap<...>`). Fix any direct accesses.

---

### ✅ 34. Import Ordering Not Standardized (Priority: 1, Complexity: 1)

**Chapter:** 1.7 (Use Declarations)
**Files:** Throughout

Imports mix `std`, `crate`, and `super` without consistent grouping or blank-line separation. Example from `interpreter.rs`:
```rust
use crate::syntax::ast::{...};
use crate::types::draw::DrawCommand;
use crate::namespaces::{...};
use crate::{Input, State, Value};
use std::cell::RefCell;
use std::collections::HashMap;
```

Per Ch.1.7: `std` first, then external crates, then `crate`/`super`.

**Fix:** Configure `rustfmt.toml` with `group_imports = "StdExternalCrate"` and run `cargo +nightly fmt`.

---

---

## Summary: Top Actions

1. **Add clippy lint config** (#1) — zero-effort, catches 53 existing warnings
2. **Fix `Env::get` cloning** (#2) — biggest per-frame performance issue remaining
3. **Use `thiserror` for error types** (#3) — makes the crate a proper Rust library
4. **Replace `unwrap()` in production** (#4) — correctness hygiene
5. **Box large `Value` variants** (#5) — reduces stack size of every `Value` instance
6. **DRY the lifecycle methods** (#6) — prevents triple-edit bugs
7. **Derive `Copy` on small types** (#7, #19, #20, #24, #25) — batch fix, 5 minutes total
