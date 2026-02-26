use std::collections::HashMap;
use crate::syntax::ast::{Span, Type};

// ─── Symbol ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Variable,
    Const,
    Function,
    Param,
    StateField,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    /// `None` while the type is still being inferred (filled in by TypeResolver).
    pub ty: Option<Type>,
    pub kind: SymbolKind,
    pub span: Span,
    /// Position in the top-level declaration sequence.
    /// Used to enforce the strict ordering rule inside function bodies.
    /// 0 for non-top-level symbols (params, local vars).
    pub declaration_order: usize,
}

impl Symbol {
    pub fn new(name: impl Into<String>, ty: Option<Type>, kind: SymbolKind, span: Span) -> Self {
        Self { name: name.into(), ty, kind, span, declaration_order: 0 }
    }
}

// ─── Scope ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ScopeKind {
    Global,
    Function,
    Block,
}

pub struct Scope {
    pub kind: ScopeKind,
    symbols: HashMap<String, Symbol>,
}

impl Scope {
    pub fn new(kind: ScopeKind) -> Self {
        Self { kind, symbols: HashMap::new() }
    }

    /// Returns `false` if a symbol with the same name already exists in this scope.
    pub fn declare(&mut self, sym: Symbol) -> bool {
        if self.symbols.contains_key(&sym.name) {
            return false;
        }
        self.symbols.insert(sym.name.clone(), sym);
        true
    }

    pub fn get(&self, name: &str) -> Option<&Symbol> {
        self.symbols.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Symbol> {
        self.symbols.get_mut(name)
    }
}

// ─── SymbolTable ──────────────────────────────────────────────────────────────

pub struct SymbolTable {
    pub scopes: Vec<Scope>,
    top_level_counter: usize,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self { scopes: vec![Scope::new(ScopeKind::Global)], top_level_counter: 0 }
    }

    pub fn push_scope(&mut self, kind: ScopeKind) {
        self.scopes.push(Scope::new(kind));
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn current_scope_kind(&self) -> &ScopeKind {
        &self.scopes.last().unwrap().kind
    }

    /// Declare a symbol in the current (innermost) scope.
    /// Returns `false` on redeclaration.
    pub fn declare(&mut self, sym: Symbol) -> bool {
        self.scopes.last_mut().unwrap().declare(sym)
    }

    /// Declare a top-level symbol (global scope), assigning it a declaration order.
    /// Returns `false` on redeclaration.
    pub fn declare_top_level(&mut self, mut sym: Symbol) -> bool {
        sym.declaration_order = self.top_level_counter;
        self.top_level_counter += 1;
        self.scopes[0].declare(sym)
    }

    /// Normal lookup: innermost scope to outermost, no ordering constraint.
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.get(name) {
                return Some(sym);
            }
        }
        None
    }

    /// Strict lookup used inside function bodies.
    /// - Non-global scopes (params, local vars): visible regardless of order.
    /// - Global scope: functions always visible; variables/consts only if
    ///   declared before `fn_order` (strict top-level ordering).
    pub fn lookup_strict(&self, name: &str, fn_order: usize) -> Option<&Symbol> {
        // Search non-global scopes from innermost outward
        for scope in self.scopes.iter().rev() {
            if scope.kind == ScopeKind::Global {
                break;
            }
            if let Some(sym) = scope.get(name) {
                return Some(sym);
            }
        }
        // Check global scope with ordering constraint
        if let Some(sym) = self.scopes[0].get(name) {
            match sym.kind {
                SymbolKind::Function => return Some(sym), // always visible
                _ if sym.declaration_order < fn_order => return Some(sym),
                _ => return None,
            }
        }
        None
    }

    /// Update the resolved type of a symbol anywhere in the table.
    pub fn update_type(&mut self, name: &str, ty: Type) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(sym) = scope.get_mut(name) {
                sym.ty = Some(ty);
                return;
            }
        }
    }
}

impl Default for SymbolTable {
    fn default() -> Self { Self::new() }
}

impl SymbolTable {
    /// All symbols in the global scope, sorted by declaration order.
    pub fn global_symbols(&self) -> Vec<&Symbol> {
        let mut syms: Vec<&Symbol> = self.scopes[0].symbols.values().collect();
        syms.sort_by_key(|s| s.declaration_order);
        syms
    }
}
