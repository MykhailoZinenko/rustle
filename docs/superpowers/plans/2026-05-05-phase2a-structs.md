# Phase 2a: Structs — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add user-defined struct types with fields, public/private methods, and deep field access to the Rustle language.

**Architecture:** Introduce a `RustleObject` trait and `Value::Object(Rc<RefCell<dyn RustleObject>>)` variant. `StructInstance` implements this trait. Struct definitions live in `Program.structs`. The compiler pipeline (lexer → parser → collector → checker → validator → interpreter) each gain struct awareness.

**Tech Stack:** Rust, rustle-lang crate. No new dependencies.

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/rustle-lang/src/syntax/token.rs` | Modify | Add `Struct` keyword token |
| `crates/rustle-lang/src/syntax/lexer.rs` | Modify | Map `"struct"` → `TokenKind::Struct` |
| `crates/rustle-lang/src/syntax/ast.rs` | Modify | Add `StructDef`, `StructField`, `StructMethod`, `Visibility` AST nodes; add `Item::Struct`; add `Expr::StructConstruction` |
| `crates/rustle-lang/src/syntax/parser.rs` | Modify | Parse `struct` declarations and `Name { field: val }` construction expressions |
| `crates/rustle-lang/src/runtime/object.rs` | Create | `RustleObject` trait + `StructInstance` implementation |
| `crates/rustle-lang/src/runtime/value.rs` | Modify | Add `Value::Object` variant; update `is_truthy`, `Clone` |
| `crates/rustle-lang/src/runtime/mod.rs` | Modify | Add `pub mod object;` |
| `crates/rustle-lang/src/error.rs` | Modify | Add error codes S016–S021, R013 |
| `crates/rustle-lang/src/analysis/symbols.rs` | Modify | Add `SymbolKind::Struct`, `SymbolKind::StructMethod` |
| `crates/rustle-lang/src/analysis/collector.rs` | Modify | Collect struct declarations into symbol table |
| `crates/rustle-lang/src/analysis/lookup.rs` | Modify | Resolve struct fields and methods in `LookupContext` |
| `crates/rustle-lang/src/analysis/checker.rs` | Modify | Type-check struct construction, field access, method calls, `this` binding |
| `crates/rustle-lang/src/analysis/validator.rs` | Modify | Validate duplicate fields/methods, private access |
| `crates/rustle-lang/src/runtime/interpreter.rs` | Modify | Evaluate struct construction, field get/set, method dispatch with `this` |
| `crates/rustle-lang/src/lib.rs` | Modify | Store structs in `Program`; re-export `object` module |
| `crates/rustle-lang/src/namespaces/mod.rs` | Modify | Add `value_type_name` handling for `Value::Object` |
| `crates/rustle-lang/tests/resolver.rs` | Modify | Add struct resolver tests |
| `crates/rustle-lang/tests/runtime.rs` | Modify | Add struct runtime tests |

---

### Task 1: Add `struct` keyword and error codes

**Files:**
- Modify: `crates/rustle-lang/src/syntax/token.rs:8-42`
- Modify: `crates/rustle-lang/src/syntax/lexer.rs:118` (keyword_or_ident call)
- Modify: `crates/rustle-lang/src/error.rs:0-90`

- [ ] **Step 1: Add `Struct` token variant**

In `crates/rustle-lang/src/syntax/token.rs`, add to the Keywords section (after `None`):

```rust
Struct,
```

In the `keyword_or_ident` function, add before the `_` catch-all:

```rust
"struct"    => TokenKind::Struct,
```

- [ ] **Step 2: Add error codes S016–S021 and R013**

In `crates/rustle-lang/src/error.rs`, add after `S015`:

```rust
/// Private method accessed from outside struct.
S016,
/// Missing required field in struct construction.
S017,
/// Unknown field in struct construction.
S018,
/// Duplicate field in struct definition.
S019,
/// Duplicate method in struct definition.
S020,
/// `this` used outside struct method.
S021,
```

Add after `R012`:

```rust
/// Field not found on struct (runtime safety net).
R013,
```

Update `ErrorCode::as_str()` to include:

```rust
Self::S016 => "S016",
Self::S017 => "S017",
Self::S018 => "S018",
Self::S019 => "S019",
Self::S020 => "S020",
Self::S021 => "S021",
Self::R013 => "R013",
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p rustle-lang`
Expected: compiles with zero errors (warnings ok)

- [ ] **Step 4: Commit**

```bash
git add crates/rustle-lang/src/syntax/token.rs crates/rustle-lang/src/error.rs
git commit -m "feat(structs): add struct keyword token and error codes S016-S021, R013"
```

---

### Task 2: Add AST nodes for struct definitions and construction

**Files:**
- Modify: `crates/rustle-lang/src/syntax/ast.rs:26-62,232-357`

- [ ] **Step 1: Add struct AST nodes**

In `crates/rustle-lang/src/syntax/ast.rs`, add after the `StateField` struct (around line 54):

```rust
// ─── Structs ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub ty: Option<Type>,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StructMethod {
    pub visibility: Visibility,
    pub def: FnDef,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<StructField>,
    pub methods: Vec<StructMethod>,
    pub span: Span,
}
```

- [ ] **Step 2: Add `Item::Struct` variant**

In the `Item` enum, add:

```rust
pub enum Item {
    FnDef(FnDef),
    Stmt(Stmt),
    Struct(StructDef),
}
```

- [ ] **Step 3: Add `Expr::StructConstruction` variant**

In the `Expr` enum, add after `Lambda`:

```rust
/// `Point { x: 5.0, y: 10.0 }`
StructConstruction {
    name: String,
    fields: Vec<(String, Expr)>,
    span: Span,
},
```

Update the `Expr::span()` method to handle it:

```rust
| Expr::StructConstruction { span, .. } => span,
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p rustle-lang`
Expected: compiles (may have warnings about unused variants — ok)

- [ ] **Step 5: Commit**

```bash
git add crates/rustle-lang/src/syntax/ast.rs
git commit -m "feat(structs): add StructDef, StructField, StructMethod, Visibility AST nodes"
```

---

### Task 3: Parse struct declarations

**Files:**
- Modify: `crates/rustle-lang/src/syntax/parser.rs:1,24-49`
- Test: `crates/rustle-lang/tests/resolver.rs`

- [ ] **Step 1: Write failing test — struct parses without error**

In `crates/rustle-lang/tests/resolver.rs`, add:

```rust
#[test]
fn struct_basic_declaration() {
    let src = r#"
        struct Point {
            let x: float = 0.0
            let y: float
        }
    "#;
    assert!(compile(src).is_ok());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rustle-lang --test resolver struct_basic_declaration -- --nocapture`
Expected: FAIL — parser doesn't recognize `struct` keyword yet

- [ ] **Step 3: Add struct parsing to parser**

In `crates/rustle-lang/src/syntax/parser.rs`, update the import line to include the new AST types:

```rust
use crate::syntax::ast::{..., StructDef, StructField, StructMethod, Visibility};
```

In the `parse()` method's match block, add a case for `TokenKind::Struct` before the `TokenKind::Eof` arm:

```rust
TokenKind::Struct => match self.parse_struct_def() {
    Ok(s) => items.push(Item::Struct(s)),
    Err(e) => { errors.push(e); self.recover(); }
},
```

Add the parsing methods:

```rust
fn parse_struct_def(&mut self) -> Result<StructDef, Error> {
    let start = self.span();
    self.expect(TokenKind::Struct)?;
    let name = self.expect_ident("struct name")?;
    self.expect(TokenKind::LBrace)?;

    let mut fields = Vec::new();
    let mut methods = Vec::new();

    while !self.check(TokenKind::RBrace) && !self.is_at_end() {
        if self.check(TokenKind::Let) {
            fields.push(self.parse_struct_field()?);
        } else if self.check(TokenKind::Plus) || self.check(TokenKind::Hash) {
            methods.push(self.parse_struct_method()?);
        } else {
            return Err(Error::new(
                ErrorCode::P001,
                self.peek().line,
                self.peek().column,
                format!("expected field (`let`) or method (`+fn`/`#fn`) in struct, found `{}`", self.peek_kind()),
            ));
        }
    }
    self.expect(TokenKind::RBrace)?;
    let span = self.span_from(&start);
    Ok(StructDef { name, fields, methods, span })
}

fn parse_struct_field(&mut self) -> Result<StructField, Error> {
    let start = self.span();
    self.expect(TokenKind::Let)?;
    let name = self.expect_ident("field name")?;

    let ty = if self.matches(TokenKind::Colon) {
        Some(self.parse_type()?)
    } else {
        None
    };

    let default = if self.matches(TokenKind::Eq) {
        Some(self.parse_expr()?)
    } else {
        None
    };

    // Must have at least a type or a default
    if ty.is_none() && default.is_none() {
        return Err(Error::new(
            ErrorCode::P007,
            start.line, start.column,
            format!("field '{name}' needs a type annotation or default value"),
        ));
    }

    let span = self.span_from(&start);
    Ok(StructField { name, ty, default, span })
}

fn parse_struct_method(&mut self) -> Result<StructMethod, Error> {
    let start = self.span();
    let visibility = if self.matches(TokenKind::Plus) {
        Visibility::Public
    } else if self.matches(TokenKind::Hash) {
        Visibility::Private
    } else {
        return Err(Error::new(
            ErrorCode::P001,
            self.peek().line, self.peek().column,
            "expected '+' (public) or '#' (private) before 'fn'",
        ));
    };
    // Now expect `fn` and parse the function definition
    let def = self.parse_fn_def()?;
    let span = self.span_from(&start);
    Ok(StructMethod { visibility, def, span })
}
```

Note: `parse_fn_def()` should already exist (used by `parse_fn_item`). If it doesn't exist as a standalone method, extract the body of `parse_fn_item` that creates a `FnDef` into a `parse_fn_def()` method and call it from both places.

Also need to handle `TokenKind::Hash` in the lexer — currently `#` is only used for hex colors. The `#` token is already lexed: `b'#'` tries hex color first. Inside struct bodies the parser sees `Hash` as a token. But actually `#` is NOT a separate token — it's part of `HexColor`. We need to add it.

In `crates/rustle-lang/src/syntax/token.rs`, add to Punctuation:

```rust
Hash,       // #
```

In `crates/rustle-lang/src/syntax/lexer.rs`, update the `b'#'` arm:

```rust
b'#' => {
    if self.is_hex_sequence() { TokenKind::HexColor(self.read_hex_color()) }
    else { TokenKind::Hash }
}
```

This replaces the error case — `#` not followed by hex digits now produces a `Hash` token instead of an error.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rustle-lang --test resolver struct_basic_declaration -- --nocapture`
Expected: PASS

- [ ] **Step 5: Write test — struct with methods**

```rust
#[test]
fn struct_with_methods() {
    let src = r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0

            +fn magnitude() -> float {
                return sqrt(this.x * this.x + this.y * this.y)
            }

            #fn internal() -> float {
                return this.x + this.y
            }
        }
    "#;
    assert!(compile(src).is_ok());
}
```

- [ ] **Step 6: Run test**

Run: `cargo test -p rustle-lang --test resolver struct_with_methods -- --nocapture`
Expected: PASS (or may need checker work — if fail, this test will be revisited in Task 6)

- [ ] **Step 7: Commit**

```bash
git add crates/rustle-lang/src/syntax/parser.rs crates/rustle-lang/src/syntax/token.rs crates/rustle-lang/src/syntax/lexer.rs crates/rustle-lang/tests/resolver.rs
git commit -m "feat(structs): parse struct declarations with fields and +fn/#fn methods"
```

---

### Task 4: Parse struct construction expressions

**Files:**
- Modify: `crates/rustle-lang/src/syntax/parser.rs`
- Test: `crates/rustle-lang/tests/resolver.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn struct_construction() {
    let src = r#"
        struct Point {
            let x: float = 0.0
            let y: float
        }
        let p: Point = Point { x: 5.0, y: 10.0 }
    "#;
    assert!(compile(src).is_ok());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rustle-lang --test resolver struct_construction -- --nocapture`
Expected: FAIL — parser doesn't recognize `Name { ... }` as a construction expression

- [ ] **Step 3: Implement construction parsing**

The tricky part: `Point { x: 5.0 }` starts with an `Ident` followed by `{`. Currently the parser sees `Ident("Point")` and returns it as a bare identifier. We need to check for `{` after an uppercase-starting ident in expression position.

In the parser's expression parsing, when we encounter an `Ident` in `parse_primary()` or equivalent, add a lookahead: if the ident starts with uppercase and the next token is `{`, parse as struct construction.

In the parser, find where `Ident` is handled in expression parsing (likely `parse_primary` or `parse_atom`). Add after the identifier is recognized but before returning:

```rust
// In parse_primary / parse_atom, when we see Ident:
TokenKind::Ident(name) => {
    // Check if this is struct construction: Name { field: val, ... }
    if name.starts_with(char::is_uppercase) && self.check(TokenKind::LBrace) {
        return self.parse_struct_construction(name, start);
    }
    // ... existing ident/call handling
}
```

Add the construction parser:

```rust
fn parse_struct_construction(&mut self, name: String, start: Span) -> Result<Expr, Error> {
    self.expect(TokenKind::LBrace)?;
    let mut fields = Vec::new();

    while !self.check(TokenKind::RBrace) && !self.is_at_end() {
        let field_name = self.expect_ident("field name")?;
        self.expect(TokenKind::Colon)?;
        let value = self.parse_expr()?;
        fields.push((field_name, value));
        if !self.check(TokenKind::RBrace) {
            self.expect(TokenKind::Comma)?;
        }
    }
    self.expect(TokenKind::RBrace)?;
    let span = self.span_from(&start);
    Ok(Expr::StructConstruction { name, fields, span })
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p rustle-lang --test resolver struct_construction -- --nocapture`
Expected: PASS (or may need collector/checker — revisited in Task 5/6)

- [ ] **Step 5: Commit**

```bash
git add crates/rustle-lang/src/syntax/parser.rs crates/rustle-lang/tests/resolver.rs
git commit -m "feat(structs): parse struct construction expressions (Name { field: val })"
```

---

### Task 5: RustleObject trait and StructInstance

**Files:**
- Create: `crates/rustle-lang/src/runtime/object.rs`
- Modify: `crates/rustle-lang/src/runtime/mod.rs`
- Modify: `crates/rustle-lang/src/runtime/value.rs`
- Modify: `crates/rustle-lang/src/namespaces/mod.rs` (value_type_name)

- [ ] **Step 1: Create `object.rs` with the RustleObject trait**

Create `crates/rustle-lang/src/runtime/object.rs`:

```rust
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use crate::error::RuntimeError;
use super::value::Value;

pub trait RustleObject: fmt::Debug {
    fn type_name(&self) -> &str;
    fn get_field(&self, name: &str) -> Option<Value>;
    fn set_field(&mut self, name: &str, val: Value) -> bool;
    fn call_method(
        &mut self,
        name: &str,
        args: &[Value],
        line: usize,
    ) -> Option<Result<Value, RuntimeError>>;
    fn clone_deep(&self) -> Box<dyn RustleObject>;
    fn field_names(&self) -> Vec<&str>;
    fn display(&self) -> String;
}

pub type StructMethods = HashMap<String, StructMethodDef>;

#[derive(Debug, Clone)]
pub struct StructMethodDef {
    pub visibility: Visibility,
    pub params: Arc<[crate::syntax::ast::Param]>,
    pub body: Arc<[crate::syntax::ast::Stmt]>,
    pub return_ty: Option<crate::syntax::ast::Type>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Visibility {
    Public,
    Private,
}

#[derive(Debug)]
pub struct StructInstance {
    pub type_name: String,
    pub fields: HashMap<String, Value>,
    pub methods: Rc<StructMethods>,
}

impl RustleObject for StructInstance {
    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn get_field(&self, name: &str) -> Option<Value> {
        self.fields.get(name).cloned()
    }

    fn set_field(&mut self, name: &str, val: Value) -> bool {
        if self.fields.contains_key(name) {
            self.fields.insert(name.to_string(), val);
            true
        } else {
            false
        }
    }

    fn call_method(
        &mut self,
        _name: &str,
        _args: &[Value],
        _line: usize,
    ) -> Option<Result<Value, RuntimeError>> {
        // Method dispatch is handled by the interpreter, not here.
        // The interpreter looks up the method in self.methods and executes
        // the body with `this` bound to the instance's Rc.
        // This method exists for future built-in methods (like .clone()).
        None
    }

    fn clone_deep(&self) -> Box<dyn RustleObject> {
        let cloned_fields = self.fields.iter().map(|(k, v)| {
            (k.clone(), deep_clone_value(v))
        }).collect();
        Box::new(StructInstance {
            type_name: self.type_name.clone(),
            fields: cloned_fields,
            methods: self.methods.clone(),
        })
    }

    fn field_names(&self) -> Vec<&str> {
        self.fields.keys().map(String::as_str).collect()
    }

    fn display(&self) -> String {
        let fields: Vec<String> = self.fields.iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        format!("{} {{ {} }}", self.type_name, fields.join(", "))
    }
}

fn deep_clone_value(v: &Value) -> Value {
    match v {
        Value::Object(rc) => {
            let cloned = rc.borrow().clone_deep();
            Value::Object(Rc::new(std::cell::RefCell::new(cloned)))
        }
        Value::List(rc) => {
            let cloned_items: Vec<Value> = rc.borrow().iter().map(deep_clone_value).collect();
            Value::List(Rc::new(std::cell::RefCell::new(cloned_items)))
        }
        other => other.clone(),
    }
}
```

Note: `Value::Object` doesn't exist yet — we add it next.

- [ ] **Step 2: Add `Value::Object` variant and update `runtime/mod.rs`**

In `crates/rustle-lang/src/runtime/mod.rs`, add:

```rust
pub mod object;
```

In `crates/rustle-lang/src/runtime/value.rs`, add the import:

```rust
use crate::runtime::object::RustleObject;
```

Add the variant to `Value`:

```rust
pub enum Value {
    // ... existing variants ...
    Object(Rc<RefCell<Box<dyn RustleObject>>>),
}
```

Note: `Rc<RefCell<Box<dyn RustleObject>>>` — the `Box` is needed because `dyn RustleObject` is unsized. `RefCell` requires `Sized` for its inner type, so we box the trait object.

Update `is_truthy`:

```rust
Value::Object(_) => true,
```

- [ ] **Step 3: Update `value_type_name` in `namespaces/mod.rs`**

Find the `value_type_name` function and add:

```rust
Value::Object(rc) => rc.borrow().type_name().to_string(),
```

(Or if it returns `&str`, you'll need to return a `String` — check the return type and adjust accordingly.)

- [ ] **Step 4: Fix any remaining compile errors**

The `Display` impl for `Value` (if it exists) and any exhaustive matches on `Value` need the `Object` arm. Search for:

Run: `cargo build -p rustle-lang 2>&1 | head -40`

Fix each exhaustive match to add `Value::Object(rc) => ...` arms.

- [ ] **Step 5: Verify it compiles**

Run: `cargo build -p rustle-lang`
Expected: compiles

- [ ] **Step 6: Commit**

```bash
git add crates/rustle-lang/src/runtime/object.rs crates/rustle-lang/src/runtime/mod.rs crates/rustle-lang/src/runtime/value.rs crates/rustle-lang/src/namespaces/
git commit -m "feat(structs): add RustleObject trait, StructInstance, Value::Object variant"
```

---

### Task 6: Collector — register struct declarations

**Files:**
- Modify: `crates/rustle-lang/src/analysis/symbols.rs`
- Modify: `crates/rustle-lang/src/analysis/collector.rs`
- Test: `crates/rustle-lang/tests/resolver.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn struct_type_in_variable() {
    let src = r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        let p: Point = Point { x: 1.0, y: 2.0 }
    "#;
    assert!(compile(src).is_ok());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rustle-lang --test resolver struct_type_in_variable -- --nocapture`
Expected: FAIL — collector doesn't know about struct types

- [ ] **Step 3: Add `SymbolKind::Struct` and `SymbolKind::StructMethod`**

In `crates/rustle-lang/src/analysis/symbols.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SymbolKind {
    Variable,
    Const,
    Function,
    Param,
    StateField,
    Struct,
    StructMethod,
}
```

- [ ] **Step 4: Add struct collection to collector**

In `crates/rustle-lang/src/analysis/collector.rs`, update the import:

```rust
use crate::syntax::ast::{..., StructDef, Type};
```

In the `collect` method, add struct handling in the `for item in &program.items` loop:

```rust
Item::Struct(s) => self.collect_struct(s),
```

Add the struct collection method:

```rust
fn collect_struct(&mut self, def: &StructDef) {
    // Register the struct name as a type
    let sym = Symbol::new(
        def.name.clone(),
        Some(Type::Named(def.name.clone())),
        SymbolKind::Struct,
        def.span,
    );
    if !self.table.declare_top_level(sym) {
        self.errors.push(Error::new(
            ErrorCode::S003,
            def.span.line, def.span.column,
            format!("'{}' is already defined", def.name),
        ));
    }

    // Register method signatures as struct-scoped symbols
    for method in &def.methods {
        let param_types: Vec<Type> = method.def.params.iter().map(|p| p.ty.clone()).collect();
        let ret_ty = method.def.return_ty.clone();
        let fn_ty = Type::Fn(param_types, ret_ty.map(Box::new));
        let sym = Symbol::new(
            format!("{}::{}", def.name, method.def.name),
            Some(fn_ty),
            SymbolKind::StructMethod,
            method.span,
        );
        self.table.declare_top_level(sym);
    }
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p rustle-lang --test resolver struct_type_in_variable -- --nocapture`
Expected: may still fail (needs checker) — if so, continue to Task 7

- [ ] **Step 6: Run all existing tests to check for regressions**

Run: `cargo test -p rustle-lang`
Expected: all existing tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/rustle-lang/src/analysis/symbols.rs crates/rustle-lang/src/analysis/collector.rs crates/rustle-lang/tests/resolver.rs
git commit -m "feat(structs): collect struct declarations in symbol table"
```

---

### Task 7: LookupContext — resolve struct fields and methods

**Files:**
- Modify: `crates/rustle-lang/src/analysis/lookup.rs`

- [ ] **Step 1: Add struct field resolution to `resolve_field`**

In `crates/rustle-lang/src/analysis/lookup.rs`, add a new tier between State and TypeRegistry in `resolve_field`:

```rust
pub fn resolve_field(&self, obj_ty: &Type, field: &str) -> Option<Type> {
    // 1. Namespace member lookup
    if let Type::Named(n) = obj_ty
        && let Some(ns) = self.registry.get(n)
            && let Some(export) = ns.get_export(field) {
                return Some(export.ty);
            }
    // 2. State fields
    if *obj_ty == Type::State {
        // ... existing state field logic ...
    }
    // 3. Struct fields — look up from program AST
    if let Type::Named(name) = obj_ty {
        if let Some(program) = self.program {
            for item in &program.items {
                if let crate::syntax::ast::Item::Struct(def) = item {
                    if def.name == *name {
                        if let Some(f) = def.fields.iter().find(|f| f.name == field) {
                            return f.ty.clone().or_else(|| {
                                f.default.as_ref().and_then(super::collector::infer_literal_type)
                            });
                        }
                        return None;
                    }
                }
            }
        }
    }
    // 4. TypeRegistry
    self.type_registry.resolve_field_type(obj_ty, field)
}
```

- [ ] **Step 2: Add struct field names to `field_names`**

```rust
pub fn field_names<'b>(&'b self, obj_ty: &Type) -> Vec<&'b str> {
    // ... existing State check ...

    // Struct fields
    if let Type::Named(name) = obj_ty {
        if let Some(program) = self.program {
            for item in &program.items {
                if let crate::syntax::ast::Item::Struct(def) = item {
                    if def.name == *name {
                        return def.fields.iter().map(|f| f.name.as_str()).collect();
                    }
                }
            }
        }
    }

    self.type_registry.field_names_for_type(obj_ty)
}
```

- [ ] **Step 3: Add struct method resolution to `get_method_type`**

```rust
pub fn get_method_type(&self, obj_ty: &Type, method: &str) -> Option<Type> {
    // 1. Namespace member lookup
    // ... existing ...

    // 2. Struct methods
    if let Type::Named(name) = obj_ty {
        if let Some(program) = self.program {
            for item in &program.items {
                if let crate::syntax::ast::Item::Struct(def) = item {
                    if def.name == *name {
                        // Built-in clone method
                        if method == "clone" {
                            return Some(Type::Fn(vec![], Some(Box::new(Type::Named(name.clone())))));
                        }
                        if let Some(m) = def.methods.iter().find(|m| m.def.name == method) {
                            let params: Vec<Type> = m.def.params.iter().map(|p| p.ty.clone()).collect();
                            let ret = m.def.return_ty.clone();
                            return Some(Type::Fn(params, ret.map(Box::new)));
                        }
                        return None;
                    }
                }
            }
        }
    }

    // 3. TypeRegistry
    let (params, ret) = self.type_registry.resolve_method_signature(obj_ty, method)?;
    Some(Type::Fn(params, ret.map(Box::new)))
}
```

- [ ] **Step 4: Add struct method names to `method_names`**

```rust
pub fn method_names(&self, obj_ty: &Type) -> Vec<&str> {
    if let Type::Named(name) = obj_ty {
        if let Some(program) = self.program {
            for item in &program.items {
                if let crate::syntax::ast::Item::Struct(def) = item {
                    if def.name == *name {
                        let mut names: Vec<&str> = def.methods.iter()
                            .map(|m| m.def.name.as_str())
                            .collect();
                        names.push("clone");
                        return names;
                    }
                }
            }
        }
    }
    self.type_registry.method_names_for_type(obj_ty)
}
```

- [ ] **Step 5: Verify it compiles**

Run: `cargo build -p rustle-lang`
Expected: compiles

- [ ] **Step 6: Commit**

```bash
git add crates/rustle-lang/src/analysis/lookup.rs
git commit -m "feat(structs): resolve struct fields and methods in LookupContext"
```

---

### Task 8: Checker — type-check struct construction and `this`

**Files:**
- Modify: `crates/rustle-lang/src/analysis/checker.rs`
- Test: `crates/rustle-lang/tests/resolver.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn struct_construction_type_checks() {
    let src = r#"
        struct Point {
            let x: float = 0.0
            let y: float
        }
        let p: Point = Point { x: 5.0, y: 10.0 }
    "#;
    assert!(compile(src).is_ok());
}

#[test]
fn struct_missing_required_field() {
    let errs = compile(r#"
        struct Point {
            let x: float = 0.0
            let y: float
        }
        let p: Point = Point { x: 5.0 }
    "#).unwrap_err();
    assert!(errs.iter().any(|e| e.code == ErrorCode::S017));
}

#[test]
fn struct_unknown_field_in_construction() {
    let errs = compile(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        let p: Point = Point { z: 5.0 }
    "#).unwrap_err();
    assert!(errs.iter().any(|e| e.code == ErrorCode::S018));
}

#[test]
fn struct_field_access_type() {
    let src = r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        let p: Point = Point { x: 1.0, y: 2.0 }
        let v: float = p.x
    "#;
    assert!(compile(src).is_ok());
}

#[test]
fn struct_this_in_method() {
    let src = r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0

            +fn sum() -> float {
                return this.x + this.y
            }
        }
    "#;
    assert!(compile(src).is_ok());
}

#[test]
fn struct_this_outside_method() {
    let errs = compile(r#"
        fn foo() -> float {
            return this.x
        }
    "#).unwrap_err();
    assert!(errs.iter().any(|e| e.code == ErrorCode::S021));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rustle-lang --test resolver struct_construction_type_checks struct_missing_required_field struct_unknown_field struct_field_access_type struct_this_in_method struct_this_outside_method -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Add `Expr::StructConstruction` handling to checker**

In `crates/rustle-lang/src/analysis/checker.rs`, find the `infer_expr` method's match block and add a case for `Expr::StructConstruction`:

```rust
Expr::StructConstruction { name, fields, span } => {
    // Find the struct definition
    let struct_def = self.find_struct_def(name);
    let Some(struct_def) = struct_def else {
        return Err(vec![Error::new(
            ErrorCode::S001, span.line, span.column,
            format!("undefined struct '{name}'"),
        )]);
    };
    let struct_def = struct_def.clone();

    // Check each provided field
    for (field_name, field_expr) in fields {
        let field_def = struct_def.fields.iter().find(|f| f.name == *field_name);
        let Some(field_def) = field_def else {
            let mut err = Error::new(
                ErrorCode::S018, span.line, span.column,
                format!("'{field_name}' is not a field of '{name}'"),
            );
            let candidates: Vec<&str> = struct_def.fields.iter().map(|f| f.name.as_str()).collect();
            if let Some(suggestion) = crate::error::suggest_similar(field_name, &candidates, 2) {
                err = err.with_hint(format!("did you mean '{suggestion}'?"));
            }
            return Err(vec![err]);
        };
        let field_def = field_def.clone();

        let expr_ty = self.infer_expr(field_expr)?;
        if let Some(ref expected_ty) = field_def.ty {
            if !types_compatible(expected_ty, &expr_ty) {
                return Err(vec![Error::new(
                    ErrorCode::S002, span.line, span.column,
                    format!("field '{field_name}' expects `{}`, got `{}`",
                        type_name(expected_ty), type_name(&expr_ty)),
                )]);
            }
        }
    }

    // Check that all required fields (no default) are provided
    let provided: Vec<&str> = fields.iter().map(|(n, _)| n.as_str()).collect();
    for field_def in &struct_def.fields {
        if field_def.default.is_none() && !provided.contains(&field_def.name.as_str()) {
            return Err(vec![Error::new(
                ErrorCode::S017, span.line, span.column,
                format!("missing required field '{}' in '{name}'", field_def.name),
            )]);
        }
    }

    Ok(Type::Named(name.clone()))
}
```

Add a helper method to find struct definitions:

```rust
fn find_struct_def(&self, name: &str) -> Option<&crate::syntax::ast::StructDef> {
    let program = self.program?;
    program.items.iter().find_map(|item| {
        if let Item::Struct(def) = item {
            if def.name == name { return Some(def); }
        }
        None
    })
}
```

Note: `self.program` needs to be stored in the `TypeResolver`. Check if it already has a reference to the program. If not, it's passed to the `run()` method — store it as a field.

- [ ] **Step 4: Handle `this` in the checker**

The checker needs to know when it's inside a struct method. Add a field to `TypeResolver`:

```rust
pub struct TypeResolver<'a> {
    // ... existing fields ...
    current_struct: Option<String>,  // Set when checking struct method bodies
}
```

When checking struct method bodies (in the code that iterates `Item::Struct`), set `self.current_struct = Some(struct_name)` before checking the body and `None` after.

In `Expr::Ident` handling, add before the existing logic:

```rust
Expr::Ident(name, span) => {
    if name == "this" {
        if let Some(ref struct_name) = self.current_struct {
            return Ok(Type::Named(struct_name.clone()));
        }
        return Err(vec![Error::new(
            ErrorCode::S021, span.line, span.column,
            "'this' is only valid inside struct methods",
        )]);
    }
    // ... existing ident logic ...
}
```

Add struct method body checking. In the checker's main traversal (wherever `Item::FnDef` bodies are checked), add handling for `Item::Struct`:

```rust
Item::Struct(def) => {
    for method in &def.methods {
        self.current_struct = Some(def.name.clone());
        self.check_fn_body(&method.def);
        self.current_struct = None;
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p rustle-lang --test resolver struct_ -- --nocapture`
Expected: all struct tests PASS

- [ ] **Step 6: Run full test suite**

Run: `cargo test -p rustle-lang`
Expected: all tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/rustle-lang/src/analysis/checker.rs crates/rustle-lang/tests/resolver.rs
git commit -m "feat(structs): type-check struct construction, field access, this binding"
```

---

### Task 9: Validator — duplicate fields/methods and private access

**Files:**
- Modify: `crates/rustle-lang/src/analysis/validator.rs`
- Test: `crates/rustle-lang/tests/resolver.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn struct_duplicate_field() {
    let errs = compile(r#"
        struct Point {
            let x: float = 0.0
            let x: float = 1.0
        }
    "#).unwrap_err();
    assert!(errs.iter().any(|e| e.code == ErrorCode::S019));
}

#[test]
fn struct_duplicate_method() {
    let errs = compile(r#"
        struct Point {
            let x: float = 0.0
            +fn foo() -> float { return 1.0 }
            +fn foo() -> float { return 2.0 }
        }
    "#).unwrap_err();
    assert!(errs.iter().any(|e| e.code == ErrorCode::S020));
}

#[test]
fn struct_private_method_access() {
    let errs = compile(r#"
        struct Point {
            let x: float = 0.0
            #fn secret() -> float { return this.x }
        }
        let p: Point = Point { x: 1.0 }
        let v: float = p.secret()
    "#).unwrap_err();
    assert!(errs.iter().any(|e| e.code == ErrorCode::S016));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rustle-lang --test resolver struct_duplicate struct_private -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Add struct validation**

In `crates/rustle-lang/src/analysis/validator.rs`, add struct validation to the `validate` method:

```rust
for item in &program.items {
    if let Item::Struct(def) = item {
        self.validate_struct(def);
    }
}
```

Add the validation method:

```rust
fn validate_struct(&mut self, def: &StructDef) {
    // Check duplicate fields
    let mut seen_fields = std::collections::HashSet::new();
    for field in &def.fields {
        if !seen_fields.insert(&field.name) {
            self.errors.push(Error::new(
                ErrorCode::S019,
                field.span.line, field.span.column,
                format!("duplicate field '{}' in '{}'", field.name, def.name),
            ));
        }
    }

    // Check duplicate methods
    let mut seen_methods = std::collections::HashSet::new();
    for method in &def.methods {
        if !seen_methods.insert(&method.def.name) {
            self.errors.push(Error::new(
                ErrorCode::S020,
                method.span.line, method.span.column,
                format!("duplicate method '{}' in '{}'", method.def.name, def.name),
            ));
        }
    }
}
```

- [ ] **Step 4: Add private method access checking**

Private access checking happens in the checker when resolving method calls. In the checker's `Expr::MethodCall` handling, after resolving the method type successfully, check visibility:

```rust
Expr::MethodCall { expr, method, args, named_args: _, span } => {
    let obj_ty = self.infer_expr(expr)?;

    // Check private access on struct methods
    if let Type::Named(ref struct_name) = obj_ty {
        if let Some(struct_def) = self.find_struct_def(struct_name) {
            if let Some(m) = struct_def.methods.iter().find(|m| m.def.name == *method) {
                if m.visibility == crate::syntax::ast::Visibility::Private {
                    let calling_from_same_struct = self.current_struct.as_deref() == Some(struct_name.as_str());
                    if !calling_from_same_struct {
                        return Err(vec![Error::new(
                            ErrorCode::S016, span.line, span.column,
                            format!("'{method}' is private on '{struct_name}'"),
                        )]);
                    }
                }
            }
        }
    }

    // ... existing method call type checking ...
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p rustle-lang --test resolver struct_ -- --nocapture`
Expected: all PASS

- [ ] **Step 6: Commit**

```bash
git add crates/rustle-lang/src/analysis/validator.rs crates/rustle-lang/src/analysis/checker.rs crates/rustle-lang/tests/resolver.rs
git commit -m "feat(structs): validate duplicate fields/methods, enforce private visibility"
```

---

### Task 10: Interpreter — struct construction and field access

**Files:**
- Modify: `crates/rustle-lang/src/runtime/interpreter.rs`
- Modify: `crates/rustle-lang/src/lib.rs`
- Test: `crates/rustle-lang/tests/runtime.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn struct_construction_and_field_read() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Point = Point { x: 5.0, y: 10.0 }
            s.v = p.x + p.y
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 15.0);
}

#[test]
fn struct_field_mutation() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Point = Point { x: 1.0, y: 2.0 }
            p.x = 99.0
            s.v = p.x
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 99.0);
}

#[test]
fn struct_default_fields() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Point = Point { y: 7.0 }
            s.v = p.x + p.y
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 7.0);
}

#[test]
fn struct_reference_semantics() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: Point = Point { x: 1.0, y: 2.0 }
            let b: Point = a
            b.x = 99.0
            s.v = a.x
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 99.0);  // shared reference
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rustle-lang --test runtime struct_ -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Store struct definitions in `Program`**

In `crates/rustle-lang/src/lib.rs`, the `Program` struct needs access to struct defs. The AST `program.items` already contains them, so the interpreter can find them through `self.program.ast.items`. No changes to `Program` struct needed — the interpreter will scan `ast.items` for `Item::Struct` when needed.

However, for efficiency, build a struct lookup table in the interpreter. In the interpreter's `Interpreter` struct, add:

```rust
struct_defs: HashMap<String, &'a StructDef>,
```

Populate it when creating the interpreter (in `new` or equivalent):

```rust
let struct_defs: HashMap<String, &StructDef> = program.ast.items.iter()
    .filter_map(|item| {
        if let Item::Struct(def) = item { Some((def.name.clone(), def)) } else { None }
    })
    .collect();
```

- [ ] **Step 4: Handle `Expr::StructConstruction` in interpreter**

In the interpreter's `eval_expr`, add:

```rust
Expr::StructConstruction { name, fields, span } => {
    let struct_def = self.struct_defs.get(name.as_str())
        .ok_or_else(|| self.err(ErrorCode::R002, span.line, format!("undefined struct: `{name}`")))?;

    // Build field map with defaults
    let mut field_values = HashMap::new();
    for field_def in &struct_def.fields {
        if let Some(ref default_expr) = field_def.default {
            field_values.insert(field_def.name.clone(), self.eval_expr(default_expr)?);
        }
    }
    // Override with provided fields
    for (field_name, field_expr) in fields {
        field_values.insert(field_name.clone(), self.eval_expr(field_expr)?);
    }

    // Build method table (shared Rc)
    let methods = self.get_or_build_methods(name);

    let instance = StructInstance {
        type_name: name.clone(),
        fields: field_values,
        methods,
    };
    Ok(Value::Object(Rc::new(RefCell::new(Box::new(instance)))))
}
```

Add a method cache so all instances share the same method table Rc:

```rust
method_cache: HashMap<String, Rc<StructMethods>>,
```

```rust
fn get_or_build_methods(&mut self, struct_name: &str) -> Rc<StructMethods> {
    if let Some(methods) = self.method_cache.get(struct_name) {
        return methods.clone();
    }
    let mut methods = StructMethods::new();
    if let Some(def) = self.struct_defs.get(struct_name) {
        for m in &def.methods {
            let vis = match m.visibility {
                crate::syntax::ast::Visibility::Public => Visibility::Public,
                crate::syntax::ast::Visibility::Private => Visibility::Private,
            };
            methods.insert(m.def.name.clone(), StructMethodDef {
                visibility: vis,
                params: m.def.params.clone(),
                body: m.def.body.clone(),
                return_ty: m.def.return_ty.clone(),
            });
        }
    }
    let rc = Rc::new(methods);
    self.method_cache.insert(struct_name.to_string(), rc.clone());
    rc
}
```

- [ ] **Step 5: Handle `Value::Object` field access in `eval_field`**

In the `eval_field` function, add before the TypeRegistry fallback:

```rust
if let Value::Object(rc) = obj {
    let guard = rc.borrow();
    return guard.get_field(field).ok_or_else(|| {
        let names = guard.field_names();
        RuntimeError::new(ErrorCode::R013, line, 0,
            format!("'{}' has no field `{field}` (available: {})", guard.type_name(), names.join(", ")))
    });
}
```

- [ ] **Step 6: Handle `Value::Object` field assignment in `assign_state_path`/`set_field_path`**

In the assignment handling code (where `Value::State` paths are handled), add similar handling for `Value::Object`. When the root of an assignment path is a `Value::Object`, follow the Rc chain:

```rust
// When the target variable is a Value::Object
Value::Object(rc) => {
    if path.len() == 1 {
        let mut guard = rc.borrow_mut();
        if !guard.set_field(&path[0], val) {
            return Err(RuntimeError::new(ErrorCode::R013, line, 0,
                format!("cannot set field '{}' on '{}'", path[0], guard.type_name())));
        }
    } else {
        // Deep path: get intermediate, recurse
        let intermediate = {
            let guard = rc.borrow();
            guard.get_field(&path[0]).ok_or_else(|| {
                RuntimeError::new(ErrorCode::R013, line, 0,
                    format!("field '{}' not found on '{}'", path[0], guard.type_name()))
            })?
        };
        // For nested Object, recurse via assign_object_path
        assign_value_path(&intermediate, &path[1..], val, line, types)?;
    }
    Ok(())
}
```

You'll need to add a recursive `assign_value_path` helper that handles both `Value::Object` and `Value::State` at each level of the chain.

- [ ] **Step 7: Run tests**

Run: `cargo test -p rustle-lang --test runtime struct_ -- --nocapture`
Expected: all PASS

- [ ] **Step 8: Commit**

```bash
git add crates/rustle-lang/src/runtime/interpreter.rs crates/rustle-lang/src/lib.rs crates/rustle-lang/tests/runtime.rs
git commit -m "feat(structs): evaluate struct construction, field read/write, reference semantics"
```

---

### Task 11: Interpreter — method dispatch with `this` binding

**Files:**
- Modify: `crates/rustle-lang/src/runtime/interpreter.rs`
- Test: `crates/rustle-lang/tests/runtime.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn struct_method_call() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0

            +fn sum() -> float {
                return this.x + this.y
            }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Point = Point { x: 3.0, y: 4.0 }
            s.v = p.sum()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 7.0);
}

#[test]
fn struct_method_calls_another_method() {
    let rt = run(r#"
        struct Calc {
            let val: float = 0.0

            +fn doubled() -> float {
                return this.helper(2.0)
            }

            #fn helper(factor: float) -> float {
                return this.val * factor
            }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let c: Calc = Calc { val: 5.0 }
            s.v = c.doubled()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 10.0);
}

#[test]
fn struct_method_mutates_this() {
    let rt = run(r#"
        struct Counter {
            let count: float = 0.0

            +fn increment() {
                this.count = this.count + 1.0
            }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let c: Counter = Counter {}
            c.increment()
            c.increment()
            c.increment()
            s.v = c.count
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 3.0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rustle-lang --test runtime struct_method -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Implement method dispatch**

In the interpreter's `eval_method` function (where `MethodCall` is handled), add Object method dispatch:

```rust
// In eval_method or wherever MethodCall expr is evaluated:
if let Value::Object(rc) = &obj {
    let type_name = rc.borrow().type_name().to_string();

    // Handle built-in clone method
    if method == "clone" {
        let cloned = rc.borrow().clone_deep();
        return Ok(Value::Object(Rc::new(RefCell::new(cloned))));
    }

    // Look up the method in the struct's method table
    let method_def = {
        let guard = rc.borrow();
        let instance = guard.downcast_ref::<StructInstance>();
        // Access methods through the StructInstance
        // Since we have Box<dyn RustleObject>, we need to get the methods
        // We stored them in StructMethodDef
        // Alternative: look up from struct_defs
        drop(guard);
        self.struct_defs.get(type_name.as_str())
            .and_then(|def| def.methods.iter().find(|m| m.def.name == method))
            .map(|m| (m.def.params.clone(), m.def.body.clone(), m.def.return_ty.clone()))
    };

    if let Some((params, body, _return_ty)) = method_def {
        // Evaluate arguments
        let mut arg_values = Vec::new();
        for arg_expr in args {
            arg_values.push(self.eval_expr(arg_expr)?);
        }

        // Create new scope for method body
        self.env.push_scope();

        // Bind `this` as a local variable (Rc clone — cheap, not deep)
        self.env.set("this".to_string(), Value::Object(rc.clone()));

        // Bind parameters
        for (param, val) in params.iter().zip(arg_values) {
            self.env.set(param.name.clone(), val);
        }

        // Execute method body
        let result = self.exec_block(&body);
        self.env.pop_scope();

        return match result {
            Ok(()) => Ok(Value::None),
            Err(e) => {
                if let Some(val) = e.return_value { Ok(val) }
                else { Err(e) }
            }
        };
    }
}
```

Note: The exact mechanism for return values depends on how the interpreter handles `return` statements — it may use a special error/control-flow type. Match the existing pattern used by `call_fn`/`call_closure`.

- [ ] **Step 4: Handle `this` in `Expr::Ident`**

`this` will be resolved like any other variable via `self.env.get("this")` — no special handling needed in the interpreter's `Expr::Ident` since we inject it into the environment during method dispatch.

- [ ] **Step 5: Run tests**

Run: `cargo test -p rustle-lang --test runtime struct_method -- --nocapture`
Expected: all PASS

- [ ] **Step 6: Run full test suite**

Run: `cargo test -p rustle-lang`
Expected: all pass

- [ ] **Step 7: Commit**

```bash
git add crates/rustle-lang/src/runtime/interpreter.rs crates/rustle-lang/tests/runtime.rs
git commit -m "feat(structs): method dispatch with this binding, including cross-method calls"
```

---

### Task 12: Deep field access, nested structs, and console output

**Files:**
- Modify: `crates/rustle-lang/src/runtime/interpreter.rs`
- Test: `crates/rustle-lang/tests/runtime.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn struct_nested_field_access() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        struct Bounds {
            let min: Point
            let max: Point
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let b: Bounds = Bounds {
                min: Point { x: 1.0, y: 2.0 },
                max: Point { x: 10.0, y: 20.0 }
            }
            s.v = b.min.x + b.max.y
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 21.0);
}

#[test]
fn struct_nested_field_mutation() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        struct Bounds {
            let min: Point
            let max: Point
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let b: Bounds = Bounds {
                min: Point { x: 1.0, y: 2.0 },
                max: Point { x: 10.0, y: 20.0 }
            }
            b.min.x = 99.0
            s.v = b.min.x
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 99.0);
}

#[test]
fn struct_nested_method_call() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
            +fn sum() -> float { return this.x + this.y }
        }
        struct Bounds {
            let min: Point
            let max: Point
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let b: Bounds = Bounds {
                min: Point { x: 3.0, y: 4.0 },
                max: Point { x: 10.0, y: 20.0 }
            }
            s.v = b.min.sum()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 7.0);
}

#[test]
fn struct_console_output() {
    let mut rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        let p: Point = Point { x: 1.0, y: 2.0 }
        console << p
    "#);
    let cmds = tick(&mut rt);
    match &cmds[0] {
        DrawCommand::Print(msg) => assert!(msg.contains("Point")),
        other => panic!("expected Print, got {other:?}"),
    }
}

#[test]
fn struct_as_state_field() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        state { let p: Point = Point { x: 1.0, y: 2.0 } }
        fn on_init(s: State) -> State {
            s.p.x = 99.0
            return s
        }
    "#);
    // Access nested: state -> p -> x
    match rt.state().0.get("p") {
        Some(Value::Object(rc)) => {
            let val = rc.borrow().get_field("x").unwrap();
            match val {
                Value::Float(x) => assert_eq!(x, 99.0),
                other => panic!("expected Float, got {other:?}"),
            }
        }
        other => panic!("expected Object, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run tests to verify some fail**

Run: `cargo test -p rustle-lang --test runtime struct_nested struct_console struct_as_state -- --nocapture`
Expected: some may pass (field chaining might work), others fail

- [ ] **Step 3: Fix any issues with deep path assignment and nested Object traversal**

The deep field assignment (`b.min.x = 99.0`) requires the assignment path handler to recognize when an intermediate value is `Value::Object` and follow the Rc chain. Ensure the `assign_value_path` helper from Task 10 handles this recursively:

```rust
fn assign_value_path(
    target: &Value,
    path: &[String],
    val: Value,
    line: usize,
    types: &TypeRegistry,
) -> Result<(), RuntimeError> {
    match target {
        Value::Object(rc) => {
            if path.len() == 1 {
                let mut guard = rc.borrow_mut();
                if !guard.set_field(&path[0], val) {
                    return Err(RuntimeError::new(ErrorCode::R013, line, 0,
                        format!("cannot set field '{}'", path[0])));
                }
                Ok(())
            } else {
                let intermediate = {
                    let guard = rc.borrow();
                    guard.get_field(&path[0]).ok_or_else(|| {
                        RuntimeError::new(ErrorCode::R013, line, 0,
                            format!("field '{}' not found", path[0]))
                    })?
                };
                assign_value_path(&intermediate, &path[1..], val, line, types)
            }
        }
        Value::State(rc) => {
            assign_state_path(rc, path, val, line, types)
        }
        _ => {
            Err(RuntimeError::new(ErrorCode::R003, line, 0,
                format!("cannot assign field on `{}`", value_type_name(target))))
        }
    }
}
```

- [ ] **Step 4: Handle console << for Object**

In the interpreter's `Print` handling, ensure `Value::Object` gets formatted via `display()`. Check how print currently formats values — likely a `format_value` helper. Add:

```rust
Value::Object(rc) => rc.borrow().display(),
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p rustle-lang --test runtime struct_ -- --nocapture`
Expected: all PASS

- [ ] **Step 6: Commit**

```bash
git add crates/rustle-lang/src/runtime/interpreter.rs crates/rustle-lang/tests/runtime.rs
git commit -m "feat(structs): deep field access, nested structs, state fields, console output"
```

---

### Task 13: `.clone()` for structs and lists

**Files:**
- Modify: `crates/rustle-lang/src/runtime/interpreter.rs`
- Modify: `crates/rustle-lang/src/types/registry.rs` (list clone method)
- Modify: `crates/rustle-lang/src/analysis/lookup.rs` (list clone type)
- Test: `crates/rustle-lang/tests/runtime.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn struct_clone_independence() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        state { let a_x: float = 0.0; let b_x: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: Point = Point { x: 1.0, y: 2.0 }
            let b: Point = a.clone()
            b.x = 99.0
            s.a_x = a.x
            s.b_x = b.x
            return s
        }
    "#);
    assert_eq!(f(&rt, "a_x"), 1.0);   // a unchanged
    assert_eq!(f(&rt, "b_x"), 99.0);  // b independent
}

#[test]
fn struct_clone_nested() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        struct Bounds {
            let min: Point
            let max: Point
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let b1: Bounds = Bounds {
                min: Point { x: 1.0, y: 2.0 },
                max: Point { x: 10.0, y: 20.0 }
            }
            let b2: Bounds = b1.clone()
            b2.min.x = 99.0
            s.v = b1.min.x
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 1.0);  // b1 unchanged after deep clone
}

#[test]
fn list_clone() {
    let rt = run(r#"
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: list[float] = [1.0, 2.0, 3.0]
            let b: list[float] = a.clone()
            b.push(99.0)
            s.v = a.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 3.0);  // a unchanged
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p rustle-lang --test runtime struct_clone list_clone -- --nocapture`
Expected: FAIL

- [ ] **Step 3: Implement struct .clone() in interpreter**

The `.clone()` call on `Value::Object` was already handled in Task 11's method dispatch using the `clone_deep` trait method. Verify it works. If not, add explicit handling:

In the method dispatch for `Value::Object`:

```rust
if method == "clone" {
    let cloned = rc.borrow().clone_deep();
    return Ok(Value::Object(Rc::new(RefCell::new(cloned))));
}
```

- [ ] **Step 4: Add list .clone() method**

In `crates/rustle-lang/src/types/registry.rs`, add a `clone` method to the `list` type descriptor. Find where list methods are registered (`push`, `pop`, `len`, etc.) and add:

```rust
MethodDesc {
    name: "clone",
    params: vec![],
    ret: None, // Will be same list type — set to None, handle specially
    call: |recv, _args, _line| {
        if let Value::List(rc) = recv {
            let cloned: Vec<Value> = rc.borrow().iter().map(|v| deep_clone_value(v)).collect();
            Ok(Some(Value::List(Rc::new(RefCell::new(cloned)))))
        } else {
            Ok(None)
        }
    },
}
```

You'll need to make `deep_clone_value` from `object.rs` accessible, or duplicate the logic. Consider making it `pub` in `object.rs`.

In `crates/rustle-lang/src/analysis/lookup.rs`, add clone to the list method type resolution:

```rust
// In resolve_method_signature for list types:
if method == "clone" {
    // Return type is same list type
    return Some((vec![], Some(obj_ty.clone())));
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p rustle-lang --test runtime struct_clone list_clone -- --nocapture`
Expected: all PASS

- [ ] **Step 6: Run full test suite**

Run: `cargo test -p rustle-lang`
Expected: all pass

- [ ] **Step 7: Commit**

```bash
git add crates/rustle-lang/src/runtime/ crates/rustle-lang/src/types/registry.rs crates/rustle-lang/src/analysis/lookup.rs crates/rustle-lang/tests/runtime.rs
git commit -m "feat(structs): .clone() deep copy for structs and lists"
```

---

### Task 14: Edge cases and struct-in-list

**Files:**
- Test: `crates/rustle-lang/tests/runtime.rs`
- Test: `crates/rustle-lang/tests/resolver.rs`

- [ ] **Step 1: Write edge case tests**

```rust
// runtime.rs
#[test]
fn struct_no_methods() {
    let rt = run(r#"
        struct Pair {
            let a: float
            let b: float
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let p: Pair = Pair { a: 3.0, b: 4.0 }
            s.v = p.a + p.b
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 7.0);
}

#[test]
fn struct_no_fields() {
    let _rt = run(r#"
        struct Marker {
            +fn tag() -> string { return "marker" }
        }
        let m: Marker = Marker {}
    "#);
}

#[test]
fn struct_in_list() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let points: list[Point] = []
            points.push(Point { x: 1.0, y: 2.0 })
            points.push(Point { x: 3.0, y: 4.0 })
            s.v = points.len()
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 2.0);
}

#[test]
fn struct_optional_field() {
    let rt = run(r#"
        struct Node {
            let value: float
            let label: string? = none
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let n: Node = Node { value: 42.0 }
            s.v = n.value
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 42.0);
}

#[test]
fn struct_method_with_struct_param() {
    let rt = run(r#"
        struct Point {
            let x: float = 0.0
            let y: float = 0.0

            +fn distance_to(o: Point) -> float {
                let dx: float = this.x - o.x
                let dy: float = this.y - o.y
                return sqrt(dx * dx + dy * dy)
            }
        }
        state { let v: float = 0.0 }
        fn on_init(s: State) -> State {
            let a: Point = Point { x: 0.0, y: 0.0 }
            let b: Point = Point { x: 3.0, y: 4.0 }
            s.v = a.distance_to(b)
            return s
        }
    "#);
    assert_eq!(f(&rt, "v"), 5.0);
}

// resolver.rs
#[test]
fn struct_type_infer_from_default() {
    let src = r#"
        struct Config {
            let width = 800.0
            let height = 600.0
            let title = "untitled"
        }
        let c: Config = Config {}
        let w: float = c.width
        let t: string = c.title
    "#;
    assert!(compile(src).is_ok());
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p rustle-lang --test runtime struct_no_methods struct_no_fields struct_in_list struct_optional struct_method_with -- --nocapture`
Run: `cargo test -p rustle-lang --test resolver struct_type_infer -- --nocapture`
Expected: all PASS

- [ ] **Step 3: Fix any failures**

If any test fails, debug and fix. Common issues:
- Empty struct construction `Marker {}` might need parser handling for no fields
- Type inference from defaults needs `infer_literal_type` to work in struct field context
- Optional fields with `none` default need the checker to handle `Type::Optional`

- [ ] **Step 4: Run full test suite**

Run: `cargo test -p rustle-lang`
Expected: all pass (including all pre-existing tests)

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p rustle-lang --all-targets`
Expected: zero warnings (fix any that appear)

- [ ] **Step 6: Commit**

```bash
git add crates/rustle-lang/tests/
git commit -m "test(structs): edge cases — empty structs, lists, optional fields, type inference"
```

---

### Task 15: Final verification and cleanup

**Files:**
- All modified files

- [ ] **Step 1: Run full test suite**

Run: `cargo test -p rustle-lang`
Expected: all tests pass

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -p rustle-lang --all-targets -- -D warnings`
Expected: zero warnings

- [ ] **Step 3: Count total tests**

Run: `cargo test -p rustle-lang 2>&1 | grep "test result"`
Expected: higher count than 526 (pre-existing) — note exact count

- [ ] **Step 4: Commit any cleanup**

If any clippy fixes or cleanup was needed:

```bash
git add -A
git commit -m "chore(structs): clippy fixes and cleanup"
```
