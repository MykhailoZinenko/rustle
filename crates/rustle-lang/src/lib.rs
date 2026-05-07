pub mod syntax;
pub mod types;
pub mod analysis;
pub mod error;
pub mod namespaces;
pub mod vm;

pub use types::draw::{CoordMeta, DrawCommand, Origin, RenderMode, ShapeData, ShapeDesc, TransformData, origin_offset};
pub use error::{Error, ErrorCode, RuntimeError};
pub use syntax::token::{Token, TokenKind};

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use analysis::resolve;
use namespaces::NamespaceRegistry;
use vm::value::StackValue;
use vm::heap::Heap;
use vm::CompiledProgram;
use vm::natives;

// ─── Value — public API type for state access ────────────────────────────────

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Value {
    Float(f64),
    Bool(bool),
    Str(String),
    Vec2(f64, f64),
    Vec3(f64, f64, f64),
    Vec4(f64, f64, f64, f64),
    Mat3(Box<[f64; 9]>),
    Mat4(Box<[f64; 16]>),
    Color { r: f64, g: f64, b: f64, a: f64 },
    List(Rc<RefCell<Vec<Value>>>),
    Shape(ShapeData),
    Transform(TransformData),
    RenderMode(RenderMode),
    ResOk(Box<Value>),
    ResErr(String),
    EnumVariant { enum_name: String, variant: String, fields: HashMap<String, Value> },
    Object { type_name: String, fields: HashMap<String, Value> },
    Input { dt: f64, mouse_x: f64, mouse_y: f64, mouse_down: bool, mouse_pressed: bool, mouse_released: bool, key_pressed: String, key_down: String, key_released: String },
    None,
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Float(v) => {
                if v.fract() == 0.0 && v.abs() < 1e15 { write!(f, "{}", *v as i64) }
                else { write!(f, "{v}") }
            }
            Value::Bool(b) => write!(f, "{b}"),
            Value::Str(s) => write!(f, "{s}"),
            Value::Vec2(x, y) => write!(f, "vec2({x}, {y})"),
            Value::Vec3(x, y, z) => write!(f, "vec3({x}, {y}, {z})"),
            Value::Vec4(x, y, z, w) => write!(f, "vec4({x}, {y}, {z}, {w})"),
            Value::Color { r, g, b, a } => write!(f, "color({r:.3}, {g:.3}, {b:.3}, {a:.3})"),
            Value::Mat3(_) => write!(f, "mat3(...)"),
            Value::Mat4(_) => write!(f, "mat4(...)"),
            Value::List(rc) => {
                let items = rc.borrow();
                write!(f, "[")?;
                for (i, v) in items.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{v}")?;
                }
                write!(f, "]")
            }
            Value::Shape(_) => write!(f, "<shape>"),
            Value::Transform(_) => write!(f, "<transform>"),
            Value::RenderMode(_) => write!(f, "<render_mode>"),
            Value::ResOk(v) => write!(f, "ok({v})"),
            Value::ResErr(e) => write!(f, "err({e})"),
            Value::Object { type_name, fields } => {
                let parts: Vec<String> = fields.iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect();
                write!(f, "{type_name} {{ {} }}", parts.join(", "))
            }
            Value::EnumVariant { enum_name, variant, .. } => write!(f, "{enum_name}.{variant}"),
            Value::Input { dt, .. } => write!(f, "input(dt={dt})"),
            Value::None => write!(f, "none"),
        }
    }
}

// ─── Public API types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct State(pub(crate) HashMap<String, Value>);

impl State {
    pub fn get(&self, key: &str) -> Option<&Value> { self.0.get(key) }
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Value)> { self.0.iter() }
    pub fn len(&self) -> usize { self.0.len() }
    pub fn is_empty(&self) -> bool { self.0.is_empty() }
}

#[derive(Debug, Clone)]
pub struct Input {
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

impl Default for Input {
    fn default() -> Self {
        Self {
            dt: 0.0, mouse_x: 0.0, mouse_y: 0.0,
            mouse_down: false, mouse_pressed: false, mouse_released: false,
            key_pressed: String::new(), key_down: String::new(), key_released: String::new(),
        }
    }
}

pub struct Program {
    pub(crate) compiled: Arc<CompiledProgram>,
    pub(crate) native_table: Vec<natives::NativeFunc>,
    #[allow(dead_code)]
    pub(crate) registry: NamespaceRegistry,
}

/// Parse and type-check source text. Returns a compiled program ready for execution.
///
/// # Errors
/// Returns `Vec<Error>` if the source has lexer, parser, or semantic errors.
///
/// # Examples
/// ```
/// let program = rustle_lang::compile("state { let x: float = 0.0 }").unwrap();
/// ```
pub fn compile(source: &str) -> Result<Program, Vec<Error>> {
    let tokens = syntax::lexer::Lexer::new(source).tokenize()?;
    let ast = syntax::parser::Parser::new(tokens).parse()?;
    let registry = NamespaceRegistry::standard();
    resolve(&ast, &registry)?;
    let compiled = vm::compiler::Compiler::compile(&ast, &registry);
    let native_table = natives::native_table();
    Ok(Program { compiled: Arc::new(compiled), native_table, registry })
}

// ─── StackValue → Value bridge (public API boundary) ─────────────────────────

fn sv_to_value(sv: StackValue, heap: &Heap, prog: &CompiledProgram) -> Value {
    match sv {
        StackValue::Float(f) => Value::Float(f),
        StackValue::Bool(b) => Value::Bool(b),
        StackValue::None => Value::None,
        StackValue::HeapRef(idx) => heap_to_value(heap.get(idx), heap, prog),
    }
}

fn heap_to_value(obj: &vm::value::HeapObject, heap: &Heap, prog: &CompiledProgram) -> Value {
    use vm::value::HeapObject;
    match obj {
        HeapObject::Str(s) => Value::Str(s.clone()),
        HeapObject::Vec2(x, y) => Value::Vec2(*x, *y),
        HeapObject::Vec3(x, y, z) => Value::Vec3(*x, *y, *z),
        HeapObject::Vec4(x, y, z, w) => Value::Vec4(*x, *y, *z, *w),
        HeapObject::Color { r, g, b, a } => Value::Color { r: *r, g: *g, b: *b, a: *a },
        HeapObject::Mat3(m) => Value::Mat3(m.clone()),
        HeapObject::Mat4(m) => Value::Mat4(m.clone()),
        HeapObject::List(items) => {
            let vals: Vec<Value> = items.iter().map(|sv| sv_to_value(*sv, heap, prog)).collect();
            Value::List(Rc::new(RefCell::new(vals)))
        }
        HeapObject::Shape(sd) => Value::Shape(sd.clone()),
        HeapObject::Transform(td) => Value::Transform(td.clone()),
        HeapObject::RenderMode(rm) => Value::RenderMode(rm.clone()),
        HeapObject::ResOk(sv) => Value::ResOk(Box::new(sv_to_value(*sv, heap, prog))),
        HeapObject::ResErr(s) => Value::ResErr(s.clone()),
        HeapObject::Object(obj) => {
            let def = &prog.struct_defs[obj.struct_def_idx as usize];
            let mut fields = HashMap::new();
            for (i, val) in obj.fields.iter().enumerate() {
                let name = def.field_names.get(i).cloned().unwrap_or_else(|| format!("_{i}"));
                fields.insert(name, sv_to_value(*val, heap, prog));
            }
            Value::Object { type_name: obj.type_name.clone(), fields }
        }
        HeapObject::EnumVariant(ev) => {
            let fields: HashMap<String, Value> = ev.field_names.iter().zip(&ev.field_values)
                .map(|(k, v)| (k.clone(), sv_to_value(*v, heap, prog))).collect();
            Value::EnumVariant { enum_name: ev.enum_name.clone(), variant: ev.variant.clone(), fields }
        }
        HeapObject::Input(io) => Value::Input {
            dt: io.dt, mouse_x: io.mouse_x, mouse_y: io.mouse_y,
            mouse_down: io.mouse_down, mouse_pressed: io.mouse_pressed, mouse_released: io.mouse_released,
            key_pressed: io.key_pressed.clone(), key_down: io.key_down.clone(), key_released: io.key_released.clone(),
        },
        _ => Value::None,
    }
}

// ─── Runtime ─────────────────────────────────────────────────────────────────

pub struct Runtime {
    program: Program,
    vm: vm::vm::Vm,
    state_ref: u32,
    cached_globals: Vec<StackValue>,
    has_on_update: bool,
    #[allow(dead_code)]
    cancel: Option<Arc<AtomicBool>>,
}

impl Runtime {
    pub fn new(program: Program) -> Result<Self, RuntimeError> {
        Self::new_inner(program, None)
    }

    pub fn new_cancellable(program: Program, cancel: Arc<AtomicBool>) -> Result<Self, RuntimeError> {
        Self::new_inner(program, Some(cancel))
    }

    fn new_inner(program: Program, cancel: Option<Arc<AtomicBool>>) -> Result<Self, RuntimeError> {
        let mut vm = vm::vm::Vm::new(program.compiled.clone(), program.native_table.clone());
        if let Some(ref c) = cancel { vm.set_cancel(c.clone()); }
        vm.run_chunk(0)?;
        let cached_globals = vm.globals.clone();
        let sv = vm.create_state(program.compiled.state_fields.len());
        let state_ref = sv.as_heap_ref().unwrap();
        if let Some(init_chunk) = program.compiled.state_init_chunk {
            vm.run_lifecycle(init_chunk, &[sv])?;
        }
        if let Some(init_chunk) = program.compiled.on_init_chunk {
            vm.run_lifecycle(init_chunk, &[StackValue::HeapRef(state_ref)])?;
        }
        let has_on_update = program.compiled.on_update_chunk.is_some();
        Ok(Self { program, vm, state_ref, cached_globals, has_on_update, cancel })
    }

    pub fn tick(&mut self, input: &Input) -> Result<Vec<DrawCommand>, RuntimeError> {
        self.vm.stack.clear();
        self.vm.frames_clear();
        self.vm.output.clear();
        self.vm.globals = self.cached_globals.clone();
        if self.has_on_update {
            if let Some(update_chunk) = self.program.compiled.on_update_chunk {
                let input_sv = self.vm.heap.alloc(vm::value::HeapObject::Input(vm::value::InputObj {
                    dt: input.dt, mouse_x: input.mouse_x, mouse_y: input.mouse_y,
                    mouse_down: input.mouse_down, mouse_pressed: input.mouse_pressed, mouse_released: input.mouse_released,
                    key_pressed: input.key_pressed.clone(), key_down: input.key_down.clone(), key_released: input.key_released.clone(),
                }));
                self.vm.run_lifecycle(update_chunk, &[StackValue::HeapRef(self.state_ref), input_sv])?;
            }
        } else {
            self.vm.run_chunk(0)?;
        }
        Ok(self.vm.take_output())
    }

    #[must_use]
    pub fn state(&self) -> State {
        let mut map = HashMap::new();
        if let vm::value::HeapObject::State(state_obj) = self.vm.heap.get(self.state_ref) {
            for (i, name) in self.program.compiled.state_fields.iter().enumerate() {
                if i < state_obj.fields.len() {
                    map.insert(name.clone(), sv_to_value(state_obj.fields[i], &self.vm.heap, &self.program.compiled));
                }
            }
        }
        State(map)
    }

    pub fn exit(&mut self) -> Result<(), RuntimeError> {
        if let Some(exit_chunk) = self.program.compiled.on_exit_chunk {
            self.vm.run_lifecycle(exit_chunk, &[StackValue::HeapRef(self.state_ref)])?;
        }
        Ok(())
    }
}
