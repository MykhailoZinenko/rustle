# Rustle — Completed Fixes

Tracking file for issues resolved from LANGUAGE_REVIEW.md.

---

### 1. AST Cloning on Every Tick
Eliminated all deep AST clones from the interpreter. Used the `let program = self.program` borrow-splitting trick to avoid cloning program items. Wrapped `FnDef.params` and `FnDef.body` (and `Expr::Lambda`, `Value::Closure`) in `Arc<[T]>` so closure creation is O(1) ref-count bump instead of deep tree clone. Added `fn_table: HashMap<&str, &FnDef>` for O(1) function lookup, replacing all linear scans. Removed `body.to_vec()` from `call_fn`/`call_closure`. Used `Arc` (not `Rc`) because `Program` crosses thread boundaries in `rustle-app`.

