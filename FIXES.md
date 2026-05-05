# Rustle — Completed Fixes

Tracking file for issues resolved from LANGUAGE_REVIEW.md.

---

### 7. `error` Function Returns Hardcoded `res<float>`
`error()` now infers inner type from the current function's return type. Falls back to `res<()>` when no context. Added `res<()>` compatibility rule so error() works in any res<T> context.

### 8. `values_equal` Uses IEEE Float Equality
Changed float comparison to bitwise (`to_bits()`) — NaN == NaN is true, +0.0 != -0.0. Applied to Float, Vec2, Vec3, Vec4, Color comparisons.

### 10. Empty List Type Inference Is Wrong
Empty lists now infer as `list[()]` instead of `list[float]`. Added compatibility rule: `list[()]` assignable to any `list[T]`.

### 15. Duplicate `as_float` Helper Functions
Removed duplicate `as_float` from interpreter.rs, imported from `namespaces::mod`.

### 17. Validator's Const Check is Redundant
Removed `check_const_reassignment` and related methods from validator. Checker (Pass 2) already handles this.

### 23. `safe_index` Truncates Floats Silently
Added `f.fract() != 0.0` check — non-integer indices now produce R006 error.

### 31. `RuntimeError` Doesn't Include Column
Added `column: usize` field to `RuntimeError`. Display format now shows `line:column`. All ~50 call sites updated.

### 21. `#` Ambiguity: Comment vs Hex Color
Removed `#` as comment marker. `#` now only introduces hex colors; bare `#` produces L001 error.

### 22. No Multiline String Support
Strings can now span multiple lines. Removed newline termination from `read_string`.

### 11. Cast Does Nothing at Runtime
Implemented actual type conversions for `as` expressions: float<->bool, float->string, bool->string, string->float (with parse error), string->bool. Added compile-time validation via `is_castable()` in checker. Invalid casts produce S002 at compile time, runtime conversion failures produce R001.

### 13. string Type Has No Methods or Operations
Added string concatenation (`+`), comparison operators (`<`, `<=`, `>`, `>=`), and 9 methods: `len()`, `contains()`, `starts_with()`, `ends_with()`, `trim()`, `to_upper()`, `to_lower()`, `replace()`, `split()`. Registered string in binop_registry for operator dispatch.

### 24. LookupContext Allocates a New TypeRegistry
LookupContext now stores `&'a TypeRegistry` reference instead of owning one. Single TypeRegistry created in `resolve()` and shared through checker and lookup. One allocation per compile instead of two.

### 27. PartialEq on TokenKind::Float
Documented: IEEE NaN inequality is acceptable because the lexer never produces NaN tokens.

### 28. Value Does Not Implement PartialEq
Documented: intentionally absent. Equality handled by `values_equal()` with bitwise float semantics.

### 30. Scope::symbols HashMap Ordering
Added `names.sort()` to `all_visible_names()` for deterministic suggestion output.

### 6. Interpreter Creates New Environment on Every Tick
Cached the base environment (imports + top-level constants) after initial `run_top_level()`. On subsequent ticks with `on_update`, the cached env is restored instead of re-running all imports and top-level statements. Fresh output buffer per tick. Static scripts (no `on_update`) still re-run top-level each frame. Removed redundant `run_top_level()` call from `run_update()`.

### 12. Span Lacks End Position
Extended `Span` with `end_line` and `end_column` fields. `Span::new()` defaults end to start (backward compatible). Added `Span::range()` for explicit ranges. Updated all 17 statement/definition parsers to use `span_from()` for end position tracking. Enables IDE squiggly underlines.

### 18. State Type is Not in TypeRegistry
Registered State in TypeRegistry with `keys()` → `list[string]` and `len()` → `float` methods. Fields remain dynamic via LookupContext. Updated `value_type_key` and `type_to_registry_key` to map State properly.

### 26. `for` Step Must Be an Assignment Statement
Parser now accepts expression statements (like `i++`, `i--`, function calls) in the `for` loop step clause, not just assignments. Falls back to `parse_expr()` when `is_path_assign()` returns false.

### 2. Rc Cycle Memory Leaks / No Recursion Depth Limit (#2 + #9)
Added a call depth limit (MAX_CALL_DEPTH = 64) to both `call_fn` and `call_closure`. Infinite recursion now produces a clean `R011` error with a descriptive message instead of crashing the Rust process with a stack overflow. The Rc cycle leak itself is documented as a known limitation — proper resolution requires either weak references (breaks user semantics), a GC, or arena allocation. The depth limit prevents the most common trigger (recursive closures) and is the actionable mitigation.

### 4. Type::Named String Comparison is Fragile
Added dedicated `Type` enum variants for all built-in types: `String`, `Vec2`, `Vec3`, `Vec4`, `Color`, `Mat3`, `Mat4`, `Transform`, `Shape`, `Circle`, `Rect`, `Line`, `Polygon`, `State`, `Input`. `Type::Named(String)` is now reserved exclusively for user-defined types (future structs/enums). Parser maps known identifiers in type position to proper variants. Updated all string comparisons (`n == "State"`, `n.as_str() == "shape"`, etc.) throughout checker, validator, lookup, registries, and namespaces. User-defined types can no longer collide with built-in type names.

### 3. No break/continue Statements
Added `break` and `continue` as keywords, AST nodes (`Stmt::Break`, `Stmt::Continue`), parser rules, and interpreter control flow. Validator checks they only appear inside loops (while/for/foreach). In loops: `break` clears its flag and exits; `continue` clears its flag and skips to next iteration. Both propagate through if/match blocks correctly via `should_stop_block()`. Added 10 new tests (5 resolver + 5 runtime).

### 5. TypeRegistry and BinopRegistry Recreated on Every Frame
Moved `TypeRegistry` and `BinopRegistry` into the `Program` struct, constructed once at compile time. Interpreter now takes `&'a TypeRegistry` and `&'a BinopRegistry` references instead of constructing its own `Default` instances per frame.

### 1. AST Cloning on Every Tick
Eliminated all deep AST clones from the interpreter. Used the `let program = self.program` borrow-splitting trick to avoid cloning program items. Wrapped `FnDef.params` and `FnDef.body` (and `Expr::Lambda`, `Value::Closure`) in `Arc<[T]>` so closure creation is O(1) ref-count bump instead of deep tree clone. Added `fn_table: HashMap<&str, &FnDef>` for O(1) function lookup, replacing all linear scans. Removed `body.to_vec()` from `call_fn`/`call_closure`. Used `Arc` (not `Rc`) because `Program` crosses thread boundaries in `rustle-app`.

