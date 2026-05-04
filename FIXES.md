# Rustle — Completed Fixes

Tracking file for issues resolved from LANGUAGE_REVIEW.md.

---

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

