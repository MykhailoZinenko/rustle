#![expect(clippy::cast_possible_truncation, reason = "VM indices are guaranteed small by construction")]
#![expect(clippy::cast_possible_wrap, reason = "jump offsets within chunk bounds")]
#![expect(clippy::match_same_arms, reason = "opcode dispatch arms kept separate for clarity")]

use std::collections::HashMap;

use crate::namespaces::{ExportKind, NamespaceRegistry};
use crate::syntax::ast::{
    self, AssignTarget, BinOp, Expr, InterpolPart, Item, MatchPattern, PrintLevel, Span, Stmt,
    UnOp, Visibility,
};

use super::chunk::Chunk;
use super::natives::{self, NativeFunc};
use super::opcode::Op;
use super::util::lookup_color;
use super::value::StackValue;
use super::{CompiledEnumDef, CompiledEnumVariant, CompiledMethodDef, CompiledProgram, CompiledStructDef};

// ─── Data structures ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct Local {
    name: String,
    depth: u32,
    is_captured: bool,
}

#[derive(Debug, Clone, Copy)]
struct Upvalue {
    index: u16,
    is_local: bool,
}

struct LoopContext {
    start_ip: usize,
    continue_ip: usize,
    break_patches: Vec<usize>,
    continue_patches: Vec<usize>,
    scope_depth: u32,
}

pub(crate) struct FnCompiler {
    pub chunk: Chunk,
    locals: Vec<Local>,
    upvalues: Vec<Upvalue>,
    scope_depth: u32,
    loop_stack: Vec<LoopContext>,
}

impl FnCompiler {
    pub fn new(name: &str) -> Self {
        Self {
            chunk: Chunk::new(name),
            locals: Vec::new(),
            upvalues: Vec::new(),
            scope_depth: 0,
            loop_stack: Vec::new(),
        }
    }

    pub fn begin_scope(&mut self) {
        self.scope_depth += 1;
    }

    /// End the current scope: pop locals that belong to it and emit Pop
    /// instructions into the owned chunk. Returns the number of locals popped.
    pub fn end_scope(&mut self, line: u32) -> u16 {
        self.scope_depth -= 1;
        let mut count = 0u16;
        while let Some(local) = self.locals.last() {
            if local.depth <= self.scope_depth {
                break;
            }
            self.chunk.emit(Op::Pop, line);
            self.locals.pop();
            count += 1;
        }
        count
    }

    pub fn declare_local(&mut self, name: &str) -> u16 {
        let slot = self.locals.len() as u16;
        self.locals.push(Local {
            name: name.to_string(),
            depth: self.scope_depth,
            is_captured: false,
        });
        slot
    }

    pub fn resolve_local(&self, name: &str) -> Option<u16> {
        // Search from innermost scope outward (last match wins).
        self.locals
            .iter()
            .enumerate()
            .rev()
            .find(|(_, l)| l.name == name)
            .map(|(i, _)| i as u16)
    }
}

#[derive(Debug)]
pub(crate) enum VarLocation {
    Local(u16),
    Upvalue(u16),
    Global(u16),
    Function(u16), // chunk index
}

// ─── Compiler ────────────────────────────────────────────────────────────────

pub struct Compiler<'a> {
    pub(crate) program: &'a ast::Program,
    pub(crate) registry: &'a NamespaceRegistry,
    pub(crate) fn_stack: Vec<FnCompiler>,
    pub(crate) chunks: Vec<Chunk>,
    pub(crate) constants: Vec<StackValue>,
    const_map: HashMap<u64, u16>,
    pub(crate) strings: Vec<String>,
    string_map: HashMap<String, u16>,
    native_table: Vec<NativeFunc>,
    native_map: HashMap<String, u16>,
    pub(crate) globals: Vec<String>,
    global_map: HashMap<String, u16>,
    pub(crate) state_fields: Vec<String>,
    state_map: HashMap<String, u16>,
    pub(crate) fn_chunks: HashMap<String, u16>,
    pub(crate) struct_defs: Vec<CompiledStructDef>,
    struct_map: HashMap<String, usize>,
    pub(crate) enum_defs: Vec<CompiledEnumDef>,
    enum_map: HashMap<String, usize>,
    compiling_top_level: bool,
    state_init_chunk: Option<u16>,
    on_init_chunk: Option<u16>,
    on_update_chunk: Option<u16>,
    on_exit_chunk: Option<u16>,
}

impl<'a> Compiler<'a> {
    #[must_use] 
    pub fn new(program: &'a ast::Program, registry: &'a NamespaceRegistry) -> Self {
        let table = natives::native_table();
        let mut native_map = HashMap::new();
        for (i, f) in table.iter().enumerate() {
            native_map.insert(f.name.to_string(), i as u16);
        }
        Self {
            program,
            registry,
            fn_stack: Vec::new(),
            chunks: Vec::new(),
            constants: Vec::new(),
            const_map: HashMap::new(),
            strings: Vec::new(),
            string_map: HashMap::new(),
            native_table: table,
            native_map,
            globals: Vec::new(),
            global_map: HashMap::new(),
            state_fields: Vec::new(),
            state_map: HashMap::new(),
            fn_chunks: HashMap::new(),
            struct_defs: Vec::new(),
            struct_map: HashMap::new(),
            enum_defs: Vec::new(),
            enum_map: HashMap::new(),
            compiling_top_level: false,
            state_init_chunk: None,
            on_init_chunk: None,
            on_update_chunk: None,
            on_exit_chunk: None,
        }
    }

    // ── Access helpers ───────────────────────────────────────────────────────

    pub(crate) fn current(&mut self) -> &mut FnCompiler {
        self.fn_stack.last_mut().expect("no active FnCompiler")
    }

    fn current_chunk(&mut self) -> &mut Chunk {
        &mut self.fn_stack.last_mut().expect("no active FnCompiler").chunk
    }

    // ── Emit helpers ────────────────────────────────────────────────────────

    pub fn emit(&mut self, op: Op, line: u32) -> usize {
        self.current_chunk().emit(op, line)
    }

    pub fn emit_constant(&mut self, val: StackValue, line: u32) {
        let idx = self.add_constant(val);
        self.emit(Op::Const(idx), line);
    }

    pub fn add_constant(&mut self, val: StackValue) -> u16 {
        let key = const_dedup_key(val);
        if let Some(&idx) = self.const_map.get(&key) {
            return idx;
        }
        let idx = self.constants.len() as u16;
        self.constants.push(val);
        self.const_map.insert(key, idx);
        idx
    }

    pub fn add_string(&mut self, s: &str) -> u16 {
        if let Some(&idx) = self.string_map.get(s) {
            return idx;
        }
        let idx = self.strings.len() as u16;
        self.strings.push(s.to_string());
        self.string_map.insert(s.to_string(), idx);
        idx
    }

    pub fn add_global(&mut self, name: &str) -> u16 {
        if let Some(&idx) = self.global_map.get(name) {
            return idx;
        }
        let idx = self.globals.len() as u16;
        self.globals.push(name.to_string());
        self.global_map.insert(name.to_string(), idx);
        idx
    }

    // ── Variable resolution ─────────────────────────────────────────────────

    pub(crate) fn resolve_variable(&mut self, name: &str) -> VarLocation {
        let fn_depth = self.fn_stack.len();

        // 1. Local in current function
        if fn_depth > 0
            && let Some(slot) = self.fn_stack[fn_depth - 1].resolve_local(name) {
                return VarLocation::Local(slot);
            }

        // 2. Upvalue (closure capture)
        if fn_depth > 1
            && let Some(uv) = self.resolve_upvalue(fn_depth - 1, name) {
                return VarLocation::Upvalue(uv);
            }

        // 3. Global
        if let Some(&idx) = self.global_map.get(name) {
            return VarLocation::Global(idx);
        }

        // 4. User-defined function → chunk index
        if let Some(&chunk_idx) = self.fn_chunks.get(name) {
            return VarLocation::Function(chunk_idx);
        }

        // 5. Check if it's a known native function
        if self.native_map.contains_key(name) {
            // Will be resolved via global set up by implicit import
            let idx = self.add_global(name);
            return VarLocation::Global(idx);
        }

        // 6. Check if it's a known constant (PI, TAU, colors, render modes, origins)
        if is_known_constant(name) {
            let idx = self.add_global(name);
            return VarLocation::Global(idx);
        }

        // 7. If nothing found, create as global (will be resolved at runtime)
        let idx = self.add_global(name);
        VarLocation::Global(idx)
    }

    fn resolve_upvalue(&mut self, fn_idx: usize, name: &str) -> Option<u16> {
        if fn_idx == 0 {
            return None;
        }

        // Check the enclosing function's locals
        if let Some(local_slot) = self.fn_stack[fn_idx - 1].resolve_local(name) {
            self.fn_stack[fn_idx - 1].locals[local_slot as usize].is_captured = true;
            return Some(self.add_upvalue(fn_idx, local_slot, true));
        }

        // Recursively check further enclosing functions
        if let Some(upvalue_idx) = self.resolve_upvalue(fn_idx - 1, name) {
            return Some(self.add_upvalue(fn_idx, upvalue_idx, false));
        }

        None
    }

    fn add_upvalue(&mut self, fn_idx: usize, index: u16, is_local: bool) -> u16 {
        let upvalues = &self.fn_stack[fn_idx].upvalues;
        // Check if we already captured this
        for (i, uv) in upvalues.iter().enumerate() {
            if uv.index == index && uv.is_local == is_local {
                return i as u16;
            }
        }
        let idx = upvalues.len() as u16;
        self.fn_stack[fn_idx].upvalues.push(Upvalue { index, is_local });
        idx
    }

    /// Emit opcodes to construct a named constant value (PI, TAU, colors, render modes, origins).
    fn emit_constant_value(&mut self, name: &str, line: u32) {
        if let Some(c) = lookup_color(name) {
            self.emit_constant(StackValue::Float(c.r), line);
            self.emit_constant(StackValue::Float(c.g), line);
            self.emit_constant(StackValue::Float(c.b), line);
            self.emit_constant(StackValue::Float(c.a), line);
            self.emit(Op::MakeColor(4), line);
            return;
        }
        match name {
            "PI" => self.emit_constant(StackValue::Float(std::f64::consts::PI), line),
            "TAU" => self.emit_constant(StackValue::Float(std::f64::consts::TAU), line),
            "sdf" => { self.emit(Op::MakeRenderMode(0), line); }
            "fill" => { self.emit(Op::MakeRenderMode(1), line); }
            "outline" => { self.emit(Op::MakeRenderMode(2), line); }
            "center" | "top_left" | "top_right" | "bottom_left" | "bottom_right"
            | "top" | "bottom" | "left" | "right" => {
                let idx = self.add_string(name);
                self.emit(Op::ConstStr(idx), line);
            }
            _ => {
                self.emit_constant(StackValue::None, line);
            }
        }
    }

    // ── Jump helpers ────────────────────────────────────────────────────────

    pub fn emit_jump(&mut self, op: Op, line: u32) -> usize {
        self.current_chunk().emit(op, line)
    }

    pub fn patch_jump_here(&mut self, jump_idx: usize) {
        let current_ip = self.current_chunk().len();
        let offset = (current_ip as i16) - (jump_idx as i16) - 1;
        self.current_chunk().patch_jump(jump_idx, offset);
    }

    fn patch_jump_to(&mut self, jump_idx: usize, target: usize) {
        let offset = (target as i16) - (jump_idx as i16) - 1;
        self.current_chunk().patch_jump(jump_idx, offset);
    }

    fn line_of(span: &Span) -> u32 {
        span.line as u32
    }

    // ── Named arg helpers for native calls ──────────────────────────────────

    fn native_named_param_offset(func_name: &str, param_name: &str) -> Option<usize> {
        match (func_name, param_name) {
            ("circle", "color") => Some(2),
            ("circle", "render") => Some(3),
            ("rect", "color") => Some(2),
            ("rect", "render") => Some(3),
            ("rect", "origin") => Some(4),
            ("line", "color") => Some(2),
            ("line", "render") => Some(3),
            ("polygon", "color") => Some(1),
            ("polygon", "render") => Some(2),
            ("text", "color") => Some(3),
            ("text", "render") => Some(4),
            _ => None,
        }
    }

    fn native_total_params(func_name: &str) -> Option<usize> {
        match func_name {
            "circle" => Some(4),
            "rect" => Some(5),
            "line" => Some(4),
            "polygon" => Some(3),
            "text" => Some(5),
            _ => None,
        }
    }

    // ── Expression compilation ──────────────────────────────────────────────

    pub fn compile_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Float(v, span) => {
                self.emit_constant(StackValue::Float(*v), Self::line_of(span));
            }

            Expr::Bool(v, span) => {
                self.emit_constant(StackValue::Bool(*v), Self::line_of(span));
            }

            Expr::None(span) => {
                self.emit_constant(StackValue::None, Self::line_of(span));
            }

            Expr::StringLit(s, span) => {
                let idx = self.add_string(s);
                self.emit(Op::ConstStr(idx), Self::line_of(span));
            }

            Expr::Interpolated(parts, span) => {
                let line = Self::line_of(span);
                if parts.is_empty() {
                    // Empty interpolation → empty string
                    let idx = self.add_string("");
                    self.emit(Op::ConstStr(idx), line);
                    return;
                }

                let mut count: u8 = 0;
                for part in parts {
                    match part {
                        InterpolPart::Lit(s) => {
                            let idx = self.add_string(s);
                            self.emit(Op::ConstStr(idx), line);
                            count += 1;
                        }
                        InterpolPart::Expr(e) => {
                            self.compile_expr(e);
                            self.emit(Op::CastString, line);
                            count += 1;
                        }
                    }
                }

                if count > 1 {
                    self.emit(Op::Concat(count), line);
                }
                // If count == 1, the single value is already on stack.
            }

            Expr::HexColor(hex, span) => {
                let line = Self::line_of(span);
                let (r, g, b, a) = parse_hex_color(hex);
                self.emit_constant(StackValue::Float(r), line);
                self.emit_constant(StackValue::Float(g), line);
                self.emit_constant(StackValue::Float(b), line);
                self.emit_constant(StackValue::Float(a), line);
                self.emit(Op::MakeColor(4), line);
            }

            Expr::Ident(name, span) => {
                let line = Self::line_of(span);
                self.compile_load_variable(name, line);
            }

            Expr::BinOp { left, op, right, span } => {
                let line = Self::line_of(span);
                match op {
                    BinOp::And => {
                        // Short-circuit: if left is falsy, result is false
                        self.compile_expr(left);
                        self.emit(Op::Truthy, line);
                        let skip = self.emit_jump(Op::JumpIfFalse(0), line);
                        // Left was truthy, evaluate right
                        self.compile_expr(right);
                        self.emit(Op::Truthy, line);
                        let end = self.emit_jump(Op::Jump(0), line);
                        // Left was falsy → push false
                        self.patch_jump_here(skip);
                        self.emit_constant(StackValue::Bool(false), line);
                        self.patch_jump_here(end);
                    }
                    BinOp::Or => {
                        // Short-circuit: if left is truthy, result is true
                        self.compile_expr(left);
                        self.emit(Op::Truthy, line);
                        let skip = self.emit_jump(Op::JumpIfTrue(0), line);
                        // Left was falsy, evaluate right
                        self.compile_expr(right);
                        self.emit(Op::Truthy, line);
                        let end = self.emit_jump(Op::Jump(0), line);
                        // Left was truthy → push true
                        self.patch_jump_here(skip);
                        self.emit_constant(StackValue::Bool(true), line);
                        self.patch_jump_here(end);
                    }
                    BinOp::Coalesce => {
                        self.compile_expr(left);
                        let skip = self.emit_jump(Op::CoalesceJump(0), line);
                        self.emit(Op::Pop, line);
                        self.compile_expr(right);
                        self.patch_jump_here(skip);
                    }
                    _ => {
                        self.compile_expr(left);
                        self.compile_expr(right);
                        let op_code = match op {
                            BinOp::Add => Op::Add,
                            BinOp::Sub => Op::Sub,
                            BinOp::Mul => Op::Mul,
                            BinOp::Div => Op::Div,
                            BinOp::Mod => Op::Mod,
                            BinOp::Eq => Op::Eq,
                            BinOp::NotEq => Op::Neq,
                            BinOp::Lt => Op::Lt,
                            BinOp::LtEq => Op::Lte,
                            BinOp::Gt => Op::Gt,
                            BinOp::GtEq => Op::Gte,
                            BinOp::And | BinOp::Or | BinOp::Coalesce => {
                                unreachable!()
                            }
                        };
                        self.emit(op_code, line);
                    }
                }
            }

            Expr::UnOp { op, operand, span } => {
                let line = Self::line_of(span);
                match op {
                    UnOp::Neg => {
                        self.compile_expr(operand);
                        self.emit(Op::Neg, line);
                    }
                    UnOp::Not => {
                        self.compile_expr(operand);
                        self.emit(Op::Not, line);
                    }
                    UnOp::PrefixInc | UnOp::PrefixDec => {
                        let is_inc = *op == UnOp::PrefixInc;
                        let add_or_sub = if is_inc { Op::Add } else { Op::Sub };
                        match operand.as_ref() {
                            Expr::Ident(_, _) => {
                                self.compile_expr(operand);
                                self.emit_constant(StackValue::Float(1.0), line);
                                self.emit(add_or_sub, line);
                                self.emit(Op::Dup, line);
                                self.compile_store_from_expr(operand, line);
                            }
                            Expr::Index { expr: container, index, .. } => {
                                // Evaluate container and index exactly once
                                self.compile_expr(container);   // [arr]
                                self.compile_expr(index);       // [arr, i]
                                self.emit(Op::DupAt(1), line);  // [arr, i, arr]
                                self.emit(Op::DupAt(1), line);  // [arr, i, arr, i]
                                self.emit(Op::GetIndex, line);  // [arr, i, old]
                                self.emit_constant(StackValue::Float(1.0), line);
                                self.emit(add_or_sub, line);    // [arr, i, new]
                                self.emit(Op::Dup, line);       // [arr, i, new, new]
                                self.emit(Op::Rot(4), line);    // [new, arr, i, new]
                                self.emit(Op::SetIndex, line);  // [new]
                            }
                            Expr::Field { expr: obj, field, .. } => {
                                let str_idx = self.add_string(field);
                                self.compile_expr(obj);         // [obj]
                                self.emit(Op::Dup, line);       // [obj, obj]
                                self.emit(Op::GetField(str_idx), line); // [obj, old]
                                self.emit_constant(StackValue::Float(1.0), line);
                                self.emit(add_or_sub, line);    // [obj, new]
                                self.emit(Op::Dup, line);       // [obj, new, new]
                                self.emit(Op::Rot(3), line);    // [new, obj, new]
                                self.emit(Op::SetField(str_idx), line); // [new]
                            }
                            _ => {
                                self.compile_expr(operand);
                                self.emit_constant(StackValue::Float(1.0), line);
                                self.emit(add_or_sub, line);
                            }
                        }
                    }
                    UnOp::PostfixInc | UnOp::PostfixDec => {
                        let is_inc = *op == UnOp::PostfixInc;
                        let add_or_sub = if is_inc { Op::Add } else { Op::Sub };
                        match operand.as_ref() {
                            Expr::Ident(_, _) => {
                                self.compile_expr(operand);
                                self.emit(Op::Dup, line);
                                self.emit_constant(StackValue::Float(1.0), line);
                                self.emit(add_or_sub, line);
                                self.compile_store_from_expr(operand, line);
                            }
                            Expr::Index { expr: container, index, .. } => {
                                self.compile_expr(container);   // [arr]
                                self.compile_expr(index);       // [arr, i]
                                self.emit(Op::DupAt(1), line);  // [arr, i, arr]
                                self.emit(Op::DupAt(1), line);  // [arr, i, arr, i]
                                self.emit(Op::GetIndex, line);  // [arr, i, old]
                                self.emit(Op::Dup, line);       // [arr, i, old, old]
                                self.emit(Op::Rot(4), line);    // [old, arr, i, old]
                                self.emit_constant(StackValue::Float(1.0), line);
                                self.emit(add_or_sub, line);    // [old, arr, i, new]
                                self.emit(Op::SetIndex, line);  // [old]
                            }
                            Expr::Field { expr: obj, field, .. } => {
                                let str_idx = self.add_string(field);
                                self.compile_expr(obj);         // [obj]
                                self.emit(Op::Dup, line);       // [obj, obj]
                                self.emit(Op::GetField(str_idx), line); // [obj, old]
                                self.emit(Op::Dup, line);       // [obj, old, old]
                                self.emit(Op::Rot(3), line);    // [old, obj, old]
                                self.emit_constant(StackValue::Float(1.0), line);
                                self.emit(add_or_sub, line);    // [old, obj, new]
                                self.emit(Op::SetField(str_idx), line); // [old]
                            }
                            _ => {
                                self.compile_expr(operand);
                            }
                        }
                    }
                }
            }

            Expr::Ternary { condition, then_expr, else_expr, span } => {
                let line = Self::line_of(span);
                self.compile_expr(condition);
                self.emit(Op::Truthy, line);
                let else_jump = self.emit_jump(Op::JumpIfFalse(0), line);
                self.compile_expr(then_expr);
                let end_jump = self.emit_jump(Op::Jump(0), line);
                self.patch_jump_here(else_jump);
                self.compile_expr(else_expr);
                self.patch_jump_here(end_jump);
            }

            Expr::Cast { expr, ty, span } => {
                let line = Self::line_of(span);
                self.compile_expr(expr);
                match ty {
                    ast::Type::Float => self.emit(Op::CastFloat, line),
                    ast::Type::String => self.emit(Op::CastString, line),
                    ast::Type::Bool => self.emit(Op::Truthy, line),
                    _ => 0, // unsupported cast — no-op for now
                };
            }

            Expr::Try { expr, span } => {
                let line = Self::line_of(span);
                // Compile the expression into a closure that captures upvalues,
                // so variables from the enclosing scope are accessible.
                let mut try_compiler = FnCompiler::new("<try>");
                try_compiler.chunk.param_count = 0;
                self.fn_stack.push(try_compiler);

                self.compile_expr(expr);
                self.emit(Op::Return, line);

                let fc = self.fn_stack.pop().unwrap();
                let upvalue_count = fc.upvalues.len() as u8;
                let upvalues: Vec<Upvalue> = fc.upvalues.clone();
                let mut chunk = fc.chunk;
                chunk.local_count = fc.locals.len() as u16;
                let chunk_idx = self.chunks.len() as u16;
                self.chunks.push(chunk);

                if upvalue_count == 0 {
                    // No captures — use the fast path (bare chunk call)
                    self.emit(Op::TryCall(chunk_idx), line);
                } else {
                    // Has captures — build a closure and call it
                    for uv in &upvalues {
                        if uv.is_local {
                            self.emit(Op::LoadLocal(uv.index), line);
                        } else {
                            self.emit(Op::LoadUpvalue(uv.index), line);
                        }
                    }
                    self.emit(Op::MakeClosure(chunk_idx, upvalue_count), line);
                    self.emit(Op::TryCallClosure, line);
                }
            }

            Expr::Call { callee, args, named_args, span } => {
                let line = Self::line_of(span);
                self.compile_call(callee, args, named_args, line);
            }

            Expr::ExprCall { callee, args, span } => {
                let line = Self::line_of(span);
                self.compile_expr(callee);
                for arg in args {
                    self.compile_expr(arg);
                }
                self.emit(Op::CallClosure(args.len() as u8), line);
            }

            Expr::Index { expr, index, span } => {
                let line = Self::line_of(span);
                self.compile_expr(expr);
                self.compile_expr(index);
                self.emit(Op::GetIndex, line);
            }

            Expr::Field { expr, field, span } => {
                let line = Self::line_of(span);
                self.compile_expr(expr);
                let str_idx = self.add_string(field);
                self.emit(Op::GetField(str_idx), line);
            }

            Expr::OptionalChain { expr, field, span } => {
                let line = Self::line_of(span);
                self.compile_expr(expr);
                let skip = self.emit_jump(Op::OptChainJump(0), line);
                let str_idx = self.add_string(field);
                self.emit(Op::GetField(str_idx), line);
                self.patch_jump_here(skip);
            }

            Expr::MethodCall { expr, method, args, named_args, span } => {
                let line = Self::line_of(span);
                self.compile_method_call(expr, method, args, named_args, line);
            }

            Expr::Transform { expr, transforms, span } => {
                let line = Self::line_of(span);
                self.compile_expr(expr);
                for t in transforms {
                    self.compile_expr(t);
                }
                self.emit(Op::ApplyTransform(transforms.len() as u8), line);
            }

            Expr::List(items, span) => {
                let line = Self::line_of(span);
                for item in items {
                    self.compile_expr(item);
                }
                self.emit(Op::MakeList(items.len() as u16), line);
            }

            Expr::Lambda { params, return_ty: _, body, span } => {
                let line = Self::line_of(span);
                let mut lambda_compiler = FnCompiler::new("<lambda>");
                lambda_compiler.chunk.param_count = params.len() as u8;

                // Declare params as locals
                for param in params.iter() {
                    lambda_compiler.declare_local(&param.name);
                }

                self.fn_stack.push(lambda_compiler);

                // Compile body statements
                for stmt in body.iter() {
                    self.compile_stmt(stmt);
                }

                self.emit_constant(StackValue::Float(0.0), line);
                self.emit(Op::Return, line);

                let fc = self.fn_stack.pop().unwrap();
                let upvalue_count = fc.upvalues.len() as u8;
                let upvalues: Vec<Upvalue> = fc.upvalues.clone();
                let mut chunk = fc.chunk;
                chunk.local_count = fc.locals.len() as u16;

                let chunk_idx = self.chunks.len() as u16;
                self.chunks.push(chunk);

                // Emit upvalue loads for the enclosing scope
                for uv in &upvalues {
                    if uv.is_local {
                        self.emit(Op::LoadLocal(uv.index), line);
                    } else {
                        self.emit(Op::LoadUpvalue(uv.index), line);
                    }
                }
                self.emit(Op::MakeClosure(chunk_idx, upvalue_count), line);
            }

            Expr::StructConstruction { name, fields, span } => {
                let line = Self::line_of(span);
                self.compile_struct_construction(name, fields, line);
            }

            Expr::EnumConstruction { enum_name, variant, fields, span } => {
                let line = Self::line_of(span);
                self.compile_enum_construction(enum_name, variant, fields, line);
            }
        }
    }

    // ── Compile helpers ─────────────────────────────────────────────────────

    fn compile_load_variable(&mut self, name: &str, line: u32) {
        let loc = self.resolve_variable(name);
        match loc {
            VarLocation::Local(slot) => {
                self.emit(Op::LoadLocal(slot), line);
            }
            VarLocation::Upvalue(idx) => {
                self.emit(Op::LoadUpvalue(idx), line);
            }
            VarLocation::Global(idx) => {
                self.emit(Op::LoadGlobal(idx), line);
            }
            VarLocation::Function(chunk_idx) => {
                self.emit(Op::MakeClosure(chunk_idx, 0), line);
            }
        }
    }

    fn compile_store_from_expr(&mut self, expr: &Expr, line: u32) {
        // Pop TOS and store it into the location described by the expression.
        // Only supports simple Ident for inc/dec operations.
        match expr {
            Expr::Ident(name, _) => {
                let loc = self.resolve_variable(name);
                match loc {
                    VarLocation::Local(slot) => {
                        self.emit(Op::StoreLocal(slot), line);
                    }
                    VarLocation::Upvalue(idx) => {
                        self.emit(Op::StoreUpvalue(idx), line);
                    }
                    VarLocation::Global(idx) => {
                        self.emit(Op::StoreGlobal(idx), line);
                    }
                    VarLocation::Function(_) => {
                        // Can't assign to a function — just pop
                        self.emit(Op::Pop, line);
                    }
                }
            }
            Expr::Index { expr: container, index, .. } => {
                self.compile_expr(container);
                self.compile_expr(index);
                self.emit(Op::SetIndex, line);
            }
            Expr::Field { expr: obj, field, .. } => {
                self.compile_expr(obj);
                let str_idx = self.add_string(field);
                self.emit(Op::SetField(str_idx), line);
            }
            _ => {
                self.emit(Op::Pop, line);
            }
        }
    }

    fn compile_call(
        &mut self,
        callee: &str,
        args: &[Expr],
        named_args: &[(String, Expr)],
        line: u32,
    ) {
        // Check if callee is a native function
        if let Some(&native_idx) = self.native_map.get(callee) {
            if named_args.is_empty() {
                // Pure positional call
                for arg in args {
                    self.compile_expr(arg);
                }
                self.emit(Op::CallNative(native_idx, args.len() as u8), line);
            } else {
                // Shape constructors or functions with named args
                if let Some(total) = Self::native_total_params(callee) {
                    let positional_count = args.len();
                    // Compile positional args
                    for arg in args {
                        self.compile_expr(arg);
                    }
                    // Build optional slot array
                    let optional_count = total - positional_count;
                    let mut optional_slots: Vec<Option<usize>> =
                        vec![None; optional_count];

                    // Map named args to their slot offsets
                    for (i, (name, _)) in named_args.iter().enumerate() {
                        if let Some(offset) = Self::native_named_param_offset(callee, name)
                        {
                            let slot = offset - positional_count;
                            if slot < optional_count {
                                optional_slots[slot] = Some(i);
                            }
                        }
                    }

                    for slot_opt in &optional_slots {
                        match slot_opt {
                            Some(arg_idx) => {
                                self.compile_expr(&named_args[*arg_idx].1);
                            }
                            None => {
                                self.emit_constant(StackValue::None, line);
                            }
                        }
                    }

                    self.emit(Op::CallNative(native_idx, total as u8), line);
                } else {
                    // Unknown function with named args — just push positional + named
                    for arg in args {
                        self.compile_expr(arg);
                    }
                    for (_, expr) in named_args {
                        self.compile_expr(expr);
                    }
                    let total = args.len() + named_args.len();
                    self.emit(Op::CallNative(native_idx, total as u8), line);
                }
            }
            return;
        }

        // Check if callee is a user-defined function
        if let Some(&chunk_idx) = self.fn_chunks.get(callee) {
            for arg in args {
                self.compile_expr(arg);
            }
            self.emit(Op::Call(chunk_idx, args.len() as u8), line);
            return;
        }

        // Otherwise, resolve as variable (closure call)
        self.compile_load_variable(callee, line);
        for arg in args {
            self.compile_expr(arg);
        }
        self.emit(Op::CallClosure(args.len() as u8), line);
    }

    fn compile_method_call(
        &mut self,
        expr: &Expr,
        method: &str,
        args: &[Expr],
        named_args: &[(String, Expr)],
        line: u32,
    ) {
        // Check if expr is a namespace identifier
        if let Expr::Ident(ns_name, _) = expr
            && self.registry.get(ns_name).is_some() {
                // It's a namespace call → resolve method as native
                // Build full name for lookup (e.g., "file.read" → "file.read")
                let full_name = if ns_name == "file" {
                    format!("{ns_name}.{method}")
                } else {
                    method.to_string()
                };

                if self.native_map.contains_key(full_name.as_str()) {
                    // Compile as native call
                    self.compile_call(&full_name, args, named_args, line);
                    return;
                }
            }

        // Regular method call: compile receiver, args, then CallMethod
        self.compile_expr(expr);
        for arg in args {
            self.compile_expr(arg);
        }
        let method_str_idx = self.add_string(method);
        self.emit(Op::CallMethod(method_str_idx, args.len() as u8), line);
    }

    fn compile_struct_construction(
        &mut self,
        name: &str,
        fields: &[(String, Expr)],
        line: u32,
    ) {
        // Look up struct def index
        let def_idx = if let Some(&idx) = self.struct_map.get(name) {
            idx
        } else {
            // Struct not registered yet — compile fields in provided order
            for (_, expr) in fields {
                self.compile_expr(expr);
            }
            self.emit(Op::MakeStruct(0, fields.len() as u8), line);
            return;
        };

        // Look up the AST struct definition for field ordering and defaults
        let ast_struct = self.program.items.iter().find_map(|item| {
            if let Item::Struct(sd) = item
                && sd.name == name {
                    return Some(sd.clone());
                }
            None
        });

        if let Some(sd) = ast_struct {
            for field_def in &sd.fields {
                if let Some((_, expr)) =
                    fields.iter().find(|(n, _)| *n == field_def.name)
                {
                    self.compile_expr(expr);
                } else if let Some(ref default) = field_def.default {
                    self.compile_expr(default);
                } else {
                    self.emit_constant(StackValue::None, line);
                }
            }
            self.emit(
                Op::MakeStruct(def_idx as u16, sd.fields.len() as u8),
                line,
            );
        } else {
            // No AST definition found — compile provided fields
            for (_, expr) in fields {
                self.compile_expr(expr);
            }
            self.emit(Op::MakeStruct(def_idx as u16, fields.len() as u8), line);
        }
    }

    fn compile_enum_construction(
        &mut self,
        enum_name: &str,
        variant: &str,
        fields: &[(String, Expr)],
        line: u32,
    ) {
        // Look up enum variant definition order from compiled enum defs
        let variant_field_names = self.enum_defs.iter().find_map(|ed| {
            if ed.name == enum_name {
                ed.variants.iter().find_map(|v| {
                    if v.name == variant {
                        Some(v.field_names.clone())
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        });

        if let Some(ref def_field_names) = variant_field_names {
            // Compile fields in definition order
            for fname in def_field_names {
                if let Some((_, expr)) = fields.iter().find(|(n, _)| n == fname) {
                    self.compile_expr(expr);
                } else {
                    self.emit_constant(StackValue::None, line);
                }
            }
        } else {
            // No definition found — compile provided fields
            for (_, expr) in fields {
                self.compile_expr(expr);
            }
        }

        let (enum_def_idx, variant_idx) = self.enum_defs.iter().enumerate()
            .find_map(|(ei, ed)| {
                if ed.name == enum_name {
                    ed.variants.iter().enumerate()
                        .find(|(_, v)| v.name == variant)
                        .map(|(vi, _)| (ei as u16, vi as u8))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| {
                let ei = self.enum_defs.len() as u16;
                (ei, 0)
            });
        self.emit(Op::MakeEnum(enum_def_idx, variant_idx), line);
    }

    // ── Statement compilation ──────────────────────────────────────────────

    pub fn compile_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl(vd) => {
                let line = Self::line_of(&vd.span);
                self.compile_expr(&vd.initializer);
                if self.compiling_top_level && self.current().scope_depth == 0 {
                    let idx = self.add_global(&vd.name);
                    self.emit(Op::StoreGlobal(idx), line);
                } else {
                    self.current().declare_local(&vd.name);
                }
            }

            Stmt::Assign(a) => {
                let line = Self::line_of(&a.span);
                match &a.target {
                    AssignTarget::Path(path) if path.len() == 1 => {
                        // Simple variable assignment: x = val
                        self.compile_expr(&a.value);
                        let name = &path[0];
                        let loc = self.resolve_variable(name);
                        match loc {
                            VarLocation::Local(slot) => {
                                self.emit(Op::StoreLocal(slot), line);
                            }
                            VarLocation::Upvalue(idx) => {
                                self.emit(Op::StoreUpvalue(idx), line);
                            }
                            VarLocation::Global(idx) => {
                                self.emit(Op::StoreGlobal(idx), line);
                            }
                            VarLocation::Function(_) => {
                                self.emit(Op::Pop, line);
                            }
                        }
                    }
                    AssignTarget::Path(path) if path.len() == 2 => {
                        // 2-segment dotted assignment: a.b = val
                        // Stack convention: [object, value] for SetField
                        let root = &path[0];
                        let field = &path[1];
                        self.compile_load_variable(root, line);
                        self.compile_expr(&a.value);
                        let str_idx = self.add_string(field);
                        self.emit(Op::SetField(str_idx), line);
                    }
                    AssignTarget::Path(path) if path.len() >= 3 => {
                        // 3+ segment dotted assignment: a.b.c = val
                        // Use Dup trick for rebuild chain
                        let root = &path[0];
                        // For the last two segments, we need to handle
                        // value-type rebuild. Emit the rebuild pattern.
                        // Load root, Dup, navigate to parent of second-to-last,
                        // then SetFieldRebuild + SetField chain.
                        //
                        // For 3 segments: a.b.c = val
                        //   LoadLocal(a), Dup, GetField("b"), <compile val>,
                        //   SetFieldRebuild("c"), SetField("b")
                        //
                        // For 4+ segments: a.b.c.d = val
                        //   LoadLocal(a), GetField("b"), Dup, GetField("c"),
                        //   <compile val>, SetFieldRebuild("d"), SetField("c")
                        //   -- assumes a.b is ref-type (mutated in-place by GetField chain)

                        if path.len() == 3 {
                            self.compile_load_variable(root, line);
                            self.emit(Op::Dup, line);
                            let str1 = self.add_string(&path[1]);
                            self.emit(Op::GetField(str1), line);
                            self.compile_expr(&a.value);
                            let str2 = self.add_string(&path[2]);
                            self.emit(Op::SetFieldRebuild(str2), line);
                            let str1b = self.add_string(&path[1]);
                            self.emit(Op::SetField(str1b), line);
                        } else {
                            // 4+ segments: navigate ref-type chain, then rebuild last two
                            self.compile_load_variable(root, line);
                            for seg in &path[1..path.len() - 2] {
                                let si = self.add_string(seg);
                                self.emit(Op::GetField(si), line);
                            }
                            self.emit(Op::Dup, line);
                            let pen = self.add_string(&path[path.len() - 2]);
                            self.emit(Op::GetField(pen), line);
                            self.compile_expr(&a.value);
                            let last = self.add_string(&path[path.len() - 1]);
                            self.emit(Op::SetFieldRebuild(last), line);
                            let penb = self.add_string(&path[path.len() - 2]);
                            self.emit(Op::SetField(penb), line);
                        }
                    }
                    AssignTarget::Path(_) => {
                        // Empty path — should not happen, but handle gracefully
                    }
                    AssignTarget::Indexed { path, indices } => {
                        // Indexed assignment: arr[i] = val, or s.arr[i] = val
                        // Load root and navigate to the container
                        if !path.is_empty() {
                            self.compile_load_variable(&path[0], line);
                            for seg in &path[1..] {
                                let si = self.add_string(seg);
                                self.emit(Op::GetField(si), line);
                            }
                        }
                        // Navigate indices up to second-to-last
                        for idx_expr in &indices[..indices.len().saturating_sub(1)] {
                            self.compile_expr(idx_expr);
                            self.emit(Op::GetIndex, line);
                        }
                        // Last index + value + SetIndex
                        if let Some(last_idx) = indices.last() {
                            self.compile_expr(last_idx);
                            self.compile_expr(&a.value);
                            self.emit(Op::SetIndex, line);
                        }
                    }
                }
            }

            Stmt::Out(out) => {
                let line = Self::line_of(&out.span);
                for shape_expr in &out.shapes {
                    self.compile_expr(shape_expr);
                    self.emit(Op::Emit, line);
                }
            }

            Stmt::Print(p) => {
                let line = Self::line_of(&p.span);
                for val_expr in &p.values {
                    self.compile_expr(val_expr);
                }
                let level = match p.level {
                    PrintLevel::Log => 0,
                    PrintLevel::Warn => 1,
                    PrintLevel::Error => 2,
                };
                self.emit(Op::Print(p.values.len() as u8, level), line);
            }

            Stmt::If(ifs) => {
                let line = Self::line_of(&ifs.span);
                self.compile_expr(&ifs.condition);
                self.emit(Op::Truthy, line);
                let else_jump = self.emit_jump(Op::JumpIfFalse(0), line);

                // then block
                self.current().begin_scope();
                for s in &ifs.then_block {
                    self.compile_stmt(s);
                }
                // end_scope needs chunk reference
                self.current().end_scope(line);

                if let Some(else_block) = &ifs.else_block {
                    let end_jump = self.emit_jump(Op::Jump(0), line);
                    self.patch_jump_here(else_jump);

                    self.current().begin_scope();
                    for s in else_block {
                        self.compile_stmt(s);
                    }
                    self.current().end_scope(line);
                    self.patch_jump_here(end_jump);
                } else {
                    self.patch_jump_here(else_jump);
                }
            }

            Stmt::IfLet { binding, expr, then_block, else_block, span } => {
                let line = Self::line_of(span);
                self.compile_expr(expr);
                // Dup value, check if None
                self.emit(Op::Dup, line);
                self.emit(Op::IsNone, line);
                let else_jump = self.emit_jump(Op::JumpIfTrue(0), line);

                self.current().begin_scope();
                self.current().declare_local(binding);

                for s in then_block {
                    self.compile_stmt(s);
                }
                self.current().end_scope(line);

                let end_jump = self.emit_jump(Op::Jump(0), line);

                // Else branch: pop the None value
                self.patch_jump_here(else_jump);
                self.emit(Op::Pop, line);

                if let Some(else_stmts) = else_block {
                    self.current().begin_scope();
                    for s in else_stmts {
                        self.compile_stmt(s);
                    }
                    self.current().end_scope(line);
                }

                self.patch_jump_here(end_jump);
            }

            Stmt::While(w) => {
                let line = Self::line_of(&w.span);
                let loop_start = self.current_chunk().len();
                self.emit(Op::CheckCancel, line);

                self.compile_expr(&w.condition);
                self.emit(Op::Truthy, line);
                let exit_jump = self.emit_jump(Op::JumpIfFalse(0), line);

                let scope_depth = self.current().scope_depth;
                self.current().loop_stack.push(LoopContext {
                    start_ip: loop_start,
                    continue_ip: loop_start,
                    break_patches: Vec::new(),
                    continue_patches: Vec::new(),
                    scope_depth,
                });

                self.current().begin_scope();
                for s in &w.body {
                    self.compile_stmt(s);
                }
                self.current().end_scope(line);

                // Loop back
                let loop_offset =
                    (loop_start as i16) - (self.current_chunk().len() as i16) - 1;
                self.emit(Op::Loop(loop_offset), line);

                self.patch_jump_here(exit_jump);

                // Patch breaks
                let ctx = self.current().loop_stack.pop().unwrap();
                for bp in ctx.break_patches {
                    self.patch_jump_here(bp);
                }
            }

            Stmt::For(f) => {
                let line = Self::line_of(&f.span);

                // Outer scope for the loop variable
                self.current().begin_scope();
                self.compile_stmt(&f.init);

                let loop_start = self.current_chunk().len();
                self.emit(Op::CheckCancel, line);

                self.compile_expr(&f.condition);
                self.emit(Op::Truthy, line);
                let exit_jump = self.emit_jump(Op::JumpIfFalse(0), line);

                let scope_depth = self.current().scope_depth;
                self.current().loop_stack.push(LoopContext {
                    start_ip: loop_start,
                    continue_ip: 0, // deferred — patched after body
                    break_patches: Vec::new(),
                    continue_patches: Vec::new(),
                    scope_depth,
                });

                // Body
                self.current().begin_scope();
                for s in &f.body {
                    self.compile_stmt(s);
                }
                self.current().end_scope(line);

                // Continue target is HERE (before step)
                let continue_target = self.current_chunk().len();

                // Patch all deferred continue jumps to land here
                let continue_patches: Vec<usize> = self.current().loop_stack.last_mut()
                    .map(|ctx| {
                        ctx.continue_ip = continue_target;
                        std::mem::take(&mut ctx.continue_patches)
                    })
                    .unwrap_or_default();
                for cp in continue_patches {
                    self.patch_jump_to(cp, continue_target);
                }

                // Step
                self.compile_stmt(&f.step);

                // Loop back to condition
                let loop_offset =
                    (loop_start as i16) - (self.current_chunk().len() as i16) - 1;
                self.emit(Op::Loop(loop_offset), line);

                self.patch_jump_here(exit_jump);

                // Patch breaks
                let ctx = self.current().loop_stack.pop().unwrap();
                for bp in ctx.break_patches {
                    self.patch_jump_here(bp);
                }

                // End outer scope
                self.current().end_scope(line);
            }

            Stmt::Foreach(fe) => {
                let line = Self::line_of(&fe.span);

                // Outer scope for iterator
                self.current().begin_scope();
                self.compile_expr(&fe.iterable);
                self.emit(Op::IterInit, line);
                self.current().declare_local("<iter>");

                let loop_start = self.current_chunk().len();
                self.emit(Op::CheckCancel, line);

                // Load iterator, advance
                let iter_slot = self.current().resolve_local("<iter>").unwrap();
                self.emit(Op::LoadLocal(iter_slot), line);
                let exit_jump = self.emit_jump(Op::IterNext(0), line);

                let scope_depth = self.current().scope_depth;
                self.current().loop_stack.push(LoopContext {
                    start_ip: loop_start,
                    continue_ip: loop_start,
                    break_patches: Vec::new(),
                    continue_patches: Vec::new(),
                    scope_depth,
                });

                self.current().begin_scope();
                self.current().declare_local(&fe.var_name);

                for s in &fe.body {
                    self.compile_stmt(s);
                }
                self.current().end_scope(line);

                // Loop back
                let loop_offset =
                    (loop_start as i16) - (self.current_chunk().len() as i16) - 1;
                self.emit(Op::Loop(loop_offset), line);

                self.patch_jump_here(exit_jump);

                // Patch breaks
                let ctx = self.current().loop_stack.pop().unwrap();
                for bp in ctx.break_patches {
                    self.patch_jump_here(bp);
                }

                // End outer scope (pops iterator)
                self.current().end_scope(line);
            }

            Stmt::Match(m) => {
                let line = Self::line_of(&m.span);
                self.compile_expr(&m.expr);

                let mut end_patches = Vec::new();

                for arm in &m.arms {
                    match &arm.pattern {
                        MatchPattern::Values(values) => {
                            // For each value, Dup + compare
                            let mut arm_skip_patches = Vec::new();
                            let mut match_jump: Option<usize> = None;

                            for (i, val) in values.iter().enumerate() {
                                self.emit(Op::Dup, line);
                                self.compile_expr(val);
                                self.emit(Op::Eq, line);
                                if i < values.len() - 1 {
                                    // If this one matches, jump to body
                                    let j = self.emit_jump(Op::JumpIfTrue(0), line);
                                    arm_skip_patches.push(j);
                                } else {
                                    // Last value: if false, skip this arm
                                    match_jump =
                                        Some(self.emit_jump(Op::JumpIfFalse(0), line));
                                }
                            }

                            // Patch early-match jumps to here (body start)
                            for p in arm_skip_patches {
                                self.patch_jump_here(p);
                            }

                            // Pop scrutinee copy, execute body
                            self.emit(Op::Pop, line); // pop scrutinee before body
                            self.current().begin_scope();
                            for s in &arm.body {
                                self.compile_stmt(s);
                            }
                            self.current().end_scope(line);
                            end_patches.push(self.emit_jump(Op::Jump(0), line));

                            // Patch the "no match" jump
                            if let Some(j) = match_jump {
                                self.patch_jump_here(j);
                            }
                        }
                        MatchPattern::EnumVariant { variant, .. } => {
                            self.emit(Op::Dup, line);
                            let variant_str = self.add_string(variant);
                            self.emit(Op::MatchEnum(variant_str), line);
                            let skip = self.emit_jump(Op::JumpIfFalse(0), line);

                            // Body
                            self.current().begin_scope();
                            for s in &arm.body {
                                self.compile_stmt(s);
                            }
                            self.current().end_scope(line);
                            end_patches.push(self.emit_jump(Op::Jump(0), line));
                            self.patch_jump_here(skip);
                        }
                        MatchPattern::Else => {
                            // Pop scrutinee, execute body
                            self.emit(Op::Pop, line);
                            self.current().begin_scope();
                            for s in &arm.body {
                                self.compile_stmt(s);
                            }
                            self.current().end_scope(line);
                            end_patches.push(self.emit_jump(Op::Jump(0), line));
                        }
                    }
                }

                // If no arm matched, pop scrutinee
                self.emit(Op::Pop, line);

                // Patch all end jumps
                for p in end_patches {
                    self.patch_jump_here(p);
                }
            }

            Stmt::Return(expr, span) => {
                let line = Self::line_of(span);
                if let Some(e) = expr {
                    self.compile_expr(e);
                } else {
                    self.emit_constant(StackValue::Float(0.0), line);
                }
                self.emit(Op::Return, line);
            }

            Stmt::Break(span) => {
                let line = Self::line_of(span);
                // Pop locals down to loop scope depth
                if let Some(ctx) = self.current().loop_stack.last() {
                    let target_depth = ctx.scope_depth;
                    let fc = self.fn_stack.last().unwrap();
                    let mut pop_count = 0u16;
                    for local in fc.locals.iter().rev() {
                        if local.depth <= target_depth {
                            break;
                        }
                        pop_count += 1;
                    }
                    for _ in 0..pop_count {
                        self.emit(Op::Pop, line);
                    }
                }
                let bp = self.emit_jump(Op::Jump(0), line);
                if let Some(ctx) = self.current().loop_stack.last_mut() {
                    ctx.break_patches.push(bp);
                }
            }

            Stmt::Continue(span) => {
                let line = Self::line_of(span);
                if let Some(ctx) = self.current().loop_stack.last() {
                    let target_depth = ctx.scope_depth;
                    let continue_ip = ctx.continue_ip;
                    let fc = self.fn_stack.last().unwrap();
                    let mut pop_count = 0u16;
                    for local in fc.locals.iter().rev() {
                        if local.depth <= target_depth {
                            break;
                        }
                        pop_count += 1;
                    }
                    for _ in 0..pop_count {
                        self.emit(Op::Pop, line);
                    }
                    if continue_ip == 0 {
                        // Deferred: target not known yet (for-loop step)
                        let jp = self.emit_jump(Op::Jump(0), line);
                        // Re-borrow mutably to push patch
                        self.current().loop_stack.last_mut().unwrap().continue_patches.push(jp);
                    } else {
                        let loop_offset =
                            (continue_ip as i16) - (self.current_chunk().len() as i16) - 1;
                        self.emit(Op::Loop(loop_offset), line);
                    }
                }
            }

            Stmt::FnVar { name, value, span } => {
                let line = Self::line_of(span);
                self.compile_expr(value);
                if self.compiling_top_level && self.current().scope_depth == 0 {
                    let idx = self.add_global(name);
                    self.emit(Op::StoreGlobal(idx), line);
                } else {
                    self.current().declare_local(name);
                }
            }

            Stmt::Expr(expr) => {
                self.compile_expr(expr);
                let line = Self::line_of(expr.span());
                self.emit(Op::Pop, line);
            }
        }
    }

    // ── Compile entry point ─────────────────────────────────────────────────

    #[must_use] 
    pub fn compile(
        program: &'a ast::Program,
        registry: &'a NamespaceRegistry,
    ) -> CompiledProgram {
        let mut c = Compiler::new(program, registry);

        // Pass 1: Register struct/enum definitions
        c.register_type_defs();

        // Pass 2: Register function names -> chunk indices
        c.register_functions();

        // Pass 3: Register state fields
        c.register_state_fields();

        // Pass 4: Compile top-level chunk (index 0)
        c.compile_top_level();

        // Pass 5: Compile user-defined functions
        c.compile_all_functions();

        // Pass 6: Compile struct methods
        c.compile_struct_methods();

        // Pass 7: Compile state initializer
        c.compile_state_init();

        CompiledProgram {
            chunks: c.chunks,
            constants: c.constants,
            strings: c.strings,
            natives: c.native_table.iter().map(|n| n.name.to_string()).collect(),
            state_init_chunk: c.state_init_chunk,
            on_init_chunk: c.on_init_chunk,
            on_update_chunk: c.on_update_chunk,
            on_exit_chunk: c.on_exit_chunk,
            state_fields: c.state_fields.clone(),
            struct_defs: c.struct_defs.clone(),
            enum_defs: c.enum_defs.clone(),
            global_count: c.globals.len() as u16,
        }
    }

    // ── Registration passes ─────────────────────────────────────────────────

    fn register_type_defs(&mut self) {
        for item in &self.program.items {
            match item {
                Item::Struct(sd) => {
                    let idx = self.struct_defs.len();
                    self.struct_map.insert(sd.name.clone(), idx);
                    self.struct_defs.push(CompiledStructDef {
                        name: sd.name.clone(),
                        field_names: sd.fields.iter().map(|f| f.name.clone()).collect(),
                        methods: HashMap::new(),
                    });
                }
                Item::Enum(ed) => {
                    let idx = self.enum_defs.len();
                    self.enum_map.insert(ed.name.clone(), idx);
                    self.enum_defs.push(CompiledEnumDef {
                        name: ed.name.clone(),
                        variants: ed
                            .variants
                            .iter()
                            .map(|v| CompiledEnumVariant {
                                name: v.name.clone(),
                                field_names: v.fields.iter().map(|f| f.name.clone()).collect(),
                            })
                            .collect(),
                    });
                }
                _ => {}
            }
        }
    }

    fn register_functions(&mut self) {
        // Reserve chunk index 0 for top-level
        let top_chunk = Chunk::new("<top_level>");
        self.chunks.push(top_chunk);

        for item in &self.program.items {
            if let Item::FnDef(fd) = item {
                let chunk_idx = self.chunks.len() as u16;
                self.chunks.push(Chunk::new(&fd.name));
                self.fn_chunks.insert(fd.name.clone(), chunk_idx);

                match fd.name.as_str() {
                    "on_init" => self.on_init_chunk = Some(chunk_idx),
                    "on_update" => self.on_update_chunk = Some(chunk_idx),
                    "on_exit" => self.on_exit_chunk = Some(chunk_idx),
                    _ => {}
                }
            }
        }
    }

    fn register_state_fields(&mut self) {
        if let Some(ref state_block) = self.program.state {
            for (i, field) in state_block.fields.iter().enumerate() {
                self.state_fields.push(field.name.clone());
                self.state_map.insert(field.name.clone(), i as u16);
            }
        }
    }

    // ── Top-level compilation ───────────────────────────────────────────────

    fn compile_top_level(&mut self) {
        self.compiling_top_level = true;
        self.fn_stack.push(FnCompiler::new("<top_level>"));

        // Initialize all implicit globals (constants + native functions available without import)
        self.compile_implicit_globals();

        // Compile explicit imports (may override implicit globals, that's fine)
        self.compile_imports();

        // Compile top-level statements
        let items: Vec<_> = self.program.items.clone();
        for item in &items {
            if let Item::Stmt(stmt) = item {
                self.compile_stmt(stmt);
            }
        }

        // Emit return
        self.emit_constant(StackValue::Float(0.0), 0);
        self.emit(Op::Return, 0);

        let fc = self.fn_stack.pop().unwrap();
        let mut chunk = fc.chunk;
        chunk.local_count = fc.locals.len() as u16;
        self.chunks[0] = chunk;
        self.compiling_top_level = false;
    }

    fn compile_implicit_globals(&mut self) {
        let ns_names = ["core", "shapes", "render", "coords"];
        for ns_name in &ns_names {
            if let Some(ns) = self.registry.get(ns_name) {
                for export in ns.exports() {
                    let name = export.name.to_string();
                    if self.global_map.contains_key(&name) {
                        continue;
                    }
                    let global_idx = self.add_global(&name);
                    match export.kind {
                        ExportKind::Function => {
                            if let Some(&native_idx) = self.native_map.get(&name) {
                                self.emit(Op::MakeNativeFnRef(native_idx), 0);
                                self.emit(Op::StoreGlobal(global_idx), 0);
                            }
                        }
                        ExportKind::Constant => {
                            self.emit_constant_value(&name, 0);
                            self.emit(Op::StoreGlobal(global_idx), 0);
                        }
                    }
                }
            }
        }
    }

    fn compile_imports(&mut self) {
        let imports: Vec<_> = self.program.imports.clone();
        for import in &imports {
            let ns_name = &import.namespace;
            if let Some(ns) = self.registry.get(ns_name) {
                let exports = ns.exports();
                let members: Vec<_> = if import.members.is_empty() {
                    exports.iter().map(|e| e.name.to_string()).collect()
                } else {
                    import.members.clone()
                };

                for member_name in &members {
                    if let Some(export) = exports.iter().find(|e| e.name == member_name.as_str())
                    {
                        let global_idx = self.add_global(member_name);
                        let line = Self::line_of(&import.span);

                        match export.kind {
                            ExportKind::Function => {
                                let lookup_name = if ns_name == "file" {
                                    format!("{ns_name}.{member_name}")
                                } else {
                                    member_name.clone()
                                };
                                if let Some(&native_idx) = self.native_map.get(&lookup_name)
                                {
                                    self.emit(Op::MakeNativeFnRef(native_idx), line);
                                    self.emit(Op::StoreGlobal(global_idx), line);
                                }
                            }
                            ExportKind::Constant => {
                                self.emit_constant_value(member_name, line);
                                self.emit(Op::StoreGlobal(global_idx), line);
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Function compilation ────────────────────────────────────────────────

    fn compile_all_functions(&mut self) {
        let items: Vec<_> = self.program.items.clone();
        for item in &items {
            if let Item::FnDef(fd) = item {
                let chunk_idx = self.fn_chunks[&fd.name];
                self.compile_function(fd, chunk_idx);
            }
        }
    }

    fn compile_function(&mut self, fd: &ast::FnDef, chunk_idx: u16) {
        let mut fc = FnCompiler::new(&fd.name);
        fc.chunk.param_count = fd.params.len() as u8;

        // Declare params as first locals
        for param in fd.params.iter() {
            fc.declare_local(&param.name);
        }

        self.fn_stack.push(fc);

        // Compile body
        for stmt in fd.body.iter() {
            self.compile_stmt(stmt);
        }

        // Default return
        let last_line = if let Some(last) = fd.body.last() {
            match last {
                Stmt::Return(_, span) => Self::line_of(span),
                _ => Self::line_of(&fd.span),
            }
        } else {
            Self::line_of(&fd.span)
        };
        self.emit_constant(StackValue::None, last_line);
        self.emit(Op::Return, last_line);

        let fc = self.fn_stack.pop().unwrap();
        let mut chunk = fc.chunk;
        chunk.local_count = fc.locals.len() as u16;
        self.chunks[chunk_idx as usize] = chunk;
    }

    // ── Struct method compilation ───────────────────────────────────────────

    fn compile_struct_methods(&mut self) {
        let items: Vec<_> = self.program.items.clone();
        for item in &items {
            if let Item::Struct(sd) = item {
                let struct_idx = self.struct_map[&sd.name];
                for method in &sd.methods {
                    let method_name =
                        format!("{}.{}", sd.name, method.def.name);
                    let chunk_idx = self.chunks.len() as u16;
                    self.chunks.push(Chunk::new(&method_name));

                    let mut fc = FnCompiler::new(&method_name);
                    // `this` is local slot 0
                    fc.declare_local("this");
                    fc.chunk.param_count = (method.def.params.len() + 1) as u8;
                    // Declare explicit params
                    for param in method.def.params.iter() {
                        fc.declare_local(&param.name);
                    }

                    self.fn_stack.push(fc);

                    for stmt in method.def.body.iter() {
                        self.compile_stmt(stmt);
                    }

                    let line = Self::line_of(&method.def.span);
                    self.emit_constant(StackValue::None, line);
                    self.emit(Op::Return, line);

                    let fc = self.fn_stack.pop().unwrap();
                    let mut chunk = fc.chunk;
                    chunk.local_count = fc.locals.len() as u16;
                    self.chunks[chunk_idx as usize] = chunk;

                    let compiled_method = CompiledMethodDef {
                        chunk_index: chunk_idx,
                        param_count: method.def.params.len() as u8,
                        is_public: method.visibility == Visibility::Public,
                    };
                    self.struct_defs[struct_idx]
                        .methods
                        .insert(method.def.name.clone(), compiled_method);
                }
            }
        }
    }

    // ── State init compilation ──────────────────────────────────────────────

    fn compile_state_init(&mut self) {
        let state_block = match &self.program.state {
            Some(sb) => sb.clone(),
            None => return,
        };

        let chunk_idx = self.chunks.len() as u16;
        self.chunks.push(Chunk::new("<state_init>"));

        let mut fc = FnCompiler::new("<state_init>");
        // Local 0 = state ref (passed by VM)
        fc.declare_local("<state>");
        fc.chunk.param_count = 1;
        self.fn_stack.push(fc);

        for field in &state_block.fields {
            let line = Self::line_of(&field.span);
            // Stack: load state first, then value, then SetField
            self.emit(Op::LoadLocal(0), line);
            self.compile_expr(&field.initializer);
            let str_idx = self.add_string(&field.name);
            self.emit(Op::SetField(str_idx), line);
        }

        let line = if state_block.fields.is_empty() {
            0
        } else {
            Self::line_of(&state_block.fields.last().unwrap().span)
        };
        self.emit_constant(StackValue::None, line);
        self.emit(Op::Return, line);

        let fc = self.fn_stack.pop().unwrap();
        let mut chunk = fc.chunk;
        chunk.local_count = fc.locals.len() as u16;
        self.chunks[chunk_idx as usize] = chunk;
        self.state_init_chunk = Some(chunk_idx);
    }
}

// ─── Utility functions ──────────────────────────────────────────────────────

fn is_known_constant(name: &str) -> bool {
    matches!(name, "PI" | "TAU" | "red" | "green" | "blue" | "white" | "black" | "transparent"
        | "sdf" | "fill" | "outline"
        | "center" | "top_left" | "top_right" | "bottom_left" | "bottom_right"
        | "top" | "bottom" | "left" | "right")
}

/// Convert a `StackValue` into a u64 key for deduplication.
fn const_dedup_key(val: StackValue) -> u64 {
    match val {
        StackValue::Float(f) => f.to_bits(),
        StackValue::Bool(b) => {
            // Use a prefix so true/false don't collide with float bits
            if b { u64::MAX } else { u64::MAX - 1 }
        }
        StackValue::None => u64::MAX - 2,
        StackValue::HeapRef(i) => {
            // HeapRefs shouldn't appear in the constant pool,
            // but handle gracefully
            u64::MAX - 3 - u64::from(i)
        }
    }
}

fn parse_hex_color(hex: &str) -> (f64, f64, f64, f64) {
    let h = hex.trim_start_matches('#');
    let r = f64::from(u8::from_str_radix(&h[0..2], 16).unwrap_or(0)) / 255.0;
    let g = f64::from(u8::from_str_radix(&h[2..4], 16).unwrap_or(0)) / 255.0;
    let b = f64::from(u8::from_str_radix(&h[4..6], 16).unwrap_or(0)) / 255.0;
    let a = if h.len() >= 8 {
        f64::from(u8::from_str_radix(&h[6..8], 16).unwrap_or(255)) / 255.0
    } else {
        1.0
    };
    (r, g, b, a)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::namespaces::NamespaceRegistry;
    use crate::syntax::ast::{self, Span};

    fn span() -> Span {
        Span::new(1, 1)
    }

    fn empty_program() -> ast::Program {
        ast::Program {
            imports: vec![],
            state: None,
            items: vec![],
        }
    }

    fn make_compiler(program: &ast::Program) -> Compiler<'_> {
        // Leak is intentional: tests need a &'static reference and the small
        // allocation is reclaimed when the process exits.
        let registry = Box::leak(Box::new(NamespaceRegistry::standard()));
        let mut c = Compiler::new(program, registry);
        c.fn_stack.push(FnCompiler::new("<test>"));
        c
    }

    // ── Literals ────────────────────────────────────────────────────────────

    #[test]
    fn compile_float_literal() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::Float(42.0, span());
        c.compile_expr(&expr);

        assert_eq!(c.current().chunk.code[0], Op::Const(0));
        assert!(matches!(c.constants[0], StackValue::Float(f) if f == 42.0));
    }

    #[test]
    fn compile_float_dedup() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        c.compile_expr(&Expr::Float(42.0, span()));
        c.compile_expr(&Expr::Float(42.0, span()));

        assert_eq!(c.constants.len(), 1);
        assert_eq!(c.current().chunk.code[0], Op::Const(0));
        assert_eq!(c.current().chunk.code[1], Op::Const(0));
    }

    #[test]
    fn compile_bool_literal() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        c.compile_expr(&Expr::Bool(true, span()));
        c.compile_expr(&Expr::Bool(false, span()));

        assert!(matches!(c.constants[0], StackValue::Bool(true)));
        assert!(matches!(c.constants[1], StackValue::Bool(false)));
    }

    #[test]
    fn compile_none_literal() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        c.compile_expr(&Expr::None(span()));

        assert_eq!(c.current().chunk.code[0], Op::Const(0));
        assert!(matches!(c.constants[0], StackValue::None));
    }

    #[test]
    fn compile_string_literal() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        c.compile_expr(&Expr::StringLit("hello".into(), span()));

        assert_eq!(c.current().chunk.code[0], Op::ConstStr(0));
        assert_eq!(c.strings[0], "hello");
    }

    #[test]
    fn compile_hex_color() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        c.compile_expr(&Expr::HexColor("FF0000".into(), span()));

        // Should emit 4 Const + MakeColor(4)
        let code = &c.current().chunk.code;
        assert_eq!(code.len(), 5);
        assert!(matches!(code[4], Op::MakeColor(4)));
        // r=1.0, g=0.0, b=0.0, a=1.0
        assert!(matches!(c.constants[0], StackValue::Float(f) if (f - 1.0).abs() < 1e-10));
        assert!(matches!(c.constants[1], StackValue::Float(f) if f == 0.0));
    }

    #[test]
    fn compile_hex_color_with_alpha() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        c.compile_expr(&Expr::HexColor("FF000080".into(), span()));

        let code = &c.current().chunk.code;
        assert_eq!(code.len(), 5);
        // alpha ~ 128/255 ~ 0.502
        let alpha = match c.constants.iter().last() {
            Some(StackValue::Float(f)) => *f,
            _ => panic!("expected float"),
        };
        assert!((alpha - 128.0 / 255.0).abs() < 1e-10);
    }

    // ── Ident resolution ────────────────────────────────────────────────────

    #[test]
    fn compile_ident_local() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        // Declare a local variable
        c.current().declare_local("x");

        c.compile_expr(&Expr::Ident("x".into(), span()));

        assert_eq!(c.current().chunk.code[0], Op::LoadLocal(0));
    }

    #[test]
    fn compile_ident_global() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        c.add_global("myGlobal");

        c.compile_expr(&Expr::Ident("myGlobal".into(), span()));

        assert_eq!(c.current().chunk.code[0], Op::LoadGlobal(0));
    }

    // ── BinOp ───────────────────────────────────────────────────────────────

    #[test]
    fn compile_binop_add() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::BinOp {
            left: Box::new(Expr::Float(1.0, span())),
            op: BinOp::Add,
            right: Box::new(Expr::Float(2.0, span())),
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        assert_eq!(code.len(), 3); // Const, Const, Add
        assert_eq!(code[2], Op::Add);
    }

    #[test]
    fn compile_binop_and_short_circuit() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::BinOp {
            left: Box::new(Expr::Bool(true, span())),
            op: BinOp::And,
            right: Box::new(Expr::Bool(false, span())),
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        // Should have: Const(true), Truthy, JumpIfFalse, Const(false), Truthy, Jump, Const(false)
        assert!(matches!(code[1], Op::Truthy));
        assert!(matches!(code[2], Op::JumpIfFalse(_)));
    }

    #[test]
    fn compile_binop_or_short_circuit() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::BinOp {
            left: Box::new(Expr::Bool(false, span())),
            op: BinOp::Or,
            right: Box::new(Expr::Bool(true, span())),
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        assert!(matches!(code[1], Op::Truthy));
        assert!(matches!(code[2], Op::JumpIfTrue(_)));
    }

    #[test]
    fn compile_binop_coalesce() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::BinOp {
            left: Box::new(Expr::None(span())),
            op: BinOp::Coalesce,
            right: Box::new(Expr::Float(42.0, span())),
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        // Const(None), CoalesceJump, Pop, Const(42.0)
        assert!(matches!(code[1], Op::CoalesceJump(_)));
        assert!(matches!(code[2], Op::Pop));
        assert!(matches!(code[3], Op::Const(_)));
    }

    // ── UnOp ────────────────────────────────────────────────────────────────

    #[test]
    fn compile_neg() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::UnOp {
            op: UnOp::Neg,
            operand: Box::new(Expr::Float(5.0, span())),
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        assert_eq!(code[1], Op::Neg);
    }

    #[test]
    fn compile_not() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::UnOp {
            op: UnOp::Not,
            operand: Box::new(Expr::Bool(true, span())),
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        assert_eq!(code[1], Op::Not);
    }

    #[test]
    fn compile_prefix_inc() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        c.current().declare_local("x");

        let expr = Expr::UnOp {
            op: UnOp::PrefixInc,
            operand: Box::new(Expr::Ident("x".into(), span())),
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        // LoadLocal(0), Const(1.0), Add, Dup, StoreLocal(0)
        assert_eq!(code[0], Op::LoadLocal(0));
        assert!(matches!(code[1], Op::Const(_)));
        assert_eq!(code[2], Op::Add);
        assert_eq!(code[3], Op::Dup);
        assert_eq!(code[4], Op::StoreLocal(0));
    }

    #[test]
    fn compile_postfix_inc() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        c.current().declare_local("x");

        let expr = Expr::UnOp {
            op: UnOp::PostfixInc,
            operand: Box::new(Expr::Ident("x".into(), span())),
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        // LoadLocal(0), Dup, Const(1.0), Add, StoreLocal(0)
        assert_eq!(code[0], Op::LoadLocal(0));
        assert_eq!(code[1], Op::Dup);
        assert!(matches!(code[2], Op::Const(_)));
        assert_eq!(code[3], Op::Add);
        assert_eq!(code[4], Op::StoreLocal(0));
    }

    // ── Ternary ─────────────────────────────────────────────────────────────

    #[test]
    fn compile_ternary() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::Ternary {
            condition: Box::new(Expr::Bool(true, span())),
            then_expr: Box::new(Expr::Float(1.0, span())),
            else_expr: Box::new(Expr::Float(2.0, span())),
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        // Const(true), Truthy, JumpIfFalse, Const(1.0), Jump, Const(2.0)
        assert!(matches!(code[1], Op::Truthy));
        assert!(matches!(code[2], Op::JumpIfFalse(_)));
        assert!(matches!(code[4], Op::Jump(_)));
    }

    // ── Cast ────────────────────────────────────────────────────────────────

    #[test]
    fn compile_cast_float() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::Cast {
            expr: Box::new(Expr::Bool(true, span())),
            ty: ast::Type::Float,
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        assert_eq!(code[1], Op::CastFloat);
    }

    #[test]
    fn compile_cast_string() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::Cast {
            expr: Box::new(Expr::Float(42.0, span())),
            ty: ast::Type::String,
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        assert_eq!(code[1], Op::CastString);
    }

    #[test]
    fn compile_cast_bool() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::Cast {
            expr: Box::new(Expr::Float(1.0, span())),
            ty: ast::Type::Bool,
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        assert_eq!(code[1], Op::Truthy);
    }

    // ── Index ───────────────────────────────────────────────────────────────

    #[test]
    fn compile_index() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        c.current().declare_local("arr");

        let expr = Expr::Index {
            expr: Box::new(Expr::Ident("arr".into(), span())),
            index: Box::new(Expr::Float(0.0, span())),
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        assert_eq!(code[0], Op::LoadLocal(0)); // arr
        assert!(matches!(code[1], Op::Const(_))); // 0.0
        assert_eq!(code[2], Op::GetIndex);
    }

    // ── Field ───────────────────────────────────────────────────────────────

    #[test]
    fn compile_field() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        c.current().declare_local("obj");

        let expr = Expr::Field {
            expr: Box::new(Expr::Ident("obj".into(), span())),
            field: "x".into(),
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        assert_eq!(code[0], Op::LoadLocal(0));
        assert!(matches!(code[1], Op::GetField(0)));
        assert_eq!(c.strings[0], "x");
    }

    // ── OptionalChain ───────────────────────────────────────────────────────

    #[test]
    fn compile_optional_chain() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        c.current().declare_local("obj");

        let expr = Expr::OptionalChain {
            expr: Box::new(Expr::Ident("obj".into(), span())),
            field: "name".into(),
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        assert_eq!(code[0], Op::LoadLocal(0));
        assert!(matches!(code[1], Op::OptChainJump(_)));
        assert!(matches!(code[2], Op::GetField(_)));
        assert_eq!(c.strings[0], "name");
    }

    // ── List ────────────────────────────────────────────────────────────────

    #[test]
    fn compile_list() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::List(
            vec![
                Expr::Float(1.0, span()),
                Expr::Float(2.0, span()),
                Expr::Float(3.0, span()),
            ],
            span(),
        );
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        // 3 Const + MakeList(3)
        assert_eq!(code.len(), 4);
        assert_eq!(code[3], Op::MakeList(3));
    }

    // ── Interpolated string ─────────────────────────────────────────────────

    #[test]
    fn compile_interpolated_string() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::Interpolated(
            vec![
                InterpolPart::Lit("hello ".into()),
                InterpolPart::Expr(Box::new(Expr::Float(42.0, span()))),
                InterpolPart::Lit("!".into()),
            ],
            span(),
        );
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        // ConstStr("hello "), Const(42.0), CastString, ConstStr("!"), Concat(3)
        assert_eq!(code[0], Op::ConstStr(0));
        assert!(matches!(code[1], Op::Const(_)));
        assert_eq!(code[2], Op::CastString);
        assert_eq!(code[3], Op::ConstStr(1));
        assert_eq!(code[4], Op::Concat(3));
    }

    #[test]
    fn compile_interpolated_empty() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::Interpolated(vec![], span());
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        assert_eq!(code[0], Op::ConstStr(0));
        assert_eq!(c.strings[0], "");
    }

    #[test]
    fn compile_interpolated_single_part() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let expr = Expr::Interpolated(
            vec![InterpolPart::Lit("only".into())],
            span(),
        );
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        // Single part: ConstStr only, no Concat
        assert_eq!(code.len(), 1);
        assert_eq!(code[0], Op::ConstStr(0));
    }

    // ── FnCompiler scope management ─────────────────────────────────────────

    #[test]
    fn fn_compiler_scope_locals() {
        let mut fc = FnCompiler::new("test");
        let mut chunk = Chunk::new("test");

        fc.begin_scope();
        let slot_a = fc.declare_local("a");
        let slot_b = fc.declare_local("b");
        assert_eq!(slot_a, 0);
        assert_eq!(slot_b, 1);

        assert_eq!(fc.resolve_local("a"), Some(0));
        assert_eq!(fc.resolve_local("b"), Some(1));
        assert_eq!(fc.resolve_local("c"), None);

        let popped = fc.end_scope(1);
        assert_eq!(popped, 2);
        assert_eq!(fc.resolve_local("a"), None);
    }

    #[test]
    fn fn_compiler_nested_scopes() {
        let mut fc = FnCompiler::new("test");

        fc.begin_scope();
        fc.declare_local("x");

        fc.begin_scope();
        fc.declare_local("y");
        assert_eq!(fc.resolve_local("x"), Some(0));
        assert_eq!(fc.resolve_local("y"), Some(1));

        fc.end_scope(1);
        assert_eq!(fc.resolve_local("x"), Some(0));
        assert_eq!(fc.resolve_local("y"), None);

        fc.end_scope(1);
        assert_eq!(fc.resolve_local("x"), None);
    }

    #[test]
    fn fn_compiler_shadowing() {
        let mut fc = FnCompiler::new("test");

        fc.begin_scope();
        fc.declare_local("x"); // slot 0

        fc.begin_scope();
        fc.declare_local("x"); // slot 1 — shadows slot 0

        // resolve_local returns innermost (slot 1)
        assert_eq!(fc.resolve_local("x"), Some(1));
    }

    // ── Constant dedup ──────────────────────────────────────────────────────

    #[test]
    fn constant_dedup_different_values() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        c.add_constant(StackValue::Float(1.0));
        c.add_constant(StackValue::Float(2.0));
        c.add_constant(StackValue::Bool(true));
        c.add_constant(StackValue::Bool(false));
        c.add_constant(StackValue::None);

        assert_eq!(c.constants.len(), 5);
    }

    #[test]
    fn constant_dedup_same_float() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let idx1 = c.add_constant(StackValue::Float(3.14));
        let idx2 = c.add_constant(StackValue::Float(3.14));

        assert_eq!(idx1, idx2);
        assert_eq!(c.constants.len(), 1);
    }

    // ── String pool dedup ───────────────────────────────────────────────────

    #[test]
    fn string_dedup() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        let idx1 = c.add_string("hello");
        let idx2 = c.add_string("hello");
        let idx3 = c.add_string("world");

        assert_eq!(idx1, idx2);
        assert_ne!(idx1, idx3);
        assert_eq!(c.strings.len(), 2);
    }

    // ── Transform ───────────────────────────────────────────────────────────

    #[test]
    fn compile_transform() {
        let program = empty_program();
        let mut c = make_compiler(&program);

        c.current().declare_local("shape");
        c.current().declare_local("t1");
        c.current().declare_local("t2");

        let expr = Expr::Transform {
            expr: Box::new(Expr::Ident("shape".into(), span())),
            transforms: vec![
                Expr::Ident("t1".into(), span()),
                Expr::Ident("t2".into(), span()),
            ],
            span: span(),
        };
        c.compile_expr(&expr);

        let code = &c.current().chunk.code;
        // LoadLocal(0), LoadLocal(1), LoadLocal(2), ApplyTransform(2)
        assert_eq!(code[0], Op::LoadLocal(0));
        assert_eq!(code[1], Op::LoadLocal(1));
        assert_eq!(code[2], Op::LoadLocal(2));
        assert_eq!(code[3], Op::ApplyTransform(2));
    }

    // ── Integration tests (compile full programs) ───────────────────────────

    fn compile_source(src: &str) -> CompiledProgram {
        let tokens = crate::syntax::lexer::Lexer::new(src).tokenize().unwrap();
        let ast = crate::syntax::parser::Parser::new(tokens).parse().unwrap();
        let registry = Box::leak(Box::new(NamespaceRegistry::standard()));
        let _ = crate::analysis::resolve(&ast, registry).unwrap();
        Compiler::compile(&ast, registry)
    }

    #[test]
    fn integration_var_decl_top_level_is_global() {
        let prog = compile_source("let x: float = 5.0");
        // top-level chunk (index 0) should contain StoreGlobal for x
        let chunk = &prog.chunks[0];
        assert!(chunk.code.iter().any(|op| matches!(op, Op::StoreGlobal(_))));
    }

    #[test]
    fn integration_var_decl_in_fn_is_local() {
        let prog = compile_source(
            "fn foo(a: float) -> float { let x: float = 1.0\n return x }",
        );
        // Find foo's chunk
        let foo_idx = prog
            .chunks
            .iter()
            .position(|c| c.name == "foo")
            .expect("foo chunk");
        let chunk = &prog.chunks[foo_idx];
        // Should NOT have StoreGlobal, the local is implicit
        assert!(!chunk.code.iter().any(|op| matches!(op, Op::StoreGlobal(_))));
        // Should have Const and Return
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Return)));
    }

    #[test]
    fn integration_assign_simple() {
        let prog = compile_source("let x: float = 1.0\n x = 2.0");
        let chunk = &prog.chunks[0];
        // Should have two StoreGlobal ops (decl + assign)
        let store_count = chunk
            .code
            .iter()
            .filter(|op| matches!(op, Op::StoreGlobal(_)))
            .count();
        assert!(store_count >= 2);
    }

    #[test]
    fn integration_assign_dotted_2seg() {
        let prog = compile_source(
            "state { let x: float = 0.0 }\nfn on_init(s: State) -> State { s.x = 5.0\n return s }",
        );
        let init_idx = prog.on_init_chunk.expect("on_init chunk");
        let chunk = &prog.chunks[init_idx as usize];
        // Should have SetField
        assert!(chunk.code.iter().any(|op| matches!(op, Op::SetField(_))));
    }

    #[test]
    fn integration_out_emit() {
        let prog = compile_source(
            "import shapes { circle }\nlet c: circle = circle(vec2(0.0, 0.0), 10.0)\nout << c",
        );
        let chunk = &prog.chunks[0];
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Emit)));
    }

    #[test]
    fn integration_print_log() {
        let prog = compile_source("console << 42.0");
        let chunk = &prog.chunks[0];
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Print(1, 0))));
    }

    #[test]
    fn integration_print_warn() {
        let prog = compile_source("console.warn << 42.0");
        let chunk = &prog.chunks[0];
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Print(1, 1))));
    }

    #[test]
    fn integration_print_error() {
        let prog = compile_source("console.error << 42.0");
        let chunk = &prog.chunks[0];
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Print(1, 2))));
    }

    #[test]
    fn integration_if_else() {
        let prog = compile_source(
            "state { let x: float = 0.0 }\nfn on_init(s: State) -> State { if true { s.x = 1.0 } else { s.x = 2.0 }\n return s }",
        );
        let init_idx = prog.on_init_chunk.unwrap();
        let chunk = &prog.chunks[init_idx as usize];
        // Should have Truthy, JumpIfFalse, Jump
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Truthy)));
        assert!(chunk.code.iter().any(|op| matches!(op, Op::JumpIfFalse(_))));
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Jump(_))));
    }

    #[test]
    fn integration_if_let() {
        let prog = compile_source(
            "let x: float? = 5.0\nif let v = x { console << v }",
        );
        let chunk = &prog.chunks[0];
        assert!(chunk.code.iter().any(|op| matches!(op, Op::IsNone)));
        assert!(chunk.code.iter().any(|op| matches!(op, Op::JumpIfTrue(_))));
    }

    #[test]
    fn integration_while_loop() {
        let prog = compile_source(
            "state { let i: float = 0.0 }\nfn on_init(s: State) -> State { while s.i < 5.0 { s.i = s.i + 1.0 }\n return s }",
        );
        let init_idx = prog.on_init_chunk.unwrap();
        let chunk = &prog.chunks[init_idx as usize];
        assert!(chunk.code.iter().any(|op| matches!(op, Op::CheckCancel)));
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Loop(_))));
    }

    #[test]
    fn integration_while_break() {
        let prog = compile_source(
            "fn foo() -> float { let x: float = 0.0\n while true { x = x + 1.0\n if x > 3.0 { break } }\n return x }",
        );
        let foo_idx = prog
            .chunks
            .iter()
            .position(|c| c.name == "foo")
            .unwrap();
        let chunk = &prog.chunks[foo_idx];
        // The break compiles to a Jump that exits the loop
        let jump_count = chunk
            .code
            .iter()
            .filter(|op| matches!(op, Op::Jump(_)))
            .count();
        assert!(jump_count >= 1);
    }

    #[test]
    fn integration_for_loop() {
        let prog = compile_source(
            "fn foo() -> float { let sum: float = 0.0\n for let i: float = 0.0; i < 5.0; i = i + 1.0 { sum = sum + i }\n return sum }",
        );
        let foo_idx = prog
            .chunks
            .iter()
            .position(|c| c.name == "foo")
            .unwrap();
        let chunk = &prog.chunks[foo_idx];
        assert!(chunk.code.iter().any(|op| matches!(op, Op::CheckCancel)));
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Loop(_))));
    }

    #[test]
    fn integration_foreach() {
        let prog = compile_source(
            "let xs: list[float] = [1.0, 2.0, 3.0]\nforeach x: float in xs { console << x }",
        );
        let chunk = &prog.chunks[0];
        assert!(chunk.code.iter().any(|op| matches!(op, Op::IterInit)));
        assert!(chunk.code.iter().any(|op| matches!(op, Op::IterNext(_))));
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Loop(_))));
    }

    #[test]
    fn integration_match_values() {
        let prog = compile_source(
            "let x: float = 2.0\nmatch x { 1.0 => { console << 1.0 } 2.0 => { console << 2.0 } else => { console << 0.0 } }",
        );
        let chunk = &prog.chunks[0];
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Dup)));
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Eq)));
    }

    #[test]
    fn integration_return_value() {
        let prog = compile_source(
            "fn add(a: float, b: float) -> float { return a + b }",
        );
        let add_idx = prog
            .chunks
            .iter()
            .position(|c| c.name == "add")
            .unwrap();
        let chunk = &prog.chunks[add_idx];
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Add)));
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Return)));
        assert_eq!(chunk.param_count, 2);
    }

    #[test]
    fn integration_fn_var() {
        let prog = compile_source(
            "fn add(a: float, b: float) -> float { return a + b }\nfn f = add",
        );
        let chunk = &prog.chunks[0];
        // f is stored as a global
        assert!(chunk.code.iter().any(|op| matches!(op, Op::StoreGlobal(_))));
    }

    #[test]
    fn integration_expr_stmt_pops() {
        let prog = compile_source("1.0 + 2.0");
        let chunk = &prog.chunks[0];
        // Expression statement should have Pop to discard result
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Pop)));
    }

    #[test]
    fn integration_function_def_creates_chunk() {
        let prog = compile_source(
            "fn double(x: float) -> float { return x * 2.0 }",
        );
        let double_idx = prog
            .chunks
            .iter()
            .position(|c| c.name == "double")
            .unwrap();
        let chunk = &prog.chunks[double_idx];
        assert_eq!(chunk.param_count, 1);
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Mul)));
    }

    #[test]
    fn integration_struct_with_methods() {
        let prog = compile_source(
            "struct Point {\n +let x: float = 0.0\n +let y: float = 0.0\n +fn sum(this: Point) -> float { return this.x + this.y }\n}",
        );
        assert_eq!(prog.struct_defs.len(), 1);
        assert_eq!(prog.struct_defs[0].name, "Point");
        assert_eq!(
            prog.struct_defs[0].field_names,
            vec!["x".to_string(), "y".to_string()]
        );
        assert!(prog.struct_defs[0].methods.contains_key("sum"));

        let method = &prog.struct_defs[0].methods["sum"];
        assert!(method.is_public);
        // In the AST, `this: Point` is an explicit param, so param_count = 1
        // The CompiledMethodDef stores method.def.params.len() which includes `this`
        assert_eq!(method.param_count, 1);

        // Method chunk should have GetField, Add, Return
        let chunk = &prog.chunks[method.chunk_index as usize];
        assert!(chunk.code.iter().any(|op| matches!(op, Op::GetField(_))));
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Add)));
    }

    #[test]
    fn integration_imports_create_globals() {
        let prog = compile_source("import shapes { circle }");
        // PI should be a global
        assert!(prog.global_count >= 1);
    }

    #[test]
    fn integration_state_init() {
        let prog = compile_source(
            "state { let speed: float = 5.0\n let name: string = \"hello\" }",
        );
        assert!(prog.state_init_chunk.is_some());
        assert_eq!(prog.state_fields, vec!["speed", "name"]);

        let init_idx = prog.state_init_chunk.unwrap();
        let chunk = &prog.chunks[init_idx as usize];
        // Should have SetField for each field
        let set_count = chunk
            .code
            .iter()
            .filter(|op| matches!(op, Op::SetField(_)))
            .count();
        assert_eq!(set_count, 2);
        // param_count = 1 (state ref)
        assert_eq!(chunk.param_count, 1);
    }

    #[test]
    fn integration_lifecycle_hooks_detected() {
        let prog = compile_source(
            "state { let x: float = 0.0 }\nfn on_init(s: State) -> State { s.x = 1.0\n return s }\nfn on_update(s: State, input: Input) -> State { s.x = s.x + 1.0\n return s }\nfn on_exit(s: State) -> State { console << s.x\n return s }",
        );
        assert!(prog.on_init_chunk.is_some());
        assert!(prog.on_update_chunk.is_some());
        assert!(prog.on_exit_chunk.is_some());
    }

    #[test]
    fn integration_enum_defs() {
        let prog = compile_source(
            "enum MyShape { Circle { radius: float }\n Rect { width: float, height: float } }",
        );
        assert_eq!(prog.enum_defs.len(), 1);
        assert_eq!(prog.enum_defs[0].name, "MyShape");
        assert_eq!(prog.enum_defs[0].variants.len(), 2);
        assert_eq!(prog.enum_defs[0].variants[0].name, "Circle");
        assert_eq!(prog.enum_defs[0].variants[1].name, "Rect");
    }

    #[test]
    fn integration_match_enum() {
        let prog = compile_source(
            "enum Kind { A\n B }\nlet k: Kind = Kind.A\nmatch k { Kind.A => { console << 1.0 } Kind.B => { console << 2.0 } }",
        );
        let chunk = &prog.chunks[0];
        assert!(chunk.code.iter().any(|op| matches!(op, Op::MatchEnum(_))));
    }

    #[test]
    fn integration_continue_in_while() {
        let prog = compile_source(
            "fn foo() -> float { let sum: float = 0.0\n let i: float = 0.0\n while i < 5.0 { i = i + 1.0\n if i == 3.0 { continue }\n sum = sum + i }\n return sum }",
        );
        let foo_idx = prog
            .chunks
            .iter()
            .position(|c| c.name == "foo")
            .unwrap();
        let chunk = &prog.chunks[foo_idx];
        // Continue compiles to a Loop (backward jump)
        let loop_count = chunk
            .code
            .iter()
            .filter(|op| matches!(op, Op::Loop(_)))
            .count();
        // At least 2: one for the while loop back-edge, one for continue
        assert!(loop_count >= 2);
    }

    #[test]
    fn integration_for_continue_runs_step() {
        // Verify that continue in a for loop jumps to the step expression
        let prog = compile_source(
            "fn foo() -> float { let sum: float = 0.0\n for let i: float = 0.0; i < 5.0; i = i + 1.0 { if i == 2.0 { continue }\n sum = sum + i }\n return sum }",
        );
        let foo_idx = prog
            .chunks
            .iter()
            .position(|c| c.name == "foo")
            .unwrap();
        let chunk = &prog.chunks[foo_idx];
        // Should have a Loop op (for-loop back-edge) and a Jump op (continue → step)
        let loop_count = chunk
            .code
            .iter()
            .filter(|op| matches!(op, Op::Loop(_)))
            .count();
        let jump_count = chunk
            .code
            .iter()
            .filter(|op| matches!(op, Op::Jump(_)))
            .count();
        assert!(loop_count >= 1);
        assert!(jump_count >= 1);
    }

    #[test]
    fn integration_assign_dotted_3seg_rebuild() {
        let prog = compile_source(
            "state { let pos: vec2 = vec2(0.0, 0.0) }\nfn on_init(s: State) -> State { s.pos.x = 5.0\n return s }",
        );
        let init_idx = prog.on_init_chunk.unwrap();
        let chunk = &prog.chunks[init_idx as usize];
        // Should have both SetFieldRebuild and SetField
        assert!(chunk
            .code
            .iter()
            .any(|op| matches!(op, Op::SetFieldRebuild(_))));
        assert!(chunk.code.iter().any(|op| matches!(op, Op::SetField(_))));
        // Should have Dup for the rebuild trick
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Dup)));
    }

    #[test]
    fn integration_indexed_assign() {
        let prog = compile_source(
            "let xs: list[float] = [1.0, 2.0, 3.0]\nxs[0] = 10.0",
        );
        let chunk = &prog.chunks[0];
        assert!(chunk.code.iter().any(|op| matches!(op, Op::SetIndex)));
    }

    #[test]
    fn integration_lambda_in_body() {
        let prog = compile_source(
            "let f: fn(float) -> float = (x: float) -> float { return x + 1.0 }",
        );
        // Should have a MakeClosure op in top-level
        let chunk = &prog.chunks[0];
        assert!(chunk
            .code
            .iter()
            .any(|op| matches!(op, Op::MakeClosure(_, _))));
    }

    #[test]
    fn integration_nested_if_else_if() {
        let prog = compile_source(
            "let x: float = 2.0\nif x == 1.0 { console << 1.0 } else { if x == 2.0 { console << 2.0 } else { console << 0.0 } }",
        );
        let chunk = &prog.chunks[0];
        // Multiple JumpIfFalse for nested if
        let jif_count = chunk
            .code
            .iter()
            .filter(|op| matches!(op, Op::JumpIfFalse(_)))
            .count();
        assert!(jif_count >= 2);
    }

    #[test]
    fn integration_private_struct_method() {
        let prog = compile_source(
            "struct Foo {\n +let x: float = 0.0\n #fn secret(this: Foo) -> float { return this.x }\n}",
        );
        let method = &prog.struct_defs[0].methods["secret"];
        assert!(!method.is_public);
    }

    #[test]
    fn integration_empty_return() {
        let prog = compile_source(
            "fn noop() { return }",
        );
        let idx = prog
            .chunks
            .iter()
            .position(|c| c.name == "noop")
            .unwrap();
        let chunk = &prog.chunks[idx];
        // Should emit None before Return for bare return
        assert!(chunk.code.iter().any(|op| matches!(op, Op::Return)));
    }

    #[test]
    fn integration_top_level_chunk_is_index_0() {
        let prog = compile_source("let x: float = 1.0");
        assert_eq!(prog.chunks[0].name, "<top_level>");
    }

    #[test]
    fn integration_global_count_matches() {
        let prog = compile_source("import shapes { circle, rect }\nlet x: float = 1.0\nlet y: float = 2.0");
        // At least circle, rect, x, y as globals
        assert!(prog.global_count >= 4);
    }
}

#[cfg(test)]
pub fn dump_program(src: &str) {
    let tokens = crate::syntax::lexer::Lexer::new(src).tokenize().unwrap();
    let ast = crate::syntax::parser::Parser::new(tokens).parse().unwrap();
    let registry = crate::namespaces::NamespaceRegistry::standard();
    crate::analysis::resolve(&ast, &registry).unwrap();
    let prog = Compiler::compile(&ast, &registry);
    for (i, chunk) in prog.chunks.iter().enumerate() {
        eprintln!("=== Chunk {} ({}) [locals={} params={}] ===", i, chunk.name, chunk.local_count, chunk.param_count);
        for (j, op) in chunk.code.iter().enumerate() {
            let line = chunk.lines[j];
            eprintln!("  {:4}: {:?}  (L{})", j, op, line);
        }
    }
    eprintln!("--- Constants ---");
    for (i, c) in prog.constants.iter().enumerate() {
        eprintln!("  [{}]: {:?}", i, c);
    }
    eprintln!("--- Strings ---");
    for (i, s) in prog.strings.iter().enumerate() {
        eprintln!("  [{}]: {:?}", i, s);
    }
    eprintln!("--- State fields: {:?}", prog.state_fields);
    eprintln!("--- state_init: {:?}, on_init: {:?}, on_update: {:?}", prog.state_init_chunk, prog.on_init_chunk, prog.on_update_chunk);
}

#[test]
fn dump_list_method_test() {
    dump_program("
        let x: float? = 5.0
        state { let val: float = x ?? 42.0 }
    ");
}
