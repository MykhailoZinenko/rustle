# Rustle — Completed Fixes

Tracking file for issues resolved from LANGUAGE_REVIEW.md.

---

### 3. No break/continue Statements
Added `break` and `continue` as keywords, AST nodes (`Stmt::Break`, `Stmt::Continue`), parser rules, and interpreter control flow. Validator checks they only appear inside loops (while/for/foreach). In loops: `break` clears its flag and exits; `continue` clears its flag and skips to next iteration. Both propagate through if/match blocks correctly via `should_stop_block()`. Added 10 new tests (5 resolver + 5 runtime).

### 5. TypeRegistry and BinopRegistry Recreated on Every Frame
Moved `TypeRegistry` and `BinopRegistry` into the `Program` struct, constructed once at compile time. Interpreter now takes `&'a TypeRegistry` and `&'a BinopRegistry` references instead of constructing its own `Default` instances per frame.

### 1. AST Cloning on Every Tick
Eliminated all deep AST clones from the interpreter. Used the `let program = self.program` borrow-splitting trick to avoid cloning program items. Wrapped `FnDef.params` and `FnDef.body` (and `Expr::Lambda`, `Value::Closure`) in `Arc<[T]>` so closure creation is O(1) ref-count bump instead of deep tree clone. Added `fn_table: HashMap<&str, &FnDef>` for O(1) function lookup, replacing all linear scans. Removed `body.to_vec()` from `call_fn`/`call_closure`. Used `Arc` (not `Rc`) because `Program` crosses thread boundaries in `rustle-app`.

