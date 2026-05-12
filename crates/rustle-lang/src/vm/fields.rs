#![expect(clippy::cast_precision_loss, reason = "list lengths fit in f64 mantissa for practical sizes")]
use super::value::{HeapObject, StackValue};
use super::heap::Heap;
use super::util::require_float;
use super::CompiledStructDef;
use crate::error::{ErrorCode, RuntimeError};
use crate::types::draw::ShapeDesc;

fn field_not_found(field: &str, type_name: &str, available: &[&str], line: usize) -> RuntimeError {
    RuntimeError::new(
        ErrorCode::R003,
        line,
        0,
        format!(
            "field '{}' not found on {}; available: {}",
            field,
            type_name,
            available.join(", ")
        ),
    )
}

// ─── get_field ───────────────────────────────────────────────────────────────

/// Read a field from the heap object at `idx`.
///
/// For value types (Vec2, Vec3, Vec4, Color) this returns a copy of the field.
/// For types whose field value is a heap object (e.g. `circle.center` → Vec2),
/// a new heap object is allocated and its `HeapRef` is returned.
pub fn get_field(
    heap: &mut Heap,
    idx: u32,
    field: &str,
    state_fields: &[String],
    struct_defs: &[CompiledStructDef],
    line: usize,
) -> Result<StackValue, RuntimeError> {
    // Phase 1: immutable borrow — handle non-allocating cases and extract
    // data for allocating cases. This avoids cloning the entire HeapObject.
    enum NeedsAlloc {
        Vec2(f64, f64),
        Str(String),
    }
    let needs_alloc: Option<NeedsAlloc> = {
        let obj = heap.get(idx);
        match obj {
            HeapObject::Vec2(x, y) => return match field {
                "x" => Ok(StackValue::Float(*x)),
                "y" => Ok(StackValue::Float(*y)),
                _ => Err(field_not_found(field, "vec2", &["x", "y"], line)),
            },
            HeapObject::Vec3(x, y, z) => return match field {
                "x" => Ok(StackValue::Float(*x)),
                "y" => Ok(StackValue::Float(*y)),
                "z" => Ok(StackValue::Float(*z)),
                _ => Err(field_not_found(field, "vec3", &["x", "y", "z"], line)),
            },
            HeapObject::Vec4(x, y, z, w) => return match field {
                "x" => Ok(StackValue::Float(*x)),
                "y" => Ok(StackValue::Float(*y)),
                "z" => Ok(StackValue::Float(*z)),
                "w" => Ok(StackValue::Float(*w)),
                _ => Err(field_not_found(field, "vec4", &["x", "y", "z", "w"], line)),
            },
            HeapObject::Color { r, g, b, a } => return match field {
                "r" => Ok(StackValue::Float(*r)),
                "g" => Ok(StackValue::Float(*g)),
                "b" => Ok(StackValue::Float(*b)),
                "a" => Ok(StackValue::Float(*a)),
                _ => Err(field_not_found(field, "color", &["r", "g", "b", "a"], line)),
            },
            HeapObject::Shape(sd) => match &sd.desc {
                ShapeDesc::Circle { center, radius } => match field {
                    "center" => Some(NeedsAlloc::Vec2(center.0, center.1)),
                    "radius" => return Ok(StackValue::Float(*radius)),
                    _ => return Err(field_not_found(field, "circle", &["center", "radius"], line)),
                },
                ShapeDesc::Rect { center, size, .. } => match field {
                    "center" => Some(NeedsAlloc::Vec2(center.0, center.1)),
                    "size"   => Some(NeedsAlloc::Vec2(size.0, size.1)),
                    _ => return Err(field_not_found(field, "rect", &["center", "size"], line)),
                },
                ShapeDesc::Line { from, to } => match field {
                    "from" => Some(NeedsAlloc::Vec2(from.0, from.1)),
                    "to"   => Some(NeedsAlloc::Vec2(to.0, to.1)),
                    _ => return Err(field_not_found(field, "line", &["from", "to"], line)),
                },
                ShapeDesc::Polygon(_) => return Err(field_not_found(field, "polygon", &[], line)),
                ShapeDesc::Text { pos, content, size } => match field {
                    "pos"     => Some(NeedsAlloc::Vec2(pos.0, pos.1)),
                    "content" => Some(NeedsAlloc::Str(content.clone())),
                    "size"    => return Ok(StackValue::Float(*size)),
                    _ => return Err(field_not_found(field, "text", &["pos", "content", "size"], line)),
                },
            },
            HeapObject::List(items) => return match field {
                "len" => Ok(StackValue::Float(items.len() as f64)),
                _ => Err(field_not_found(field, "list", &["len"], line)),
            },
            HeapObject::ResOk(inner) => match field {
                "ok"    => return Ok(StackValue::Bool(true)),
                "value" => return Ok(*inner),
                "error" => Some(NeedsAlloc::Str(String::new())),
                _ => return Err(field_not_found(field, "res", &["ok", "value", "error"], line)),
            },
            HeapObject::ResErr(msg) => match field {
                "ok"    => return Ok(StackValue::Bool(false)),
                "value" => return Err(RuntimeError::new(
                    ErrorCode::R003, line, 0,
                    "cannot access .value on failed result",
                )),
                "error" => Some(NeedsAlloc::Str(msg.clone())),
                _ => return Err(field_not_found(field, "res", &["ok", "error"], line)),
            },
            HeapObject::Input(io) => match field {
                "dt"             => return Ok(StackValue::Float(io.dt)),
                "mouse_x"        => return Ok(StackValue::Float(io.mouse_x)),
                "mouse_y"        => return Ok(StackValue::Float(io.mouse_y)),
                "mouse_down"     => return Ok(StackValue::Bool(io.mouse_down)),
                "mouse_pressed"  => return Ok(StackValue::Bool(io.mouse_pressed)),
                "mouse_released" => return Ok(StackValue::Bool(io.mouse_released)),
                "key_pressed"    => Some(NeedsAlloc::Str(io.key_pressed.clone())),
                "key_down"       => Some(NeedsAlloc::Str(io.key_down.clone())),
                "key_released"   => Some(NeedsAlloc::Str(io.key_released.clone())),
                _ => return Err(field_not_found(
                    field, "input",
                    &["dt", "mouse_x", "mouse_y", "mouse_down", "mouse_pressed",
                      "mouse_released", "key_pressed", "key_down", "key_released"],
                    line,
                )),
            },
            HeapObject::State(state) => {
                let slot = state_fields.iter().position(|n| n == field)
                    .ok_or_else(|| {
                        let available: Vec<&str> = state_fields.iter().map(String::as_str).collect();
                        RuntimeError::new(ErrorCode::R003, line, 0,
                            format!("field '{}' not found on state; available: {}", field, available.join(", ")))
                    })?;
                return Ok(state.fields[slot]);
            }
            HeapObject::Object(obj) => {
                let def = struct_defs.get(obj.struct_def_idx as usize)
                    .ok_or_else(|| RuntimeError::new(ErrorCode::R003, line, 0,
                        format!("invalid struct_def_idx {} for type '{}'", obj.struct_def_idx, obj.type_name)))?;
                let slot = def.field_names.iter().position(|n| n == field)
                    .ok_or_else(|| {
                        let available: Vec<&str> = def.field_names.iter().map(String::as_str).collect();
                        RuntimeError::new(ErrorCode::R003, line, 0,
                            format!("field '{}' not found on {}; available: {}", field, obj.type_name, available.join(", ")))
                    })?;
                return Ok(obj.fields[slot]);
            }
            HeapObject::EnumVariant(ev) => {
                let slot = ev.field_names.iter().position(|n| n == field)
                    .ok_or_else(|| {
                        let available: Vec<&str> = ev.field_names.iter().map(String::as_str).collect();
                        RuntimeError::new(ErrorCode::R003, line, 0,
                            format!("field '{}' not found on {}.{}; available: {}", field, ev.enum_name, ev.variant, available.join(", ")))
                    })?;
                return Ok(ev.field_values[slot]);
            }
            other => return Err(RuntimeError::new(
                ErrorCode::R003, line, 0,
                format!("field access not supported on {:?}", std::mem::discriminant(other)),
            )),
        }
    };

    // Phase 2: allocate from the extracted data (borrow on heap is now released)
    match needs_alloc {
        Some(NeedsAlloc::Vec2(x, y)) => Ok(heap.alloc(HeapObject::Vec2(x, y))),
        Some(NeedsAlloc::Str(s)) => Ok(heap.alloc(HeapObject::Str(s))),
        None => unreachable!(),
    }
}

// ─── set_field_rebuild ────────────────────────────────────────────────────────

/// Immutable field "write" for VALUE types (Vec2, Vec3, Vec4, Color).
///
/// Allocates a *new* heap object with the named field replaced by `val` and
/// returns a `HeapRef` to it. The original object is not modified.
pub fn set_field_rebuild(
    heap: &mut Heap,
    idx: u32,
    field: &str,
    val: StackValue,
    line: usize,
) -> Result<StackValue, RuntimeError> {
    let f = require_float(val, line)?;
    // Phase 1: read scalars from immutable borrow
    let new_obj = {
        let obj = heap.get(idx);
        match obj {
            HeapObject::Vec2(x, y) => match field {
                "x" => Ok(HeapObject::Vec2(f, *y)),
                "y" => Ok(HeapObject::Vec2(*x, f)),
                _ => Err(field_not_found(field, "vec2", &["x", "y"], line)),
            },
            HeapObject::Vec3(x, y, z) => match field {
                "x" => Ok(HeapObject::Vec3(f, *y, *z)),
                "y" => Ok(HeapObject::Vec3(*x, f, *z)),
                "z" => Ok(HeapObject::Vec3(*x, *y, f)),
                _ => Err(field_not_found(field, "vec3", &["x", "y", "z"], line)),
            },
            HeapObject::Vec4(x, y, z, w) => match field {
                "x" => Ok(HeapObject::Vec4(f, *y, *z, *w)),
                "y" => Ok(HeapObject::Vec4(*x, f, *z, *w)),
                "z" => Ok(HeapObject::Vec4(*x, *y, f, *w)),
                "w" => Ok(HeapObject::Vec4(*x, *y, *z, f)),
                _ => Err(field_not_found(field, "vec4", &["x", "y", "z", "w"], line)),
            },
            HeapObject::Color { r, g, b, a } => match field {
                "r" => Ok(HeapObject::Color { r: f, g: *g, b: *b, a: *a }),
                "g" => Ok(HeapObject::Color { r: *r, g: f, b: *b, a: *a }),
                "b" => Ok(HeapObject::Color { r: *r, g: *g, b: f, a: *a }),
                "a" => Ok(HeapObject::Color { r: *r, g: *g, b: *b, a: f }),
                _ => Err(field_not_found(field, "color", &["r", "g", "b", "a"], line)),
            },
            _ => Err(RuntimeError::new(
                ErrorCode::R011, line, 0,
                "set_field_rebuild is only valid for value types (vec2, vec3, vec4, color)",
            )),
        }
    }?;
    // Phase 2: allocate (borrow released)
    Ok(heap.alloc(new_obj))
}

// ─── set_field_mutate ─────────────────────────────────────────────────────────

/// In-place field write for REFERENCE types (State, Object).
///
/// Mutates the heap object directly — callers do not need to write back.
/// If the container is in the persistent heap and `val` is a frame-arena ref,
/// `val` is promoted to the persistent heap first.
pub fn set_field_mutate(
    heap: &mut Heap,
    idx: u32,
    field: &str,
    val: StackValue,
    state_fields: &[String],
    struct_defs: &[CompiledStructDef],
    line: usize,
) -> Result<(), RuntimeError> {
    let val = if !Heap::is_frame_ref(idx) { heap.promote(val) } else { val };
    match heap.get_mut(idx) {
        HeapObject::State(state) => {
            let slot = state_fields
                .iter()
                .position(|n| n == field)
                .ok_or_else(|| {
                    let available: Vec<&str> = state_fields.iter().map(String::as_str).collect();
                    RuntimeError::new(
                        ErrorCode::R003,
                        line,
                        0,
                        format!(
                            "field '{}' not found on state; available: {}",
                            field,
                            available.join(", ")
                        ),
                    )
                })?;
            state.fields[slot] = val;
            Ok(())
        }

        HeapObject::Object(obj) => {
            // We need the struct def — borrow it before mutating the heap.
            // Clone enough info to avoid borrow conflicts.
            let def_idx = obj.struct_def_idx as usize;
            let type_name = obj.type_name.clone();
            let _ = obj; // release the mutable borrow

            let def = struct_defs
                .get(def_idx)
                .ok_or_else(|| RuntimeError::new(
                    ErrorCode::R003,
                    line,
                    0,
                    format!("invalid struct_def_idx {def_idx} for type '{type_name}'"),
                ))?;

            let slot = def
                .field_names
                .iter()
                .position(|n| n == field)
                .ok_or_else(|| {
                    let available: Vec<&str> = def.field_names.iter().map(String::as_str).collect();
                    RuntimeError::new(
                        ErrorCode::R003,
                        line,
                        0,
                        format!(
                            "field '{}' not found on {}; available: {}",
                            field,
                            type_name,
                            available.join(", ")
                        ),
                    )
                })?;

            // Now take the mutable borrow again to write.
            if let HeapObject::Object(obj) = heap.get_mut(idx) {
                obj.fields[slot] = val;
            }
            Ok(())
        }

        _ => Err(RuntimeError::new(
            ErrorCode::R011,
            line,
            0,
            "set_field_mutate is only valid for reference types (state, object)".to_string(),
        )),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::value::{HeapObject, InputObj, StateObj, StructObj, EnumVariantObj};
    use crate::vm::heap::Heap;
    use crate::types::draw::{ShapeData, ShapeDesc, RenderMode, CoordMeta};

    // ── helpers ──────────────────────────────────────────────────────────────

    fn no_state() -> Vec<String> { vec![] }
    fn no_defs() -> Vec<CompiledStructDef> { vec![] }

    fn get(heap: &mut Heap, sv: StackValue, field: &str) -> Result<StackValue, RuntimeError> {
        let idx = sv.as_heap_ref().unwrap();
        get_field(heap, idx, field, &no_state(), &no_defs(), 1)
    }

    fn rebuild(heap: &mut Heap, sv: StackValue, field: &str, val: StackValue) -> Result<StackValue, RuntimeError> {
        let idx = sv.as_heap_ref().unwrap();
        set_field_rebuild(heap, idx, field, val, 1)
    }

    // ── Vec2 ─────────────────────────────────────────────────────────────────

    #[test]
    fn vec2_x_and_y() {
        let mut heap = Heap::new();
        let v = heap.alloc(HeapObject::Vec2(3.0, 7.0));
        assert!(matches!(get(&mut heap, v, "x"), Ok(StackValue::Float(f)) if f == 3.0));
        assert!(matches!(get(&mut heap, v, "y"), Ok(StackValue::Float(f)) if f == 7.0));
    }

    #[test]
    fn vec2_unknown_field_error() {
        let mut heap = Heap::new();
        let v = heap.alloc(HeapObject::Vec2(0.0, 0.0));
        let err = get(&mut heap, v, "z").unwrap_err();
        assert_eq!(err.code, ErrorCode::R003);
        assert!(err.message.contains("vec2"));
        assert!(err.message.contains("z"));
        assert!(err.message.contains("x"));
        assert!(err.message.contains("y"));
    }

    // ── Vec3 ─────────────────────────────────────────────────────────────────

    #[test]
    fn vec3_x_y_z() {
        let mut heap = Heap::new();
        let v = heap.alloc(HeapObject::Vec3(1.0, 2.0, 3.0));
        assert!(matches!(get(&mut heap, v, "x"), Ok(StackValue::Float(f)) if f == 1.0));
        assert!(matches!(get(&mut heap, v, "y"), Ok(StackValue::Float(f)) if f == 2.0));
        assert!(matches!(get(&mut heap, v, "z"), Ok(StackValue::Float(f)) if f == 3.0));
    }

    // ── Vec4 ─────────────────────────────────────────────────────────────────

    #[test]
    fn vec4_x_y_z_w() {
        let mut heap = Heap::new();
        let v = heap.alloc(HeapObject::Vec4(1.0, 2.0, 3.0, 4.0));
        assert!(matches!(get(&mut heap, v, "x"), Ok(StackValue::Float(f)) if f == 1.0));
        assert!(matches!(get(&mut heap, v, "y"), Ok(StackValue::Float(f)) if f == 2.0));
        assert!(matches!(get(&mut heap, v, "z"), Ok(StackValue::Float(f)) if f == 3.0));
        assert!(matches!(get(&mut heap, v, "w"), Ok(StackValue::Float(f)) if f == 4.0));
    }

    // ── Color ─────────────────────────────────────────────────────────────────

    #[test]
    fn color_r_g_b_a() {
        let mut heap = Heap::new();
        let c = heap.alloc(HeapObject::Color { r: 0.1, g: 0.2, b: 0.3, a: 0.4 });
        assert!(matches!(get(&mut heap, c, "r"), Ok(StackValue::Float(f)) if (f - 0.1).abs() < 1e-10));
        assert!(matches!(get(&mut heap, c, "g"), Ok(StackValue::Float(f)) if (f - 0.2).abs() < 1e-10));
        assert!(matches!(get(&mut heap, c, "b"), Ok(StackValue::Float(f)) if (f - 0.3).abs() < 1e-10));
        assert!(matches!(get(&mut heap, c, "a"), Ok(StackValue::Float(f)) if (f - 0.4).abs() < 1e-10));
    }

    // ── Shape: circle ─────────────────────────────────────────────────────────

    fn make_circle(heap: &mut Heap, cx: f64, cy: f64, r: f64) -> StackValue {
        let sd = ShapeData::new(
            ShapeDesc::Circle { center: (cx, cy), radius: r },
            RenderMode::Sdf,
            CoordMeta::default(),
        );
        heap.alloc(HeapObject::Shape(sd))
    }

    #[test]
    fn circle_center_returns_vec2() {
        let mut heap = Heap::new();
        let shape = make_circle(&mut heap, 10.0, 20.0, 5.0);
        let center_sv = get(&mut heap, shape, "center").unwrap();
        let center_idx = center_sv.as_heap_ref().unwrap();
        assert!(matches!(heap.get(center_idx), HeapObject::Vec2(x, y) if *x == 10.0 && *y == 20.0));
    }

    #[test]
    fn circle_radius_returns_float() {
        let mut heap = Heap::new();
        let shape = make_circle(&mut heap, 0.0, 0.0, 5.0);
        assert!(matches!(get(&mut heap, shape, "radius"), Ok(StackValue::Float(f)) if f == 5.0));
    }

    #[test]
    fn circle_unknown_field_error() {
        let mut heap = Heap::new();
        let shape = make_circle(&mut heap, 0.0, 0.0, 1.0);
        let err = get(&mut heap, shape, "diameter").unwrap_err();
        assert_eq!(err.code, ErrorCode::R003);
        assert!(err.message.contains("circle"));
    }

    // ── Shape: rect ───────────────────────────────────────────────────────────

    fn make_rect(heap: &mut Heap) -> StackValue {
        use crate::types::draw::Origin;
        let sd = ShapeData::new(
            ShapeDesc::Rect { center: (1.0, 2.0), size: (10.0, 20.0), origin: Origin::Center },
            RenderMode::Sdf,
            CoordMeta::default(),
        );
        heap.alloc(HeapObject::Shape(sd))
    }

    #[test]
    fn rect_center_and_size() {
        let mut heap = Heap::new();
        let shape = make_rect(&mut heap);

        let c = get(&mut heap, shape, "center").unwrap();
        let c_idx = c.as_heap_ref().unwrap();
        assert!(matches!(heap.get(c_idx), HeapObject::Vec2(x, y) if *x == 1.0 && *y == 2.0));

        let s = get(&mut heap, shape, "size").unwrap();
        let s_idx = s.as_heap_ref().unwrap();
        assert!(matches!(heap.get(s_idx), HeapObject::Vec2(x, y) if *x == 10.0 && *y == 20.0));
    }

    // ── Shape: line ───────────────────────────────────────────────────────────

    fn make_line(heap: &mut Heap) -> StackValue {
        let sd = ShapeData::new(
            ShapeDesc::Line { from: (0.0, 0.0), to: (100.0, 50.0) },
            RenderMode::Sdf,
            CoordMeta::default(),
        );
        heap.alloc(HeapObject::Shape(sd))
    }

    #[test]
    fn line_from_and_to() {
        let mut heap = Heap::new();
        let shape = make_line(&mut heap);

        let from = get(&mut heap, shape, "from").unwrap();
        let from_idx = from.as_heap_ref().unwrap();
        assert!(matches!(heap.get(from_idx), HeapObject::Vec2(x, y) if *x == 0.0 && *y == 0.0));

        let to = get(&mut heap, shape, "to").unwrap();
        let to_idx = to.as_heap_ref().unwrap();
        assert!(matches!(heap.get(to_idx), HeapObject::Vec2(x, y) if *x == 100.0 && *y == 50.0));
    }

    // ── Shape: text ───────────────────────────────────────────────────────────

    fn make_text(heap: &mut Heap) -> StackValue {
        let sd = ShapeData::new(
            ShapeDesc::Text { pos: (5.0, 10.0), content: "hello".into(), size: 14.0 },
            RenderMode::Sdf,
            CoordMeta::default(),
        );
        heap.alloc(HeapObject::Shape(sd))
    }

    #[test]
    fn text_pos_content_size() {
        let mut heap = Heap::new();
        let shape = make_text(&mut heap);

        let pos = get(&mut heap, shape, "pos").unwrap();
        let pos_idx = pos.as_heap_ref().unwrap();
        assert!(matches!(heap.get(pos_idx), HeapObject::Vec2(x, y) if *x == 5.0 && *y == 10.0));

        let content = get(&mut heap, shape, "content").unwrap();
        let content_idx = content.as_heap_ref().unwrap();
        assert!(matches!(heap.get(content_idx), HeapObject::Str(s) if s == "hello"));

        assert!(matches!(get(&mut heap, shape, "size"), Ok(StackValue::Float(f)) if f == 14.0));
    }

    // ── List ─────────────────────────────────────────────────────────────────

    #[test]
    fn list_len() {
        let mut heap = Heap::new();
        let l = heap.alloc(HeapObject::List(vec![
            StackValue::Float(1.0),
            StackValue::Float(2.0),
            StackValue::Float(3.0),
        ]));
        assert!(matches!(get(&mut heap, l, "len"), Ok(StackValue::Float(f)) if f == 3.0));
    }

    #[test]
    fn list_empty_len() {
        let mut heap = Heap::new();
        let l = heap.alloc(HeapObject::List(vec![]));
        assert!(matches!(get(&mut heap, l, "len"), Ok(StackValue::Float(f)) if f == 0.0));
    }

    // ── ResOk ────────────────────────────────────────────────────────────────

    #[test]
    fn res_ok_fields() {
        let mut heap = Heap::new();
        let ok = heap.alloc(HeapObject::ResOk(StackValue::Float(42.0)));

        assert!(matches!(get(&mut heap, ok, "ok"), Ok(StackValue::Bool(true))));
        assert!(matches!(get(&mut heap, ok, "value"), Ok(StackValue::Float(f)) if f == 42.0));

        let error_sv = get(&mut heap, ok, "error").unwrap();
        let error_idx = error_sv.as_heap_ref().unwrap();
        assert!(matches!(heap.get(error_idx), HeapObject::Str(s) if s.is_empty()));
    }

    // ── ResErr ───────────────────────────────────────────────────────────────

    #[test]
    fn res_err_fields() {
        let mut heap = Heap::new();
        let err_val = heap.alloc(HeapObject::ResErr("file not found".into()));

        assert!(matches!(get(&mut heap, err_val, "ok"), Ok(StackValue::Bool(false))));

        let value_err = get(&mut heap, err_val, "value").unwrap_err();
        assert_eq!(value_err.code, ErrorCode::R003);
        assert!(value_err.message.contains("cannot access .value on failed result"));

        let error_sv = get(&mut heap, err_val, "error").unwrap();
        let error_idx = error_sv.as_heap_ref().unwrap();
        assert!(matches!(heap.get(error_idx), HeapObject::Str(s) if s == "file not found"));
    }

    // ── Input ─────────────────────────────────────────────────────────────────

    fn make_input(heap: &mut Heap) -> StackValue {
        heap.alloc(HeapObject::Input(InputObj {
            dt: 0.016,
            mouse_x: 100.0,
            mouse_y: 200.0,
            mouse_down: true,
            mouse_pressed: false,
            mouse_released: true,
            key_pressed: "a".into(),
            key_down: "b".into(),
            key_released: "c".into(),
        }))
    }

    #[test]
    fn input_dt() {
        let mut heap = Heap::new();
        let inp = make_input(&mut heap);
        assert!(matches!(get(&mut heap, inp, "dt"), Ok(StackValue::Float(f)) if (f - 0.016).abs() < 1e-10));
    }

    #[test]
    fn input_mouse_fields() {
        let mut heap = Heap::new();
        let inp = make_input(&mut heap);
        assert!(matches!(get(&mut heap, inp, "mouse_x"), Ok(StackValue::Float(f)) if f == 100.0));
        assert!(matches!(get(&mut heap, inp, "mouse_y"), Ok(StackValue::Float(f)) if f == 200.0));
        assert!(matches!(get(&mut heap, inp, "mouse_down"), Ok(StackValue::Bool(true))));
        assert!(matches!(get(&mut heap, inp, "mouse_pressed"), Ok(StackValue::Bool(false))));
        assert!(matches!(get(&mut heap, inp, "mouse_released"), Ok(StackValue::Bool(true))));
    }

    #[test]
    fn input_key_fields() {
        let mut heap = Heap::new();
        let inp = make_input(&mut heap);

        let kp = get(&mut heap, inp, "key_pressed").unwrap();
        let kp_idx = kp.as_heap_ref().unwrap();
        assert!(matches!(heap.get(kp_idx), HeapObject::Str(s) if s == "a"));

        let kd = get(&mut heap, inp, "key_down").unwrap();
        let kd_idx = kd.as_heap_ref().unwrap();
        assert!(matches!(heap.get(kd_idx), HeapObject::Str(s) if s == "b"));

        let kr = get(&mut heap, inp, "key_released").unwrap();
        let kr_idx = kr.as_heap_ref().unwrap();
        assert!(matches!(heap.get(kr_idx), HeapObject::Str(s) if s == "c"));
    }

    // ── State ─────────────────────────────────────────────────────────────────

    #[test]
    fn state_field_lookup() {
        let mut heap = Heap::new();
        let state = heap.alloc(HeapObject::State(StateObj {
            fields: vec![StackValue::Float(1.0), StackValue::Float(2.0)],
        }));
        let state_fields = vec!["x".into(), "y".into()];

        let idx = state.as_heap_ref().unwrap();
        let x = get_field(&mut heap, idx, "x", &state_fields, &no_defs(), 1).unwrap();
        assert!(matches!(x, StackValue::Float(f) if f == 1.0));
        let y = get_field(&mut heap, idx, "y", &state_fields, &no_defs(), 1).unwrap();
        assert!(matches!(y, StackValue::Float(f) if f == 2.0));
    }

    #[test]
    fn state_unknown_field_error() {
        let mut heap = Heap::new();
        let state = heap.alloc(HeapObject::State(StateObj { fields: vec![StackValue::Float(0.0)] }));
        let state_fields = vec!["speed".into()];
        let idx = state.as_heap_ref().unwrap();
        let err = get_field(&mut heap, idx, "velocity", &state_fields, &no_defs(), 5).unwrap_err();
        assert_eq!(err.code, ErrorCode::R003);
        assert!(err.message.contains("velocity"));
        assert!(err.message.contains("speed"));
    }

    // ── Object ────────────────────────────────────────────────────────────────

    fn make_struct_def(name: &str, fields: &[&str]) -> CompiledStructDef {
        use std::collections::HashMap;
        CompiledStructDef {
            name: name.into(),
            field_names: fields.iter().map(|s| s.to_string()).collect(),
            methods: HashMap::new(),
        }
    }

    #[test]
    fn object_field_lookup() {
        let mut heap = Heap::new();
        let obj = heap.alloc(HeapObject::Object(StructObj {
            type_name: "Player".into(),
            struct_def_idx: 0,
            fields: vec![StackValue::Float(10.0), StackValue::Bool(true)],
        }));
        let defs = vec![make_struct_def("Player", &["hp", "alive"])];
        let idx = obj.as_heap_ref().unwrap();

        let hp = get_field(&mut heap, idx, "hp", &no_state(), &defs, 1).unwrap();
        assert!(matches!(hp, StackValue::Float(f) if f == 10.0));

        let alive = get_field(&mut heap, idx, "alive", &no_state(), &defs, 1).unwrap();
        assert!(matches!(alive, StackValue::Bool(true)));
    }

    #[test]
    fn object_unknown_field_error() {
        let mut heap = Heap::new();
        let obj = heap.alloc(HeapObject::Object(StructObj {
            type_name: "Foo".into(),
            struct_def_idx: 0,
            fields: vec![],
        }));
        let defs = vec![make_struct_def("Foo", &["bar"])];
        let idx = obj.as_heap_ref().unwrap();
        let err = get_field(&mut heap, idx, "baz", &no_state(), &defs, 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::R003);
        assert!(err.message.contains("Foo"));
        assert!(err.message.contains("baz"));
        assert!(err.message.contains("bar"));
    }

    // ── EnumVariant ───────────────────────────────────────────────────────────

    #[test]
    fn enum_variant_field_lookup() {
        let mut heap = Heap::new();
        let ev = heap.alloc(HeapObject::EnumVariant(EnumVariantObj {
            enum_name: "Shape".into(),
            variant: "Circle".into(),
            field_names: vec!["radius".into()],
            field_values: vec![StackValue::Float(5.0)],
        }));
        let idx = ev.as_heap_ref().unwrap();
        let r = get_field(&mut heap, idx, "radius", &no_state(), &no_defs(), 1).unwrap();
        assert!(matches!(r, StackValue::Float(f) if f == 5.0));
    }

    #[test]
    fn enum_variant_unknown_field_error() {
        let mut heap = Heap::new();
        let ev = heap.alloc(HeapObject::EnumVariant(EnumVariantObj {
            enum_name: "E".into(),
            variant: "V".into(),
            field_names: vec!["x".into()],
            field_values: vec![StackValue::Float(0.0)],
        }));
        let idx = ev.as_heap_ref().unwrap();
        let err = get_field(&mut heap, idx, "y", &no_state(), &no_defs(), 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::R003);
        assert!(err.message.contains("y"));
        assert!(err.message.contains("x"));
    }

    // ── set_field_rebuild ─────────────────────────────────────────────────────

    #[test]
    fn rebuild_vec2_x() {
        let mut heap = Heap::new();
        let orig = heap.alloc(HeapObject::Vec2(1.0, 2.0));
        let new_sv = rebuild(&mut heap, orig, "x", StackValue::Float(5.0)).unwrap();
        let new_idx = new_sv.as_heap_ref().unwrap();
        assert!(matches!(heap.get(new_idx), HeapObject::Vec2(x, y) if *x == 5.0 && *y == 2.0));
        // Original unchanged
        let orig_idx = orig.as_heap_ref().unwrap();
        assert!(matches!(heap.get(orig_idx), HeapObject::Vec2(x, y) if *x == 1.0 && *y == 2.0));
    }

    #[test]
    fn rebuild_vec2_y() {
        let mut heap = Heap::new();
        let orig = heap.alloc(HeapObject::Vec2(1.0, 2.0));
        let new_sv = rebuild(&mut heap, orig, "y", StackValue::Float(9.0)).unwrap();
        let new_idx = new_sv.as_heap_ref().unwrap();
        assert!(matches!(heap.get(new_idx), HeapObject::Vec2(x, y) if *x == 1.0 && *y == 9.0));
    }

    #[test]
    fn rebuild_vec3_all_fields() {
        let mut heap = Heap::new();
        let orig = heap.alloc(HeapObject::Vec3(1.0, 2.0, 3.0));

        let nx = rebuild(&mut heap, orig, "x", StackValue::Float(10.0)).unwrap();
        let nx_idx = nx.as_heap_ref().unwrap();
        assert!(matches!(heap.get(nx_idx), HeapObject::Vec3(x, y, z) if *x == 10.0 && *y == 2.0 && *z == 3.0));

        let ny = rebuild(&mut heap, orig, "y", StackValue::Float(20.0)).unwrap();
        let ny_idx = ny.as_heap_ref().unwrap();
        assert!(matches!(heap.get(ny_idx), HeapObject::Vec3(x, y, z) if *x == 1.0 && *y == 20.0 && *z == 3.0));

        let nz = rebuild(&mut heap, orig, "z", StackValue::Float(30.0)).unwrap();
        let nz_idx = nz.as_heap_ref().unwrap();
        assert!(matches!(heap.get(nz_idx), HeapObject::Vec3(x, y, z) if *x == 1.0 && *y == 2.0 && *z == 30.0));
    }

    #[test]
    fn rebuild_vec4_all_fields() {
        let mut heap = Heap::new();
        let orig = heap.alloc(HeapObject::Vec4(1.0, 2.0, 3.0, 4.0));

        for (field, expected) in &[("x", (10.0, 2.0, 3.0, 4.0)), ("y", (1.0, 20.0, 3.0, 4.0)),
                                    ("z", (1.0, 2.0, 30.0, 4.0)), ("w", (1.0, 2.0, 3.0, 40.0))] {
            let new_val = match *field { "x" => 10.0, "y" => 20.0, "z" => 30.0, _ => 40.0 };
            let new_sv = rebuild(&mut heap, orig, field, StackValue::Float(new_val)).unwrap();
            let new_idx = new_sv.as_heap_ref().unwrap();
            let (ex, ey, ez, ew) = *expected;
            assert!(matches!(heap.get(new_idx),
                HeapObject::Vec4(x, y, z, w) if *x == ex && *y == ey && *z == ez && *w == ew),
                "field {} failed", field);
        }
    }

    #[test]
    fn rebuild_color_all_fields() {
        let mut heap = Heap::new();
        let orig = heap.alloc(HeapObject::Color { r: 0.1, g: 0.2, b: 0.3, a: 0.4 });

        let nr = rebuild(&mut heap, orig, "r", StackValue::Float(0.9)).unwrap();
        let nr_idx = nr.as_heap_ref().unwrap();
        assert!(matches!(heap.get(nr_idx), HeapObject::Color { r, g, b, a }
            if (*r - 0.9).abs() < 1e-10 && (*g - 0.2).abs() < 1e-10));

        let ng = rebuild(&mut heap, orig, "g", StackValue::Float(0.8)).unwrap();
        let ng_idx = ng.as_heap_ref().unwrap();
        assert!(matches!(heap.get(ng_idx), HeapObject::Color { r, g, b, a }
            if (*r - 0.1).abs() < 1e-10 && (*g - 0.8).abs() < 1e-10));
    }

    #[test]
    fn rebuild_bool_coerces_to_float() {
        let mut heap = Heap::new();
        let orig = heap.alloc(HeapObject::Vec2(1.0, 2.0));
        let result = rebuild(&mut heap, orig, "x", StackValue::Bool(true)).unwrap();
        let idx = result.as_heap_ref().unwrap();
        assert!(matches!(heap.get(idx), HeapObject::Vec2(x, _) if (*x - 1.0).abs() < 1e-10));
    }

    #[test]
    fn rebuild_unknown_field_error() {
        let mut heap = Heap::new();
        let orig = heap.alloc(HeapObject::Vec2(1.0, 2.0));
        let err = rebuild(&mut heap, orig, "z", StackValue::Float(0.0)).unwrap_err();
        assert_eq!(err.code, ErrorCode::R003);
    }

    #[test]
    fn rebuild_non_value_type_error() {
        let mut heap = Heap::new();
        let list = heap.alloc(HeapObject::List(vec![]));
        let idx = list.as_heap_ref().unwrap();
        let err = set_field_rebuild(&mut heap, idx, "len", StackValue::Float(0.0), 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::R011);
    }

    // ── set_field_mutate ──────────────────────────────────────────────────────

    #[test]
    fn mutate_state_field() {
        let mut heap = Heap::new();
        let state = heap.alloc(HeapObject::State(StateObj {
            fields: vec![StackValue::Float(0.0), StackValue::Float(0.0)],
        }));
        let state_fields = vec!["x".into(), "y".into()];
        let idx = state.as_heap_ref().unwrap();

        set_field_mutate(&mut heap, idx, "x", StackValue::Float(99.0), &state_fields, &no_defs(), 1).unwrap();

        // Verify in-place mutation
        let x = get_field(&mut heap, idx, "x", &state_fields, &no_defs(), 1).unwrap();
        assert!(matches!(x, StackValue::Float(f) if f == 99.0));
        // y unchanged
        let y = get_field(&mut heap, idx, "y", &state_fields, &no_defs(), 1).unwrap();
        assert!(matches!(y, StackValue::Float(f) if f == 0.0));
    }

    #[test]
    fn mutate_state_unknown_field_error() {
        let mut heap = Heap::new();
        let state = heap.alloc(HeapObject::State(StateObj { fields: vec![StackValue::Float(0.0)] }));
        let state_fields = vec!["x".into()];
        let idx = state.as_heap_ref().unwrap();
        let err = set_field_mutate(&mut heap, idx, "z", StackValue::Float(0.0), &state_fields, &no_defs(), 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::R003);
        assert!(err.message.contains("z"));
        assert!(err.message.contains("x"));
    }

    #[test]
    fn mutate_object_field() {
        let mut heap = Heap::new();
        let obj = heap.alloc(HeapObject::Object(StructObj {
            type_name: "Player".into(),
            struct_def_idx: 0,
            fields: vec![StackValue::Float(100.0)],
        }));
        let defs = vec![make_struct_def("Player", &["hp"])];
        let idx = obj.as_heap_ref().unwrap();

        set_field_mutate(&mut heap, idx, "hp", StackValue::Float(50.0), &no_state(), &defs, 1).unwrap();

        let hp = get_field(&mut heap, idx, "hp", &no_state(), &defs, 1).unwrap();
        assert!(matches!(hp, StackValue::Float(f) if f == 50.0));
    }

    #[test]
    fn mutate_object_unknown_field_error() {
        let mut heap = Heap::new();
        let obj = heap.alloc(HeapObject::Object(StructObj {
            type_name: "Foo".into(),
            struct_def_idx: 0,
            fields: vec![],
        }));
        let defs = vec![make_struct_def("Foo", &["x"])];
        let idx = obj.as_heap_ref().unwrap();
        let err = set_field_mutate(&mut heap, idx, "z", StackValue::Float(0.0), &no_state(), &defs, 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::R003);
        assert!(err.message.contains("z"));
        assert!(err.message.contains("x"));
    }

    #[test]
    fn mutate_non_reference_type_error() {
        let mut heap = Heap::new();
        let v = heap.alloc(HeapObject::Vec2(0.0, 0.0));
        let idx = v.as_heap_ref().unwrap();
        let err = set_field_mutate(&mut heap, idx, "x", StackValue::Float(1.0), &no_state(), &no_defs(), 1).unwrap_err();
        assert_eq!(err.code, ErrorCode::R011);
    }
}
