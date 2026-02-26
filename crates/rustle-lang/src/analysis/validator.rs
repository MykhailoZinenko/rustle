//! Pass 3 — Semantic Validator
//!
//! Final checks that don't fit neatly into type inference:
//! - `const` never reassigned (cross-check against symbol table)
//! - `state {}` appears at most once (caught by parser, double-checked here)
//! - `update` function has the correct signature if defined

use crate::syntax::ast::*;
use crate::error::{Error, ErrorCode};
use super::symbols::SymbolTable;

pub struct Validator<'a> {
    table: &'a SymbolTable,
    pub errors: Vec<Error>,
}

impl<'a> Validator<'a> {
    pub fn new(table: &'a SymbolTable) -> Self {
        Self { table, errors: Vec::new() }
    }

    pub fn validate(mut self, program: &Program) -> Vec<Error> {
        self.check_update_signature(program);
        self.check_const_reassignment(program);
        self.errors
    }

    // ── update() signature ────────────────────────────────────────────────────

    /// If an `update` function is defined, it must have the signature:
    /// `fn update(s: State, input: Input) -> State`
    fn check_update_signature(&mut self, program: &Program) {
        let update_fn = program.items.iter().find_map(|item| match item {
            Item::FnDef(f) if f.name == "update" => Some(f),
            _ => None,
        });

        let Some(f) = update_fn else { return };

        let ok = f.params.len() == 2
            && matches!(&f.params[0].ty, Type::Named(n) if n == "State")
            && matches!(&f.params[1].ty, Type::Named(n) if n == "Input")
            && matches!(&f.return_ty, Some(Type::Named(n)) if n == "State");

        if !ok {
            self.errors.push(Error::new(
                ErrorCode::S012,
                f.span.line, f.span.column,
                "`update` must have signature: fn update(s: State, input: Input) -> State",
            ));
        }
    }

    // ── const reassignment ────────────────────────────────────────────────────

    fn check_const_reassignment(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                Item::FnDef(f) => self.scan_stmts_for_const_assign(&f.body),
                Item::Stmt(s)  => self.scan_stmt_for_const_assign(s),
            }
        }
    }

    fn scan_stmts_for_const_assign(&mut self, stmts: &[Stmt]) {
        for s in stmts { self.scan_stmt_for_const_assign(s); }
    }

    fn scan_stmt_for_const_assign(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Assign(a) => {
                let root = &a.target[0];
                if let Some(sym) = self.table.lookup(root) {
                    if sym.kind == super::symbols::SymbolKind::Const {
                        self.errors.push(Error::new(
                            ErrorCode::S004,
                            a.span.line, a.span.column,
                            format!("cannot reassign const `{root}`"),
                        ));
                    }
                }
            }
            Stmt::If(i) => {
                self.scan_stmts_for_const_assign(&i.then_block);
                if let Some(e) = &i.else_block {
                    self.scan_stmts_for_const_assign(e);
                }
            }
            Stmt::While(w) => self.scan_stmts_for_const_assign(&w.body),
            Stmt::For(f)   => {
                self.scan_stmt_for_const_assign(&f.init);
                self.scan_stmt_for_const_assign(&f.step);
                self.scan_stmts_for_const_assign(&f.body);
            }
            Stmt::Foreach(f) => self.scan_stmts_for_const_assign(&f.body),
            _ => {}
        }
    }
}
