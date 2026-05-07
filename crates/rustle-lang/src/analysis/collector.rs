//! Pass 1 — Symbol Collector
//!
//! Walks top-level items in order and populates the symbol table:
//! - Pre-seeds core (always-available) symbols
//! - Processes `import` declarations against the namespace registry
//! - Records function signatures (skips bodies)
//! - Records top-level variable declarations with declaration order
//! - Records `state {}` field names

use crate::syntax::ast::{Span, Program, Item, ImportDecl, Type, StateBlock, FnDef, Stmt, Expr};
use crate::error::{Error, ErrorCode};
use crate::namespaces::{NamespaceRegistry, core_exports};
use super::symbols::{Symbol, SymbolKind, SymbolTable};

pub struct Collector<'a> {
    registry: &'a NamespaceRegistry,
    pub table: SymbolTable,
    pub errors: Vec<Error>,
}

impl<'a> Collector<'a> {
    #[must_use] 
    pub fn new(registry: &'a NamespaceRegistry) -> Self {
        let mut table = SymbolTable::new();
        // Pre-seed with core symbols (always available, no import needed)
        for export in core_exports() {
            let sym = Symbol::new(export.name, Some(export.ty), SymbolKind::Function, Span::new(0, 0));
            table.declare_top_level(sym);
        }
        Self { registry, table, errors: Vec::new() }
    }

    #[must_use] 
    pub fn collect(mut self, program: &Program) -> (SymbolTable, Vec<Error>) {
        // Process imports first so imported names are available for the rest
        for import in &program.imports {
            self.collect_import(import);
        }

        // State block
        if let Some(state) = &program.state {
            self.collect_state(state);
        }

        // Top-level items in order
        for item in &program.items {
            match item {
                Item::FnDef(f)  => self.collect_fn_sig(f),
                Item::Stmt(s)   => self.collect_top_stmt(s),
                Item::Struct(s) => self.collect_struct(s),
                Item::Enum(e) => self.collect_enum(e),
            }
        }

        (self.table, self.errors)
    }

    // ── Imports ───────────────────────────────────────────────────────────────

    fn collect_import(&mut self, import: &ImportDecl) {
        let Some(ns) = self.registry.get(&import.namespace) else {
            self.errors.push(Error::new(
                ErrorCode::S005,
                import.span.line, import.span.column,
                format!("unknown namespace `{}`", import.namespace),
            ));
            return;
        };

        if import.members.is_empty() {
            // `import render` — add the namespace itself as a named symbol
            let sym = Symbol::new(
                import.namespace.clone(),
                Some(Type::Named(import.namespace.clone())),
                SymbolKind::Variable,
                import.span,
            );
            self.table.declare_top_level(sym);
        } else {
            // `import shapes { circle, rect }`
            for member in &import.members {
                if let Some(export) = ns.get_export(member) {
                    let kind = match export.kind {
                        crate::namespaces::ExportKind::Function => SymbolKind::Function,
                        crate::namespaces::ExportKind::Constant => SymbolKind::Variable,
                    };
                    let sym = Symbol::new(
                        export.name,
                        Some(export.ty),
                        kind,
                        import.span,
                    );
                    if !self.table.declare_top_level(sym) {
                        self.errors.push(Error::new(
                            ErrorCode::S003,
                            import.span.line, import.span.column,
                            format!("`{member}` already declared"),
                        ));
                    }
                } else {
                    let mut err = Error::new(
                        ErrorCode::S006,
                        import.span.line, import.span.column,
                        format!("`{}` does not export `{member}`", import.namespace),
                    );
                    if let Some(ns) = self.registry.get(&import.namespace) {
                        let exports = ns.exports();
                        let names: Vec<&str> = exports.iter().map(|e| e.name).collect();
                        err = err.with_hint(format!("available exports: {}", names.join(", ")));
                    }
                    self.errors.push(err);
                }
            }
        }
    }

    // ── State block ───────────────────────────────────────────────────────────

    fn collect_state(&mut self, state: &StateBlock) {
        for field in &state.fields {
            let sym = Symbol::new(
                // State fields are accessed as `s.field` inside update —
                // we register them under a namespaced key for the resolver to use.
                format!("__state__{}", field.name),
                field.ty.clone(),
                SymbolKind::StateField,
                field.span,
            );
            self.table.declare_top_level(sym);
        }
    }

    // ── Function signatures ───────────────────────────────────────────────────

    fn collect_fn_sig(&mut self, f: &FnDef) {
        let param_types: Vec<Type> = f.params.iter().map(|p| p.ty.clone()).collect();
        let fn_ty = Type::Fn(param_types, f.return_ty.clone().map(Box::new));
        let sym = Symbol::new(f.name.clone(), Some(fn_ty), SymbolKind::Function, f.span);
        if !self.table.declare_top_level(sym) {
            self.errors.push(Error::new(
                ErrorCode::S003,
                f.span.line, f.span.column,
                format!("function `{}` already declared", f.name),
            ));
        }
    }

    // ── Struct definitions ────────────────────────────────────────────────────

    fn collect_struct(&mut self, def: &crate::syntax::ast::StructDef) {
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
    }

    fn collect_enum(&mut self, def: &crate::syntax::ast::EnumDef) {
        let sym = Symbol::new(
            def.name.clone(),
            Some(Type::Named(def.name.clone())),
            SymbolKind::Enum,
            def.span,
        );
        if !self.table.declare_top_level(sym) {
            self.errors.push(Error::new(
                ErrorCode::S003,
                def.span.line, def.span.column,
                format!("'{}' is already defined", def.name),
            ));
        }
    }

    // ── Top-level statements ──────────────────────────────────────────────────

    fn collect_top_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl(v) => {
                // Try to infer type from a simple literal initializer.
                // Complex initializers are resolved in pass 2 (TypeResolver).
                let ty = v.ty.clone().or_else(|| infer_literal_type(&v.initializer));
                let kind = if v.is_const { SymbolKind::Const } else { SymbolKind::Variable };
                let sym = Symbol::new(v.name.clone(), ty, kind, v.span);
                if !self.table.declare_top_level(sym) {
                    self.errors.push(Error::new(
                        ErrorCode::S003,
                        v.span.line, v.span.column,
                        format!("`{}` already declared", v.name),
                    ));
                }
            }
            Stmt::FnVar { name, span, .. } => {
                // `fn f = expr` — type resolved in pass 2
                let sym = Symbol::new(name.clone(), None, SymbolKind::Function, *span);
                if !self.table.declare_top_level(sym) {
                    self.errors.push(Error::new(
                        ErrorCode::S003,
                        span.line, span.column,
                        format!("`{name}` already declared"),
                    ));
                }
            }
            // Other top-level statements are not declarations — skip
            _ => {}
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Try to determine the type of a simple literal expression without a full
/// inference pass. Returns `None` for complex expressions.
#[must_use] 
pub fn infer_literal_type(expr: &Expr) -> Option<Type> {
    match expr {
        Expr::Float(_, _)     => Some(Type::Float),
        Expr::Bool(_, _)      => Some(Type::Bool),
        Expr::StringLit(_, _) => Some(Type::String),
        Expr::HexColor(_, _)  => Some(Type::Color),
        // can't infer inner type from bare none
        Expr::List(items, _)  => {
            // Infer element type from the first item
            items.first()
                .and_then(infer_literal_type)
                .map(|elem_ty| Type::List(Box::new(elem_ty)))
        }
        _ => None,
    }
}
