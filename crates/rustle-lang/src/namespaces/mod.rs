use crate::syntax::ast::Type;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExportKind { Function, Constant }

#[derive(Debug, Clone)]
pub struct Export {
    pub name: &'static str,
    pub kind: ExportKind,
    pub ty:   Type,
}

pub trait NamespaceInfo: Send + Sync {
    fn name(&self) -> &'static str;
    fn exports(&self) -> Vec<Export>;

    fn get_export(&self, name: &str) -> Option<Export> {
        self.exports().into_iter().find(|e| e.name == name)
    }
}

pub struct NamespaceRegistry {
    providers: Vec<Box<dyn NamespaceInfo>>,
}

impl NamespaceRegistry {
    #[must_use]
    pub fn new() -> Self { Self { providers: Vec::new() } }

    pub fn register(&mut self, p: Box<dyn NamespaceInfo>) {
        self.providers.push(p);
    }

    #[must_use]
    pub fn get(&self, name: &str) -> Option<&dyn NamespaceInfo> {
        self.providers.iter().find(|p| p.name() == name).map(std::convert::AsRef::as_ref)
    }

    #[must_use]
    pub fn standard() -> Self {
        let mut r = Self::new();
        r.register(Box::new(CoreInfo));
        r.register(Box::new(ShapesInfo));
        r.register(Box::new(RenderInfo));
        r.register(Box::new(CoordsInfo));
        r.register(Box::new(FileInfo));
        r
    }
}

impl Default for NamespaceRegistry {
    fn default() -> Self { Self::standard() }
}

// ─── Core namespace ──────────────────────────────────────────────────────────

struct CoreInfo;

impl NamespaceInfo for CoreInfo {
    fn name(&self) -> &'static str { "core" }
    fn exports(&self) -> Vec<Export> {
        use ExportKind::{Function as F, Constant as C};
        vec![
            Export { name: "sin",   kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "cos",   kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "tan",   kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "asin",  kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "acos",  kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "atan",  kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "sqrt",  kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "abs",   kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "floor", kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "ceil",  kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "round", kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "sign",  kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "fract", kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "atan2", kind: F, ty: Type::Fn(vec![Type::Float, Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "pow",   kind: F, ty: Type::Fn(vec![Type::Float, Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "min",   kind: F, ty: Type::Fn(vec![Type::Float, Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "max",   kind: F, ty: Type::Fn(vec![Type::Float, Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "clamp", kind: F, ty: Type::Fn(vec![Type::Float, Type::Float, Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "lerp",  kind: F, ty: Type::Fn(vec![Type::Float, Type::Float, Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "vec2",  kind: F, ty: Type::Fn(vec![Type::Float, Type::Float], Some(Box::new(Type::Vec2))) },
            Export { name: "vec3",  kind: F, ty: Type::Fn(vec![Type::Float, Type::Float, Type::Float], Some(Box::new(Type::Vec3))) },
            Export { name: "vec4",  kind: F, ty: Type::Fn(vec![Type::Float, Type::Float, Type::Float, Type::Float], Some(Box::new(Type::Vec4))) },
            Export { name: "color", kind: F, ty: Type::Fn(vec![Type::Float, Type::Float, Type::Float], Some(Box::new(Type::Color))) },
            Export { name: "transform", kind: F, ty: Type::Fn(vec![], Some(Box::new(Type::Transform))) },
            Export { name: "mat3",  kind: F, ty: Type::Fn(vec![], Some(Box::new(Type::Mat3))) },
            Export { name: "mat4",  kind: F, ty: Type::Fn(vec![], Some(Box::new(Type::Mat4))) },
            Export { name: "mat3_translate", kind: F, ty: Type::Fn(vec![Type::Float, Type::Float], Some(Box::new(Type::Mat3))) },
            Export { name: "mat3_rotate",    kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Mat3))) },
            Export { name: "mat3_scale",     kind: F, ty: Type::Fn(vec![Type::Float, Type::Float], Some(Box::new(Type::Mat3))) },
            Export { name: "mat4_translate", kind: F, ty: Type::Fn(vec![Type::Float, Type::Float, Type::Float], Some(Box::new(Type::Mat4))) },
            Export { name: "mat4_scale",    kind: F, ty: Type::Fn(vec![Type::Float, Type::Float, Type::Float], Some(Box::new(Type::Mat4))) },
            Export { name: "mat4_rotate_x", kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Mat4))) },
            Export { name: "mat4_rotate_y", kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Mat4))) },
            Export { name: "mat4_rotate_z", kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Mat4))) },
            Export { name: "ok",    kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(Type::Res(Box::new(Type::Float))))) },
            Export { name: "error", kind: F, ty: Type::Fn(vec![Type::String], Some(Box::new(Type::Res(Box::new(Type::Float))))) },
            Export { name: "PI",  kind: C, ty: Type::Float },
            Export { name: "TAU", kind: C, ty: Type::Float },
            Export { name: "red",         kind: C, ty: Type::Color },
            Export { name: "green",       kind: C, ty: Type::Color },
            Export { name: "blue",        kind: C, ty: Type::Color },
            Export { name: "white",       kind: C, ty: Type::Color },
            Export { name: "black",       kind: C, ty: Type::Color },
            Export { name: "transparent", kind: C, ty: Type::Color },
        ]
    }
}

pub(crate) fn core_exports() -> Vec<Export> { CoreInfo.exports() }

// ─── Shapes namespace ────────────────────────────────────────────────────────

struct ShapesInfo;

impl NamespaceInfo for ShapesInfo {
    fn name(&self) -> &'static str { "shapes" }
    fn exports(&self) -> Vec<Export> {
        use ExportKind::{Function as F, Constant as C};
        vec![
            Export { name: "circle",  kind: F, ty: Type::Fn(vec![Type::Vec2, Type::Float], Some(Box::new(Type::Circle))) },
            Export { name: "rect",    kind: F, ty: Type::Fn(vec![Type::Vec2, Type::Vec2], Some(Box::new(Type::Rect))) },
            Export { name: "line",    kind: F, ty: Type::Fn(vec![Type::Vec2, Type::Vec2], Some(Box::new(Type::Line))) },
            Export { name: "polygon", kind: F, ty: Type::Fn(vec![Type::List(Box::new(Type::Vec2))], Some(Box::new(Type::Polygon))) },
            Export { name: "shape",   kind: F, ty: Type::Fn(vec![Type::List(Box::new(Type::Vec2))], Some(Box::new(Type::Polygon))) },
            Export { name: "text",    kind: F, ty: Type::Fn(vec![Type::Vec2, Type::String, Type::Float], Some(Box::new(Type::TextShape))) },
            Export { name: "center",       kind: C, ty: Type::String },
            Export { name: "top_left",     kind: C, ty: Type::String },
            Export { name: "top_right",    kind: C, ty: Type::String },
            Export { name: "bottom_left",  kind: C, ty: Type::String },
            Export { name: "bottom_right", kind: C, ty: Type::String },
            Export { name: "top",          kind: C, ty: Type::String },
            Export { name: "bottom",       kind: C, ty: Type::String },
            Export { name: "left",         kind: C, ty: Type::String },
            Export { name: "right",        kind: C, ty: Type::String },
        ]
    }
}

// ─── Render namespace ────────────────────────────────────────────────────────

struct RenderInfo;

impl NamespaceInfo for RenderInfo {
    fn name(&self) -> &'static str { "render" }
    fn exports(&self) -> Vec<Export> {
        use ExportKind::{Function as F, Constant as C};
        let rm = Type::Named("render_mode".into());
        vec![
            Export { name: "sdf",     kind: C, ty: rm.clone() },
            Export { name: "fill",    kind: C, ty: rm.clone() },
            Export { name: "outline", kind: C, ty: rm.clone() },
            Export { name: "stroke",  kind: F, ty: Type::Fn(vec![Type::Float], Some(Box::new(rm))) },
        ]
    }
}

// ─── Coords namespace ────────────────────────────────────────────────────────

struct CoordsInfo;

impl NamespaceInfo for CoordsInfo {
    fn name(&self) -> &'static str { "coords" }
    fn exports(&self) -> Vec<Export> {
        use ExportKind::{Function as F, Constant as C};
        vec![
            Export { name: "resolution", kind: F, ty: Type::Fn(vec![Type::Float, Type::Float], Some(Box::new(Type::Float))) },
            Export { name: "origin",     kind: F, ty: Type::Fn(vec![Type::String], Some(Box::new(Type::Float))) },
            Export { name: "center",       kind: C, ty: Type::String },
            Export { name: "top_left",     kind: C, ty: Type::String },
            Export { name: "top_right",    kind: C, ty: Type::String },
            Export { name: "bottom_left",  kind: C, ty: Type::String },
            Export { name: "bottom_right", kind: C, ty: Type::String },
            Export { name: "top",          kind: C, ty: Type::String },
            Export { name: "bottom",       kind: C, ty: Type::String },
            Export { name: "left",         kind: C, ty: Type::String },
            Export { name: "right",        kind: C, ty: Type::String },
        ]
    }
}

// ─── File namespace ──────────────────────────────────────────────────────────

struct FileInfo;

impl NamespaceInfo for FileInfo {
    fn name(&self) -> &'static str { "file" }
    fn exports(&self) -> Vec<Export> {
        use ExportKind::Function as F;
        vec![
            Export { name: "read",       kind: F, ty: Type::Fn(vec![Type::String], Some(Box::new(Type::Res(Box::new(Type::String))))) },
            Export { name: "read_lines", kind: F, ty: Type::Fn(vec![Type::String], Some(Box::new(Type::Res(Box::new(Type::List(Box::new(Type::String))))))) },
            Export { name: "write",      kind: F, ty: Type::Fn(vec![Type::String, Type::String], Some(Box::new(Type::Res(Box::new(Type::Bool))))) },
            Export { name: "append",     kind: F, ty: Type::Fn(vec![Type::String, Type::String], Some(Box::new(Type::Res(Box::new(Type::Bool))))) },
        ]
    }
}
