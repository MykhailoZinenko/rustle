use crate::syntax::ast::Type;
use crate::types::draw::{Origin, ShapeData, ShapeDesc};
use crate::error::RuntimeError;
use crate::Value;
use std::collections::HashMap;
use super::{
    Export, ExportKind, NamespaceInfo, NamespaceProvider, RuntimeState,
    as_float, as_vec2, as_vertices, check_argc, render_mode_from_named,
};

fn named(s: &str) -> Type { Type::Named(s.into()) }

fn origin_const(name: &'static str) -> Export {
    Export { name, kind: ExportKind::Constant, ty: named("origin") }
}

pub struct ShapesNamespace;

impl NamespaceInfo for ShapesNamespace {
    fn name(&self) -> &'static str { "shapes" }

    fn exports(&self) -> Vec<Export> {
        vec![
            Export { name: "circle",  kind: ExportKind::Function,
                ty: Type::Fn(vec![named("vec2"), Type::Float], Some(Box::new(named("circle")))) },
            Export { name: "rect",    kind: ExportKind::Function,
                ty: Type::Fn(vec![named("vec2"), named("vec2")], Some(Box::new(named("rect")))) },
            Export { name: "line",    kind: ExportKind::Function,
                ty: Type::Fn(vec![named("vec2"), named("vec2")], Some(Box::new(named("line")))) },
            Export { name: "polygon", kind: ExportKind::Function,
                ty: Type::Fn(vec![Type::List(Box::new(named("vec2")))], Some(Box::new(named("polygon")))) },
            Export { name: "shape",   kind: ExportKind::Function,
                ty: Type::Fn(vec![Type::List(Box::new(named("vec2")))], Some(Box::new(named("polygon")))) },
            // Origin constants
            origin_const("center"),
            origin_const("top_left"), origin_const("top_right"),
            origin_const("bottom_left"), origin_const("bottom_right"),
            origin_const("top"), origin_const("bottom"),
            origin_const("left"), origin_const("right"),
        ]
    }
}

impl NamespaceProvider for ShapesNamespace {
    fn call(
        &self,
        name: &str,
        args: &[Value],
        named_args: &HashMap<String, Value>,
        state: &mut RuntimeState,
        line: usize,
    ) -> Result<Option<Value>, RuntimeError> {
        let coord_meta  = state.coord_meta.clone();
        let render_mode = render_mode_from_named(named_args, line)?;

        let desc = match name {
            "circle" => {
                check_argc(name, args, 2, line)?;
                let center = as_vec2(&args[0], line)?;
                let radius = as_float(&args[1], line)?;
                ShapeDesc::Circle { center, radius }
            }
            "rect" => {
                check_argc(name, args, 2, line)?;
                let center = as_vec2(&args[0], line)?;
                let size   = as_vec2(&args[1], line)?;
                let origin = origin_from_named(named_args);
                ShapeDesc::Rect { center, size, origin }
            }
            "line" => {
                check_argc(name, args, 2, line)?;
                let from = as_vec2(&args[0], line)?;
                let to   = as_vec2(&args[1], line)?;
                ShapeDesc::Line { from, to }
            }
            "polygon" | "shape" => {
                check_argc(name, args, 1, line)?;
                ShapeDesc::Polygon(as_vertices(&args[0], line)?)
            }
            _ => return Ok(None),
        };

        Ok(Some(Value::Shape(ShapeData::new(desc, render_mode, coord_meta))))
    }

    fn get_constant(&self, name: &str) -> Option<Value> {
        match name {
            "center" | "top_left" | "top_right" | "bottom_left" | "bottom_right"
            | "top" | "bottom" | "left" | "right" => Some(Value::Str(name.into())),
            _ => None,
        }
    }
}

fn origin_from_named(named: &HashMap<String, Value>) -> Origin {
    match named.get("origin") {
        Some(Value::Str(s)) => match s.as_str() {
            "top_left"     => Origin::TopLeft,
            "top_right"    => Origin::TopRight,
            "bottom_left"  => Origin::BottomLeft,
            "bottom_right" => Origin::BottomRight,
            "top"          => Origin::Top,
            "bottom"       => Origin::Bottom,
            "left"         => Origin::Left,
            "right"        => Origin::Right,
            _              => Origin::Center,
        },
        _ => Origin::Center,
    }
}
