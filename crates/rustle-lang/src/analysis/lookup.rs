//! Centralized field and method lookup for the resolver.
//!
//! Resolves `obj.field` and `obj.method` by consulting, in order:
//! 1. Namespace members  (shapes.circle, render.fill)
//! 2. State block fields (s.t, s.speed)
//! 3. TypeRegistry       (vec2.x, res.ok, transform.move, list.pop, …)
//!
//! TypeRegistry is the single source of truth for all built-in type info,
//! including generic types like `res<T>` and `list<T>`.

use crate::syntax::ast::*;
use crate::namespaces::NamespaceRegistry;
use crate::types::registry::TypeRegistry;
use super::collector::infer_literal_type;

pub struct LookupContext<'a> {
    pub program:  Option<&'a Program>,
    pub registry: &'a NamespaceRegistry,
    type_registry: TypeRegistry,
}

impl<'a> LookupContext<'a> {
    pub fn new(program: Option<&'a Program>, registry: &'a NamespaceRegistry) -> Self {
        Self { program, registry, type_registry: TypeRegistry::default() }
    }

    /// Resolve the type of `obj.field`.
    pub fn resolve_field(&self, obj_ty: &Type, field: &str) -> Option<Type> {
        // 1. Namespace member lookup.
        if let Type::Named(n) = obj_ty {
            if let Some(ns) = self.registry.get(n) {
                if let Some(export) = ns.get_export(field) {
                    return Some(export.ty);
                }
            }
            // State fields are dynamic — look them up from the parsed program.
            if n == "State" {
                if let Some(program) = self.program {
                    if let Some(state) = &program.state {
                        if let Some(f) = state.fields.iter().find(|f| f.name == field) {
                            return f.ty.clone().or_else(|| infer_literal_type(&f.initializer));
                        }
                    }
                }
                return None;
            }
        }
        // 2. TypeRegistry — handles all built-in types including generics.
        self.type_registry.resolve_field_type(obj_ty, field)
    }

    /// Resolve the return type of `obj.method(args)`.
    /// Returns `Type::Fn(params, ret)` so the checker can validate arg types.
    pub fn get_method_type(&self, obj_ty: &Type, method: &str) -> Option<Type> {
        // 1. Namespace member lookup.
        if let Type::Named(n) = obj_ty {
            if let Some(ns) = self.registry.get(n) {
                if let Some(export) = ns.get_export(method) {
                    return Some(export.ty);
                }
            }
        }
        // 2. TypeRegistry — handles all built-in types including generics.
        let (params, ret) = self.type_registry.resolve_method_signature(obj_ty, method)?;
        Some(Type::Fn(params, ret.map(Box::new)))
    }
}
