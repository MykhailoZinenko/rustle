# Rustle — Completed Practices Fixes

Tracking file for issues resolved from PRACTICES_REVIEW.md.

---

### 7. Span should derive Copy
Added `Copy` to Span derive (already done by user in prior edit).

### 19. ExportKind should derive Copy
Added `Copy` to `ExportKind` derive in `namespaces/mod.rs`.

### 20. BinOp and UnOp should derive Copy
Already had `Copy` (done by user in prior edit).

### 24. PrintLevel should derive Copy
Already had `Copy` (done by user in prior edit).

### 25. ScopeKind should derive Copy
Added `Copy` to `ScopeKind` derive in `analysis/symbols.rs`.

### 11. Origin uses manual from_str instead of FromStr trait
Replaced `Origin::from_str()` method with `impl std::str::FromStr for Origin`. Updated callers in `coords.rs` and `shapes.rs` to use `.parse::<Origin>()`.

### 12. Default implementations could be derived
Derived `Default` on `Origin` (`#[default]` on `Center`), `CoordMeta`, and `RenderMode` (`#[default]` on `Sdf`). Removed manual `impl Default` blocks. Left `TransformData` manual (non-zero defaults).

### 1. No clippy lint configuration
Added `[workspace.lints.clippy]` to root `Cargo.toml` with `all`, `pedantic`, `redundant_clone`, `large_enum_variant`, `needless_collect`. Added `[lints] workspace = true` to all 3 crate Cargo.tomls.

### 13. Collapsible if statements
Fixed all 17 collapsible `if` patterns across checker, interpreter, collector.

### 14. Redundant reference creation
Removed 11 unnecessary `&` references that the compiler auto-derefs.

### 26. assert_eq! with literal bool in tests
Changed `assert_eq!(x, true)` → `assert!(x)` and `assert_eq!(x, false)` → `assert!(!x)` across 12 test sites.

### 3. Error types don't implement std::error::Error
Added `impl std::error::Error for Error {}` and `impl std::error::Error for RuntimeError {}`.

### 4. unwrap() in production code
Replaced 5 `unwrap()` calls with `expect("reason")` in `symbols.rs`, `interpreter.rs`, `checker.rs`.

### 15. Duplicate named() helpers
Consolidated 5 separate `fn named()` helpers into `Type::from_name()` on the Type enum. Removed duplicated match logic from all namespace files.

### 10. ErrorCode::as_str() dead abstraction
Removed dead `is_error()` method (always returned true). Simplified `resolve()` partition to direct assignment.

### 16. read_number silently swallows parse errors
Changed `read_number` to return `Result<f64, Error>`. Unparseable numbers now produce L005 error instead of silently becoming 0.0.

### 27. No #[non_exhaustive] on public enums
Added `#[non_exhaustive]` to `ErrorCode`, `ShapeDesc`, `RenderMode`, `DrawCommand`, `Value`. Updated IDE crate matches with wildcard arms.

### 33. Scope::symbols field visibility
Already private — no change needed.

### 6. Massive code duplication in lifecycle methods
Extracted shared `run_lifecycle()` method. `run_update`, `run_init`, `run_on_exit` reduced from ~30 lines each to 2-3 lines each.

### 29. call_fn and call_closure nearly identical
Merged into single `call_body()` method. Callers pass `&HashMap::new()` for plain functions, actual captures for closures.

### 5. Value enum large variant size disparity
Boxed `Closure` variant into `Value::Closure(Box<ClosureData>)` with a dedicated `ClosureData` struct. Reduces stack size of every `Value` instance.

### 9. No doc comments on public API
Added `//!` crate doc to `lib.rs`, `///` docs to all public structs/functions/enums in `lib.rs`, `error.rs`, `value.rs`. Added doc test for `compile()`.

### 32. No doc tests
Added runnable `# Examples` doc test to `compile()`.

### 23. ErrorCode semantic names
Added `///` doc comments to all 42 `ErrorCode` variants explaining what each code means.

### 17. No rustfmt.toml configuration
Created `rustfmt.toml` with `reorder_imports`, `imports_granularity = "Crate"`, `group_imports = "StdExternalCrate"`.

