use crate::syntax::ast::Type;
use crate::types::draw::Origin;
use crate::error::RuntimeError;
use crate::Value;
use std::collections::HashMap;
use super::{Export, ExportKind, NamespaceInfo, NamespaceProvider, RuntimeState, as_float, check_argc};

fn vfn(name: &'static str, params: Vec<Type>) -> Export {
    Export { name, kind: ExportKind::Function, ty: Type::Fn(params, None) }
}
fn named(s: &str) -> Type { Type::Named(s.into()) }
fn origin_const(name: &'static str) -> Export {
    Export { name, kind: ExportKind::Constant, ty: named("origin") }
}

pub struct CoordsNamespace;

impl NamespaceInfo for CoordsNamespace {
    fn name(&self) -> &'static str { "coords" }

    fn exports(&self) -> Vec<Export> {
        vec![
            vfn("resolution", vec![Type::Float, Type::Float]),
            vfn("origin",     vec![named("origin")]),
            origin_const("center"),
            origin_const("top_left"), origin_const("top_right"),
            origin_const("bottom_left"), origin_const("bottom_right"),
            origin_const("top"), origin_const("bottom"),
            origin_const("left"), origin_const("right"),
        ]
    }
}

impl NamespaceProvider for CoordsNamespace {
    fn call(
        &self,
        name: &str,
        args: &[Value],
        _named: &HashMap<String, Value>,
        state: &mut RuntimeState,
        line: usize,
    ) -> Result<Option<Value>, RuntimeError> {
        match name {
            "resolution" => {
                check_argc(name, args, 2, line)?;
                state.coord_meta.px_width  = as_float(&args[0], line)?;
                state.coord_meta.px_height = as_float(&args[1], line)?;
                Ok(Some(Value::Float(0.0)))
            }
            "origin" => {
                check_argc(name, args, 1, line)?;
                let s = match &args[0] {
                    Value::Str(s) => s.clone(),
                    other => return Err(RuntimeError::new(line, format!(
                        "`origin` expects an origin constant, got `{:?}`", other
                    ))),
                };
                state.coord_meta.origin = parse_origin(&s)
                    .ok_or_else(|| RuntimeError::new(line, format!("unknown origin: `{s}`")))?;
                Ok(Some(Value::Float(0.0)))
            }
            _ => Ok(None),
        }
    }

    fn get_constant(&self, name: &str) -> Option<Value> {
        match name {
            "center" | "top_left" | "top_right" | "bottom_left" | "bottom_right"
            | "top" | "bottom" | "left" | "right" => Some(Value::Str(name.into())),
            _ => None,
        }
    }
}

fn parse_origin(s: &str) -> Option<Origin> {
    match s {
        "center"       => Some(Origin::Center),
        "top_left"     => Some(Origin::TopLeft),
        "top_right"    => Some(Origin::TopRight),
        "bottom_left"  => Some(Origin::BottomLeft),
        "bottom_right" => Some(Origin::BottomRight),
        "top"          => Some(Origin::Top),
        "bottom"       => Some(Origin::Bottom),
        "left"         => Some(Origin::Left),
        "right"        => Some(Origin::Right),
        _              => None,
    }
}
