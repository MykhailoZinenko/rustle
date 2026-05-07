use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{ErrorCode, RuntimeError};
use crate::types::draw::{DrawCommand, CoordMeta, RenderMode};

use super::opcode::Op;
use super::value::*;
use super::heap::Heap;
use super::CompiledProgram;
use super::natives::NativeFunc;

const MAX_CALL_DEPTH: usize = 256;

struct CallFrame {
    chunk_idx: u16,
    ip: usize,
    stack_base: usize,
    closure_ref: Option<u32>,
}

pub struct Vm {
    program: Arc<CompiledProgram>,
    native_table: Vec<NativeFunc>,
    pub stack: Vec<StackValue>,
    pub heap: Heap,
    frames: Vec<CallFrame>,
    pub globals: Vec<StackValue>,
    pub output: Vec<DrawCommand>,
    pub coord_meta: CoordMeta,
    cancel: Option<Arc<AtomicBool>>,
}

impl Vm {
    pub fn new(program: Arc<CompiledProgram>, native_table: Vec<NativeFunc>) -> Self {
        let global_count = program.global_count.max(256) as usize;
        Self {
            program,
            native_table,
            stack: Vec::with_capacity(256),
            heap: Heap::new(),
            frames: Vec::with_capacity(64),
            globals: vec![StackValue::None; global_count],
            output: Vec::new(),
            coord_meta: CoordMeta::default(),
            cancel: None,
        }
    }

    pub fn set_cancel(&mut self, cancel: Arc<AtomicBool>) {
        self.cancel = Some(cancel);
    }

    pub fn take_output(&mut self) -> Vec<DrawCommand> {
        std::mem::take(&mut self.output)
    }

    pub fn frames_clear(&mut self) {
        self.frames.clear();
    }

    // ── Main entry points ───────────────────────────────────────────────────

    pub fn run_chunk(&mut self, chunk_idx: u16) -> Result<StackValue, RuntimeError> {
        let base = self.stack.len();
        self.frames.push(CallFrame {
            chunk_idx,
            ip: 0,
            stack_base: base,
            closure_ref: None,
        });
        self.run()?;
        Ok(self.stack.pop().unwrap_or(StackValue::None))
    }

    pub fn run_lifecycle(
        &mut self,
        chunk_idx: u16,
        args: &[StackValue],
    ) -> Result<StackValue, RuntimeError> {
        let base = self.stack.len();
        for arg in args {
            self.stack.push(*arg);
        }
        self.frames.push(CallFrame {
            chunk_idx,
            ip: 0,
            stack_base: base,
            closure_ref: None,
        });
        self.run()?;
        Ok(self.stack.pop().unwrap_or(StackValue::None))
    }

    pub fn create_state(&mut self, field_count: usize) -> StackValue {
        let fields = vec![StackValue::None; field_count];
        self.heap.alloc(HeapObject::State(StateObj { fields }))
    }

    pub fn call_closure_sync(
        &mut self,
        closure_sv: StackValue,
        args: &[StackValue],
        line: usize,
    ) -> Result<StackValue, RuntimeError> {
        let closure_idx = match closure_sv {
            StackValue::HeapRef(idx) => idx,
            _ => {
                return Err(RuntimeError::new(
                    ErrorCode::R001,
                    line,
                    0,
                    "expected closure",
                ))
            }
        };
        let (chunk_index, _upvalue_count) = match self.heap.get(closure_idx) {
            HeapObject::Closure(co) => (co.chunk_index, co.upvalues.len()),
            HeapObject::NativeFnRef(idx) => {
                let native_idx = *idx;
                let result = (self.native_table[native_idx as usize].func)(
                    &mut self.heap,
                    args,
                    &mut self.coord_meta,
                    line,
                )?;
                return Ok(result);
            }
            _ => {
                return Err(RuntimeError::new(
                    ErrorCode::R009,
                    line,
                    0,
                    "not callable",
                ))
            }
        };

        if self.frames.len() >= MAX_CALL_DEPTH {
            return Err(RuntimeError::new(
                ErrorCode::R011,
                line,
                0,
                format!("maximum call depth ({MAX_CALL_DEPTH}) exceeded"),
            ));
        }

        let saved_stack = self.stack.len();
        let saved_frames = self.frames.len();
        let base = self.stack.len();
        for arg in args {
            self.stack.push(*arg);
        }
        self.frames.push(CallFrame {
            chunk_idx: chunk_index,
            ip: 0,
            stack_base: base,
            closure_ref: Some(closure_idx),
        });
        self.run_until_frame(saved_frames)?;
        let result = self.stack.pop().unwrap_or(StackValue::None);
        self.stack.truncate(saved_stack);
        Ok(result)
    }

    // ── Execution loop ──────────────────────────────────────────────────────

    fn run(&mut self) -> Result<(), RuntimeError> {
        self.run_until_frame(0)
    }

    fn run_until_frame(&mut self, stop_at_depth: usize) -> Result<(), RuntimeError> {
        loop {
            let frame = match self.frames.last() {
                Some(f) => f,
                None => return Ok(()),
            };
            if self.frames.len() <= stop_at_depth {
                return Ok(());
            }
            let chunk_idx = frame.chunk_idx;
            let ip = frame.ip;
            let chunk = &self.program.chunks[chunk_idx as usize];
            if ip >= chunk.code.len() {
                // Fell off the end of a chunk — implicit return None
                self.frames.pop();
                continue;
            }
            let op = chunk.code[ip];
            let line = chunk.lines[ip] as usize;
            self.frames.last_mut().unwrap().ip += 1;

            match self.execute_op(op, line) {
                Ok(false) => {}
                Ok(true) => {
                    if self.frames.len() <= stop_at_depth {
                        return Ok(());
                    }
                }
                Err(mut e) => {
                    while self.frames.len() > stop_at_depth {
                        let frame = self.frames.pop().unwrap();
                        let name = &self.program.chunks[frame.chunk_idx as usize].name;
                        let frame_line = if frame.ip > 0 {
                            self.program.chunks[frame.chunk_idx as usize].lines[frame.ip - 1]
                                as usize
                        } else {
                            0
                        };
                        e.push_frame(name, frame_line);
                    }
                    return Err(e);
                }
            }
        }
    }

    // ── Opcode dispatch ─────────────────────────────────────────────────────

    /// Returns Ok(false) to continue, Ok(true) for return, Err for runtime error.
    fn execute_op(&mut self, op: Op, line: usize) -> Result<bool, RuntimeError> {
        match op {
            // ── Stack ───────────────────────────────────────────────────────
            Op::Const(idx) => {
                let val = self.program.constants[idx as usize];
                self.stack.push(val);
            }
            Op::ConstStr(idx) => {
                let s = self.program.strings[idx as usize].clone();
                let sv = self.heap.alloc(HeapObject::Str(s));
                self.stack.push(sv);
            }
            Op::Pop => {
                self.stack.pop();
            }
            Op::Dup => {
                let val = *self.stack.last().unwrap();
                self.stack.push(val);
            }

            // ── Variables ───────────────────────────────────────────────────
            Op::LoadLocal(slot) => {
                let base = self.frames.last().unwrap().stack_base;
                let val = self.stack[base + slot as usize];
                self.stack.push(val);
            }
            Op::StoreLocal(slot) => {
                let val = self.stack.pop().unwrap();
                let base = self.frames.last().unwrap().stack_base;
                self.stack[base + slot as usize] = val;
            }
            Op::LoadUpvalue(idx) => {
                let closure_ref = self.frames.last().unwrap().closure_ref;
                let val = match closure_ref {
                    Some(ci) => match self.heap.get(ci) {
                        HeapObject::Closure(co) => co.upvalues[idx as usize],
                        _ => StackValue::None,
                    },
                    None => StackValue::None,
                };
                self.stack.push(val);
            }
            Op::StoreUpvalue(idx) => {
                let val = self.stack.pop().unwrap();
                let closure_ref = self.frames.last().unwrap().closure_ref;
                if let Some(ci) = closure_ref {
                    if let HeapObject::Closure(co) = self.heap.get_mut(ci) {
                        co.upvalues[idx as usize] = val;
                    }
                }
            }
            Op::LoadGlobal(idx) => {
                let val = if (idx as usize) < self.globals.len() {
                    self.globals[idx as usize]
                } else {
                    StackValue::None
                };
                self.stack.push(val);
            }
            Op::StoreGlobal(idx) => {
                let val = self.stack.pop().unwrap();
                let idx = idx as usize;
                if idx >= self.globals.len() {
                    self.globals.resize(idx + 1, StackValue::None);
                }
                self.globals[idx] = val;
            }

            // ── Arithmetic ──────────────────────────────────────────────────
            Op::Add => {
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack
                    .push(super::ops::exec_add(a, b, &mut self.heap, line)?);
            }
            Op::Sub => {
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack
                    .push(super::ops::exec_sub(a, b, &mut self.heap, line)?);
            }
            Op::Mul => {
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack
                    .push(super::ops::exec_mul(a, b, &mut self.heap, line)?);
            }
            Op::Div => {
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack
                    .push(super::ops::exec_div(a, b, &mut self.heap, line)?);
            }
            Op::Mod => {
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack
                    .push(super::ops::exec_mod(a, b, &mut self.heap, line)?);
            }
            Op::Neg => {
                let v = self.stack.pop().unwrap();
                self.stack
                    .push(super::ops::exec_neg(v, &mut self.heap, line)?);
            }

            // ── Comparison ──────────────────────────────────────────────────
            Op::Gt => {
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack
                    .push(super::ops::exec_gt(a, b, &self.heap, line)?);
            }
            Op::Lt => {
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack
                    .push(super::ops::exec_lt(a, b, &self.heap, line)?);
            }
            Op::Gte => {
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack
                    .push(super::ops::exec_gte(a, b, &self.heap, line)?);
            }
            Op::Lte => {
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack
                    .push(super::ops::exec_lte(a, b, &self.heap, line)?);
            }
            Op::Eq => {
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack
                    .push(StackValue::Bool(values_equal(a, b, &self.heap)));
            }
            Op::Neq => {
                let b = self.stack.pop().unwrap();
                let a = self.stack.pop().unwrap();
                self.stack
                    .push(StackValue::Bool(!values_equal(a, b, &self.heap)));
            }

            // ── Logical ─────────────────────────────────────────────────────
            Op::Not => {
                let v = self.stack.pop().unwrap();
                self.stack
                    .push(StackValue::Bool(!is_truthy(v, &self.heap)));
            }

            // ── Control flow ────────────────────────────────────────────────
            Op::Jump(offset) => {
                let frame = self.frames.last_mut().unwrap();
                frame.ip = (frame.ip as i64 + offset as i64) as usize;
            }
            Op::JumpIfFalse(offset) => {
                let v = self.stack.pop().unwrap();
                if !is_truthy(v, &self.heap) {
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = (frame.ip as i64 + offset as i64) as usize;
                }
            }
            Op::JumpIfTrue(offset) => {
                let v = self.stack.pop().unwrap();
                if is_truthy(v, &self.heap) {
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = (frame.ip as i64 + offset as i64) as usize;
                }
            }
            Op::Loop(offset) => {
                let frame = self.frames.last_mut().unwrap();
                frame.ip = (frame.ip as i64 + offset as i64) as usize;
            }

            // ── Functions ───────────────────────────────────────────────────
            Op::Call(chunk_idx, argc) => {
                if self.frames.len() >= MAX_CALL_DEPTH {
                    return Err(RuntimeError::new(
                        ErrorCode::R011,
                        line,
                        0,
                        format!("maximum call depth ({MAX_CALL_DEPTH}) exceeded"),
                    ));
                }
                let argc = argc as usize;
                let base = self.stack.len() - argc;
                self.frames.push(CallFrame {
                    chunk_idx,
                    ip: 0,
                    stack_base: base,
                    closure_ref: None,
                });
            }
            Op::Return => {
                let result = self.stack.pop().unwrap_or(StackValue::None);
                let frame = self.frames.pop().unwrap();
                self.stack.truncate(frame.stack_base);
                self.stack.push(result);
                if self.frames.is_empty() {
                    return Ok(true);
                }
            }
            Op::CallClosure(argc) => {
                let argc = argc as usize;
                let closure_pos = self.stack.len() - argc - 1;
                let closure_sv = self.stack[closure_pos];

                match closure_sv {
                    StackValue::HeapRef(idx) => match self.heap.get(idx) {
                        HeapObject::Closure(co) => {
                            if self.frames.len() >= MAX_CALL_DEPTH {
                                return Err(RuntimeError::new(
                                    ErrorCode::R011,
                                    line,
                                    0,
                                    format!(
                                        "maximum call depth ({MAX_CALL_DEPTH}) exceeded"
                                    ),
                                ));
                            }
                            let chunk_index = co.chunk_index;
                            // Remove the closure from the stack, keeping args in place.
                            // Stack: [... closure, arg0, arg1, ...]
                            // We want: [... arg0, arg1, ...] with base at closure_pos
                            self.stack.remove(closure_pos);
                            let base = closure_pos;
                            self.frames.push(CallFrame {
                                chunk_idx: chunk_index,
                                ip: 0,
                                stack_base: base,
                                closure_ref: Some(idx),
                            });
                        }
                        HeapObject::NativeFnRef(native_idx) => {
                            let native_idx = *native_idx as usize;
                            // Collect args
                            let args: Vec<StackValue> =
                                self.stack[closure_pos + 1..].to_vec();
                            self.stack.truncate(closure_pos);
                            let result = (self.native_table[native_idx].func)(
                                &mut self.heap,
                                &args,
                                &mut self.coord_meta,
                                line,
                            )?;
                            self.stack.push(result);
                        }
                        _ => {
                            return Err(RuntimeError::new(
                                ErrorCode::R009,
                                line,
                                0,
                                "not callable",
                            ));
                        }
                    },
                    _ => {
                        return Err(RuntimeError::new(
                            ErrorCode::R009,
                            line,
                            0,
                            "not callable",
                        ));
                    }
                }
            }
            Op::CallNative(native_idx, argc) => {
                let argc = argc as usize;
                let start = self.stack.len() - argc;
                let args: Vec<StackValue> = self.stack[start..].to_vec();
                self.stack.truncate(start);
                let result = (self.native_table[native_idx as usize].func)(
                    &mut self.heap,
                    &args,
                    &mut self.coord_meta,
                    line,
                )?;
                self.stack.push(result);
            }

            // ── Methods ─────────────────────────────────────────────────────
            Op::CallMethod(method_str_idx, argc) => {
                let argc = argc as usize;
                let method_name = self.program.strings[method_str_idx as usize].clone();
                let receiver_pos = self.stack.len() - argc - 1;
                let receiver_sv = self.stack[receiver_pos];

                match receiver_sv {
                    StackValue::HeapRef(recv_idx) => {
                        // Check for struct method call
                        let is_struct = matches!(self.heap.get(recv_idx), HeapObject::Object(_));
                        if is_struct {
                            self.call_struct_method(
                                recv_idx,
                                &method_name,
                                argc,
                                receiver_pos,
                                line,
                            )?;
                        } else if matches!(self.heap.get(recv_idx), HeapObject::List(_))
                            && super::methods::is_list_hof(&method_name)
                        {
                            let args: Vec<StackValue> =
                                self.stack[receiver_pos + 1..].to_vec();
                            self.stack.truncate(receiver_pos);
                            let result = super::list_ops::exec_list_hof(
                                self,
                                recv_idx,
                                &method_name,
                                &args,
                                line,
                            )?;
                            self.stack.push(result);
                        } else {
                            // Delegate to methods::call_method
                            let args: Vec<StackValue> =
                                self.stack[receiver_pos + 1..].to_vec();
                            self.stack.truncate(receiver_pos);
                            let result = super::methods::call_method(
                                &mut self.heap,
                                recv_idx,
                                &method_name,
                                &args,
                                &self.program.state_fields,
                                line,
                            )?;
                            self.stack.push(result);
                        }
                    }
                    _ => {
                        return Err(RuntimeError::new(
                            ErrorCode::R004,
                            line,
                            0,
                            format!("cannot call method `{}` on non-object", method_name),
                        ));
                    }
                }
            }

            // ── Fields + indexing ────────────────────────────────────────────
            Op::GetField(str_idx) => {
                let field = self.program.strings[str_idx as usize].clone();
                let receiver = self.stack.pop().unwrap();
                match receiver {
                    StackValue::HeapRef(idx) => {
                        let result = super::fields::get_field(
                            &mut self.heap,
                            idx,
                            &field,
                            &self.program.state_fields,
                            &self.program.struct_defs,
                            line,
                        )?;
                        self.stack.push(result);
                    }
                    _ => {
                        return Err(RuntimeError::new(
                            ErrorCode::R003,
                            line,
                            0,
                            format!("cannot access field `{}` on non-object", field),
                        ));
                    }
                }
            }
            Op::SetField(str_idx) => {
                let field = self.program.strings[str_idx as usize].clone();
                let val = self.stack.pop().unwrap();
                let obj = self.stack.pop().unwrap();
                match obj {
                    StackValue::HeapRef(idx) => {
                        super::fields::set_field_mutate(
                            &mut self.heap,
                            idx,
                            &field,
                            val,
                            &self.program.state_fields,
                            &self.program.struct_defs,
                            line,
                        )?;
                    }
                    _ => {
                        return Err(RuntimeError::new(
                            ErrorCode::R011,
                            line,
                            0,
                            "SetField on non-heap value",
                        ));
                    }
                }
            }
            Op::SetFieldRebuild(str_idx) => {
                let field = self.program.strings[str_idx as usize].clone();
                let new_val = self.stack.pop().unwrap();
                let obj = self.stack.pop().unwrap();
                match obj {
                    StackValue::HeapRef(idx) => {
                        let is_ref_type = matches!(
                            self.heap.get(idx),
                            HeapObject::Object(_) | HeapObject::State(_)
                        );
                        if is_ref_type {
                            super::fields::set_field_mutate(
                                &mut self.heap,
                                idx,
                                &field,
                                new_val,
                                &self.program.state_fields,
                                &self.program.struct_defs,
                                line,
                            )?;
                            self.stack.push(obj);
                        } else {
                            let rebuilt = super::fields::set_field_rebuild(
                                &mut self.heap,
                                idx,
                                &field,
                                new_val,
                                line,
                            )?;
                            self.stack.push(rebuilt);
                        }
                    }
                    _ => {
                        return Err(RuntimeError::new(
                            ErrorCode::R011,
                            line,
                            0,
                            "SetFieldRebuild on non-heap value",
                        ));
                    }
                }
            }
            Op::GetIndex => {
                let index = self.stack.pop().unwrap();
                let obj = self.stack.pop().unwrap();
                match (obj, index) {
                    (StackValue::HeapRef(list_idx), StackValue::Float(f)) => {
                        if f < 0.0 {
                            return Err(RuntimeError::new(
                                ErrorCode::R006, line, 0,
                                format!("negative index: {}", f as i64),
                            ));
                        }
                        let i = f as usize;
                        let items = match self.heap.get(list_idx) {
                            HeapObject::List(items) => items,
                            _ => {
                                return Err(RuntimeError::new(
                                    ErrorCode::R001,
                                    line,
                                    0,
                                    "index access on non-list",
                                ));
                            }
                        };
                        if i >= items.len() {
                            return Err(RuntimeError::new(
                                ErrorCode::R005,
                                line,
                                0,
                                format!(
                                    "index {} out of bounds for list of length {}",
                                    i,
                                    items.len()
                                ),
                            ));
                        }
                        let val = items[i];
                        self.stack.push(val);
                    }
                    (StackValue::HeapRef(_), _) => {
                        return Err(RuntimeError::new(
                            ErrorCode::R001,
                            line,
                            0,
                            "index must be a float",
                        ));
                    }
                    _ => {
                        return Err(RuntimeError::new(
                            ErrorCode::R001,
                            line,
                            0,
                            "index access on non-list",
                        ));
                    }
                }
            }
            Op::SetIndex => {
                let val = self.stack.pop().unwrap();
                let index = self.stack.pop().unwrap();
                let obj = self.stack.pop().unwrap();
                match (obj, index) {
                    (StackValue::HeapRef(list_idx), StackValue::Float(f)) => {
                        let i = f as usize;
                        match self.heap.get_mut(list_idx) {
                            HeapObject::List(items) => {
                                if i >= items.len() {
                                    return Err(RuntimeError::new(
                                        ErrorCode::R005,
                                        line,
                                        0,
                                        format!(
                                            "index {} out of bounds for list of length {}",
                                            i,
                                            items.len()
                                        ),
                                    ));
                                }
                                items[i] = val;
                            }
                            _ => {
                                return Err(RuntimeError::new(
                                    ErrorCode::R001,
                                    line,
                                    0,
                                    "index assignment on non-list",
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(RuntimeError::new(
                            ErrorCode::R001,
                            line,
                            0,
                            "invalid index assignment",
                        ));
                    }
                }
            }

            // ── Construction ────────────────────────────────────────────────
            Op::MakeVec2 => {
                let y = self.stack.pop().unwrap().as_float().unwrap_or(0.0);
                let x = self.stack.pop().unwrap().as_float().unwrap_or(0.0);
                let sv = self.heap.alloc(HeapObject::Vec2(x, y));
                self.stack.push(sv);
            }
            Op::MakeVec3 => {
                let z = self.stack.pop().unwrap().as_float().unwrap_or(0.0);
                let y = self.stack.pop().unwrap().as_float().unwrap_or(0.0);
                let x = self.stack.pop().unwrap().as_float().unwrap_or(0.0);
                let sv = self.heap.alloc(HeapObject::Vec3(x, y, z));
                self.stack.push(sv);
            }
            Op::MakeVec4 => {
                let w = self.stack.pop().unwrap().as_float().unwrap_or(0.0);
                let z = self.stack.pop().unwrap().as_float().unwrap_or(0.0);
                let y = self.stack.pop().unwrap().as_float().unwrap_or(0.0);
                let x = self.stack.pop().unwrap().as_float().unwrap_or(0.0);
                let sv = self.heap.alloc(HeapObject::Vec4(x, y, z, w));
                self.stack.push(sv);
            }
            Op::MakeColor(n) => {
                let a = if n == 4 {
                    self.stack.pop().unwrap().as_float().unwrap_or(1.0)
                } else {
                    1.0
                };
                let b = self.stack.pop().unwrap().as_float().unwrap_or(0.0);
                let g = self.stack.pop().unwrap().as_float().unwrap_or(0.0);
                let r = self.stack.pop().unwrap().as_float().unwrap_or(0.0);
                let sv = self.heap.alloc(HeapObject::Color { r, g, b, a });
                self.stack.push(sv);
            }
            Op::MakeList(count) => {
                let count = count as usize;
                let start = self.stack.len() - count;
                let items: Vec<StackValue> = self.stack.drain(start..).collect();
                let sv = self.heap.alloc(HeapObject::List(items));
                self.stack.push(sv);
            }
            Op::MakeClosure(chunk_idx, upvalue_count) => {
                let uv_count = upvalue_count as usize;
                let start = self.stack.len() - uv_count;
                let upvalues: Vec<StackValue> = self.stack.drain(start..).collect();
                let sv = self.heap.alloc(HeapObject::Closure(ClosureObj {
                    chunk_index: chunk_idx,
                    upvalues,
                }));
                self.stack.push(sv);
            }
            Op::MakeStruct(def_idx, field_count) => {
                let fc = field_count as usize;
                let start = self.stack.len() - fc;
                let fields: Vec<StackValue> = self.stack.drain(start..).collect();
                let type_name =
                    self.program.struct_defs[def_idx as usize].name.clone();
                let sv = self.heap.alloc(HeapObject::Object(StructObj {
                    type_name,
                    struct_def_idx: def_idx,
                    fields,
                }));
                self.stack.push(sv);
            }
            Op::MakeEnum(enum_str, variant_str, field_count) => {
                let fc = field_count as usize;
                let start = self.stack.len() - fc;
                let field_values: Vec<StackValue> = self.stack.drain(start..).collect();

                let enum_name = self.program.strings[enum_str as usize].clone();
                let variant_name = self.program.strings[variant_str as usize].clone();

                // Look up field names from enum_defs
                let field_names = self
                    .program
                    .enum_defs
                    .iter()
                    .find(|e| e.name == enum_name)
                    .and_then(|e| e.variants.iter().find(|v| v.name == variant_name))
                    .map(|v| v.field_names.clone())
                    .unwrap_or_default();

                let sv =
                    self.heap
                        .alloc(HeapObject::EnumVariant(EnumVariantObj {
                            enum_name,
                            variant: variant_name,
                            field_names,
                            field_values,
                        }));
                self.stack.push(sv);
            }
            Op::MakeNativeFnRef(idx) => {
                let sv = self.heap.alloc(HeapObject::NativeFnRef(idx));
                self.stack.push(sv);
            }
            Op::MakeRenderMode(code) => {
                let rm = match code {
                    0 => RenderMode::Sdf,
                    1 => RenderMode::Fill,
                    2 => RenderMode::Outline,
                    _ => RenderMode::Sdf,
                };
                let sv = self.heap.alloc(HeapObject::RenderMode(rm));
                self.stack.push(sv);
            }

            // ── Output ──────────────────────────────────────────────────────
            Op::Emit => {
                let val = self.stack.pop().unwrap();
                if let StackValue::HeapRef(idx) = val {
                    if let HeapObject::Shape(sd) = self.heap.get(idx) {
                        self.output.push(DrawCommand::DrawShape(sd.clone()));
                    }
                }
            }
            Op::Print(count, level) => {
                let count = count as usize;
                let start = self.stack.len() - count;
                let parts: Vec<String> = self.stack[start..]
                    .iter()
                    .map(|&sv| display(sv, &self.heap))
                    .collect();
                self.stack.truncate(start);
                let msg = parts.join(" ");
                let cmd = match level {
                    1 => DrawCommand::Warn(msg),
                    2 => DrawCommand::Error(msg),
                    _ => DrawCommand::Print(msg),
                };
                self.output.push(cmd);
            }

            // ── Optionals ───────────────────────────────────────────────────
            Op::CoalesceJump(offset) => {
                let top = *self.stack.last().unwrap();
                if !matches!(top, StackValue::None) {
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = (frame.ip as i64 + offset as i64) as usize;
                } else {
                    // Is None — fall through. The compiler emits Pop next.
                }
            }
            Op::OptChainJump(offset) => {
                let top = *self.stack.last().unwrap();
                if matches!(top, StackValue::None) {
                    // Is None — jump (keep None on stack)
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = (frame.ip as i64 + offset as i64) as usize;
                }
                // Not None — fall through
            }
            Op::IsNone => {
                let top = *self.stack.last().unwrap();
                self.stack
                    .push(StackValue::Bool(matches!(top, StackValue::None)));
            }

            // ── Casts ───────────────────────────────────────────────────────
            Op::Truthy => {
                let v = self.stack.pop().unwrap();
                self.stack
                    .push(StackValue::Bool(is_truthy(v, &self.heap)));
            }
            Op::CastFloat => {
                let v = self.stack.pop().unwrap();
                let f = match v {
                    StackValue::Float(f) => f,
                    StackValue::Bool(b) => {
                        if b {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    StackValue::HeapRef(idx) => match self.heap.get(idx) {
                        HeapObject::Str(s) => s.parse::<f64>().map_err(|_| {
                            RuntimeError::new(
                                ErrorCode::R001,
                                line,
                                0,
                                format!("cannot convert string '{}' to float", s),
                            )
                        })?,
                        _ => {
                            return Err(RuntimeError::new(
                                ErrorCode::R001,
                                line,
                                0,
                                "cannot cast to float",
                            ));
                        }
                    },
                    _ => {
                        return Err(RuntimeError::new(
                            ErrorCode::R001,
                            line,
                            0,
                            "cannot cast to float",
                        ));
                    }
                };
                self.stack.push(StackValue::Float(f));
            }
            Op::CastString => {
                let v = self.stack.pop().unwrap();
                let s = display(v, &self.heap);
                let sv = self.heap.alloc(HeapObject::Str(s));
                self.stack.push(sv);
            }

            // ── Iteration ───────────────────────────────────────────────────
            Op::IterInit => {
                let list_sv = self.stack.pop().unwrap();
                match list_sv {
                    StackValue::HeapRef(idx) => {
                        let items = match self.heap.get(idx) {
                            HeapObject::List(items) => items.clone(),
                            _ => {
                                return Err(RuntimeError::new(
                                    ErrorCode::R001,
                                    line,
                                    0,
                                    "IterInit on non-list",
                                ));
                            }
                        };
                        let sv = self.heap.alloc(HeapObject::Iterator(IteratorObj {
                            items,
                            index: 0,
                        }));
                        self.stack.push(sv);
                    }
                    _ => {
                        return Err(RuntimeError::new(
                            ErrorCode::R001,
                            line,
                            0,
                            "IterInit on non-list",
                        ));
                    }
                }
            }
            Op::IterNext(offset) => {
                let iter_sv = self.stack.pop().unwrap();
                match iter_sv {
                    StackValue::HeapRef(idx) => {
                        let (exhausted, next_val) = {
                            match self.heap.get_mut(idx) {
                                HeapObject::Iterator(it) => {
                                    if it.index >= it.items.len() {
                                        (true, StackValue::None)
                                    } else {
                                        let val = it.items[it.index];
                                        it.index += 1;
                                        (false, val)
                                    }
                                }
                                _ => {
                                    return Err(RuntimeError::new(
                                        ErrorCode::R001,
                                        line,
                                        0,
                                        "IterNext on non-iterator",
                                    ));
                                }
                            }
                        };
                        if exhausted {
                            // Jump past loop body
                            let frame = self.frames.last_mut().unwrap();
                            frame.ip = (frame.ip as i64 + offset as i64) as usize;
                        } else {
                            // Push next value only; iterator stays in its local slot
                            // (it was mutated in-place on the heap via get_mut)
                            self.stack.push(next_val);
                        }
                    }
                    _ => {
                        return Err(RuntimeError::new(
                            ErrorCode::R001,
                            line,
                            0,
                            "IterNext on non-iterator",
                        ));
                    }
                }
            }

            // ── Pattern matching ────────────────────────────────────────────
            Op::MatchEnum(str_idx) => {
                let variant_name = &self.program.strings[str_idx as usize];
                let top = *self.stack.last().unwrap();
                let matches = match top {
                    StackValue::HeapRef(idx) => match self.heap.get(idx) {
                        HeapObject::EnumVariant(ev) => ev.variant == *variant_name,
                        _ => false,
                    },
                    _ => false,
                };
                self.stack.push(StackValue::Bool(matches));
            }
            Op::GetEnumField(str_idx) => {
                let field_name = self.program.strings[str_idx as usize].clone();
                let enum_sv = self.stack.pop().unwrap();
                match enum_sv {
                    StackValue::HeapRef(idx) => {
                        let val = match self.heap.get(idx) {
                            HeapObject::EnumVariant(ev) => {
                                let pos = ev
                                    .field_names
                                    .iter()
                                    .position(|n| n == &field_name)
                                    .ok_or_else(|| {
                                        RuntimeError::new(
                                            ErrorCode::R003,
                                            line,
                                            0,
                                            format!(
                                                "field '{}' not found on enum variant",
                                                field_name
                                            ),
                                        )
                                    })?;
                                ev.field_values[pos]
                            }
                            _ => {
                                return Err(RuntimeError::new(
                                    ErrorCode::R001,
                                    line,
                                    0,
                                    "GetEnumField on non-enum",
                                ));
                            }
                        };
                        self.stack.push(val);
                    }
                    _ => {
                        return Err(RuntimeError::new(
                            ErrorCode::R001,
                            line,
                            0,
                            "GetEnumField on non-heap value",
                        ));
                    }
                }
            }

            // ── String ──────────────────────────────────────────────────────
            Op::Concat(count) => {
                let count = count as usize;
                let start = self.stack.len() - count;
                let parts: Vec<String> = self.stack[start..]
                    .iter()
                    .map(|&sv| display(sv, &self.heap))
                    .collect();
                self.stack.truncate(start);
                let result = parts.concat();
                let sv = self.heap.alloc(HeapObject::Str(result));
                self.stack.push(sv);
            }

            // ── Transforms ──────────────────────────────────────────────────
            Op::ApplyTransform(count) => {
                let count = count as usize;
                // Pop transforms (they are on top)
                let transform_start = self.stack.len() - count;
                let transforms: Vec<StackValue> =
                    self.stack.drain(transform_start..).collect();
                // Pop shape
                let shape_sv = self.stack.pop().unwrap();
                match shape_sv {
                    StackValue::HeapRef(idx) => {
                        let mut sd = match self.heap.get(idx) {
                            HeapObject::Shape(sd) => sd.clone(),
                            _ => {
                                return Err(RuntimeError::new(
                                    ErrorCode::R001,
                                    line,
                                    0,
                                    "ApplyTransform on non-shape",
                                ));
                            }
                        };
                        for t_sv in transforms {
                            if let StackValue::HeapRef(ti) = t_sv {
                                if let HeapObject::Transform(td) = self.heap.get(ti) {
                                    sd.transforms.push(td.clone());
                                }
                            }
                        }
                        let sv = self.heap.alloc(HeapObject::Shape(sd));
                        self.stack.push(sv);
                    }
                    _ => {
                        return Err(RuntimeError::new(
                            ErrorCode::R001,
                            line,
                            0,
                            "ApplyTransform on non-shape",
                        ));
                    }
                }
            }

            // ── Try / Result ────────────────────────────────────────────────
            Op::TryCall(chunk_idx) => {
                let saved_stack = self.stack.len();
                let saved_frames = self.frames.len();
                let base = self.stack.len();
                self.frames.push(CallFrame {
                    chunk_idx,
                    ip: 0,
                    stack_base: base,
                    closure_ref: None,
                });
                match self.run_until_frame(saved_frames) {
                    Ok(()) => {
                        let result = self.stack.pop().unwrap_or(StackValue::None);
                        self.stack.truncate(saved_stack);
                        let sv = self.heap.alloc(HeapObject::ResOk(result));
                        self.stack.push(sv);
                    }
                    Err(e) => {
                        self.stack.truncate(saved_stack);
                        self.frames.truncate(saved_frames);
                        let sv = self.heap.alloc(HeapObject::ResErr(e.message));
                        self.stack.push(sv);
                    }
                }
            }

            // ── Cancellation ────────────────────────────────────────────────
            Op::CheckCancel => {
                if let Some(ref cancel) = self.cancel {
                    if cancel.load(Ordering::Relaxed) {
                        return Err(RuntimeError::new(
                            ErrorCode::R010,
                            line,
                            0,
                            "script cancelled",
                        ));
                    }
                }
            }

            // ── Nop ─────────────────────────────────────────────────────────
            Op::Nop => {}
        }

        Ok(false)
    }

    // ── Struct method dispatch helper ────────────────────────────────────────

    fn call_struct_method(
        &mut self,
        recv_idx: u32,
        method_name: &str,
        argc: usize,
        receiver_pos: usize,
        line: usize,
    ) -> Result<(), RuntimeError> {
        // Extract struct info before mutating stack
        let (def_idx, _type_name) = match self.heap.get(recv_idx) {
            HeapObject::Object(obj) => (obj.struct_def_idx, obj.type_name.clone()),
            _ => unreachable!(),
        };

        // Handle .clone()
        if method_name == "clone" {
            self.stack.truncate(receiver_pos);
            let cloned = deep_clone(StackValue::HeapRef(recv_idx), &mut self.heap);
            self.stack.push(cloned);
            return Ok(());
        }

        let def = &self.program.struct_defs[def_idx as usize];
        if let Some(method_def) = def.methods.get(method_name) {
            if self.frames.len() >= MAX_CALL_DEPTH {
                return Err(RuntimeError::new(
                    ErrorCode::R011,
                    line,
                    0,
                    format!("maximum call depth ({MAX_CALL_DEPTH}) exceeded"),
                ));
            }
            let chunk_idx = method_def.chunk_index;
            // Stack layout: [... receiver, arg0, arg1, ...]
            // receiver IS local 0 (this), args are local 1+
            let base = receiver_pos;
            self.frames.push(CallFrame {
                chunk_idx,
                ip: 0,
                stack_base: base,
                closure_ref: None,
            });
            Ok(())
        } else {
            // Try methods::call_method as fallback (for built-in object methods)
            let args: Vec<StackValue> = self.stack[receiver_pos + 1..].to_vec();
            self.stack.truncate(receiver_pos);
            let result = super::methods::call_method(
                &mut self.heap,
                recv_idx,
                method_name,
                &args,
                &self.program.state_fields,
                line,
            )?;
            self.stack.push(result);
            Ok(())
        }
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::chunk::Chunk;
    use crate::vm::natives::native_table;
    use crate::vm::{CompiledEnumDef, CompiledEnumVariant, CompiledStructDef};
    use std::collections::HashMap;

    fn make_program(
        chunks: Vec<Chunk>,
        constants: Vec<StackValue>,
        strings: Vec<String>,
    ) -> Arc<CompiledProgram> {
        Arc::new(CompiledProgram {
            chunks,
            constants,
            strings,
            natives: vec![],
            state_init_chunk: None,
            on_init_chunk: None,
            on_update_chunk: None,
            on_exit_chunk: None,
            state_fields: vec![],
            struct_defs: vec![],
            enum_defs: vec![],
            global_count: 16,
        })
    }

    fn make_vm(program: Arc<CompiledProgram>) -> Vm {
        Vm::new(program, native_table())
    }

    fn make_chunk(name: &str, ops: Vec<(Op, u32)>, local_count: u16) -> Chunk {
        let mut chunk = Chunk::new(name);
        chunk.local_count = local_count;
        for (op, line) in ops {
            chunk.emit(op, line);
        }
        chunk
    }

    // ── Arithmetic ──────────────────────────────────────────────────────────

    #[test]
    fn arithmetic_add() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::Const(1), 1),
                (Op::Add, 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![StackValue::Float(3.0), StackValue::Float(4.0)],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 7.0));
    }

    // ── Locals ──────────────────────────────────────────────────────────────

    #[test]
    fn locals_store_and_load() {
        // Simulate VarDecl: push value (creates local slot 0), then
        // reassign with StoreLocal, then load back.
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),      // VarDecl: push initial value → local slot 0
                (Op::Const(1), 1),      // push new value
                (Op::StoreLocal(0), 1), // reassign slot 0
                (Op::LoadLocal(0), 1),  // load back
                (Op::Return, 1),
            ],
            1,
        );
        let prog = make_program(
            vec![chunk],
            vec![StackValue::Float(0.0), StackValue::Float(42.0)],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 42.0));
    }

    // ── Globals ─────────────────────────────────────────────────────────────

    #[test]
    fn globals_store_and_load() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::StoreGlobal(5), 1),
                (Op::LoadGlobal(5), 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(vec![chunk], vec![StackValue::Float(99.0)], vec![]);
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 99.0));
    }

    // ── Jump ────────────────────────────────────────────────────────────────

    #[test]
    fn jump_forward_skips() {
        // Push 1, jump over push 2, push 3, add, return
        // Result should be 1 + 3 = 4 (skipping the push 2)
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),      // 0: push 1
                (Op::Jump(1), 1),       // 1: jump to 3 (skip instruction at 2)
                (Op::Const(1), 1),      // 2: push 2 (skipped)
                (Op::Const(2), 1),      // 3: push 3
                (Op::Add, 1),           // 4: add
                (Op::Return, 1),        // 5: return
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![
                StackValue::Float(1.0),
                StackValue::Float(2.0),
                StackValue::Float(3.0),
            ],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 4.0));
    }

    // ── JumpIfFalse ─────────────────────────────────────────────────────────

    #[test]
    fn jump_if_false_taken() {
        // Push false, JumpIfFalse(1), push 10, push 20, return
        // false → jump, so result is 20
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),          // 0: push false
                (Op::JumpIfFalse(1), 1),    // 1: jump to 3
                (Op::Const(1), 1),          // 2: push 10 (skipped)
                (Op::Const(2), 1),          // 3: push 20
                (Op::Return, 1),            // 4: return
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![
                StackValue::Bool(false),
                StackValue::Float(10.0),
                StackValue::Float(20.0),
            ],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 20.0));
    }

    #[test]
    fn jump_if_false_not_taken() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),          // 0: push true
                (Op::JumpIfFalse(1), 1),    // 1: no jump
                (Op::Const(1), 1),          // 2: push 10
                (Op::Return, 1),            // 3: return
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![StackValue::Bool(true), StackValue::Float(10.0)],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 10.0));
    }

    // ── Call + Return ────────────────────────────────────────────────────────

    #[test]
    fn call_and_return() {
        // Chunk 0: push 5.0, call chunk 1 (argc=1), return
        // Chunk 1: load local 0, const 10, add, return
        let main_chunk = make_chunk(
            "main",
            vec![
                (Op::Const(0), 1),      // push 5.0
                (Op::Call(1, 1), 1),    // call chunk 1 with 1 arg
                (Op::Return, 1),
            ],
            0,
        );
        let fn_chunk = make_chunk(
            "double_add",
            vec![
                (Op::LoadLocal(0), 1),  // load arg
                (Op::Const(1), 1),      // push 10
                (Op::Add, 1),
                (Op::Return, 1),
            ],
            1,
        );
        let prog = make_program(
            vec![main_chunk, fn_chunk],
            vec![StackValue::Float(5.0), StackValue::Float(10.0)],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 15.0));
    }

    // ── CallClosure ─────────────────────────────────────────────────────────

    #[test]
    fn call_closure() {
        // Chunk 0: make closure(chunk 1, 0 upvalues), push 7, call_closure(1), return
        // Chunk 1: load local 0, const 3, add, return
        let main_chunk = make_chunk(
            "main",
            vec![
                (Op::MakeClosure(1, 0), 1),
                (Op::Const(0), 1),          // push 7
                (Op::CallClosure(1), 1),
                (Op::Return, 1),
            ],
            0,
        );
        let fn_chunk = make_chunk(
            "adder",
            vec![
                (Op::LoadLocal(0), 1),
                (Op::Const(1), 1),          // push 3
                (Op::Add, 1),
                (Op::Return, 1),
            ],
            1,
        );
        let prog = make_program(
            vec![main_chunk, fn_chunk],
            vec![StackValue::Float(7.0), StackValue::Float(3.0)],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 10.0));
    }

    // ── CallNative ──────────────────────────────────────────────────────────

    #[test]
    fn call_native_sin() {
        let nt = native_table();
        let sin_idx = nt
            .iter()
            .position(|f| f.name == "sin")
            .unwrap() as u16;

        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::CallNative(sin_idx, 1), 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(vec![chunk], vec![StackValue::Float(0.0)], vec![]);
        let mut vm = Vm::new(prog, nt);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f.abs() < 1e-12));
    }

    // ── GetField ────────────────────────────────────────────────────────────

    #[test]
    fn get_field_vec2_x() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),          // push x=3
                (Op::Const(1), 1),          // push y=4
                (Op::MakeVec2, 1),
                (Op::GetField(0), 1),       // .x
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![StackValue::Float(3.0), StackValue::Float(4.0)],
            vec!["x".to_string()],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 3.0));
    }

    // ── SetField (mutate) ───────────────────────────────────────────────────

    #[test]
    fn set_field_state() {
        // State is pre-stored in global 0. Load it, set field "x" to 42, get field "x"
        let chunk = make_chunk(
            "test",
            vec![
                (Op::LoadGlobal(0), 1),     // load state
                (Op::Const(0), 1),          // push 42
                (Op::SetField(0), 1),       // state.x = 42
                (Op::LoadGlobal(0), 1),     // load state again
                (Op::GetField(0), 1),       // .x
                (Op::Return, 1),
            ],
            0,
        );

        let prog = Arc::new(CompiledProgram {
            chunks: vec![chunk],
            constants: vec![StackValue::Float(42.0)],
            strings: vec!["x".to_string()],
            natives: vec![],
            state_init_chunk: None,
            on_init_chunk: None,
            on_update_chunk: None,
            on_exit_chunk: None,
            state_fields: vec!["x".to_string()],
            struct_defs: vec![],
            enum_defs: vec![],
            global_count: 16,
        });

        let mut vm = Vm::new(prog, native_table());

        // Create a state object and store it in global 0
        let state_sv = vm.create_state(1);
        vm.globals[0] = state_sv;

        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 42.0));
    }

    // ── SetFieldRebuild ─────────────────────────────────────────────────────

    #[test]
    fn set_field_rebuild_vec2() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),          // push 1.0 (x)
                (Op::Const(1), 1),          // push 2.0 (y)
                (Op::MakeVec2, 1),
                (Op::Const(2), 1),          // push 99.0 (new x)
                (Op::SetFieldRebuild(0), 1),// rebuild with x=99
                (Op::GetField(0), 1),       // .x on rebuilt
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![
                StackValue::Float(1.0),
                StackValue::Float(2.0),
                StackValue::Float(99.0),
            ],
            vec!["x".to_string()],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 99.0));
    }

    // ── GetIndex / SetIndex ─────────────────────────────────────────────────

    #[test]
    fn get_index_list() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::Const(1), 1),
                (Op::MakeList(2), 1),
                (Op::Const(2), 1),          // index 1
                (Op::GetIndex, 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![
                StackValue::Float(10.0),
                StackValue::Float(20.0),
                StackValue::Float(1.0),
            ],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 20.0));
    }

    #[test]
    fn set_index_list() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::Const(1), 1),
                (Op::MakeList(2), 1),       // [10, 20]
                (Op::StoreGlobal(0), 1),
                (Op::LoadGlobal(0), 1),     // list
                (Op::Const(2), 1),          // index 0
                (Op::Const(3), 1),          // value 99
                (Op::SetIndex, 1),
                (Op::LoadGlobal(0), 1),     // list
                (Op::Const(2), 1),          // index 0
                (Op::GetIndex, 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![
                StackValue::Float(10.0),
                StackValue::Float(20.0),
                StackValue::Float(0.0),
                StackValue::Float(99.0),
            ],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 99.0));
    }

    // ── MakeVec2/MakeList/MakeStruct/MakeEnum ───────────────────────────────

    #[test]
    fn make_vec2() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::Const(1), 1),
                (Op::MakeVec2, 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![StackValue::Float(5.0), StackValue::Float(6.0)],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        let idx = result.as_heap_ref().unwrap();
        assert!(matches!(vm.heap.get(idx), HeapObject::Vec2(x, y) if *x == 5.0 && *y == 6.0));
    }

    #[test]
    fn make_list() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::Const(1), 1),
                (Op::Const(2), 1),
                (Op::MakeList(3), 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![
                StackValue::Float(1.0),
                StackValue::Float(2.0),
                StackValue::Float(3.0),
            ],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        let idx = result.as_heap_ref().unwrap();
        match vm.heap.get(idx) {
            HeapObject::List(items) => assert_eq!(items.len(), 3),
            _ => panic!("expected list"),
        }
    }

    #[test]
    fn make_struct() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::Const(1), 1),
                (Op::MakeStruct(0, 2), 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = CompiledProgram {
            chunks: vec![chunk],
            constants: vec![StackValue::Float(10.0), StackValue::Float(20.0)],
            strings: vec![],
            natives: vec![],
            state_init_chunk: None,
            on_init_chunk: None,
            on_update_chunk: None,
            on_exit_chunk: None,
            state_fields: vec![],
            struct_defs: vec![CompiledStructDef {
                name: "Point".to_string(),
                field_names: vec!["x".to_string(), "y".to_string()],
                methods: HashMap::new(),
            }],
            enum_defs: vec![],
            global_count: 16,
        };
        let mut vm = Vm::new(Arc::new(prog), native_table());
        let result = vm.run_chunk(0).unwrap();
        let idx = result.as_heap_ref().unwrap();
        match vm.heap.get(idx) {
            HeapObject::Object(obj) => {
                assert_eq!(obj.type_name, "Point");
                assert_eq!(obj.fields.len(), 2);
            }
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn make_enum() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::MakeEnum(0, 1, 1), 1),    // enum_str=0, variant_str=1, 1 field
                (Op::Return, 1),
            ],
            0,
        );
        let prog = CompiledProgram {
            chunks: vec![chunk],
            constants: vec![StackValue::Float(5.0)],
            strings: vec!["Shape".to_string(), "Circle".to_string()],
            natives: vec![],
            state_init_chunk: None,
            on_init_chunk: None,
            on_update_chunk: None,
            on_exit_chunk: None,
            state_fields: vec![],
            struct_defs: vec![],
            enum_defs: vec![CompiledEnumDef {
                name: "Shape".to_string(),
                variants: vec![CompiledEnumVariant {
                    name: "Circle".to_string(),
                    field_names: vec!["radius".to_string()],
                }],
            }],
            global_count: 16,
        };
        let mut vm = Vm::new(Arc::new(prog), native_table());
        let result = vm.run_chunk(0).unwrap();
        let idx = result.as_heap_ref().unwrap();
        match vm.heap.get(idx) {
            HeapObject::EnumVariant(ev) => {
                assert_eq!(ev.enum_name, "Shape");
                assert_eq!(ev.variant, "Circle");
                assert_eq!(ev.field_names, vec!["radius"]);
            }
            _ => panic!("expected enum variant"),
        }
    }

    // ── Emit ────────────────────────────────────────────────────────────────

    #[test]
    fn emit_shape() {
        let nt = native_table();
        let circle_idx = nt.iter().position(|f| f.name == "circle").unwrap() as u16;
        let vec2_idx = nt.iter().position(|f| f.name == "vec2").unwrap() as u16;

        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),                  // x=0
                (Op::Const(0), 1),                  // y=0
                (Op::CallNative(vec2_idx, 2), 1),   // vec2(0, 0)
                (Op::Const(1), 1),                  // radius=10
                (Op::CallNative(circle_idx, 2), 1), // circle(center, 10)
                (Op::Emit, 1),
                (Op::Const(0), 1),                  // push a dummy return value
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![StackValue::Float(0.0), StackValue::Float(10.0)],
            vec![],
        );
        let mut vm = Vm::new(prog, nt);
        vm.run_chunk(0).unwrap();
        assert_eq!(vm.output.len(), 1);
        assert!(matches!(&vm.output[0], DrawCommand::DrawShape(_)));
    }

    // ── Print ───────────────────────────────────────────────────────────────

    #[test]
    fn print_values() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::Const(1), 1),
                (Op::Print(2, 0), 1),
                (Op::Const(0), 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![StackValue::Float(1.0), StackValue::Float(2.0)],
            vec![],
        );
        let mut vm = make_vm(prog);
        vm.run_chunk(0).unwrap();
        assert_eq!(vm.output.len(), 1);
        match &vm.output[0] {
            DrawCommand::Print(msg) => assert_eq!(msg, "1 2"),
            _ => panic!("expected Print"),
        }
    }

    // ── Truthy / CastFloat / CastString ─────────────────────────────────────

    #[test]
    fn truthy_cast() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),      // push 0.0
                (Op::Truthy, 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(vec![chunk], vec![StackValue::Float(0.0)], vec![]);
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Bool(false)));
    }

    #[test]
    fn cast_float_from_bool() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::CastFloat, 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(vec![chunk], vec![StackValue::Bool(true)], vec![]);
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 1.0));
    }

    #[test]
    fn cast_string() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::CastString, 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(vec![chunk], vec![StackValue::Float(42.0)], vec![]);
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        let idx = result.as_heap_ref().unwrap();
        assert!(matches!(vm.heap.get(idx), HeapObject::Str(s) if s == "42"));
    }

    // ── IterInit / IterNext ─────────────────────────────────────────────────

    #[test]
    fn iter_init_and_next() {
        // Build list [10, 20, 30], iterate and verify IterNext yields each element.
        let chunk = make_chunk(
            "test",
            vec![
                // Build list
                (Op::Const(0), 1),          // 0: push 10
                (Op::Const(1), 1),          // 1: push 20
                (Op::MakeList(2), 1),       // 2: make list [10, 20]
                (Op::IterInit, 1),          // 3: create iterator (slot 0)
                // First iteration
                (Op::LoadLocal(0), 1),      // 4: load iterator
                (Op::IterNext(3), 1),       // 5: advance → push 10 (slot 1), or jump to 9
                // item (10.0) is now at slot 1
                (Op::Pop, 1),              // 6: pop item (end inner scope)
                (Op::Loop(-4), 1),          // 7: back to instruction 4
                // After exhaustion (IterNext jumped here)
                (Op::Pop, 1),              // 8: pop iterator (end outer scope)
                (Op::Const(2), 1),          // 9: push result (sentinel 99.0)
                (Op::Return, 1),            // 10
            ],
            1,
        );
        let prog = make_program(
            vec![chunk],
            vec![
                StackValue::Float(10.0),
                StackValue::Float(20.0),
                StackValue::Float(99.0),
            ],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 99.0));
    }

    // ── MatchEnum ───────────────────────────────────────────────────────────

    #[test]
    fn match_enum_variant() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::MakeEnum(0, 1, 1), 1),    // Shape.Circle(5.0)
                (Op::MatchEnum(1), 1),          // match "Circle"
                (Op::Return, 1),
            ],
            0,
        );
        let prog = Arc::new(CompiledProgram {
            chunks: vec![chunk],
            constants: vec![StackValue::Float(5.0)],
            strings: vec!["Shape".to_string(), "Circle".to_string()],
            natives: vec![],
            state_init_chunk: None,
            on_init_chunk: None,
            on_update_chunk: None,
            on_exit_chunk: None,
            state_fields: vec![],
            struct_defs: vec![],
            enum_defs: vec![CompiledEnumDef {
                name: "Shape".to_string(),
                variants: vec![CompiledEnumVariant {
                    name: "Circle".to_string(),
                    field_names: vec!["radius".to_string()],
                }],
            }],
            global_count: 16,
        });
        let mut vm = Vm::new(prog, native_table());
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Bool(true)));
    }

    // ── TryCall ─────────────────────────────────────────────────────────────

    #[test]
    fn try_call_success() {
        // Chunk 0: TryCall(1), return
        // Chunk 1: push 42, return
        let main_chunk = make_chunk(
            "main",
            vec![
                (Op::TryCall(1), 1),
                (Op::Return, 1),
            ],
            0,
        );
        let try_chunk = make_chunk(
            "try_body",
            vec![
                (Op::Const(0), 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![main_chunk, try_chunk],
            vec![StackValue::Float(42.0)],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        let idx = result.as_heap_ref().unwrap();
        assert!(matches!(vm.heap.get(idx), HeapObject::ResOk(StackValue::Float(f)) if *f == 42.0));
    }

    #[test]
    fn try_call_error() {
        // Chunk 0: TryCall(1), return
        // Chunk 1: push 1, push 0, div (division by zero)
        let main_chunk = make_chunk(
            "main",
            vec![
                (Op::TryCall(1), 1),
                (Op::Return, 1),
            ],
            0,
        );
        let try_chunk = make_chunk(
            "try_body",
            vec![
                (Op::Const(0), 1),
                (Op::Const(1), 1),
                (Op::Div, 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![main_chunk, try_chunk],
            vec![StackValue::Float(1.0), StackValue::Float(0.0)],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        let idx = result.as_heap_ref().unwrap();
        assert!(matches!(vm.heap.get(idx), HeapObject::ResErr(msg) if msg.contains("division by zero")));
    }

    // ── CheckCancel ─────────────────────────────────────────────────────────

    #[test]
    fn check_cancel_triggers() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::CheckCancel, 1),
                (Op::Const(0), 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(vec![chunk], vec![StackValue::Float(1.0)], vec![]);
        let mut vm = make_vm(prog);
        let cancel = Arc::new(AtomicBool::new(true));
        vm.set_cancel(cancel);
        let err = vm.run_chunk(0).unwrap_err();
        assert_eq!(err.code, ErrorCode::R010);
    }

    // ── Concat ──────────────────────────────────────────────────────────────

    #[test]
    fn concat_strings() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::ConstStr(0), 1),
                (Op::ConstStr(1), 1),
                (Op::Concat(2), 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![],
            vec!["hello ".to_string(), "world".to_string()],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        let idx = result.as_heap_ref().unwrap();
        assert!(matches!(vm.heap.get(idx), HeapObject::Str(s) if s == "hello world"));
    }

    // ── CoalesceJump / OptChainJump / IsNone ────────────────────────────────

    #[test]
    fn coalesce_jump_non_none() {
        // Push 42, CoalesceJump(1), push 99, return
        // 42 is not None → jump over push 99 → return 42
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),              // push 42
                (Op::CoalesceJump(1), 1),       // not none, jump to return
                (Op::Const(1), 1),              // push 99 (skipped)
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![StackValue::Float(42.0), StackValue::Float(99.0)],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 42.0));
    }

    #[test]
    fn coalesce_jump_none() {
        // Push None, CoalesceJump(1), push 99, return
        // None → pop and fall through → push 99 → return 99
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),              // push None
                (Op::CoalesceJump(1), 1),       // is none, fall through
                (Op::Const(1), 1),              // push 99
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![StackValue::None, StackValue::Float(99.0)],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 99.0));
    }

    #[test]
    fn is_none_true() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::IsNone, 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(vec![chunk], vec![StackValue::None], vec![]);
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        // IsNone doesn't consume TOS, so TOS is Bool(true), with None still below
        assert!(matches!(result, StackValue::Bool(true)));
    }

    #[test]
    fn is_none_false() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::IsNone, 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(vec![chunk], vec![StackValue::Float(5.0)], vec![]);
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Bool(false)));
    }

    // ── MakeNativeFnRef / MakeRenderMode ────────────────────────────────────

    #[test]
    fn make_native_fn_ref() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::MakeNativeFnRef(5), 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(vec![chunk], vec![], vec![]);
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        let idx = result.as_heap_ref().unwrap();
        assert!(matches!(vm.heap.get(idx), HeapObject::NativeFnRef(5)));
    }

    #[test]
    fn make_render_mode() {
        for (code, expected) in [(0u8, "Sdf"), (1, "Fill"), (2, "Outline")] {
            let chunk = make_chunk(
                "test",
                vec![
                    (Op::MakeRenderMode(code), 1),
                    (Op::Return, 1),
                ],
                0,
            );
            let prog = make_program(vec![chunk], vec![], vec![]);
            let mut vm = make_vm(prog);
            let result = vm.run_chunk(0).unwrap();
            let idx = result.as_heap_ref().unwrap();
            match vm.heap.get(idx) {
                HeapObject::RenderMode(rm) => {
                    let name = match rm {
                        RenderMode::Sdf => "Sdf",
                        RenderMode::Fill => "Fill",
                        RenderMode::Outline => "Outline",
                        RenderMode::Stroke(_) => "Stroke",
                    };
                    assert_eq!(name, expected, "code {} should produce {}", code, expected);
                }
                _ => panic!("expected RenderMode"),
            }
        }
    }

    // ── MAX_CALL_DEPTH exceeded ─────────────────────────────────────────────

    #[test]
    fn max_call_depth_exceeded() {
        // Chunk 0: call chunk 0 (infinite recursion)
        let chunk = make_chunk(
            "recurse",
            vec![
                (Op::Call(0, 0), 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(vec![chunk], vec![], vec![]);
        let mut vm = make_vm(prog);
        let err = vm.run_chunk(0).unwrap_err();
        assert_eq!(err.code, ErrorCode::R011);
        assert!(err.message.contains("call depth"));
    }

    // ── Stack trace on error ────────────────────────────────────────────────

    #[test]
    fn stack_trace_on_error() {
        // Chunk 0 "main": call chunk 1
        // Chunk 1 "inner": divide by zero
        let main_chunk = make_chunk(
            "main",
            vec![
                (Op::Call(1, 0), 1),
                (Op::Return, 1),
            ],
            0,
        );
        let inner_chunk = make_chunk(
            "inner",
            vec![
                (Op::Const(0), 2),
                (Op::Const(1), 2),
                (Op::Div, 2),
                (Op::Return, 2),
            ],
            0,
        );
        let prog = make_program(
            vec![main_chunk, inner_chunk],
            vec![StackValue::Float(1.0), StackValue::Float(0.0)],
            vec![],
        );
        let mut vm = make_vm(prog);
        let err = vm.run_chunk(0).unwrap_err();
        assert_eq!(err.code, ErrorCode::R007);
        // Stack trace should contain frame names
        let frame_names: Vec<&str> = err.stack.iter().map(|f| f.function.as_str()).collect();
        assert!(frame_names.contains(&"inner"), "stack should contain 'inner', got {:?}", frame_names);
        assert!(frame_names.contains(&"main"), "stack should contain 'main', got {:?}", frame_names);
    }

    // ── Eq / Neq ────────────────────────────────────────────────────────────

    #[test]
    fn eq_and_neq() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::Const(0), 1),
                (Op::Eq, 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(vec![chunk], vec![StackValue::Float(5.0)], vec![]);
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Bool(true)));
    }

    // ── Not ─────────────────────────────────────────────────────────────────

    #[test]
    fn not_true() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::Not, 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(vec![chunk], vec![StackValue::Bool(true)], vec![]);
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Bool(false)));
    }

    // ── Dup / Pop ───────────────────────────────────────────────────────────

    #[test]
    fn dup_and_pop() {
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),
                (Op::Dup, 1),
                (Op::Pop, 1),
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(vec![chunk], vec![StackValue::Float(7.0)], vec![]);
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::Float(f) if f == 7.0));
    }

    // ── OptChainJump ────────────────────────────────────────────────────────

    #[test]
    fn opt_chain_jump_none() {
        // Push None, OptChainJump(1), push 99, return
        // None → jump (keep None) → return None
        // Wait, jump skips push 99, goes to return. But there's no pop, so
        // stack has [None] and return pops that.
        let chunk = make_chunk(
            "test",
            vec![
                (Op::Const(0), 1),              // push None
                (Op::OptChainJump(1), 1),       // is none, jump to return
                (Op::Const(1), 1),              // push 99 (skipped)
                (Op::Return, 1),
            ],
            0,
        );
        let prog = make_program(
            vec![chunk],
            vec![StackValue::None, StackValue::Float(99.0)],
            vec![],
        );
        let mut vm = make_vm(prog);
        let result = vm.run_chunk(0).unwrap();
        assert!(matches!(result, StackValue::None));
    }
}
