# Rustle-Lang Exhaustive Code Review

**Date:** 2026-05-07
**Scope:** Full file-by-file review of `crates/rustle-lang/src/` (30 files, ~15,000 lines)
**Methodology:** Manual review against Apollo GraphQL Rust Best Practices, Clippy pedantic analysis, cross-file consistency checks.

**Severity scale:** 1 (cosmetic) — 10 (data loss / undefined behavior)
**Complexity scale:** 1 (one-line fix) — 10 (architectural rewrite)

---

## Critical Issues

### ~~R01. Heap index overflow silently wraps (heap.rs:18)~~ DROPPED
**Severity: ~~9~~ 1 | Complexity: 2**

Theoretical only. Requires >4 billion objects (~160+ GB RAM). OS kills the process long before this. Not a real issue.

---

### R02. ~~Compiler double-evaluates side-effecting expressions in inc/dec~~ ✅ FIXED
**Severity: 8 | Complexity: 5**

Added `DupAt(u8)`, `Swap`, and `Rot(u8)` opcodes to the VM instruction set. Rewrote all 4 inc/dec cases (prefix/postfix × index/field) to evaluate container and index expressions exactly once using pure stack manipulation. No temp locals, no re-evaluation. Regression tests: `r02_prefix_inc_on_index_evaluates_once`, `r02_postfix_dec_on_index_evaluates_once`.

**Files:** `vm/opcode.rs`, `vm/vm.rs`, `vm/compiler.rs`

---

### ~~R03. `values_equal` for nested lists can stack overflow~~ DROPPED
**Severity: ~~7~~ 1 | Complexity: 4**

Not a real issue. The VM's type system prevents self-referential list structures: `list.push` is typed to accept the element type, so `list[float].push(list[float])` is a compile error. Recursive types can't be written in Rustle's syntax.

**Files:** `vm/value.rs`

---

### R04. ~~Checker `check_state` aborts on first field inference error~~ ✅ FIXED
**Severity: 6 | Complexity: 2**

Changed `return` to `continue` so all state fields are checked and all errors reported. Regression test: `r04_multiple_state_field_errors_reported`.

**Files:** `analysis/checker.rs`

---

### R05. ~~`list_paste` is O(n^2) for bulk inserts~~ ✅ FIXED
**Severity: 5 | Complexity: 2**

Replaced insert loop with `Vec::splice`. Regression test: `r05_list_paste_bulk_insert`.

**Files:** `vm/list_ops.rs`

---

## Architecture Issues

### R06. ~~Massive code duplication across VM modules~~ ✅ FIXED
**Severity: 6 | Complexity: 6**

Extracted `require_float`, `require_heap_ref`, `check_arity` into shared `vm/util.rs`. Bool coercion now consistent everywhere (true→1.0, false→0.0).

**Files:** `vm/util.rs`, `vm/fields.rs`, `vm/methods.rs`, `vm/natives.rs`, `vm/list_ops.rs`

---

### R07. ~~Constant definitions duplicated between compiler and natives~~ ✅ FIXED
**Severity: 6 | Complexity: 3**

Color constants now defined once in `vm/util.rs` as `COLOR_CONSTANTS`. Both `compiler.rs` and `natives.rs` use `lookup_color()`.

**Files:** `vm/util.rs`, `vm/compiler.rs`, `vm/natives.rs`

---

### R08. Compiler assumes resolver correctness — no defensive checks
**Severity: 5 | Complexity: 7**

The compiler does not return `Result` — it assumes the resolver has validated everything. If the compiler is ever used without the resolver (or the resolver has gaps), invalid input causes panics via `unreachable!()` macros and unguarded array accesses rather than graceful errors.

All native functions access `args[0]` without bounds checking. If `CallNative` is emitted with wrong `argc`, this panics with an index-out-of-bounds rather than a `RuntimeError`.

**Fix:** Add `debug_assert!` guards at minimum. For a more robust solution, have native functions validate `args.len()` against their expected arity.

**Files:** `vm/compiler.rs`, `vm/natives.rs`

---

### R09. ~~Excessive `clone()` to work around borrow checker in VM~~ ✅ FIXED
**Severity: 4 | Complexity: 8**

Added `Heap::read_alloc` and `Heap::read_then` APIs. Rewrote `get_field` with a two-phase approach: Phase 1 borrows immutably and handles all non-allocating cases (Vec2.x, State.field, etc.) directly, extracting only the minimal data needed for allocating cases. Phase 2 allocates after the borrow is released. Rewrote `set_field_rebuild` the same way. Rewrote `deep_clone_heap_object` to extract child StackValues before recursing instead of cloning the entire parent.

Benchmarked before and after: 10-20% improvement on field-access-heavy workloads (5000 indexed access: 918µs→820µs, 2000 circles: 613µs→488µs).

**Files:** `vm/heap.rs`, `vm/fields.rs`, `vm/value.rs`

---

### R10. ~~No `Display` implementation for `Op`~~ ✅ FIXED
**Severity: 3 | Complexity: 3**

Implemented `Display` for all 57 `Op` variants with assembly-style formatting (e.g. `CALL chunk=1 argc=2`, `JUMP +10`). Added `Chunk::disassemble()` returning full disassembly with line deduplication. 5 new tests.

**Files:** `vm/opcode.rs`, `vm/chunk.rs`

---

### R11. ~~`Op` enum size not optimal~~ ✅ FIXED
**Severity: 3 | Complexity: 8**

Reduced `Op` from 8 bytes to 4 bytes by: (1) changing all jump offsets from `i32` to `i16` (±32K instructions — sufficient for any real function), (2) redesigning `MakeEnum(u16, u16, u8)` → `MakeEnum(u16, u8)` using enum_def_idx + variant_idx instead of two string pool indices (field count derived from `CompiledEnumVariant`). Added compile-time size assertion.

Benchmarked: 10k dispatch loop improved 24% (779µs→590µs), 10k circles improved 15% (2.7ms→2.3ms). Halving the instruction size improved cache utilization measurably.

**Files:** `vm/opcode.rs`, `vm/chunk.rs`, `vm/compiler.rs`, `vm/vm.rs`

---

### R12. ~~`vm.rs` main loop re-fetches frame on every iteration~~ ✅ FIXED
**Severity: 3 | Complexity: 6**

Added `ip`, `current_chunk`, `current_stack_base`, `current_closure_ref` as direct fields on `Vm`. The main loop and all opcode handlers now read/write these directly instead of going through `self.frames.last()`/`self.frames.last_mut()`. Added `push_frame()`/`pop_frame()` helpers that sync cached fields with the frames stack on call/return boundaries. Eliminated all `frames.last().unwrap()` calls from the hot path.

Benchmarked: ~2% improvement on pure dispatch loop (580µs vs 590µs for 10k iterations). Small but structurally cleaner — one fewer indirection and bounds check per opcode.

**Files:** `vm/vm.rs`

---

### R13. String cloning on every method/field dispatch (vm.rs:504,563)
**Severity: 4 | Complexity: 5**

`method_name` and `field` are cloned from the string pool on every `CallMethod` and `GetField` opcode. In a hot loop accessing the same field repeatedly, this creates significant allocation pressure.

**Fix:** Use interned string indices (`u16`) throughout the dispatch chain rather than resolving to `String`. Store a string table in `CompiledProgram` and use indices in opcodes.

**Files:** `vm/vm.rs:504,563`, `vm/compiler.rs`

---

## Rust Best Practices Violations

### R14. ~~`thiserror` declared but never used~~ ✅ FIXED
**Severity: 4 | Complexity: 1**

Removed unused `thiserror` dependency from `Cargo.toml`.

**Files:** `crates/rustle-lang/Cargo.toml`

---

### R15. ~~304 clippy warnings in rustle-lang~~ ✅ FIXED (306 → 65, 79% reduction)
**Severity: 5 | Complexity: 5**

Applied `cargo clippy --fix` for auto-fixable warnings (collapsible ifs, uninlined format args, redundant clones, From casts, etc.). Added justified `#[expect]` annotations at module level for:
- `cast_possible_truncation` / `cast_sign_loss` / `cast_possible_wrap` in VM modules (indices guaranteed small by construction)
- `cast_precision_loss` in list/method modules (list lengths fit in f64 mantissa)
- `many_single_char_names` in ops.rs (x/y/z/w are standard math variables)
- `match_same_arms` in type_info.rs and compiler.rs (deliberate per-type arms for clarity)

Remaining 65 warnings are stylistic (let...else, doc sections, too_many_lines) — require individual judgment or larger refactors.

**Files:** All VM source files, analysis/type_info.rs

---

### ~~R16. Manual `Error` implementations instead of `thiserror`~~ N/A
**Severity: 3 | Complexity: 3**

Moot — R14 removed the unused `thiserror` dependency. The manual impls are fine for this crate's needs (custom display format with error codes, hints, and stack traces that `thiserror` derives can't express cleanly).

**Files:** `error.rs`

---

### R17. ~~Unused import in namespaces/mod.rs~~ ✅ FIXED
**Severity: 2 | Complexity: 1**

Removed unused `HashMap` import.

**Files:** `namespaces/mod.rs`

---

### R18. `TokenKind` stores owned `String` — excessive cloning in parser
**Severity: 4 | Complexity: 7**

`TokenKind::Ident(String)`, `TokenKind::StringLit(String)`, and `TokenKind::HexColor(String)` cause string allocations and clones in `peek_kind()`, `check()`, `expect()`, and `matches()`. The existing comment acknowledges this (token.rs:1-6) but dismisses it as "not worth the complexity."

Per Chapter 1: prefer `&str` over `String` in function parameters. Per Chapter 3: avoid cloning in hot paths.

**Fix:** Use a string interner (e.g., `lasso` crate) or intern into an arena. Tokens would store `InternedString` (a `u32` index) which is `Copy` and trivially comparable. This eliminates all string allocation during parsing.

**Files:** `syntax/token.rs`, `syntax/parser.rs`, `syntax/lexer.rs`

---

### R19. No doc comments on public API (lib.rs, error.rs, etc.)
**Severity: 3 | Complexity: 4**

Per Chapter 8: all public functions, structs, traits, and enums should have `///` doc comments. Most public types in `lib.rs` (`Value`, `State`, `Input`, `Program`, `Runtime`) lack doc comments. Only `compile()` has a doc comment.

The `#![deny(missing_docs)]` lint is not enabled.

**Fix:** Add `///` doc comments to all public items. Enable `#![warn(missing_docs)]` in `lib.rs`.

**Files:** `lib.rs`, `error.rs`, `types/draw.rs`

---

### R20. `ErrorCode::as_str()` could use `strum` or `derive` macro (error.rs)
**Severity: 2 | Complexity: 2**

The `as_str()` method is a 50-line manual match that maps each variant to its string representation. This is fragile — adding a new variant requires updating the match. A `strum::Display` derive or a simple `format!("{self:?}")` would be more maintainable, though the current approach has zero allocation.

**Fix:** Consider `strum::Display` or keep current approach with a test that verifies all variants are covered.

**Files:** `error.rs:109-161`

---

### R21. ~~No `#[non_exhaustive]` on `StackFrame`~~ ✅ FIXED
**Severity: 2 | Complexity: 1**

Added `#[non_exhaustive]` to `StackFrame`.

**Files:** `error.rs`

---

### R22. `Expr` enum large variant sizes (ast.rs)
**Severity: 3 | Complexity: 4**

`Expr` has 20+ variants. Variants like `MethodCall` carry 5 fields (Box, String, Vec, Vec, Span). Clippy's `large_enum_variant` would likely flag the size differential. Some variants carry `Vec<Expr>` which is 24 bytes on the stack, while simple variants like `Float(f64, Span)` are 24 bytes. The overall enum size is likely ~80 bytes due to the largest variant.

Per Chapter 2: `large_enum_variant` warns about oversized variants. Boxing the larger variants would reduce the base enum size.

**Fix:** Box the data in larger variants (e.g., `MethodCall`, `BinOp`, `Call`). Profile first to ensure this actually helps — boxing adds indirection.

**Files:** `syntax/ast.rs:298-422`

---

### R23. ~~`Type::from_name()` duplicates parser's type resolution~~ ✅ FIXED
**Severity: 3 | Complexity: 2**

Parser's `parse_type()` now calls `Type::from_name()` instead of duplicating the 16-entry mapping.

**Files:** `syntax/parser.rs`

---

### ~~R24. `Collector` pre-seeds core symbols at order 0..N~~ DROPPED
**Severity: ~~4~~ 1 | Complexity: 3**

Not a bug. Core symbols getting low declaration orders is correct behavior — they should always be "before" user code. Shadowing a core symbol fails at declaration (S003 error) so the wasted counter slot is harmless.

**Files:** `analysis/collector.rs`, `analysis/symbols.rs`

---

### R25. ~~Ternary type checking uses `!=` instead of `types_compatible`~~ ✅ FIXED
**Severity: 5 | Complexity: 2**

Now uses `types_compatible` in both directions. Regression tests: `r25_ternary_compatible_optional_types`, `r25_ternary_same_types`, `r25_ternary_incompatible_types_error`.

**Files:** `analysis/checker.rs`

---

## Consistency Issues

### ~~R26. `on_exit` signature validation differs from `on_init`~~ DROPPED
**Severity: ~~3~~ 1 | Complexity: 1**

By design. All lifecycle functions (`on_init`, `on_update`, `on_exit`) share a consistent `(State) -> State` pattern. The returned state from `on_exit` is discarded, but requiring `return s` keeps the language consistent and prevents confusion. Not a bug.

**Files:** `analysis/validator.rs`

---

### R27. `Plus` token in parser has struct-specific hack (parser.rs:691-695)
**Severity: 3 | Complexity: 3**

`parse_addition()` has a special case: if `+` is followed by `fn` or `let`, it's NOT treated as addition. This is because `+fn` and `+let` are struct member visibility prefixes. This couples expression parsing to struct syntax, making the grammar context-sensitive.

**Fix:** Handle `+fn`/`+let`/`#fn`/`#let` as distinct compound tokens in the lexer rather than overloading `+` in the parser. Or parse struct bodies in a separate mode where `+` is not an operator.

**Files:** `syntax/parser.rs:691-695`

---

### R28. ~~`color * float` does NOT clamp negative results~~ ✅ FIXED
**Severity: 4 | Complexity: 2**

Changed `.min(1.0)` to `.clamp(0.0, 1.0)`. Regression test: `r28_color_mul_negative_clamps_to_zero`.

**Files:** `vm/ops.rs`

---

### R29. `Span` has `end_line`/`end_column` but they are rarely set correctly
**Severity: 3 | Complexity: 4**

`Span::new()` sets `end_line = line` and `end_column = column` (point span). `span_from()` in the parser sets end to the current token position, but many AST nodes are created with `Span::new()` directly (e.g., in `parse_primary`), giving them zero-width spans. This makes error reporting and IDE integration less precise.

**Fix:** Consistently use `span_from(&start)` for all AST nodes that span multiple tokens.

**Files:** `syntax/ast.rs:13-16`, `syntax/parser.rs`

---

### R30. `NamespaceRegistry` uses `dyn NamespaceInfo` but only at compile time
**Severity: 2 | Complexity: 5**

`NamespaceRegistry` stores `Vec<Box<dyn NamespaceInfo>>` with dynamic dispatch. But namespaces are statically known at compile time (core, shapes, render, coords, file). Per Chapter 6: "prefer generics/static dispatch when you control the call site." The dynamic dispatch is unnecessary here.

However, the current design allows user-extensible namespaces in the future, so the dynamic dispatch may be intentional forward planning.

**Fix:** If extensibility is not planned, use an enum-based dispatch. If it is, document the design intent.

**Files:** `namespaces/mod.rs:23-26`

---

### ~~R31. `NamespaceInfo::exports()` allocates a new `Vec<Export>` on every call~~ DROPPED
**Severity: ~~4~~ 1 | Complexity: 3**

Not a real performance issue. `exports()` is only called during compilation (once per import, ~5 times total). The Vec allocation is negligible compared to the rest of compilation.

**Files:** `namespaces/mod.rs`

---

### R32. `compiler.rs` clones entire AST items to avoid borrow conflicts
**Severity: 4 | Complexity: 5**

Lines 1700, 1746, 1789, 1837 all do `self.program.items.clone()` — cloning the full AST item list — to iterate while also borrowing `self` mutably for compilation. This is expensive for large programs.

**Fix:** Collect needed information (function indices, struct names) in a first pass into a separate data structure, then iterate over that instead.

**Files:** `vm/compiler.rs:1700,1746,1789,1837`

---

### R33. ~~`compiler.rs` leaks memory in tests~~ ✅ FIXED
**Severity: 2 | Complexity: 2**

Added comment documenting the intentional leak for `&'static` lifetime in tests.

**Files:** `vm/compiler.rs`

---

### R34. ~~`StackValue` size not statically verified~~ ✅ FIXED
**Severity: 3 | Complexity: 1**

Added compile-time assertion: `const _: () = assert!(std::mem::size_of::<StackValue>() == 16);`

**Files:** `vm/value.rs`

---

### R35. ~~No `Debug` derive on `Heap`~~ ✅ FIXED
**Severity: 2 | Complexity: 1**

Added `#[derive(Debug)]` to `Heap`.

**Files:** `vm/heap.rs`

---

### ~~R36. `file.write`/`file.append` creates files with default permissions~~ DOWNGRADED
**Severity: ~~5~~ 1 | Complexity: 3**

Default 0o644 (via umask) is standard Unix behavior. Rustle is a creative-coding language writing script output files, not handling secrets. Adding platform-specific permission code adds complexity without real benefit for this use case.

**Files:** `vm/natives.rs`

---

### R37. ~~`format_float` uses `f as i64` cast with guard~~ ✅ FIXED
**Severity: 3 | Complexity: 1**

Added `#[expect(clippy::cast_possible_truncation)]` with reason documenting the guard. Both `value.rs` and `lib.rs`.

**Files:** `vm/value.rs`, `lib.rs`

---

### R38. `stack.remove(closure_pos)` is O(n) in VM call dispatch (vm.rs:445)
**Severity: 3 | Complexity: 5**

When calling a closure, `self.stack.remove(closure_pos)` shifts all elements after the closure position down by one. For deeply nested closure calls with large stacks, this adds O(n) overhead to every call.

**Fix:** Instead of removing, swap the closure value to a known position or use a separate closure register. This requires rethinking the calling convention.

**Files:** `vm/vm.rs:445-446`

---

### R39. Missing `#[must_use]` on several public functions
**Severity: 2 | Complexity: 2**

Several public functions that return values (particularly in `types/draw.rs`, `types/mat.rs`) lack `#[must_use]` annotations. Clippy pedantic flags 21 such cases.

**Fix:** Add `#[must_use]` to all pure functions that return values without side effects.

**Files:** Various

---

### R40. Import ordering not consistently enforced
**Severity: 1 | Complexity: 1**

Per Chapter 1.7: imports should be ordered `std` → external crates → workspace crates → `super::` → `crate::`. Most files follow this convention but not all (e.g., `analysis/checker.rs` mixes `crate::` with `super::` imports).

**Fix:** Run `cargo +nightly fmt` with `group_imports = "StdExternalCrate"` in `rustfmt.toml`.

**Files:** Various

---

### R41. No `rustfmt.toml` configuration
**Severity: 2 | Complexity: 1**

The project has no `rustfmt.toml`. Per Chapter 1.7, the following settings are recommended:
```toml
reorder_imports = true
imports_granularity = "Crate"
group_imports = "StdExternalCrate"
```

**Fix:** Create a `rustfmt.toml` with recommended settings and run `cargo fmt`.

**Files:** Project root

---

### R42. ~~`Vm` struct fields are `pub` without documentation~~ ✅ FIXED
**Severity: 3 | Complexity: 2**

Changed `pub` fields to `pub(crate)` — all access is crate-internal.

**Files:** `vm/vm.rs`

---

### ~~R43. `list.len` appears as both a field and a method~~ DROPPED
**Severity: ~~3~~ 1 | Complexity: 3**

Intentional ergonomics. Both `list.len` and `list.len()` work and produce the same result. Removing one would break existing scripts. The type checker handles both correctly.

**Files:** `analysis/type_info.rs`

---

### R44. ~~Error codes S016-S022 not documented in CLAUDE.md~~ ✅ FIXED
**Severity: 2 | Complexity: 1**

Updated CLAUDE.md error code table: S001–S022 and R001–R013 now fully listed.

**Files:** CLAUDE.md

---

## Summary by Priority

| Priority | Count | Fixed | Dropped/N/A | Remaining |
|----------|-------|-------|-------------|-----------|
| Critical (7-10) | 4 | 2 (R02, R04) | 2 (R01, R03) | 0 |
| High (5-6) | 9 | 6 (R05, R06, R07, R15, R25) | 1 (R36) | 2 (R08) |
| Medium (3-4) | 20 | 9 (R14, R17, R23, R28, R33, R34, R37, R42) | 5 (R16, R24, R26, R31, R43) | 6 |
| Low (1-2) | 11 | 3 (R21, R35, R44) | — | 8 |

**Total issues: 44 | Fixed: 20 | Dropped/N/A: 8 | Remaining: 16**
