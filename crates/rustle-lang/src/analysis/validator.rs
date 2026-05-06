//! Pass 3 — Semantic Validator
//!
//! Final checks that don't fit neatly into type inference:
//! - `on_update`, `on_init`, `on_exit` have correct signatures if defined
//! - `break` / `continue` only appear inside loops

use crate::{
    error::{Error, ErrorCode},
    syntax::ast::{Item, Program, Stmt, StructDef, Type},
};

pub struct Validator {
    pub errors: Vec<Error>,
    loop_depth: usize,
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator {
    #[must_use]
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            loop_depth: 0,
        }
    }

    #[must_use]
    pub fn validate(mut self, program: &Program) -> Vec<Error> {
        self.check_on_update_signature(program);
        self.check_on_init_signature(program);
        self.check_on_exit_signature(program);
        self.check_break_continue(program);
        self.check_struct_definitions(program);
        self.errors
    }

    fn check_on_update_signature(&mut self, program: &Program) {
        let f = program.items.iter().find_map(|item| match item {
            Item::FnDef(f) if f.name == "on_update" => Some(f),
            _ => None,
        });
        let Some(f) = f else { return };
        let ok = f.params.len() == 2
            && f.params[0].ty == Type::State
            && f.params[1].ty == Type::Input
            && f.return_ty.as_ref() == Some(&Type::State);
        if !ok {
            self.errors.push(Error::new(
                ErrorCode::S012,
                f.span.line,
                f.span.column,
                "`on_update` must have signature: fn on_update(s: State, input: Input) -> State",
            ));
        }
    }

    fn check_on_init_signature(&mut self, program: &Program) {
        let f = program.items.iter().find_map(|item| match item {
            Item::FnDef(f) if f.name == "on_init" => Some(f),
            _ => None,
        });
        let Some(f) = f else { return };
        let ok = f.params.len() == 1
            && f.params[0].ty == Type::State
            && f.return_ty.as_ref() == Some(&Type::State);
        if !ok {
            self.errors.push(Error::new(
                ErrorCode::S012,
                f.span.line,
                f.span.column,
                "`on_init` must have signature: fn on_init(s: State) -> State",
            ));
        }
    }

    fn check_on_exit_signature(&mut self, program: &Program) {
        let f = program.items.iter().find_map(|item| match item {
            Item::FnDef(f) if f.name == "on_exit" => Some(f),
            _ => None,
        });
        let Some(f) = f else { return };
        let ok = f.params.len() == 1
            && f.params[0].ty == Type::State
            && f.return_ty.as_ref() == Some(&Type::State);
        if !ok {
            self.errors.push(Error::new(
                ErrorCode::S012,
                f.span.line,
                f.span.column,
                "`on_exit` must have signature: fn on_exit(s: State) -> State",
            ));
        }
    }

    // ── struct definition validation ────────────────────────────────────────

    fn check_struct_definitions(&mut self, program: &Program) {
        for item in &program.items {
            if let Item::Struct(def) = item {
                self.validate_struct(def);
            }
        }
    }

    fn validate_struct(&mut self, def: &StructDef) {
        let mut seen_fields = std::collections::HashSet::new();
        for field in &def.fields {
            if !seen_fields.insert(&field.name) {
                self.errors.push(Error::new(
                    ErrorCode::S019,
                    field.span.line,
                    field.span.column,
                    format!("duplicate field '{}' in '{}'", field.name, def.name),
                ));
            }
        }

        let mut seen_methods = std::collections::HashSet::new();
        for method in &def.methods {
            if !seen_methods.insert(&method.def.name) {
                self.errors.push(Error::new(
                    ErrorCode::S020,
                    method.span.line,
                    method.span.column,
                    format!("duplicate method '{}' in '{}'", method.def.name, def.name),
                ));
            }
        }
    }

    // ── break / continue validation ──────────────────────────────────────────

    fn check_break_continue(&mut self, program: &Program) {
        for item in &program.items {
            match item {
                Item::FnDef(f) => self.scan_stmts_for_break_continue(&f.body),
                Item::Stmt(s) => self.scan_stmt_for_break_continue(s),
                Item::Struct(_) | Item::Enum(_) => {}
            }
        }
    }

    fn scan_stmts_for_break_continue(&mut self, stmts: &[Stmt]) {
        for s in stmts {
            self.scan_stmt_for_break_continue(s);
        }
    }

    fn scan_stmt_for_break_continue(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Break(span) => {
                if self.loop_depth == 0 {
                    self.errors.push(Error::new(
                        ErrorCode::S014,
                        span.line,
                        span.column,
                        "`break` can only be used inside a loop",
                    ));
                }
            }
            Stmt::Continue(span) => {
                if self.loop_depth == 0 {
                    self.errors.push(Error::new(
                        ErrorCode::S014,
                        span.line,
                        span.column,
                        "`continue` can only be used inside a loop",
                    ));
                }
            }
            Stmt::While(w) => {
                self.loop_depth += 1;
                self.scan_stmts_for_break_continue(&w.body);
                self.loop_depth -= 1;
            }
            Stmt::For(f) => {
                self.loop_depth += 1;
                self.scan_stmts_for_break_continue(&f.body);
                self.loop_depth -= 1;
            }
            Stmt::Foreach(f) => {
                self.loop_depth += 1;
                self.scan_stmts_for_break_continue(&f.body);
                self.loop_depth -= 1;
            }
            Stmt::If(i) => {
                self.scan_stmts_for_break_continue(&i.then_block);
                if let Some(e) = &i.else_block {
                    self.scan_stmts_for_break_continue(e);
                }
            }
            Stmt::IfLet {
                then_block,
                else_block,
                ..
            } => {
                self.scan_stmts_for_break_continue(then_block);
                if let Some(e) = else_block {
                    self.scan_stmts_for_break_continue(e);
                }
            }
            Stmt::Match(m) => {
                for arm in &m.arms {
                    self.scan_stmts_for_break_continue(&arm.body);
                }
            }
            _ => {}
        }
    }
}
