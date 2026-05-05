//! Centralized field and method lookup for the resolver.
//!
//! Resolves `obj.field` and `obj.method` by consulting, in order:
//! 1. Namespace members  (shapes.circle, render.fill)
//! 2. State block fields (s.t, s.speed)
//! 3. `TypeRegistry`       (vec2.x, res.ok, transform.move, list.pop, …)
//!
//! `TypeRegistry` is the single source of truth for all built-in type info,
//! including generic types like `res<T>` and `list<T>`.

use crate::syntax::ast::{Program, Type};
use crate::namespaces::NamespaceRegistry;
use crate::types::registry::TypeRegistry;
use super::collector::infer_literal_type;

pub struct LookupContext<'a> {
    pub program:  Option<&'a Program>,
    pub registry: &'a NamespaceRegistry,
    pub(super) type_registry: &'a TypeRegistry,
}

impl<'a> LookupContext<'a> {
    #[must_use]
    pub fn new(program: Option<&'a Program>, registry: &'a NamespaceRegistry, type_registry: &'a TypeRegistry) -> Self {
        Self { program, registry, type_registry }
    }

    /// Resolve the type of `obj.field`.
    #[must_use] 
    pub fn resolve_field(&self, obj_ty: &Type, field: &str) -> Option<Type> {
        // 1. Namespace member lookup (only for user-defined Named types used as namespace refs).
        if let Type::Named(n) = obj_ty
            && let Some(ns) = self.registry.get(n)
                && let Some(export) = ns.get_export(field) {
                    return Some(export.ty);
                }
        // 2. State fields are dynamic — look them up from the parsed program.
        if *obj_ty == Type::State {
            if let Some(program) = self.program
                && let Some(state) = &program.state
                    && let Some(f) = state.fields.iter().find(|f| f.name == field) {
                        return f.ty.clone().or_else(|| infer_literal_type(&f.initializer));
                    }
            return None;
        }
        // 3. Struct fields — look up from program AST.
        if let Type::Named(name) = obj_ty {
            if let Some(program) = self.program {
                for item in &program.items {
                    if let crate::syntax::ast::Item::Struct(def) = item {
                        if def.name == *name {
                            if let Some(f) = def.fields.iter().find(|f| f.name == field) {
                                return f.ty.clone().or_else(|| {
                                    f.default.as_ref().and_then(infer_literal_type)
                                });
                            }
                            return None; // struct found but field doesn't exist
                        }
                    }
                }
            }
        }
        // 4. TypeRegistry — handles all built-in types including generics.
        self.type_registry.resolve_field_type(obj_ty, field)
    }

    /// Return all field names available on the given type.
    #[must_use]
    pub fn field_names<'b>(&'b self, obj_ty: &Type) -> Vec<&'b str> {
        if *obj_ty == Type::State {
            if let Some(program) = self.program
                && let Some(state) = &program.state {
                    return state.fields.iter().map(|f| f.name.as_str()).collect();
                }
            return Vec::new();
        }
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

    /// Return all method names available on the given type.
    #[must_use]
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

    /// Resolve the return type of `obj.method(args)`.
    /// Returns `Type::Fn(params, ret)` so the checker can validate arg types.
    pub fn get_method_type(&self, obj_ty: &Type, method: &str) -> Option<Type> {
        // 1. Namespace member lookup.
        if let Type::Named(n) = obj_ty
            && let Some(ns) = self.registry.get(n)
                && let Some(export) = ns.get_export(method) {
                    return Some(export.ty);
                }
        // 2. Struct methods — look up from program AST.
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
        // 3. TypeRegistry — handles all built-in types including generics.
        let (params, ret) = self.type_registry.resolve_method_signature(obj_ty, method)?;
        Some(Type::Fn(params, ret.map(Box::new)))
    }
}
