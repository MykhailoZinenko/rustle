/// The complete instruction set for the Rustle bytecode VM.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    // ── Stack ────────────────────────────────────────────────────────────────
    /// Push constant pool[idx] onto the stack.
    Const(u16),
    /// Push string pool[idx] as a heap-allocated string.
    ConstStr(u16),
    /// Pop the top-of-stack value, discard it.
    Pop,
    /// Duplicate the top-of-stack value.
    Dup,

    // ── Variables ────────────────────────────────────────────────────────────
    /// Push the value of local variable slot[idx].
    LoadLocal(u16),
    /// Pop TOS and write it into local variable slot[idx].
    StoreLocal(u16),
    /// Push an upvalue captured by the enclosing closure.
    LoadUpvalue(u16),
    /// Pop TOS and write it into an upvalue captured by the enclosing closure.
    StoreUpvalue(u16),
    /// Push the value of a global variable at the given index.
    LoadGlobal(u16),
    /// Pop TOS and write it into a global variable at the given index.
    StoreGlobal(u16),

    // ── Arithmetic (float fast-path, type-dispatch fallback) ─────────────────
    /// Pop two values, push their sum.
    Add,
    /// Pop two values, push lhs − rhs.
    Sub,
    /// Pop two values, push their product.
    Mul,
    /// Pop two values, push lhs / rhs.
    Div,
    /// Pop two values, push lhs % rhs.
    Mod,
    /// Negate TOS (push −TOS).
    Neg,

    // ── Comparison ───────────────────────────────────────────────────────────
    /// Pop two values, push bool(lhs > rhs).
    Gt,
    /// Pop two values, push bool(lhs < rhs).
    Lt,
    /// Pop two values, push bool(lhs >= rhs).
    Gte,
    /// Pop two values, push bool(lhs <= rhs).
    Lte,
    /// Pop two values, push bool(lhs == rhs).
    Eq,
    /// Pop two values, push bool(lhs != rhs).
    Neq,

    // ── Logical ──────────────────────────────────────────────────────────────
    /// Pop TOS, push bool(!is_truthy(TOS)).
    Not,

    // ── Control flow ─────────────────────────────────────────────────────────
    /// Unconditional jump. Offset is relative to the instruction *after* this one.
    Jump(i32),
    /// Pop TOS; jump if it is falsy. Offset relative to next instruction.
    JumpIfFalse(i32),
    /// Pop TOS; jump if it is truthy. Offset relative to next instruction.
    JumpIfTrue(i32),
    /// Backward jump for loop bodies. Offset is negative, relative to next instruction.
    Loop(i32),

    // ── Functions ────────────────────────────────────────────────────────────
    /// Call the function at chunk_index with argc arguments on the stack.
    Call(u16, u8),
    /// Call native function native_table[index] with argc arguments.
    CallNative(u16, u8),
    /// Call the closure that sits below argc arguments on the stack.
    CallClosure(u8),
    /// Return from the current function, leaving TOS as the return value.
    Return,

    // ── Methods ──────────────────────────────────────────────────────────────
    /// Call a method: receiver sits below argc args. method_name is a string-pool index.
    CallMethod(u16, u8),

    // ── Fields + indexing ────────────────────────────────────────────────────
    /// Push the named field of TOS. field_name is a string-pool index.
    GetField(u16),
    /// Pop a value and set the named field on the object below it (ref types only).
    SetField(u16),
    /// Pop new_val and a value-type object, rebuild the object with the field replaced, push result.
    SetFieldRebuild(u16),
    /// Pop index, pop object, push object[index].
    GetIndex,
    /// Pop value, pop index, pop object; set object[index] = value.
    SetIndex,

    // ── Construction ─────────────────────────────────────────────────────────
    /// Pop two floats, push a Vec2.
    MakeVec2,
    /// Pop three floats, push a Vec3.
    MakeVec3,
    /// Pop four floats, push a Vec4.
    MakeVec4,
    /// Pop 3 or 4 floats (component_count), push a Color.
    MakeColor(u8),
    /// Pop element_count values, push a List.
    MakeList(u16),
    /// Create a closure from chunk_index, capturing upvalue_count upvalues from the stack.
    MakeClosure(u16, u8),
    /// Create a struct instance: struct_def_idx identifies the type, field_count values are on the stack.
    MakeStruct(u16, u8),
    /// Create an enum variant. Indices into the string pool for enum and variant names.
    MakeEnum(u16, u16, u8),

    // ── Output ───────────────────────────────────────────────────────────────
    /// Pop a shape value and emit it as a DrawCommand::DrawShape.
    Emit,
    /// Pop value_count values and emit a console message at the given level.
    /// level: 0 = log, 1 = warn, 2 = error.
    Print(u8, u8),

    // ── Optionals ────────────────────────────────────────────────────────────
    /// Null-coalescing short-circuit: if TOS is not None, jump (keeping TOS); if None, pop and fall through.
    CoalesceJump(i32),
    /// Optional chaining: if TOS is None, jump (keeping None); if not None, fall through.
    OptChainJump(i32),
    /// Push bool(TOS is None) without consuming TOS.
    IsNone,

    // ── Casts ────────────────────────────────────────────────────────────────
    /// Pop TOS, push bool(is_truthy(TOS)).
    Truthy,
    /// Pop TOS, push float conversion.
    CastFloat,
    /// Pop TOS, push string conversion (heap-allocated Str).
    CastString,

    // ── Iteration ────────────────────────────────────────────────────────────
    /// Pop a list, push a heap-allocated Iterator.
    IterInit,
    /// Advance the iterator on TOS; if exhausted, jump by offset, else push next element.
    IterNext(i32),

    // ── Pattern matching ─────────────────────────────────────────────────────
    /// Check whether TOS enum variant name matches string pool[idx]; push bool, don't consume TOS.
    MatchEnum(u16),
    /// Extract a field from the TOS enum variant by string-pool index.
    GetEnumField(u16),

    // ── String ───────────────────────────────────────────────────────────────
    /// Pop n values, concatenate their string representations, push result.
    Concat(u8),

    // ── Transforms ───────────────────────────────────────────────────────────
    /// Pop n transforms and a shape, apply transforms, push resulting shape.
    ApplyTransform(u8),

    // ── Try / Result ─────────────────────────────────────────────────────────
    /// Run chunk_idx in a protected frame; push ResOk(value) or ResErr(msg).
    TryCall(u16),

    // ── Cancellation ─────────────────────────────────────────────────────────
    /// Check the cancellation flag; abort execution if set.
    CheckCancel,

    // ── Value construction for imports ───────────────────────────────────────
    /// Push a HeapRef to a NativeFnRef(idx). Used when importing native functions as values.
    MakeNativeFnRef(u16),
    /// Push a HeapRef to a RenderMode. 0=Sdf, 1=Fill, 2=Outline.
    MakeRenderMode(u8),

    /// No-operation.
    Nop,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_is_copy() {
        let op = Op::Add;
        let _copy = op;
        let _still_original = op; // confirms Copy
    }

    #[test]
    fn jump_variants_exist() {
        let _j  = Op::Jump(10);
        let _jf = Op::JumpIfFalse(-5);
        let _jt = Op::JumpIfTrue(3);
        let _l  = Op::Loop(-8);
        let _cj = Op::CoalesceJump(2);
        let _oj = Op::OptChainJump(4);
        let _it = Op::IterNext(6);
    }

    #[test]
    fn construction_ops() {
        let _mk_list    = Op::MakeList(3);
        let _mk_closure = Op::MakeClosure(0, 2);
        let _mk_struct  = Op::MakeStruct(1, 4);
        let _mk_enum    = Op::MakeEnum(0, 1, 2);
        let _mk_color   = Op::MakeColor(4);
    }
}
