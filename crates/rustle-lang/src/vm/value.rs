use crate::types::draw::{RenderMode, ShapeData, TransformData};

use super::heap::Heap;

// ─── Stack value (16 bytes, Copy) ─────────────────────────────────────────────

/// A value that lives directly on the VM stack.
///
/// This is always 16 bytes (driven by `Float(f64)`).  Reference types are
/// represented as an index into the [`Heap`].
#[derive(Debug, Clone, Copy)]
pub enum StackValue {
    Float(f64),
    Bool(bool),
    None,
    HeapRef(u32),
}

impl StackValue {
    /// Extract the inner `f64`, if this is `Float`.
    #[must_use]
    pub fn as_float(self) -> Option<f64> {
        if let Self::Float(f) = self { Some(f) } else { None }
    }

    /// Extract the inner `bool`, if this is `Bool`.
    #[must_use]
    pub fn as_bool(self) -> Option<bool> {
        if let Self::Bool(b) = self { Some(b) } else { None }
    }

    /// Extract the heap index, if this is `HeapRef`.
    #[must_use]
    pub fn as_heap_ref(self) -> Option<u32> {
        if let Self::HeapRef(i) = self { Some(i) } else { None }
    }
}

// ─── Heap-object sub-structs ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ClosureObj {
    pub chunk_index: u16,
    pub upvalues: Vec<StackValue>,
}

#[derive(Debug, Clone)]
pub struct StateObj {
    pub fields: Vec<StackValue>,
}

#[derive(Debug, Clone)]
pub struct StructObj {
    pub type_name: String,
    pub struct_def_idx: u16,
    pub fields: Vec<StackValue>,
}

#[derive(Debug, Clone)]
pub struct EnumVariantObj {
    pub enum_name: String,
    pub variant: String,
    pub field_names: Vec<String>,
    pub field_values: Vec<StackValue>,
}

#[derive(Debug, Clone)]
pub struct InputObj {
    pub dt: f64,
    pub mouse_x: f64,
    pub mouse_y: f64,
    pub mouse_down: bool,
    pub mouse_pressed: bool,
    pub mouse_released: bool,
    pub key_pressed: String,
    pub key_down: String,
    pub key_released: String,
}

#[derive(Debug, Clone)]
pub struct IteratorObj {
    pub items: Vec<StackValue>,
    pub index: usize,
}

// ─── Heap object ─────────────────────────────────────────────────────────────

/// A heap-allocated value referenced by a [`StackValue::HeapRef`].
#[derive(Debug, Clone)]
pub enum HeapObject {
    Str(String),
    Vec2(f64, f64),
    Vec3(f64, f64, f64),
    Vec4(f64, f64, f64, f64),
    Color { r: f64, g: f64, b: f64, a: f64 },
    Mat3(Box<[f64; 9]>),
    Mat4(Box<[f64; 16]>),
    List(Vec<StackValue>),
    Closure(ClosureObj),
    Shape(ShapeData),
    Transform(TransformData),
    RenderMode(RenderMode),
    ResOk(StackValue),
    ResErr(String),
    State(StateObj),
    Object(StructObj),
    EnumVariant(EnumVariantObj),
    Input(InputObj),
    Iterator(IteratorObj),
    /// Index into the VM's native-function dispatch table.
    NativeFnRef(u16),
}

// ─── Truthiness ──────────────────────────────────────────────────────────────

/// Determine whether a value is truthy in a boolean context.
///
/// - `Float`: non-zero and non-NaN
/// - `Bool`: the boolean itself
/// - `None`: always false
/// - `Str`: non-empty
/// - `List`: non-empty
/// - everything else on the heap: true
#[must_use]
pub fn is_truthy(sv: StackValue, heap: &Heap) -> bool {
    match sv {
        StackValue::Float(f) => f != 0.0 && !f.is_nan(),
        StackValue::Bool(b) => b,
        StackValue::None => false,
        StackValue::HeapRef(idx) => match heap.get(idx) {
            HeapObject::Str(s) => !s.is_empty(),
            HeapObject::List(v) => !v.is_empty(),
            _ => true,
        },
    }
}

// ─── Equality ─────────────────────────────────────────────────────────────────

/// Deep structural equality. Float uses bitwise comparison (NaN == NaN is true).
#[must_use]
pub fn values_equal(a: StackValue, b: StackValue, heap: &Heap) -> bool {
    match (a, b) {
        (StackValue::Float(x), StackValue::Float(y)) => x.to_bits() == y.to_bits(),
        (StackValue::Bool(x),  StackValue::Bool(y))  => x == y,
        (StackValue::None,     StackValue::None)      => true,
        (StackValue::HeapRef(ia), StackValue::HeapRef(ib)) => {
            if ia == ib {
                return true;
            }
            heap_objects_equal(heap.get(ia), heap.get(ib), heap)
        }
        _ => false,
    }
}

fn heap_objects_equal(a: &HeapObject, b: &HeapObject, heap: &Heap) -> bool {
    match (a, b) {
        (HeapObject::Str(x), HeapObject::Str(y)) => x == y,
        (HeapObject::Vec2(ax, ay), HeapObject::Vec2(bx, by)) => {
            ax.to_bits() == bx.to_bits() && ay.to_bits() == by.to_bits()
        }
        (HeapObject::Vec3(ax, ay, az), HeapObject::Vec3(bx, by, bz)) => {
            ax.to_bits() == bx.to_bits()
                && ay.to_bits() == by.to_bits()
                && az.to_bits() == bz.to_bits()
        }
        (HeapObject::Vec4(ax, ay, az, aw), HeapObject::Vec4(bx, by, bz, bw)) => {
            ax.to_bits() == bx.to_bits()
                && ay.to_bits() == by.to_bits()
                && az.to_bits() == bz.to_bits()
                && aw.to_bits() == bw.to_bits()
        }
        (
            HeapObject::Color { r: ar, g: ag, b: ab, a: aa },
            HeapObject::Color { r: br, g: bg, b: bb, a: ba },
        ) => {
            ar.to_bits() == br.to_bits()
                && ag.to_bits() == bg.to_bits()
                && ab.to_bits() == bb.to_bits()
                && aa.to_bits() == ba.to_bits()
        }
        (HeapObject::List(la), HeapObject::List(lb)) => {
            la.len() == lb.len()
                && la
                    .iter()
                    .zip(lb.iter())
                    .all(|(&va, &vb)| values_equal(va, vb, heap))
        }
        (HeapObject::EnumVariant(ea), HeapObject::EnumVariant(eb)) => {
            ea.enum_name == eb.enum_name
                && ea.variant == eb.variant
                && ea.field_values.len() == eb.field_values.len()
                && ea
                    .field_values
                    .iter()
                    .zip(eb.field_values.iter())
                    .all(|(&va, &vb)| values_equal(va, vb, heap))
        }
        (HeapObject::Object(sa), HeapObject::Object(sb)) => {
            sa.type_name == sb.type_name
                && sa.fields.len() == sb.fields.len()
                && sa
                    .fields
                    .iter()
                    .zip(sb.fields.iter())
                    .all(|(&va, &vb)| values_equal(va, vb, heap))
        }
        _ => false,
    }
}

// ─── Display ─────────────────────────────────────────────────────────────────

/// Format a value for console output. Matches the tree-walker's `Display` behaviour.
#[must_use]
pub fn display(sv: StackValue, heap: &Heap) -> String {
    match sv {
        StackValue::Float(f) => format_float(f),
        StackValue::Bool(b) => if b { "true" } else { "false" }.into(),
        StackValue::None => "none".into(),
        StackValue::HeapRef(idx) => display_heap(heap.get(idx), heap),
    }
}

fn format_float(f: f64) -> String {
    if f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{f}")
    }
}

fn display_heap(obj: &HeapObject, heap: &Heap) -> String {
    match obj {
        HeapObject::Str(s) => s.clone(),
        HeapObject::Vec2(x, y) => format!("vec2({x}, {y})"),
        HeapObject::Vec3(x, y, z) => format!("vec3({x}, {y}, {z})"),
        HeapObject::Vec4(x, y, z, w) => format!("vec4({x}, {y}, {z}, {w})"),
        HeapObject::Color { r, g, b, a } => {
            format!("color({r:.3}, {g:.3}, {b:.3}, {a:.3})")
        }
        HeapObject::Mat3(m) => {
            format!(
                "mat3({}, {}, {}, {}, {}, {}, {}, {}, {})",
                m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8]
            )
        }
        HeapObject::Mat4(m) => {
            format!(
                "mat4({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7],
                m[8], m[9], m[10], m[11], m[12], m[13], m[14], m[15]
            )
        }
        HeapObject::List(items) => {
            let parts: Vec<String> = items.iter().map(|&sv| display(sv, heap)).collect();
            format!("[{}]", parts.join(", "))
        }
        HeapObject::Shape(_) => "<shape>".into(),
        HeapObject::Transform(_) => "<transform>".into(),
        HeapObject::RenderMode(_) => "<render_mode>".into(),
        HeapObject::NativeFnRef(n) => format!("<fn:{n}>"),
        HeapObject::Closure(_) => "<closure>".into(),
        HeapObject::State(_) => "<state>".into(),
        HeapObject::ResOk(inner) => format!("ok({})", display(*inner, heap)),
        HeapObject::ResErr(msg) => format!("err({msg})"),
        HeapObject::Object(s) => format!("{} {{ ... }}", s.type_name),
        HeapObject::EnumVariant(e) => format!("{}.{}", e.enum_name, e.variant),
        HeapObject::Input(i) => format!("input(dt={})", i.dt),
        HeapObject::Iterator(_) => "<iterator>".into(),
    }
}

// ─── Deep clone ──────────────────────────────────────────────────────────────

/// Produce a deep copy of `sv`, allocating new heap objects as needed.
///
/// Value-typed `StackValue`s (`Float`, `Bool`, `None`) are returned unchanged.
/// `HeapRef`s get a freshly allocated copy with recursively cloned contents.
pub fn deep_clone(sv: StackValue, heap: &mut Heap) -> StackValue {
    match sv {
        StackValue::Float(_) | StackValue::Bool(_) | StackValue::None => sv,
        StackValue::HeapRef(idx) => {
            // Snapshot the data out of the heap before any recursive allocation.
            let cloned_obj = deep_clone_heap_object(idx, heap);
            heap.alloc(cloned_obj)
        }
    }
}

/// Clone a heap object by index, recursing into sub-values.
///
/// The snapshot-then-recurse pattern ensures we don't hold a reference
/// into `heap` while also mutating it.
fn deep_clone_heap_object(idx: u32, heap: &mut Heap) -> HeapObject {
    match heap.get(idx).clone() {
        HeapObject::List(items) => {
            let cloned: Vec<StackValue> = items
                .iter()
                .map(|&item| deep_clone(item, heap))
                .collect();
            HeapObject::List(cloned)
        }
        HeapObject::Object(s) => {
            let StructObj { type_name, struct_def_idx, fields } = s;
            let cloned_fields: Vec<StackValue> = fields
                .iter()
                .map(|&f| deep_clone(f, heap))
                .collect();
            HeapObject::Object(StructObj { type_name, struct_def_idx, fields: cloned_fields })
        }
        HeapObject::State(s) => {
            let cloned_fields: Vec<StackValue> = s.fields
                .iter()
                .map(|&f| deep_clone(f, heap))
                .collect();
            HeapObject::State(StateObj { fields: cloned_fields })
        }
        HeapObject::EnumVariant(e) => {
            let EnumVariantObj { enum_name, variant, field_names, field_values } = e;
            let cloned_values: Vec<StackValue> = field_values
                .iter()
                .map(|&f| deep_clone(f, heap))
                .collect();
            HeapObject::EnumVariant(EnumVariantObj {
                enum_name,
                variant,
                field_names,
                field_values: cloned_values,
            })
        }
        HeapObject::Closure(c) => {
            let ClosureObj { chunk_index, upvalues } = c;
            let cloned_upvalues: Vec<StackValue> = upvalues
                .iter()
                .map(|&u| deep_clone(u, heap))
                .collect();
            HeapObject::Closure(ClosureObj { chunk_index, upvalues: cloned_upvalues })
        }
        HeapObject::ResOk(inner) => {
            let cloned_inner = deep_clone(inner, heap);
            HeapObject::ResOk(cloned_inner)
        }
        // Shallow-clone everything else (strings, primitives, shapes, etc.)
        other => other,
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::heap::Heap;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn alloc_str(heap: &mut Heap, s: &str) -> StackValue {
        heap.alloc(HeapObject::Str(s.into()))
    }

    fn alloc_list(heap: &mut Heap, items: Vec<StackValue>) -> StackValue {
        heap.alloc(HeapObject::List(items))
    }

    // ── StackValue helpers ────────────────────────────────────────────────────

    #[test]
    fn stack_value_as_float() {
        assert_eq!(StackValue::Float(3.14).as_float(), Some(3.14));
        assert_eq!(StackValue::Bool(true).as_float(), None);
        assert_eq!(StackValue::None.as_float(), None);
    }

    #[test]
    fn stack_value_as_bool() {
        assert_eq!(StackValue::Bool(false).as_bool(), Some(false));
        assert_eq!(StackValue::Float(1.0).as_bool(), None);
        assert_eq!(StackValue::None.as_bool(), None);
    }

    #[test]
    fn stack_value_as_heap_ref() {
        assert_eq!(StackValue::HeapRef(7).as_heap_ref(), Some(7));
        assert_eq!(StackValue::Float(1.0).as_heap_ref(), None);
        assert_eq!(StackValue::None.as_heap_ref(), None);
    }

    // ── is_truthy ─────────────────────────────────────────────────────────────

    #[test]
    fn truthy_float() {
        let h = Heap::new();
        assert!(is_truthy(StackValue::Float(1.0), &h));
        assert!(is_truthy(StackValue::Float(-0.001), &h));
        assert!(!is_truthy(StackValue::Float(0.0), &h));
        assert!(!is_truthy(StackValue::Float(f64::NAN), &h));
    }

    #[test]
    fn truthy_bool() {
        let h = Heap::new();
        assert!(is_truthy(StackValue::Bool(true), &h));
        assert!(!is_truthy(StackValue::Bool(false), &h));
    }

    #[test]
    fn truthy_none() {
        let h = Heap::new();
        assert!(!is_truthy(StackValue::None, &h));
    }

    #[test]
    fn truthy_str() {
        let mut h = Heap::new();
        let empty = alloc_str(&mut h, "");
        let nonempty = alloc_str(&mut h, "hi");
        assert!(!is_truthy(empty, &h));
        assert!(is_truthy(nonempty, &h));
    }

    #[test]
    fn truthy_list() {
        let mut h = Heap::new();
        let empty = alloc_list(&mut h, vec![]);
        let nonempty = alloc_list(&mut h, vec![StackValue::Float(1.0)]);
        assert!(!is_truthy(empty, &h));
        assert!(is_truthy(nonempty, &h));
    }

    #[test]
    fn truthy_vec2_and_closure_are_true() {
        let mut h = Heap::new();
        let v2 = h.alloc(HeapObject::Vec2(0.0, 0.0));
        let cl = h.alloc(HeapObject::Closure(ClosureObj { chunk_index: 0, upvalues: vec![] }));
        assert!(is_truthy(v2, &h));
        assert!(is_truthy(cl, &h));
    }

    // ── values_equal ──────────────────────────────────────────────────────────

    #[test]
    fn equal_floats_bitwise() {
        let h = Heap::new();
        assert!(values_equal(StackValue::Float(1.5), StackValue::Float(1.5), &h));
        assert!(!values_equal(StackValue::Float(1.0), StackValue::Float(2.0), &h));
        // NaN == NaN via bits
        let nan = f64::NAN;
        assert!(values_equal(StackValue::Float(nan), StackValue::Float(nan), &h));
    }

    #[test]
    fn equal_bools() {
        let h = Heap::new();
        assert!(values_equal(StackValue::Bool(true), StackValue::Bool(true), &h));
        assert!(!values_equal(StackValue::Bool(true), StackValue::Bool(false), &h));
    }

    #[test]
    fn equal_none() {
        let h = Heap::new();
        assert!(values_equal(StackValue::None, StackValue::None, &h));
    }

    #[test]
    fn equal_str() {
        let mut h = Heap::new();
        let a = alloc_str(&mut h, "hello");
        let b = alloc_str(&mut h, "hello");
        let c = alloc_str(&mut h, "world");
        assert!(values_equal(a, b, &h));
        assert!(!values_equal(a, c, &h));
    }

    #[test]
    fn equal_vec2_component_wise() {
        let mut h = Heap::new();
        let a = h.alloc(HeapObject::Vec2(1.0, 2.0));
        let b = h.alloc(HeapObject::Vec2(1.0, 2.0));
        let c = h.alloc(HeapObject::Vec2(1.0, 3.0));
        assert!(values_equal(a, b, &h));
        assert!(!values_equal(a, c, &h));
    }

    #[test]
    fn equal_vec3() {
        let mut h = Heap::new();
        let a = h.alloc(HeapObject::Vec3(1.0, 2.0, 3.0));
        let b = h.alloc(HeapObject::Vec3(1.0, 2.0, 3.0));
        let c = h.alloc(HeapObject::Vec3(1.0, 2.0, 4.0));
        assert!(values_equal(a, b, &h));
        assert!(!values_equal(a, c, &h));
    }

    #[test]
    fn equal_vec4() {
        let mut h = Heap::new();
        let a = h.alloc(HeapObject::Vec4(1.0, 2.0, 3.0, 4.0));
        let b = h.alloc(HeapObject::Vec4(1.0, 2.0, 3.0, 4.0));
        let c = h.alloc(HeapObject::Vec4(0.0, 2.0, 3.0, 4.0));
        assert!(values_equal(a, b, &h));
        assert!(!values_equal(a, c, &h));
    }

    #[test]
    fn equal_color() {
        let mut h = Heap::new();
        let a = h.alloc(HeapObject::Color { r: 1.0, g: 0.5, b: 0.0, a: 1.0 });
        let b = h.alloc(HeapObject::Color { r: 1.0, g: 0.5, b: 0.0, a: 1.0 });
        let c = h.alloc(HeapObject::Color { r: 1.0, g: 0.5, b: 0.0, a: 0.5 });
        assert!(values_equal(a, b, &h));
        assert!(!values_equal(a, c, &h));
    }

    #[test]
    fn equal_list_same_elements() {
        let mut h = Heap::new();
        let a = alloc_list(&mut h, vec![StackValue::Float(1.0), StackValue::Float(2.0)]);
        let b = alloc_list(&mut h, vec![StackValue::Float(1.0), StackValue::Float(2.0)]);
        assert!(values_equal(a, b, &h));
    }

    #[test]
    fn equal_list_different_elements() {
        let mut h = Heap::new();
        let a = alloc_list(&mut h, vec![StackValue::Float(1.0)]);
        let b = alloc_list(&mut h, vec![StackValue::Float(2.0)]);
        assert!(!values_equal(a, b, &h));
    }

    #[test]
    fn equal_list_different_length() {
        let mut h = Heap::new();
        let a = alloc_list(&mut h, vec![StackValue::Float(1.0), StackValue::Float(2.0)]);
        let b = alloc_list(&mut h, vec![StackValue::Float(1.0)]);
        assert!(!values_equal(a, b, &h));
    }

    #[test]
    fn equal_enum_variant_same() {
        let mut h = Heap::new();
        let a = h.alloc(HeapObject::EnumVariant(EnumVariantObj {
            enum_name: "Color".into(),
            variant: "Red".into(),
            field_names: vec![],
            field_values: vec![StackValue::Float(1.0)],
        }));
        let b = h.alloc(HeapObject::EnumVariant(EnumVariantObj {
            enum_name: "Color".into(),
            variant: "Red".into(),
            field_names: vec![],
            field_values: vec![StackValue::Float(1.0)],
        }));
        assert!(values_equal(a, b, &h));
    }

    #[test]
    fn equal_enum_variant_different_variant() {
        let mut h = Heap::new();
        let a = h.alloc(HeapObject::EnumVariant(EnumVariantObj {
            enum_name: "Color".into(),
            variant: "Red".into(),
            field_names: vec![],
            field_values: vec![],
        }));
        let b = h.alloc(HeapObject::EnumVariant(EnumVariantObj {
            enum_name: "Color".into(),
            variant: "Blue".into(),
            field_names: vec![],
            field_values: vec![],
        }));
        assert!(!values_equal(a, b, &h));
    }

    #[test]
    fn equal_enum_variant_different_fields() {
        let mut h = Heap::new();
        let a = h.alloc(HeapObject::EnumVariant(EnumVariantObj {
            enum_name: "E".into(),
            variant: "V".into(),
            field_names: vec![],
            field_values: vec![StackValue::Float(1.0)],
        }));
        let b = h.alloc(HeapObject::EnumVariant(EnumVariantObj {
            enum_name: "E".into(),
            variant: "V".into(),
            field_names: vec![],
            field_values: vec![StackValue::Float(2.0)],
        }));
        assert!(!values_equal(a, b, &h));
    }

    #[test]
    fn equal_object_same() {
        let mut h = Heap::new();
        let a = h.alloc(HeapObject::Object(StructObj {
            type_name: "Foo".into(),
            struct_def_idx: 0,
            fields: vec![StackValue::Float(42.0)],
        }));
        let b = h.alloc(HeapObject::Object(StructObj {
            type_name: "Foo".into(),
            struct_def_idx: 0,
            fields: vec![StackValue::Float(42.0)],
        }));
        assert!(values_equal(a, b, &h));
    }

    #[test]
    fn equal_object_different_type() {
        let mut h = Heap::new();
        let a = h.alloc(HeapObject::Object(StructObj {
            type_name: "Foo".into(),
            struct_def_idx: 0,
            fields: vec![],
        }));
        let b = h.alloc(HeapObject::Object(StructObj {
            type_name: "Bar".into(),
            struct_def_idx: 1,
            fields: vec![],
        }));
        assert!(!values_equal(a, b, &h));
    }

    #[test]
    fn equal_same_heap_ref() {
        let mut h = Heap::new();
        let r = alloc_str(&mut h, "shared");
        // same index → equal without deep compare
        assert!(values_equal(r, r, &h));
    }

    #[test]
    fn equal_cross_type_false() {
        let h = Heap::new();
        assert!(!values_equal(StackValue::Float(1.0), StackValue::Bool(true), &h));
        assert!(!values_equal(StackValue::None, StackValue::Bool(false), &h));
        assert!(!values_equal(StackValue::Float(0.0), StackValue::None, &h));
    }

    // ── display ───────────────────────────────────────────────────────────────

    #[test]
    fn display_float_whole_number() {
        let h = Heap::new();
        assert_eq!(display(StackValue::Float(5.0), &h), "5");
        assert_eq!(display(StackValue::Float(-3.0), &h), "-3");
        assert_eq!(display(StackValue::Float(0.0), &h), "0");
    }

    #[test]
    fn display_float_decimal() {
        let h = Heap::new();
        assert_eq!(display(StackValue::Float(5.5), &h), "5.5");
        assert_eq!(display(StackValue::Float(0.1), &h), "0.1");
    }

    #[test]
    fn display_float_large_uses_default() {
        let h = Heap::new();
        // 1e16 >= 1e15, so use default float format
        let s = display(StackValue::Float(1e16), &h);
        assert!(s.contains("e") || s.contains("0000"), "got: {s}");
    }

    #[test]
    fn display_bool() {
        let h = Heap::new();
        assert_eq!(display(StackValue::Bool(true), &h), "true");
        assert_eq!(display(StackValue::Bool(false), &h), "false");
    }

    #[test]
    fn display_none() {
        let h = Heap::new();
        assert_eq!(display(StackValue::None, &h), "none");
    }

    #[test]
    fn display_str() {
        let mut h = Heap::new();
        let r = alloc_str(&mut h, "hello world");
        assert_eq!(display(r, &h), "hello world");
    }

    #[test]
    fn display_color_three_decimals() {
        let mut h = Heap::new();
        let r = h.alloc(HeapObject::Color { r: 1.0, g: 0.5, b: 0.0, a: 0.75 });
        assert_eq!(display(r, &h), "color(1.000, 0.500, 0.000, 0.750)");
    }

    #[test]
    fn display_list_nested() {
        let mut h = Heap::new();
        let inner = alloc_list(&mut h, vec![StackValue::Float(1.0), StackValue::Float(2.0)]);
        let outer = alloc_list(&mut h, vec![inner, StackValue::Float(3.0)]);
        assert_eq!(display(outer, &h), "[[1, 2], 3]");
    }

    #[test]
    fn display_enum_variant() {
        let mut h = Heap::new();
        let r = h.alloc(HeapObject::EnumVariant(EnumVariantObj {
            enum_name: "Shape".into(),
            variant: "Circle".into(),
            field_names: vec![],
            field_values: vec![],
        }));
        assert_eq!(display(r, &h), "Shape.Circle");
    }

    #[test]
    fn display_res_ok_and_err() {
        let mut h = Heap::new();
        let ok = h.alloc(HeapObject::ResOk(StackValue::Float(42.0)));
        assert_eq!(display(ok, &h), "ok(42)");

        let err = h.alloc(HeapObject::ResErr("file not found".into()));
        assert_eq!(display(err, &h), "err(file not found)");
    }

    #[test]
    fn display_native_fn_ref() {
        let mut h = Heap::new();
        let r = h.alloc(HeapObject::NativeFnRef(3));
        assert_eq!(display(r, &h), "<fn:3>");
    }

    #[test]
    fn display_vec2_vec3_vec4() {
        let mut h = Heap::new();
        let v2 = h.alloc(HeapObject::Vec2(1.0, 2.0));
        let v3 = h.alloc(HeapObject::Vec3(1.0, 2.0, 3.0));
        let v4 = h.alloc(HeapObject::Vec4(1.0, 2.0, 3.0, 4.0));
        assert_eq!(display(v2, &h), "vec2(1, 2)");
        assert_eq!(display(v3, &h), "vec3(1, 2, 3)");
        assert_eq!(display(v4, &h), "vec4(1, 2, 3, 4)");
    }

    #[test]
    fn display_closure_state_shape_transform() {
        let mut h = Heap::new();
        let cl = h.alloc(HeapObject::Closure(ClosureObj { chunk_index: 0, upvalues: vec![] }));
        let st = h.alloc(HeapObject::State(StateObj { fields: vec![] }));
        assert_eq!(display(cl, &h), "<closure>");
        assert_eq!(display(st, &h), "<state>");
    }

    #[test]
    fn display_object() {
        let mut h = Heap::new();
        let o = h.alloc(HeapObject::Object(StructObj {
            type_name: "Player".into(),
            struct_def_idx: 0,
            fields: vec![],
        }));
        assert_eq!(display(o, &h), "Player { ... }");
    }

    #[test]
    fn display_input() {
        let mut h = Heap::new();
        let i = h.alloc(HeapObject::Input(InputObj {
            dt: 0.016,
            mouse_x: 0.0, mouse_y: 0.0,
            mouse_down: false, mouse_pressed: false, mouse_released: false,
            key_pressed: String::new(), key_down: String::new(), key_released: String::new(),
        }));
        assert_eq!(display(i, &h), "input(dt=0.016)");
    }

    #[test]
    fn display_iterator() {
        let mut h = Heap::new();
        let it = h.alloc(HeapObject::Iterator(IteratorObj { items: vec![], index: 0 }));
        assert_eq!(display(it, &h), "<iterator>");
    }

    // ── deep_clone ────────────────────────────────────────────────────────────

    #[test]
    fn deep_clone_primitives_unchanged() {
        let mut h = Heap::new();
        assert!(matches!(deep_clone(StackValue::Float(3.14), &mut h), StackValue::Float(f) if f == 3.14));
        assert!(matches!(deep_clone(StackValue::Bool(true), &mut h), StackValue::Bool(true)));
        assert!(matches!(deep_clone(StackValue::None, &mut h), StackValue::None));
        assert_eq!(h.len(), 0); // no allocations for primitives
    }

    #[test]
    fn deep_clone_list_is_independent() {
        let mut h = Heap::new();
        let orig = alloc_list(&mut h, vec![StackValue::Float(1.0), StackValue::Float(2.0)]);
        let cloned = deep_clone(orig, &mut h);

        // Verify they are different heap slots
        let orig_idx = orig.as_heap_ref().unwrap();
        let clone_idx = cloned.as_heap_ref().unwrap();
        assert_ne!(orig_idx, clone_idx);

        // Mutate original list
        if let HeapObject::List(items) = h.get_mut(orig_idx) {
            items.push(StackValue::Float(99.0));
        }

        // Clone should be unaffected
        let clone_len = if let HeapObject::List(items) = h.get(clone_idx) {
            items.len()
        } else {
            panic!("not a list")
        };
        assert_eq!(clone_len, 2);
    }

    #[test]
    fn deep_clone_nested_list() {
        let mut h = Heap::new();
        let inner = alloc_list(&mut h, vec![StackValue::Float(10.0)]);
        let outer = alloc_list(&mut h, vec![inner]);
        let cloned_outer = deep_clone(outer, &mut h);

        // Both outer and inner should be freshly allocated
        let cloned_outer_idx = cloned_outer.as_heap_ref().unwrap();
        let orig_inner_idx = inner.as_heap_ref().unwrap();

        let cloned_inner = if let HeapObject::List(items) = h.get(cloned_outer_idx) {
            items[0]
        } else {
            panic!("not a list")
        };
        let cloned_inner_idx = cloned_inner.as_heap_ref().unwrap();
        assert_ne!(cloned_inner_idx, orig_inner_idx);
    }

    #[test]
    fn deep_clone_object() {
        let mut h = Heap::new();
        let orig = h.alloc(HeapObject::Object(StructObj {
            type_name: "Foo".into(),
            struct_def_idx: 0,
            fields: vec![StackValue::Float(1.0)],
        }));
        let cloned = deep_clone(orig, &mut h);
        let orig_idx = orig.as_heap_ref().unwrap();
        let clone_idx = cloned.as_heap_ref().unwrap();
        assert_ne!(orig_idx, clone_idx);

        // Mutate clone fields
        if let HeapObject::Object(s) = h.get_mut(clone_idx) {
            s.fields[0] = StackValue::Float(99.0);
        }
        // Original unchanged
        if let HeapObject::Object(s) = h.get(orig_idx) {
            assert!(matches!(s.fields[0], StackValue::Float(f) if f == 1.0));
        }
    }

    #[test]
    fn deep_clone_closure_upvalues() {
        let mut h = Heap::new();
        let orig = h.alloc(HeapObject::Closure(ClosureObj {
            chunk_index: 5,
            upvalues: vec![StackValue::Float(7.0)],
        }));
        let cloned = deep_clone(orig, &mut h);
        let orig_idx = orig.as_heap_ref().unwrap();
        let clone_idx = cloned.as_heap_ref().unwrap();
        assert_ne!(orig_idx, clone_idx);

        if let HeapObject::Closure(c) = h.get(clone_idx) {
            assert_eq!(c.chunk_index, 5);
            assert!(matches!(c.upvalues[0], StackValue::Float(f) if f == 7.0));
        }
    }

    #[test]
    fn deep_clone_str_is_new_allocation() {
        let mut h = Heap::new();
        let orig = alloc_str(&mut h, "hello");
        let cloned = deep_clone(orig, &mut h);
        let orig_idx = orig.as_heap_ref().unwrap();
        let clone_idx = cloned.as_heap_ref().unwrap();
        assert_ne!(orig_idx, clone_idx);
        assert!(matches!(h.get(clone_idx), HeapObject::Str(s) if s == "hello"));
    }

    #[test]
    fn deep_clone_res_ok() {
        let mut h = Heap::new();
        let orig = h.alloc(HeapObject::ResOk(StackValue::Float(5.0)));
        let cloned = deep_clone(orig, &mut h);
        let clone_idx = cloned.as_heap_ref().unwrap();
        assert!(matches!(h.get(clone_idx), HeapObject::ResOk(StackValue::Float(f)) if *f == 5.0));
    }
}
