# Rustle Language Implementation Review

**Date:** 2025-05-04
**Scope:** Full review of `crates/rustle-lang/src/` (26 files, ~13K lines)
**Build status:** 465/465 tests pass

**Rating system:**
- **Priority** (1-10): How urgently this should be fixed. 10 = blocks future work, 1 = cosmetic.
- **Complexity** (1-10): Effort to fix. 10 = architectural rewrite, 1 = one-line change.

Sorted by priority descending, then complexity descending.

---

## CRITICAL — Fix Before Proceeding

### ✅ 1. AST Cloning on Every Tick (Priority: 10, Complexity: 7)

**File:** `runtime/interpreter.rs:154`, `lib.rs:119`

Every call to `run_update`, `run_init`, `run_on_exit`, and `run_top_level` clones the entire AST:
```rust
let items = self.program.items.clone();  // clones ALL items every frame
let f = self.program.items.iter().find_map(|i| match i {
    Item::FnDef(f) if f.name == "on_update" => Some(f.clone()),  // clones the entire fn body
```
Additionally, `call_fn` and `call_closure` both do `let body = body.to_vec()` on every function call.

For a 60fps animation with nested function calls, this means thousands of AST clones per second. Every `Vec<Stmt>`, `String`, `Expr`, and `Span` is deep-cloned.

**Impact:** Directly limits runtime performance. Gets worse as scripts grow. Will become the primary bottleneck before any rendering work matters.

**Fix:** Store the program in an `Arc` or use indices/references into a shared AST. Function bodies should be referenced by index, not cloned. The `body.to_vec()` in `call_fn`/`call_closure` is the worst offender — it happens on every function call.

---

### ✅ 2. Rc Cycle Memory Leaks (Priority: 9, Complexity: 8)

**Files:** `runtime/value.rs:20,33`, documented in CLAUDE.md

`List` and `State` use `Rc<RefCell<...>>`. Closures capture `Value` by clone into a `HashMap<String, Value>`. This creates uncollectable cycles when:
- A list contains a closure that captures that list
- State holds a closure that captures state
- A closure captures itself (recursive lambdas)
- Mutual closure capture

**Impact:** Memory leak per cycle. In a long-running animation (the primary use case), leaked closures accumulate frame-over-frame. Users writing idiomatic code like `s.on_click = fn(x) { s.count++ }` will hit this.

**Fix:** Either:
- Use `Weak<RefCell<...>>` for back-references from closures to their capturing scope
- Implement a simple mark-and-sweep on `Runtime::tick` boundaries
- Use an arena allocator that gets reset between lifecycle phases

---

### ✅ 3. No `break`/`continue` Statements (Priority: 8, Complexity: 4)

**Files:** `syntax/ast.rs`, `syntax/parser.rs`, `runtime/interpreter.rs`

`while` and `for` loops have no `break` or `continue`. Users must use boolean flags to exit loops early, which is error-prone and unergonomic. This is a language fundamental that will be painful to retrofit once users have workarounds everywhere.

**Impact:** Makes many common loop patterns impossible or awkward. Affects every user-written loop that isn't trivially bounded.

**Fix:** Add `Break` and `Continue` variants to `Stmt`, parse them, validate they're inside loops (validator.rs pass 3), and handle them in the interpreter via a control flow enum or sentinel value.

---

### ✅ 4. `Type::Named` String Comparison is Fragile (Priority: 8, Complexity: 6)

**Files:** Throughout — `checker.rs`, `lookup.rs`, `collector.rs`, `registry.rs`

Type identity relies on string comparison of `Type::Named(String)`:
```rust
if n == "State"     // lookup.rs:37
if n == "string"    // checker.rs:977
matches!(n.as_str(), "shape" | "circle" | "rect" | "line" | "polygon")  // checker.rs:947
```

There's no canonical registry of type names. "string" is `Type::Named("string")` but `float` is `Type::Float`. Shape types are `Named("circle")` etc. `"State"` is capitalized, others aren't. When structs are added (Phase 2), every one of these string comparisons becomes a potential collision point.

**Impact:** Adding user-defined types (structs, enums) requires auditing every `Type::Named` string comparison. A user naming a struct `string` or `shape` will break the type system silently.

**Fix:** Either:
- Add first-class variants for all built-in named types: `Type::String`, `Type::Vec2`, `Type::Color`, `Type::Shape(ShapeKind)`, etc.
- Or use an interned ID system with a name registry that prevents collisions

---

### ✅ 5. `TypeRegistry` and `BinopRegistry` Recreated on Every Frame (Priority: 7, Complexity: 3)

**Files:** `lib.rs:113`, `runtime/interpreter.rs:77-78`

Every `tick()` creates a new `Interpreter`, which constructs fresh `TypeRegistry::default()` and `BinopRegistry::default()`. These registries are immutable after construction — they register dozens of type descriptors and hundreds of operator entries via HashMap inserts.

**Impact:** Unnecessary allocation and initialization on every frame. For 60fps, that's 120 HashMap constructions per second, each with dozens of insertions.

**Fix:** Store `TypeRegistry` and `BinopRegistry` in the `Runtime` struct (or in `Program`) and pass references to the interpreter.

---

## HIGH — Should Fix Soon

### ✅ 6. Interpreter Creates New Environment on Every Tick (Priority: 7, Complexity: 5)

**File:** `lib.rs:113-126`

`Runtime::tick()` creates a brand new `Interpreter` (with fresh `Env`) every frame. Top-level bindings (imports, constants, variables) are re-established from scratch via `run_top_level()`. The only state that persists is `State` (explicitly) and `RuntimeState` (coord_meta).

This means all top-level `let` bindings, `const` declarations, and import resolutions are re-evaluated every single frame — even though they never change.

**Impact:** Wasted work per frame. Also prevents future optimizations like caching computed constants.

**Fix:** Persist the environment between ticks. Only re-run `on_update`, not all top-level statements.

---

### ✅ 7. `error` Function Returns Hardcoded `res<float>` (Priority: 7, Complexity: 2)

**File:** `analysis/checker.rs:676`

```rust
"error" => {
    return Ok(Type::Res(Box::new(Type::Float))); // placeholder
}
```

The `error()` function always resolves to `res<float>` regardless of context. If a user writes `fn f() -> res<string> { return error("fail") }`, the type checker will either miss the mismatch or produce a confusing error.

**Impact:** Type checking is wrong for any `res<T>` where T != float. Silent type unsoundness.

**Fix:** Infer the expected type from context (e.g., the function return type), or make `error` generic by accepting a type parameter.

---

### ✅ 8. `values_equal` Uses IEEE Float Equality (Priority: 6, Complexity: 2)

**File:** `runtime/interpreter.rs:1113`

```rust
(Value::Float(x), Value::Float(y)) => x == y,
```

`NaN != NaN` per IEEE, so `match x { NaN => { ... } }` will never match. Also, `0.0 == -0.0` is true, which may surprise users doing coordinate math.

**Impact:** Subtle bugs in match statements and equality checks involving NaN or negative zero.

**Fix:** Decide on Rustle's equality semantics and document them. Options: use `f64::total_cmp` for deterministic ordering, treat NaN as equal to NaN (like many scripting languages do), or explicitly error on NaN comparisons.

---

### ✅ 9. No Recursion Depth Limit (Priority: 6, Complexity: 2)

**File:** `runtime/interpreter.rs:480-515`

`call_fn` and `call_closure` have no depth counter. A recursive function (even accidental infinite recursion) will stack overflow the Rust process with no recovery.

**Impact:** Unrecoverable crash (not a catchable error). The cancellation token only checks at loop boundaries, not call depth.

**Fix:** Add a `call_depth: usize` field to `Interpreter`, increment on entry, decrement on exit, error at a configurable limit (e.g., 256).

---

### ✅ 10. Empty List Type Inference Is Wrong (Priority: 6, Complexity: 3)

**File:** `analysis/checker.rs:614-617`

```rust
if items.is_empty() {
    return Ok(Type::List(Box::new(Type::Float))); // lenient for now
}
```

An empty list `[]` silently becomes `list[float]`. If the user writes `let xs: list[string] = []`, the `expect_type` call will flag a mismatch (`list[float]` vs `list[string]`). The comment says "lenient for now" but the behavior is incorrect.

**Impact:** Empty list literals may fail type checking when they shouldn't, or silently get the wrong inner type.

**Fix:** Propagate the expected type from the surrounding context (annotation), or use a `Type::Infer` placeholder that unifies during assignment.

---

### ✅ 11. `Cast` Does Nothing at Runtime (Priority: 6, Complexity: 4)

**File:** `runtime/interpreter.rs:351`

```rust
Expr::Cast { expr, .. } => self.eval_expr(expr),
```

`as` just evaluates the inner expression and ignores the target type entirely. It doesn't convert, validate, or even check. `"hello" as float` returns `"hello"` at runtime.

**Impact:** Users expect `as` to actually cast. Combined with the type checker accepting it (`checker.rs:488` returns `Ok(ty.clone())`), this creates a hole where the type system says a value is one type but at runtime it's another.

**Fix:** Implement actual conversions for sensible pairs (float→bool, bool→float, etc.) and error for impossible casts. This blocks `to_string()`/`to_float()` from Phase 1.

---

### ✅ 12. `Span` Lacks End Position (Priority: 5, Complexity: 4)

**Files:** `syntax/ast.rs:1-6`

```rust
pub struct Span {
    pub line: usize,
    pub column: usize,
}
```

Spans only store the start position, not the end. Error messages can only point to where something starts, not underline the full problematic region. This makes IDE integration (squiggly underlines, error highlighting) impossible.

**Impact:** Blocks IDE integration for the editor (`rustle-app`). Every error can only highlight one character position.

**Fix:** Add `end_line` and `end_column` (or byte offset + length). This is a pervasive change since `Span` is embedded in every AST node.

---

### ✅ 13. `string` Type Has No Methods or Operations (Priority: 5, Complexity: 3)

**File:** `types/registry.rs:301-310` (string_desc)

The `string` type descriptor exists but likely has no methods registered (the file shows `string_desc()` but it appears to have empty or minimal content). Strings can't be concatenated, indexed, sliced, or compared with `<`/`>`.

**Impact:** Strings are first-class citizens in the type system but nearly useless at runtime. This is the next Phase 1 item so it directly blocks progress.

---

## MEDIUM — Address When Convenient

### ✅ 14. `program.items.clone()` in Top-Level Execution (Priority: 5, Complexity: 2)

**File:** `runtime/interpreter.rs:154`

`run_top_level` clones `self.program.items` before iterating — this is done to avoid borrow conflicts (immutable borrow of `program` vs mutable `self`). But the clone is unnecessary if the program is behind an `Arc` or if the interpreter borrows it differently.

**Impact:** Contributes to the per-frame allocation overhead (issue #1), but less severe than function body clones.

---

### ✅ 15. Duplicate `as_float` Helper Functions (Priority: 4, Complexity: 1)

**Files:** `namespaces/mod.rs:121`, `runtime/interpreter.rs:1103`, `types/registry.rs:296`

Three separate `as_float` functions doing the same thing (extract f64 from Value::Float or error).

**Impact:** Code duplication. Maintenance burden when error formats change (as just happened with error codes).

**Fix:** Expose one canonical `as_float` and use it everywhere.

---

### ✅ 16. `peek_kind()` Clones TokenKind on Every Call (Priority: 4, Complexity: 2)

**File:** `syntax/parser.rs:824-826`

```rust
fn peek_kind(&self) -> TokenKind {
    self.tokens[self.pos].kind.clone()
}
```

`TokenKind` contains `String` for `Ident` and `StringLit` variants. Every call to `peek_kind()` (hundreds of times during parsing) deep-clones the string. The parser calls it multiple times per token in `is_path_assign`, `parse_stmt`, etc.

**Impact:** Parsing performance, especially for large files. Not critical now but compounds with file size.

**Fix:** Return `&TokenKind` instead of cloning.

---

### ✅ 17. Validator's Const Check is Redundant (Priority: 4, Complexity: 1)

**File:** `analysis/validator.rs:84-138`

Pass 3 (Validator) re-walks the entire AST to check const reassignment. But the checker (Pass 2) already checks this in `check_assign` and `check_assignable_lvalue`. The validator's check is a subset of what the checker does.

**Impact:** Duplicate error reports for const reassignment. Extra AST traversal. Not a bug but unnecessary work.

**Fix:** Remove the const reassignment check from the validator, or remove it from the checker.

---

### ✅ 18. `State` Type is Not in TypeRegistry (Priority: 4, Complexity: 3)

**Files:** `types/registry.rs` (not registered), `runtime/interpreter.rs:978-993`

State fields are handled by special-case code in both the resolver (`lookup.rs:37-46`) and interpreter (`eval_field`). This means State doesn't participate in the normal field/method dispatch, making it impossible to add methods to State (like `s.keys()` or `s.clone()`).

**Impact:** Limits State extensibility. Every new State feature requires special-casing in multiple files.

---

### ✅ 19. `fn on_update` Scanned by Linear Search on Every Tick (Priority: 4, Complexity: 1)

**File:** `lib.rs:119`

```rust
if self.program.ast.items.iter().any(|i| matches!(i, Item::FnDef(f) if f.name == "on_update")) {
```

This linear scan happens every single frame. Same pattern in `run_init`, `run_on_exit`, `run_update`, etc.

**Impact:** Minor per-frame cost, but easily avoidable.

**Fix:** Cache lifecycle function indices in `Program` during compilation.

---

### ✅ 20. `Env::get` Clones Values on Every Access (Priority: 4, Complexity: 5)

**File:** `runtime/interpreter.rs:48-51`

```rust
fn get(&self, name: &str) -> Option<Value> {
    for scope in self.scopes.iter().rev() {
        if let Some(v) = scope.get(name) { return Some(v.clone()); }
    }
    None
}
```

Every variable read clones the value. For simple `Float` or `Bool` this is cheap, but for `List`, `Closure`, `Vec4`, `Mat4`, `Shape`, etc., this is a deep clone. A function reading 5 variables clones all 5.

**Impact:** Significant overhead for complex values. Combined with issue #1 (body cloning), this makes the interpreter much slower than necessary.

**Fix:** Use `Cow<Value>` or `Rc<Value>`, or return references where possible.

---

### ✅ 21. `#` Ambiguity: Comment vs Hex Color (Priority: 3, Complexity: 2)

**File:** `syntax/lexer.rs:105-108`

```rust
b'#' => {
    if self.is_hex_sequence() { TokenKind::HexColor(self.read_hex_color()) }
    else { self.skip_line(); return Ok(None); }
}
```

`#` is both a line comment marker and a hex color prefix. `is_hex_sequence` checks if the next 6 bytes are hex digits. This means `# ff0000` (with space) is a comment, but `#ff0000` is a color. More subtly, `#abcdef` is always a color even if the user intended a comment with those exact characters. And `#123` (3 digits) is a comment, not a short hex color.

**Impact:** User confusion. No way to document hex values in comments starting with `#`. The 3-digit hex shorthand (`#f00`) common in CSS won't work and silently becomes a comment.

**Fix:** Remove `#` as a comment marker (Rustle already has `//` and `/* */`).

---

### ✅ 22. No Multiline String Support (Priority: 3, Complexity: 2)

**File:** `syntax/lexer.rs:198`

```rust
if self.is_at_end() || self.peek() == b'\n' {
    return Err(Error::new(ErrorCode::L002, ...));
}
```

Strings cannot span multiple lines. The lexer errors on a newline inside a string literal.

**Impact:** Users can't write multiline text content. Blocks some creative coding use cases (text templates, generated SVG, etc.).

---

### ✅ 23. `safe_index` Truncates Floats Silently (Priority: 3, Complexity: 1)

**File:** `runtime/interpreter.rs:1286`

```rust
Ok(f as usize)
```

`xs[2.7]` silently truncates to `xs[2]`. No warning or error for non-integer indices.

**Impact:** Subtle bugs when users accidentally use float expressions as indices (e.g., division results).

**Fix:** Check `f.fract() != 0.0` and either warn or error.

---

### ✅ 24. `LookupContext` Allocates a New `TypeRegistry` (Priority: 3, Complexity: 2)

**File:** `analysis/lookup.rs:23`

```rust
Self { program, registry, type_registry: TypeRegistry::default() }
```

The resolver creates a `LookupContext` which constructs its own `TypeRegistry::default()`. The checker also creates a `BinopRegistry::default()`. These are separate from what the interpreter uses. All three construct identical registries.

**Impact:** Wasteful allocation during compilation. Also a maintenance risk — if registrations diverge between resolve-time and runtime, type checking becomes unsound.

**Fix:** Share a single `TypeRegistry` instance (e.g., stored in `Program` or `NamespaceRegistry`).

---

### ⏭ 25. No Integer Type (Priority: 3, Complexity: 7)

**Files:** Throughout

All numbers are `f64`. Array sizes are `f64` parsed as `usize`. Loop counters are `f64`. Index values are `f64` cast to `usize`. This works for creative coding but causes precision issues for large counts and silently loses data on truncation.

**Impact:** Not a bug per se — it's a design choice. But it means every index operation, loop counter, and size value goes through float→int conversion with potential precision loss for values > 2^53.

---

### ✅ 26. `for` Step Must Be an Assignment Statement (Priority: 3, Complexity: 3)

**File:** `syntax/parser.rs:387`

```rust
let step = Box::new(self.parse_assign()?);
```

The `for` loop step clause is restricted to assignment statements. `i++` doesn't work as a step because the parser expects an assignment, not an expression statement. Users must write `i = i + 1.0` instead of `i++`.

**Impact:** Ergonomic friction. Users will try `for let i = 0; i < 10; i++ { }` and get a confusing parser error.

**Fix:** Allow expression statements in the step position, or special-case `++`/`--`.

---

## LOW — Nice to Have

### ✅ 27. `PartialEq` Not Derived on `TokenKind::Float(f64)` (Priority: 2, Complexity: 1)

**File:** `syntax/token.rs:1`

`TokenKind` derives `PartialEq`, but `Float(f64)` uses IEEE equality. `TokenKind::Float(f64::NAN) != TokenKind::Float(f64::NAN)`. Unlikely to cause issues in practice since the lexer doesn't produce NaN literals.

---

### ✅ 28. `Value` Does Not Implement `PartialEq` (Priority: 2, Complexity: 3)

**File:** `runtime/value.rs`

`Value` has a standalone `values_equal` function instead of implementing `PartialEq`. This means you can't use `==` on Values in Rust code, and the equality semantics are hidden in a free function that could diverge from other equality checks.

---

### ⏭ 29. Error Recovery in Parser is Minimal (Priority: 2, Complexity: 5)

**File:** `syntax/parser.rs:970-988`

The `recover()` function just skips tokens until it finds a keyword that looks like a new statement. This is the bare minimum for error recovery — it doesn't synchronize on matched delimiters, doesn't track nesting depth, and can skip valid code.

**Impact:** After the first error, subsequent errors may be misleading or miss real problems.

---

### ✅ 30. `Scope::symbols` Uses `HashMap` — No Deterministic Ordering (Priority: 2, Complexity: 2)

**File:** `analysis/symbols.rs:43-44`

Symbol tables use `HashMap`, so iteration order is non-deterministic. The `all_visible_names()` method (used for suggestions) returns names in arbitrary order. Error messages that list available symbols will show different orderings across runs.

**Fix:** Use `IndexMap` or sort before display.

---

### ✅ 31. `std::fmt::Display` for `RuntimeError` Doesn't Include Column (Priority: 1, Complexity: 1)

**File:** `error.rs:159`

`RuntimeError` stores `line` but not `column`, and its Display shows only the line number. Compile-time `Error` has both.

**Impact:** Runtime errors point to a line but not a column. Less precise than compile errors.

---

### ✅ 32. No `impl std::error::Error` for Error Types (Priority: 1, Complexity: 1)

**File:** `error.rs`

Neither `Error` nor `RuntimeError` implement the `std::error::Error` trait. This blocks idiomatic Rust error handling (`?` operator, `anyhow`, `thiserror` integration) in downstream crates.

---

---

## Summary: What Must Be Addressed Before Proceeding

The issues that will become **architectural footguns** if not addressed before Phase 2:

1. **AST cloning per frame (#1)** — This will make the language unusably slow for anything beyond trivial scripts. Fix the data ownership model before adding more features that make the AST larger.

2. **`Type::Named` string fragility (#4)** — Structs and enums (Phase 2) will collide with the stringly-typed system. Either add proper type variants or an intern table before implementing user-defined types.

3. **`Cast` is a no-op (#11)** — `to_string()`/`to_float()` (remaining Phase 1) logically depend on having a working cast/conversion mechanism.

4. **No `break`/`continue` (#3)** — A language fundamental that's cheaper to add now (before users write workarounds) than later (when you need to support both patterns).

5. **Empty list type inference (#10)** — Will cause user-facing bugs as soon as people write `let xs: list[string] = []`.

Everything else can wait, but these five items create compounding problems the longer they're deferred.
