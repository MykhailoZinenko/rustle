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

### 17. No rustfmt.toml configuration
Created `rustfmt.toml` with `reorder_imports`, `imports_granularity = "Crate"`, `group_imports = "StdExternalCrate"`.

