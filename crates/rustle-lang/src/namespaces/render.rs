use crate::syntax::ast::Type;
use crate::types::draw::RenderMode;
use crate::error::RuntimeError;
use crate::Value;
use std::collections::HashMap;
use super::{Export, ExportKind, NamespaceInfo, NamespaceProvider, RuntimeState, as_float, check_argc};

fn named(s: &str) -> Type { Type::Named(s.into()) }

pub struct RenderNamespace;

impl NamespaceInfo for RenderNamespace {
    fn name(&self) -> &'static str { "render" }

    fn exports(&self) -> Vec<Export> {
        vec![
            Export { name: "sdf",     kind: ExportKind::Constant, ty: named("render_mode") },
            Export { name: "fill",    kind: ExportKind::Constant, ty: named("render_mode") },
            Export { name: "outline", kind: ExportKind::Constant, ty: named("render_mode") },
            Export {
                name: "stroke",
                kind: ExportKind::Function,
                ty: Type::Fn(vec![Type::Float], Some(Box::new(named("render_mode")))),
            },
            Export { name: "gl", kind: ExportKind::Constant, ty: named("gl") },
        ]
    }
}

impl NamespaceProvider for RenderNamespace {
    fn call(
        &self,
        name: &str,
        args: &[Value],
        _named: &HashMap<String, Value>,
        _state: &mut RuntimeState,
        line: usize,
    ) -> Result<Option<Value>, RuntimeError> {
        match name {
            "stroke" => {
                check_argc(name, args, 1, line)?;
                Ok(Some(Value::RenderMode(RenderMode::Stroke(as_float(&args[0], line)?))))
            }
            _ => Ok(None),
        }
    }

    fn get_constant(&self, name: &str) -> Option<Value> {
        match name {
            "sdf"     => Some(Value::RenderMode(RenderMode::Sdf)),
            "fill"    => Some(Value::RenderMode(RenderMode::Fill)),
            "outline" => Some(Value::RenderMode(RenderMode::Outline)),
            _ => None,
        }
    }
}
