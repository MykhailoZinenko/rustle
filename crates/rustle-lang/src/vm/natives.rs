#![expect(clippy::cast_possible_truncation, reason = "VM indices are guaranteed small by construction")]
#![expect(clippy::match_same_arms, reason = "each shape type handled separately for clarity")]
use std::io::Write as IoWrite;
use std::str::FromStr;

use crate::error::{ErrorCode, RuntimeError};
use crate::types::draw::{CoordMeta, Origin, RenderMode, ShapeData, ShapeDesc, TransformData};
use crate::types::mat::{
    m3_identity, m3_rotate2d, m3_scale2d, m3_translate2d,
    m4_identity, m4_rotate_x, m4_rotate_y, m4_rotate_z, m4_scale_xyz, m4_translate,
};

use super::heap::Heap;
use super::util::lookup_color;
use super::value::{HeapObject, StackValue, display};

// ─── Native function type ─────────────────────────────────────────────────────

pub type NativeFuncImpl = fn(
    heap: &mut Heap,
    args: &[StackValue],
    coord_meta: &mut CoordMeta,
    line: usize,
) -> Result<StackValue, RuntimeError>;

pub struct NativeFunc {
    pub name: &'static str,
    pub func: NativeFuncImpl,
}

impl Clone for NativeFunc {
    fn clone(&self) -> Self {
        Self { name: self.name, func: self.func }
    }
}

// ─── Public helpers ──────────────────────────────────────────────────────────

/// Build the full native-function dispatch table.
/// The compiler maps names to indices; the VM dispatches by index.
#[must_use] 
pub fn native_table() -> Vec<NativeFunc> {
    vec![
        // Core math — 1-arg
        NativeFunc { name: "sin",   func: native_sin   },
        NativeFunc { name: "cos",   func: native_cos   },
        NativeFunc { name: "tan",   func: native_tan   },
        NativeFunc { name: "asin",  func: native_asin  },
        NativeFunc { name: "acos",  func: native_acos  },
        NativeFunc { name: "atan",  func: native_atan  },
        NativeFunc { name: "sqrt",  func: native_sqrt  },
        NativeFunc { name: "abs",   func: native_abs   },
        NativeFunc { name: "floor", func: native_floor },
        NativeFunc { name: "ceil",  func: native_ceil  },
        NativeFunc { name: "round", func: native_round },
        NativeFunc { name: "sign",  func: native_sign  },
        NativeFunc { name: "fract", func: native_fract },
        // Core math — 2-arg
        NativeFunc { name: "atan2", func: native_atan2 },
        NativeFunc { name: "pow",   func: native_pow   },
        NativeFunc { name: "min",   func: native_min   },
        NativeFunc { name: "max",   func: native_max   },
        // Core math — 3-arg
        NativeFunc { name: "clamp", func: native_clamp },
        NativeFunc { name: "lerp",  func: native_lerp  },
        // Constructors
        NativeFunc { name: "vec2",          func: native_vec2          },
        NativeFunc { name: "vec3",          func: native_vec3          },
        NativeFunc { name: "vec4",          func: native_vec4          },
        NativeFunc { name: "color",         func: native_color         },
        NativeFunc { name: "transform",     func: native_transform     },
        NativeFunc { name: "mat3",          func: native_mat3          },
        NativeFunc { name: "mat4",          func: native_mat4          },
        NativeFunc { name: "mat3_translate",func: native_mat3_translate},
        NativeFunc { name: "mat3_rotate",   func: native_mat3_rotate   },
        NativeFunc { name: "mat3_scale",    func: native_mat3_scale    },
        NativeFunc { name: "mat4_translate",func: native_mat4_translate},
        NativeFunc { name: "mat4_scale",    func: native_mat4_scale    },
        NativeFunc { name: "mat4_rotate_x", func: native_mat4_rotate_x },
        NativeFunc { name: "mat4_rotate_y", func: native_mat4_rotate_y },
        NativeFunc { name: "mat4_rotate_z", func: native_mat4_rotate_z },
        // Result helpers
        NativeFunc { name: "ok",    func: native_ok    },
        NativeFunc { name: "error", func: native_error },
        // Shape constructors
        NativeFunc { name: "circle",  func: native_circle  },
        NativeFunc { name: "rect",    func: native_rect    },
        NativeFunc { name: "line",    func: native_line    },
        NativeFunc { name: "polygon", func: native_polygon },
        NativeFunc { name: "text",    func: native_text    },
        // Render
        NativeFunc { name: "stroke", func: native_stroke },
        // Coords
        NativeFunc { name: "resolution", func: native_resolution },
        NativeFunc { name: "origin",     func: native_origin     },
        // File I/O
        NativeFunc { name: "file.read",       func: native_file_read       },
        NativeFunc { name: "file.read_lines", func: native_file_read_lines },
        NativeFunc { name: "file.write",      func: native_file_write      },
        NativeFunc { name: "file.append",     func: native_file_append     },
    ]
}

/// Look up a native function index by name.
#[must_use]
pub fn native_index_by_name(table: &[NativeFunc], name: &str) -> Option<u16> {
    table.iter().position(|f| f.name == name).map(|i| i as u16)
}

/// Resolve a compile-time constant by name and allocate it on the heap.
pub fn resolve_constant(name: &str, heap: &mut Heap) -> Option<StackValue> {
    match name {
        "PI"  => Some(StackValue::Float(std::f64::consts::PI)),
        "TAU" => Some(StackValue::Float(std::f64::consts::TAU)),
        "sdf"     => Some(heap.alloc(HeapObject::RenderMode(RenderMode::Sdf))),
        "fill"    => Some(heap.alloc(HeapObject::RenderMode(RenderMode::Fill))),
        "outline" => Some(heap.alloc(HeapObject::RenderMode(RenderMode::Outline))),
        _ => {
            lookup_color(name).map(|c| heap.alloc(HeapObject::Color { r: c.r, g: c.g, b: c.b, a: c.a }))
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

use super::util::require_float;

fn require_vec2(sv: StackValue, heap: &Heap, line: usize) -> Result<(f64, f64), RuntimeError> {
    match sv {
        StackValue::HeapRef(idx) => match heap.get(idx) {
            HeapObject::Vec2(x, y) => Ok((*x, *y)),
            _ => Err(RuntimeError::new(ErrorCode::R001, line, 0, "expected vec2")),
        },
        _ => Err(RuntimeError::new(ErrorCode::R001, line, 0, "expected vec2")),
    }
}

fn require_string(sv: StackValue, heap: &Heap, line: usize) -> Result<String, RuntimeError> {
    match sv {
        StackValue::HeapRef(idx) => match heap.get(idx) {
            HeapObject::Str(s) => Ok(s.clone()),
            _ => Err(RuntimeError::new(ErrorCode::R001, line, 0, "expected string")),
        },
        _ => Err(RuntimeError::new(ErrorCode::R001, line, 0, "expected string")),
    }
}

fn optional_color(sv: StackValue, heap: &Heap) -> Option<[f64; 4]> {
    match sv {
        StackValue::HeapRef(idx) => match heap.get(idx) {
            HeapObject::Color { r, g, b, a } => Some([*r, *g, *b, *a]),
            _ => None,
        },
        StackValue::None => None,
        _ => None,
    }
}

fn optional_render_mode(sv: StackValue, heap: &Heap) -> Option<RenderMode> {
    match sv {
        StackValue::HeapRef(idx) => match heap.get(idx) {
            HeapObject::RenderMode(rm) => Some(rm.clone()),
            _ => None,
        },
        StackValue::None => None,
        _ => None,
    }
}

fn optional_origin_str(sv: StackValue, heap: &Heap) -> Option<String> {
    match sv {
        StackValue::HeapRef(idx) => match heap.get(idx) {
            HeapObject::Str(s) => Some(s.clone()),
            _ => None,
        },
        StackValue::None => None,
        _ => None,
    }
}

// ─── Math — 1-arg ─────────────────────────────────────────────────────────────

fn native_sin(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    Ok(StackValue::Float(require_float(args[0], line)?.sin()))
}

fn native_cos(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    Ok(StackValue::Float(require_float(args[0], line)?.cos()))
}

fn native_tan(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    Ok(StackValue::Float(require_float(args[0], line)?.tan()))
}

fn native_asin(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    Ok(StackValue::Float(require_float(args[0], line)?.asin()))
}

fn native_acos(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    Ok(StackValue::Float(require_float(args[0], line)?.acos()))
}

fn native_atan(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    Ok(StackValue::Float(require_float(args[0], line)?.atan()))
}

fn native_sqrt(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    Ok(StackValue::Float(require_float(args[0], line)?.sqrt()))
}

fn native_abs(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    Ok(StackValue::Float(require_float(args[0], line)?.abs()))
}

fn native_floor(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    Ok(StackValue::Float(require_float(args[0], line)?.floor()))
}

fn native_ceil(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    Ok(StackValue::Float(require_float(args[0], line)?.ceil()))
}

fn native_round(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    Ok(StackValue::Float(require_float(args[0], line)?.round()))
}

fn native_sign(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    let x = require_float(args[0], line)?;
    let s = if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 };
    Ok(StackValue::Float(s))
}

fn native_fract(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    let x = require_float(args[0], line)?;
    Ok(StackValue::Float(x - x.floor()))
}

// ─── Math — 2-arg ─────────────────────────────────────────────────────────────

fn native_atan2(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    let y = require_float(args[0], line)?;
    let x = require_float(args[1], line)?;
    Ok(StackValue::Float(y.atan2(x)))
}

fn native_pow(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    let base = require_float(args[0], line)?;
    let exp  = require_float(args[1], line)?;
    Ok(StackValue::Float(base.powf(exp)))
}

fn native_min(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    let a = require_float(args[0], line)?;
    let b = require_float(args[1], line)?;
    Ok(StackValue::Float(a.min(b)))
}

fn native_max(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    let a = require_float(args[0], line)?;
    let b = require_float(args[1], line)?;
    Ok(StackValue::Float(a.max(b)))
}

// ─── Math — 3-arg ─────────────────────────────────────────────────────────────

fn native_clamp(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    let v   = require_float(args[0], line)?;
    let lo  = require_float(args[1], line)?;
    let hi  = require_float(args[2], line)?;
    Ok(StackValue::Float(v.clamp(lo, hi)))
}

fn native_lerp(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    let a = require_float(args[0], line)?;
    let b = require_float(args[1], line)?;
    let t = require_float(args[2], line)?;
    Ok(StackValue::Float(a + (b - a) * t))
}

// ─── Constructors ─────────────────────────────────────────────────────────────

fn native_vec2(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let x = require_float(args[0], line)?;
    let y = require_float(args[1], line)?;
    Ok(heap.alloc(HeapObject::Vec2(x, y)))
}

fn native_vec3(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let x = require_float(args[0], line)?;
    let y = require_float(args[1], line)?;
    let z = require_float(args[2], line)?;
    Ok(heap.alloc(HeapObject::Vec3(x, y, z)))
}

fn native_vec4(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let x = require_float(args[0], line)?;
    let y = require_float(args[1], line)?;
    let z = require_float(args[2], line)?;
    let w = require_float(args[3], line)?;
    Ok(heap.alloc(HeapObject::Vec4(x, y, z, w)))
}

fn native_color(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let r = require_float(args[0], line)?;
    let g = require_float(args[1], line)?;
    let b = require_float(args[2], line)?;
    let a = if args.len() == 4 { require_float(args[3], line)? } else { 1.0 };
    Ok(heap.alloc(HeapObject::Color { r, g, b, a }))
}

fn native_transform(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, _line: usize) -> Result<StackValue, RuntimeError> {
    let _ = args;
    Ok(heap.alloc(HeapObject::Transform(TransformData::default())))
}

fn native_mat3(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, _line: usize) -> Result<StackValue, RuntimeError> {
    let _ = args;
    Ok(heap.alloc(HeapObject::Mat3(Box::new(m3_identity()))))
}

fn native_mat4(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, _line: usize) -> Result<StackValue, RuntimeError> {
    let _ = args;
    Ok(heap.alloc(HeapObject::Mat4(Box::new(m4_identity()))))
}

fn native_mat3_translate(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let x = require_float(args[0], line)?;
    let y = require_float(args[1], line)?;
    Ok(heap.alloc(HeapObject::Mat3(Box::new(m3_translate2d(x, y)))))
}

fn native_mat3_rotate(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let deg = require_float(args[0], line)?;
    Ok(heap.alloc(HeapObject::Mat3(Box::new(m3_rotate2d(deg.to_radians())))))
}

fn native_mat3_scale(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let sx = require_float(args[0], line)?;
    let sy = require_float(args[1], line)?;
    Ok(heap.alloc(HeapObject::Mat3(Box::new(m3_scale2d(sx, sy)))))
}

fn native_mat4_translate(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let x = require_float(args[0], line)?;
    let y = require_float(args[1], line)?;
    let z = require_float(args[2], line)?;
    Ok(heap.alloc(HeapObject::Mat4(Box::new(m4_translate(x, y, z)))))
}

fn native_mat4_scale(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let sx = require_float(args[0], line)?;
    let sy = require_float(args[1], line)?;
    let sz = require_float(args[2], line)?;
    Ok(heap.alloc(HeapObject::Mat4(Box::new(m4_scale_xyz(sx, sy, sz)))))
}

fn native_mat4_rotate_x(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let deg = require_float(args[0], line)?;
    Ok(heap.alloc(HeapObject::Mat4(Box::new(m4_rotate_x(deg.to_radians())))))
}

fn native_mat4_rotate_y(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let deg = require_float(args[0], line)?;
    Ok(heap.alloc(HeapObject::Mat4(Box::new(m4_rotate_y(deg.to_radians())))))
}

fn native_mat4_rotate_z(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let deg = require_float(args[0], line)?;
    Ok(heap.alloc(HeapObject::Mat4(Box::new(m4_rotate_z(deg.to_radians())))))
}

// ─── Result helpers ───────────────────────────────────────────────────────────

fn native_ok(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, _line: usize) -> Result<StackValue, RuntimeError> {
    let inner = args[0];
    Ok(heap.alloc(HeapObject::ResOk(inner)))
}

fn native_error(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    // Accept any value — convert to string representation.
    let msg = match args[0] {
        StackValue::HeapRef(idx) => match heap.get(idx) {
            HeapObject::Str(s) => s.clone(),
            _ => display(args[0], heap),
        },
        sv => display(sv, heap),
    };
    let _ = line;
    Ok(heap.alloc(HeapObject::ResErr(msg)))
}

// ─── Shape constructors ───────────────────────────────────────────────────────

/// circle(center, radius [, color, render])
fn native_circle(heap: &mut Heap, args: &[StackValue], cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    // Copy data out of heap before allocating.
    let center = require_vec2(args[0], heap, line)?;
    let radius  = require_float(args[1], line)?;
    let color   = if args.len() > 2 { optional_color(args[2], heap) } else { None };
    let render  = if args.len() > 3 { optional_render_mode(args[3], heap) } else { None };

    let render_mode = render.unwrap_or(RenderMode::Sdf);
    let mut sd = ShapeData::new(ShapeDesc::Circle { center, radius }, render_mode, cm.clone());
    if let Some(c) = color {
        sd.color = c;
    }
    Ok(heap.alloc(HeapObject::Shape(sd)))
}

/// rect(center, size [, color, render, origin])
fn native_rect(heap: &mut Heap, args: &[StackValue], cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let center = require_vec2(args[0], heap, line)?;
    let size   = require_vec2(args[1], heap, line)?;
    let color  = if args.len() > 2 { optional_color(args[2], heap) } else { None };
    let render = if args.len() > 3 { optional_render_mode(args[3], heap) } else { None };
    let origin_str = if args.len() > 4 { optional_origin_str(args[4], heap) } else { None };

    let origin = match origin_str {
        Some(s) => Origin::from_str(&s).map_err(|e| {
            RuntimeError::new(ErrorCode::R011, line, 0, e)
        })?,
        None => Origin::Center,
    };
    let render_mode = render.unwrap_or(RenderMode::Sdf);
    let mut sd = ShapeData::new(ShapeDesc::Rect { center, size, origin }, render_mode, cm.clone());
    if let Some(c) = color {
        sd.color = c;
    }
    Ok(heap.alloc(HeapObject::Shape(sd)))
}

/// line(from, to [, color, render])
fn native_line(heap: &mut Heap, args: &[StackValue], cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let from   = require_vec2(args[0], heap, line)?;
    let to     = require_vec2(args[1], heap, line)?;
    let color  = if args.len() > 2 { optional_color(args[2], heap) } else { None };
    let render = if args.len() > 3 { optional_render_mode(args[3], heap) } else { None };

    let render_mode = render.unwrap_or(RenderMode::Sdf);
    let mut sd = ShapeData::new(ShapeDesc::Line { from, to }, render_mode, cm.clone());
    if let Some(c) = color {
        sd.color = c;
    }
    Ok(heap.alloc(HeapObject::Shape(sd)))
}

/// polygon(vertices [, color, render])
/// vertices is a `HeapRef` to a List whose items are `HeapRefs` to Vec2.
fn native_polygon(heap: &mut Heap, args: &[StackValue], cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    // Extract vertex list first (snapshot before mutating heap).
    let verts = extract_vec2_list(args[0], heap, line)?;
    if verts.len() < 3 {
        return Err(RuntimeError::new(ErrorCode::R008, line, 0,
            format!("polygon requires at least 3 vertices, got {}", verts.len())));
    }
    let color  = if args.len() > 1 { optional_color(args[1], heap) } else { None };
    let render = if args.len() > 2 { optional_render_mode(args[2], heap) } else { None };

    let render_mode = render.unwrap_or(RenderMode::Sdf);
    let mut sd = ShapeData::new(ShapeDesc::Polygon(verts), render_mode, cm.clone());
    if let Some(c) = color {
        sd.color = c;
    }
    Ok(heap.alloc(HeapObject::Shape(sd)))
}

/// text(pos, content, size [, color, render])
fn native_text(heap: &mut Heap, args: &[StackValue], cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let pos     = require_vec2(args[0], heap, line)?;
    let content = require_string(args[1], heap, line)?;
    let size    = require_float(args[2], line)?;
    let color   = if args.len() > 3 { optional_color(args[3], heap) } else { None };
    let render  = if args.len() > 4 { optional_render_mode(args[4], heap) } else { None };

    let render_mode = render.unwrap_or(RenderMode::Sdf);
    let mut sd = ShapeData::new(ShapeDesc::Text { pos, content, size }, render_mode, cm.clone());
    if let Some(c) = color {
        sd.color = c;
    }
    Ok(heap.alloc(HeapObject::Shape(sd)))
}

/// Extract a `Vec<(f64, f64)>` from a `StackValue` that must be a `HeapRef` to a `List`
/// where each item is a `HeapRef` to `Vec2`.
fn extract_vec2_list(sv: StackValue, heap: &Heap, line: usize) -> Result<Vec<(f64, f64)>, RuntimeError> {
    let idx = match sv {
        StackValue::HeapRef(i) => i,
        _ => return Err(RuntimeError::new(ErrorCode::R001, line, 0, "expected list of vec2")),
    };
    let items: Vec<StackValue> = match heap.get(idx) {
        HeapObject::List(v) => v.clone(),
        _ => return Err(RuntimeError::new(ErrorCode::R001, line, 0, "expected list of vec2")),
    };
    items.iter().map(|&item| require_vec2(item, heap, line)).collect()
}

// ─── Render ───────────────────────────────────────────────────────────────────

fn native_stroke(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let width = require_float(args[0], line)?;
    Ok(heap.alloc(HeapObject::RenderMode(RenderMode::Stroke(width))))
}

// ─── Coords ───────────────────────────────────────────────────────────────────

fn native_resolution(heap: &mut Heap, args: &[StackValue], cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let _ = heap;
    let w = require_float(args[0], line)?;
    let h = require_float(args[1], line)?;
    cm.px_width  = w;
    cm.px_height = h;
    Ok(StackValue::Float(0.0))
}

fn native_origin(heap: &mut Heap, args: &[StackValue], cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let s = require_string(args[0], heap, line)?;
    let origin = Origin::from_str(&s).map_err(|e| {
        RuntimeError::new(ErrorCode::R011, line, 0, e)
    })?;
    cm.origin = origin;
    Ok(StackValue::Float(0.0))
}

// ─── File I/O ─────────────────────────────────────────────────────────────────

fn native_file_read(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let path = require_string(args[0], heap, line)?;
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let sv = heap.alloc(HeapObject::Str(contents));
            Ok(heap.alloc(HeapObject::ResOk(sv)))
        }
        Err(e) => Ok(heap.alloc(HeapObject::ResErr(e.to_string()))),
    }
}

fn native_file_read_lines(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let path = require_string(args[0], heap, line)?;
    match std::fs::read_to_string(&path) {
        Ok(contents) => {
            let lines: Vec<StackValue> = contents
                .lines()
                .map(|l| heap.alloc(HeapObject::Str(l.to_owned())))
                .collect();
            let list_sv = heap.alloc(HeapObject::List(lines));
            Ok(heap.alloc(HeapObject::ResOk(list_sv)))
        }
        Err(e) => Ok(heap.alloc(HeapObject::ResErr(e.to_string()))),
    }
}

fn native_file_write(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let path    = require_string(args[0], heap, line)?;
    let content = require_string(args[1], heap, line)?;
    match std::fs::write(&path, content.as_bytes()) {
        Ok(()) => Ok(heap.alloc(HeapObject::ResOk(StackValue::Bool(true)))),
        Err(e) => Ok(heap.alloc(HeapObject::ResErr(e.to_string()))),
    }
}

fn native_file_append(heap: &mut Heap, args: &[StackValue], _cm: &mut CoordMeta, line: usize) -> Result<StackValue, RuntimeError> {
    let path    = require_string(args[0], heap, line)?;
    let content = require_string(args[1], heap, line)?;
    let result = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| f.write_all(content.as_bytes()));
    match result {
        Ok(()) => Ok(heap.alloc(HeapObject::ResOk(StackValue::Bool(true)))),
        Err(e) => Ok(heap.alloc(HeapObject::ResErr(e.to_string()))),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::heap::Heap;
    use crate::vm::value::HeapObject;
    use std::f64::consts::PI;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn cm() -> CoordMeta { CoordMeta::default() }

    fn call1(f: NativeFuncImpl, a: StackValue) -> Result<StackValue, RuntimeError> {
        let mut h = Heap::new();
        let mut cm = cm();
        f(&mut h, &[a], &mut cm, 1)
    }

    fn call1h(heap: &mut Heap, f: NativeFuncImpl, a: StackValue) -> Result<StackValue, RuntimeError> {
        let mut cm = cm();
        f(heap, &[a], &mut cm, 1)
    }

    fn call2(heap: &mut Heap, f: NativeFuncImpl, a: StackValue, b: StackValue) -> Result<StackValue, RuntimeError> {
        let mut cm = cm();
        f(heap, &[a, b], &mut cm, 1)
    }

    fn call3(heap: &mut Heap, f: NativeFuncImpl, a: StackValue, b: StackValue, c: StackValue) -> Result<StackValue, RuntimeError> {
        let mut cm_val = cm();
        f(heap, &[a, b, c], &mut cm_val, 1)
    }

    fn float(f: f64) -> StackValue { StackValue::Float(f) }
    fn as_float(sv: StackValue) -> f64 { sv.as_float().expect("expected Float") }

    fn alloc_vec2(heap: &mut Heap, x: f64, y: f64) -> StackValue {
        heap.alloc(HeapObject::Vec2(x, y))
    }

    fn alloc_str(heap: &mut Heap, s: &str) -> StackValue {
        heap.alloc(HeapObject::Str(s.to_owned()))
    }

    // ── Math 1-arg ───────────────────────────────────────────────────────────

    #[test]
    fn math_sin_zero() {
        assert!((as_float(call1(native_sin, float(0.0)).unwrap()) - 0.0).abs() < 1e-12);
    }

    #[test]
    fn math_cos_zero() {
        assert!((as_float(call1(native_cos, float(0.0)).unwrap()) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn math_sqrt_four() {
        assert!((as_float(call1(native_sqrt, float(4.0)).unwrap()) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn math_abs_neg() {
        assert!((as_float(call1(native_abs, float(-5.0)).unwrap()) - 5.0).abs() < 1e-12);
    }

    #[test]
    fn math_floor() {
        assert_eq!(as_float(call1(native_floor, float(2.7)).unwrap()), 2.0);
    }

    #[test]
    fn math_ceil() {
        assert_eq!(as_float(call1(native_ceil, float(2.1)).unwrap()), 3.0);
    }

    #[test]
    fn math_round() {
        assert_eq!(as_float(call1(native_round, float(2.5)).unwrap()), 3.0);
    }

    #[test]
    fn math_sign() {
        assert_eq!(as_float(call1(native_sign, float(-3.0)).unwrap()), -1.0);
        assert_eq!(as_float(call1(native_sign, float(0.0)).unwrap()),   0.0);
        assert_eq!(as_float(call1(native_sign, float(5.0)).unwrap()),   1.0);
    }

    #[test]
    fn math_fract() {
        let v = as_float(call1(native_fract, float(2.7)).unwrap());
        assert!((v - 0.7).abs() < 1e-10);
    }

    // ── Math 2-arg ───────────────────────────────────────────────────────────

    #[test]
    fn math_atan2() {
        let mut h = Heap::new();
        let v = as_float(call2(&mut h, native_atan2, float(1.0), float(0.0)).unwrap());
        assert!((v - PI / 2.0).abs() < 1e-12);
    }

    #[test]
    fn math_pow() {
        let mut h = Heap::new();
        assert_eq!(as_float(call2(&mut h, native_pow, float(2.0), float(3.0)).unwrap()), 8.0);
    }

    #[test]
    fn math_min() {
        let mut h = Heap::new();
        assert_eq!(as_float(call2(&mut h, native_min, float(3.0), float(5.0)).unwrap()), 3.0);
    }

    #[test]
    fn math_max() {
        let mut h = Heap::new();
        assert_eq!(as_float(call2(&mut h, native_max, float(3.0), float(5.0)).unwrap()), 5.0);
    }

    // ── Math 3-arg ───────────────────────────────────────────────────────────

    #[test]
    fn math_clamp() {
        let mut h = Heap::new();
        assert_eq!(as_float(call3(&mut h, native_clamp, float(10.0), float(0.0), float(5.0)).unwrap()), 5.0);
        assert_eq!(as_float(call3(&mut h, native_clamp, float(-1.0), float(0.0), float(5.0)).unwrap()), 0.0);
        assert_eq!(as_float(call3(&mut h, native_clamp, float(3.0),  float(0.0), float(5.0)).unwrap()), 3.0);
    }

    #[test]
    fn math_lerp() {
        let mut h = Heap::new();
        assert_eq!(as_float(call3(&mut h, native_lerp, float(0.0), float(10.0), float(0.5)).unwrap()), 5.0);
        assert_eq!(as_float(call3(&mut h, native_lerp, float(0.0), float(10.0), float(0.0)).unwrap()), 0.0);
        assert_eq!(as_float(call3(&mut h, native_lerp, float(0.0), float(10.0), float(1.0)).unwrap()), 10.0);
    }

    // ── Constructors ─────────────────────────────────────────────────────────

    #[test]
    fn ctor_vec2() {
        let mut h = Heap::new();
        let sv = call2(&mut h, native_vec2, float(1.0), float(2.0)).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        assert!(matches!(h.get(idx), HeapObject::Vec2(x, y) if *x == 1.0 && *y == 2.0));
    }

    #[test]
    fn ctor_vec3() {
        let mut h = Heap::new();
        let mut cm = cm();
        let sv = native_vec3(&mut h, &[float(1.0), float(2.0), float(3.0)], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        assert!(matches!(h.get(idx), HeapObject::Vec3(1.0, 2.0, 3.0)));
    }

    #[test]
    fn ctor_vec4() {
        let mut h = Heap::new();
        let mut cm = cm();
        let sv = native_vec4(&mut h, &[float(1.0), float(2.0), float(3.0), float(4.0)], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        assert!(matches!(h.get(idx), HeapObject::Vec4(1.0, 2.0, 3.0, 4.0)));
    }

    #[test]
    fn ctor_color_three_args_alpha_one() {
        let mut h = Heap::new();
        let mut cm = cm();
        let sv = native_color(&mut h, &[float(0.5), float(0.6), float(0.7)], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        match h.get(idx) {
            HeapObject::Color { r, g, b, a } => {
                assert!((r - 0.5).abs() < 1e-12);
                assert!((g - 0.6).abs() < 1e-12);
                assert!((b - 0.7).abs() < 1e-12);
                assert!((a - 1.0).abs() < 1e-12);
            }
            _ => panic!("not a color"),
        }
    }

    #[test]
    fn ctor_color_four_args() {
        let mut h = Heap::new();
        let mut cm = cm();
        let sv = native_color(&mut h, &[float(0.1), float(0.2), float(0.3), float(0.4)], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        match h.get(idx) {
            HeapObject::Color { r: _, g: _, b: _, a } => {
                assert!((a - 0.4).abs() < 1e-12);
            }
            _ => panic!("not a color"),
        }
    }

    #[test]
    fn ctor_transform_default() {
        let mut h = Heap::new();
        let mut cm = cm();
        let sv = native_transform(&mut h, &[], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        assert!(matches!(h.get(idx), HeapObject::Transform(_)));
    }

    #[test]
    fn ctor_mat3_identity() {
        let mut h = Heap::new();
        let mut cm = cm();
        let sv = native_mat3(&mut h, &[], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        match h.get(idx) {
            HeapObject::Mat3(m) => {
                let ident = m3_identity();
                assert_eq!(m.as_ref(), &ident);
            }
            _ => panic!("not mat3"),
        }
    }

    #[test]
    fn ctor_mat4_identity() {
        let mut h = Heap::new();
        let mut cm = cm();
        let sv = native_mat4(&mut h, &[], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        assert!(matches!(h.get(idx), HeapObject::Mat4(_)));
    }

    #[test]
    fn ctor_mat3_translate() {
        let mut h = Heap::new();
        let mut cm = cm();
        let sv = native_mat3_translate(&mut h, &[float(3.0), float(4.0)], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        match h.get(idx) {
            HeapObject::Mat3(m) => {
                let expected = m3_translate2d(3.0, 4.0);
                assert_eq!(m.as_ref(), &expected);
            }
            _ => panic!("not mat3"),
        }
    }

    #[test]
    fn ctor_mat3_rotate_zero() {
        let mut h = Heap::new();
        let mut cm = cm();
        let sv = native_mat3_rotate(&mut h, &[float(0.0)], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        match h.get(idx) {
            HeapObject::Mat3(m) => {
                // 0 degrees → identity
                let ident = m3_identity();
                for (a, b) in m.iter().zip(ident.iter()) {
                    assert!((a - b).abs() < 1e-12, "differ at element: {a} vs {b}");
                }
            }
            _ => panic!("not mat3"),
        }
    }

    // ── Result helpers ───────────────────────────────────────────────────────

    #[test]
    fn result_ok() {
        let mut h = Heap::new();
        let mut cm = cm();
        let sv = native_ok(&mut h, &[float(5.0)], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        assert!(matches!(h.get(idx), HeapObject::ResOk(StackValue::Float(f)) if *f == 5.0));
    }

    #[test]
    fn result_error() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "bad");
        let mut cm = cm();
        let sv = native_error(&mut h, &[s], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        assert!(matches!(h.get(idx), HeapObject::ResErr(msg) if msg == "bad"));
    }

    // ── Shape constructors ───────────────────────────────────────────────────

    #[test]
    fn shape_circle_required_only() {
        let mut h = Heap::new();
        let center = alloc_vec2(&mut h, 0.0, 0.0);
        let mut cm = cm();
        let sv = native_circle(&mut h, &[center, float(10.0)], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        match h.get(idx) {
            HeapObject::Shape(sd) => {
                assert!(matches!(sd.desc, ShapeDesc::Circle { radius, .. } if radius == 10.0));
                assert!(matches!(sd.render_mode, RenderMode::Sdf));
            }
            _ => panic!("not a shape"),
        }
    }

    #[test]
    fn shape_circle_with_color_and_render() {
        let mut h = Heap::new();
        let center = alloc_vec2(&mut h, 5.0, 5.0);
        let color = h.alloc(HeapObject::Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 });
        let render = h.alloc(HeapObject::RenderMode(RenderMode::Fill));
        let mut cm = cm();
        let sv = native_circle(&mut h, &[center, float(20.0), color, render], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        match h.get(idx) {
            HeapObject::Shape(sd) => {
                assert_eq!(sd.color, [1.0, 0.0, 0.0, 1.0]);
                assert!(matches!(sd.render_mode, RenderMode::Fill));
            }
            _ => panic!("not a shape"),
        }
    }

    #[test]
    fn shape_rect_with_origin() {
        let mut h = Heap::new();
        let center = alloc_vec2(&mut h, 0.0, 0.0);
        let size   = alloc_vec2(&mut h, 100.0, 50.0);
        let orig_s = alloc_str(&mut h, "top_left");
        let mut cm = cm();
        let sv = native_rect(&mut h, &[center, size, StackValue::None, StackValue::None, orig_s], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        match h.get(idx) {
            HeapObject::Shape(sd) => {
                assert!(matches!(sd.desc, ShapeDesc::Rect { origin: Origin::TopLeft, .. }));
            }
            _ => panic!("not a shape"),
        }
    }

    #[test]
    fn shape_line() {
        let mut h = Heap::new();
        let from = alloc_vec2(&mut h, 0.0, 0.0);
        let to   = alloc_vec2(&mut h, 1.0, 1.0);
        let mut cm = cm();
        let sv = native_line(&mut h, &[from, to], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        assert!(matches!(h.get(idx), HeapObject::Shape(_)));
    }

    #[test]
    fn shape_polygon_three_verts() {
        let mut h = Heap::new();
        let v0 = alloc_vec2(&mut h, 0.0, 0.0);
        let v1 = alloc_vec2(&mut h, 1.0, 0.0);
        let v2 = alloc_vec2(&mut h, 0.5, 1.0);
        let list = h.alloc(HeapObject::List(vec![v0, v1, v2]));
        let mut cm = cm();
        let sv = native_polygon(&mut h, &[list], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        match h.get(idx) {
            HeapObject::Shape(sd) => {
                assert!(matches!(&sd.desc, ShapeDesc::Polygon(pts) if pts.len() == 3));
            }
            _ => panic!("not a shape"),
        }
    }

    #[test]
    fn shape_polygon_too_few_verts_error() {
        let mut h = Heap::new();
        let v0 = alloc_vec2(&mut h, 0.0, 0.0);
        let v1 = alloc_vec2(&mut h, 1.0, 0.0);
        let list = h.alloc(HeapObject::List(vec![v0, v1]));
        let mut cm = cm();
        let result = native_polygon(&mut h, &[list], &mut cm, 1);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code, ErrorCode::R008);
    }

    #[test]
    fn shape_text() {
        let mut h = Heap::new();
        let pos     = alloc_vec2(&mut h, 10.0, 20.0);
        let content = alloc_str(&mut h, "hello");
        let mut cm = cm();
        let sv = native_text(&mut h, &[pos, content, float(16.0)], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        match h.get(idx) {
            HeapObject::Shape(sd) => {
                assert!(matches!(&sd.desc, ShapeDesc::Text { size, .. } if *size == 16.0));
            }
            _ => panic!("not a shape"),
        }
    }

    // ── Render ───────────────────────────────────────────────────────────────

    #[test]
    fn render_stroke() {
        let mut h = Heap::new();
        let sv = call1h(&mut h, native_stroke, float(2.0)).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        assert!(matches!(h.get(idx), HeapObject::RenderMode(RenderMode::Stroke(w)) if *w == 2.0));
    }

    // ── Coords ───────────────────────────────────────────────────────────────

    #[test]
    fn coords_resolution_sets_cm() {
        let mut h = Heap::new();
        let mut cm = CoordMeta::default();
        native_resolution(&mut h, &[float(800.0), float(600.0)], &mut cm, 1).unwrap();
        assert_eq!(cm.px_width, 800.0);
        assert_eq!(cm.px_height, 600.0);
    }

    #[test]
    fn coords_origin_sets_cm() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "top_left");
        let mut cm = CoordMeta::default();
        native_origin(&mut h, &[s], &mut cm, 1).unwrap();
        assert!(matches!(cm.origin, Origin::TopLeft));
    }

    #[test]
    fn coords_origin_invalid_error() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "invalid_origin");
        let mut cm = CoordMeta::default();
        let result = native_origin(&mut h, &[s], &mut cm, 1);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, ErrorCode::R011);
    }

    // ── File I/O ─────────────────────────────────────────────────────────────

    #[test]
    fn file_read_nonexistent_is_res_err() {
        let mut h = Heap::new();
        let s = alloc_str(&mut h, "/tmp/__rustle_nonexistent_file_xyz__.txt");
        let mut cm = cm();
        let sv = native_file_read(&mut h, &[s], &mut cm, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        assert!(matches!(h.get(idx), HeapObject::ResErr(_)));
    }

    #[test]
    fn file_write_and_read_roundtrip() {
        let path = "/tmp/__rustle_test_write_read__.txt";
        let mut h = Heap::new();
        let mut cm_val = cm();

        // Write
        let ps = alloc_str(&mut h, path);
        let cs = alloc_str(&mut h, "hello rustle");
        let sv = native_file_write(&mut h, &[ps, cs], &mut cm_val, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        assert!(matches!(h.get(idx), HeapObject::ResOk(_)));

        // Read back
        let ps2 = alloc_str(&mut h, path);
        let sv2 = native_file_read(&mut h, &[ps2], &mut cm_val, 1).unwrap();
        let idx2 = sv2.as_heap_ref().unwrap();
        match h.get(idx2) {
            HeapObject::ResOk(inner) => {
                let inner_idx = inner.as_heap_ref().unwrap();
                assert!(matches!(h.get(inner_idx), HeapObject::Str(s) if s == "hello rustle"));
            }
            other => panic!("expected ResOk, got {:?}", other),
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn file_read_lines() {
        let path = "/tmp/__rustle_test_read_lines__.txt";
        std::fs::write(path, "line1\nline2\nline3").unwrap();

        let mut h = Heap::new();
        let mut cm_val = cm();
        let s = alloc_str(&mut h, path);
        let sv = native_file_read_lines(&mut h, &[s], &mut cm_val, 1).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        match h.get(idx) {
            HeapObject::ResOk(inner) => {
                let list_idx = inner.as_heap_ref().unwrap();
                match h.get(list_idx) {
                    HeapObject::List(items) => assert_eq!(items.len(), 3),
                    other => panic!("expected list, got {:?}", other),
                }
            }
            other => panic!("expected ResOk, got {:?}", other),
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn file_append() {
        let path = "/tmp/__rustle_test_append__.txt";
        let _ = std::fs::remove_file(path);

        let mut h = Heap::new();
        let mut cm_val = cm();

        let p1 = alloc_str(&mut h, path);
        let c1 = alloc_str(&mut h, "first");
        native_file_append(&mut h, &[p1, c1], &mut cm_val, 1).unwrap();

        let p2 = alloc_str(&mut h, path);
        let c2 = alloc_str(&mut h, "_second");
        native_file_append(&mut h, &[p2, c2], &mut cm_val, 1).unwrap();

        let contents = std::fs::read_to_string(path).unwrap();
        assert_eq!(contents, "first_second");

        let _ = std::fs::remove_file(path);
    }

    // ── Constants ────────────────────────────────────────────────────────────

    #[test]
    fn constant_pi() {
        let mut h = Heap::new();
        match resolve_constant("PI", &mut h) {
            Some(StackValue::Float(f)) => assert!((f - PI).abs() < 1e-12),
            _ => panic!("expected Float(PI)"),
        }
    }

    #[test]
    fn constant_tau() {
        let mut h = Heap::new();
        match resolve_constant("TAU", &mut h) {
            Some(StackValue::Float(f)) => assert!((f - std::f64::consts::TAU).abs() < 1e-12),
            _ => panic!("expected Float(TAU)"),
        }
    }

    #[test]
    fn constant_red() {
        let mut h = Heap::new();
        let sv = resolve_constant("red", &mut h).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        assert!(matches!(h.get(idx),
            HeapObject::Color { r, g, b, a }
            if *r == 1.0 && *g == 0.0 && *b == 0.0 && *a == 1.0
        ));
    }

    #[test]
    fn constant_sdf() {
        let mut h = Heap::new();
        let sv = resolve_constant("sdf", &mut h).unwrap();
        let idx = sv.as_heap_ref().unwrap();
        assert!(matches!(h.get(idx), HeapObject::RenderMode(RenderMode::Sdf)));
    }

    #[test]
    fn constant_unknown_none() {
        let mut h = Heap::new();
        assert!(resolve_constant("unknown_thing", &mut h).is_none());
    }

    // ── native_table / index_by_name ─────────────────────────────────────────

    #[test]
    fn table_has_expected_names() {
        let t = native_table();
        let names: Vec<&str> = t.iter().map(|f| f.name).collect();
        for expected in &["sin", "cos", "vec2", "color", "circle", "rect", "resolution", "file.read"] {
            assert!(names.contains(expected), "missing: {}", expected);
        }
    }

    #[test]
    fn index_by_name_found_and_not_found() {
        let t = native_table();
        assert!(native_index_by_name(&t, "sin").is_some());
        assert!(native_index_by_name(&t, "nonexistent_fn_xyz").is_none());
    }

    #[test]
    fn indices_are_unique() {
        let t = native_table();
        let mut names = std::collections::HashSet::new();
        for f in &t {
            assert!(names.insert(f.name), "duplicate name: {}", f.name);
        }
    }
}
