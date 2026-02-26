pub mod symbols;
pub mod collector;
pub mod lookup;
pub mod checker;
pub mod validator;

#[cfg(test)]
mod tests;

use crate::syntax::ast;
use crate::error::Error;
use crate::namespaces::NamespaceRegistry;
use collector::Collector;
use checker::TypeResolver;
use validator::Validator;
pub use symbols::SymbolTable;

// ─── Result ───────────────────────────────────────────────────────────────────

pub struct ResolveResult {
    pub symbol_table: SymbolTable,
    pub warnings: Vec<Error>,
}

// ─── Entry point ─────────────────────────────────────────────────────────────

/// Full resolver pipeline:
/// 1. Collector   — build symbol table from declarations and imports
/// 2. TypeResolver — infer and check all types
/// 3. Validator   — final semantic checks
///
/// Returns `Ok(ResolveResult)` if there are no errors, `Err(errors)` otherwise.
pub fn resolve(
    program: &ast::Program,
    registry: &NamespaceRegistry,
) -> Result<ResolveResult, Vec<Error>> {
    let mut all_errors: Vec<Error> = Vec::new();

    // ── Pass 1: collect symbols ───────────────────────────────────────────────
    let (table, collect_errors) = Collector::new(registry).collect(program);
    all_errors.extend(collect_errors);

    // ── Pass 2: type inference and checking ───────────────────────────────────
    let (table, type_errors) = TypeResolver::new(table, registry).run(program);
    all_errors.extend(type_errors);

    // ── Pass 3: semantic validation ───────────────────────────────────────────
    let validate_errors = Validator::new(&table).validate(program);
    all_errors.extend(validate_errors);

    // ─────────────────────────────────────────────────────────────────────────
    let (errors, warnings): (Vec<_>, Vec<_>) = all_errors
        .into_iter()
        .partition(|e| e.code.is_error());

    if errors.is_empty() {
        Ok(ResolveResult { symbol_table: table, warnings })
    } else {
        Err(errors)
    }
}
