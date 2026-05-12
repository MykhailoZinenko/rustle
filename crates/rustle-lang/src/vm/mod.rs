pub mod opcode;
pub mod chunk;
pub mod value;
pub mod heap;
pub mod ops;
pub mod fields;
pub mod methods;

pub mod natives;
pub mod compiler;

pub mod vm;

pub mod list_ops;
pub mod util;

use std::collections::HashMap;

use chunk::Chunk;

/// A fully compiled Rustle program, ready for execution by the VM.
#[derive(Debug, Clone)]
pub struct CompiledProgram {
    /// All compiled function bodies. Index 0 is always the top-level chunk.
    pub chunks: Vec<Chunk>,
    /// Constant pool for non-string primitives (floats, bools, etc.).
    pub constants: Vec<value::StackValue>,
    /// Interned string pool shared across all chunks.
    pub strings: Vec<String>,
    /// Native function names, used for dispatch at `CallNative` instructions.
    pub natives: Vec<String>,
    /// Chunk index for the `state {}` initializer, if present.
    pub state_init_chunk: Option<u16>,
    /// Chunk index for `fn on_init(s)`, if present.
    pub on_init_chunk: Option<u16>,
    /// Chunk index for `fn on_update(s, input)`, if present.
    pub on_update_chunk: Option<u16>,
    /// Chunk index for `fn on_exit(s)`, if present.
    pub on_exit_chunk: Option<u16>,
    /// Ordered list of `state {}` field names.
    pub state_fields: Vec<String>,
    /// Compiled struct definitions.
    pub struct_defs: Vec<CompiledStructDef>,
    /// Compiled enum definitions.
    pub enum_defs: Vec<CompiledEnumDef>,
    /// Number of global variable slots.
    pub global_count: u16,
}

/// Compiled definition for a user-defined struct.
#[derive(Debug, Clone)]
pub struct CompiledStructDef {
    pub name: String,
    pub field_names: Vec<String>,
    pub methods: HashMap<String, CompiledMethodDef>,
}

/// A compiled method within a struct definition.
#[derive(Debug, Clone)]
pub struct CompiledMethodDef {
    /// Index of the method's chunk in `CompiledProgram::chunks`.
    pub chunk_index: u16,
    /// Number of explicit parameters (not counting the implicit `this`).
    pub param_count: u8,
    pub is_public: bool,
}

/// Compiled definition for a user-defined enum.
#[derive(Debug, Clone)]
pub struct CompiledEnumDef {
    pub name: String,
    pub variants: Vec<CompiledEnumVariant>,
}

/// One variant within a compiled enum definition.
#[derive(Debug, Clone)]
pub struct CompiledEnumVariant {
    pub name: String,
    pub field_names: Vec<String>,
}
