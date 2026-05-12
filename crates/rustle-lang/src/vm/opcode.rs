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
    /// Duplicate the value at `depth` positions below TOS (0 = Dup).
    DupAt(u8),
    /// Swap the top two stack values.
    Swap,
    /// Rotate: move TOS to `depth` positions down, shifting everything above it up.
    /// `Rot(2)` on `[a, b, c]` → `[a, c, b]` (same as Swap).
    /// `Rot(3)` on `[a, b, c, d]` → `[a, d, b, c]` (TOS goes 3 deep).
    Rot(u8),

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
    /// Pop TOS, push `bool(!is_truthy(TOS))`.
    Not,

    // ── Control flow ─────────────────────────────────────────────────────────
    /// Unconditional jump. Offset is relative to the instruction *after* this one.
    Jump(i16),
    /// Pop TOS; jump if it is falsy. Offset relative to next instruction.
    JumpIfFalse(i16),
    /// Pop TOS; jump if it is truthy. Offset relative to next instruction.
    JumpIfTrue(i16),
    /// Backward jump for loop bodies. Offset is negative, relative to next instruction.
    Loop(i16),

    // ── Functions ────────────────────────────────────────────────────────────
    /// Call the function at `chunk_index` with argc arguments on the stack.
    Call(u16, u8),
    /// Call native function `native_table`[index] with argc arguments.
    CallNative(u16, u8),
    /// Call the closure that sits below argc arguments on the stack.
    CallClosure(u8),
    /// Return from the current function, leaving TOS as the return value.
    Return,

    // ── Methods ──────────────────────────────────────────────────────────────
    /// Call a method: receiver sits below argc args. `method_name` is a string-pool index.
    CallMethod(u16, u8),

    // ── Fields + indexing ────────────────────────────────────────────────────
    /// Push the named field of TOS. `field_name` is a string-pool index.
    GetField(u16),
    /// Pop a value and set the named field on the object below it (ref types only).
    SetField(u16),
    /// Pop `new_val` and a value-type object, rebuild the object with the field replaced, push result.
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
    /// Pop 3 or 4 floats (`component_count`), push a Color.
    MakeColor(u8),
    /// Pop `element_count` values, push a List.
    MakeList(u16),
    /// Create a closure from `chunk_index`, capturing `upvalue_count` upvalues from the stack.
    MakeClosure(u16, u8),
    /// Create a struct instance: `struct_def_idx` identifies the type, `field_count` values are on the stack.
    MakeStruct(u16, u8),
    /// Create an enum variant. `enum_def_idx` indexes into `CompiledProgram::enum_defs`,
    /// `variant_idx` indexes into that enum's `variants` list.
    /// Field count is derived from the `CompiledEnumVariant`.
    MakeEnum(u16, u8),

    // ── Output ───────────────────────────────────────────────────────────────
    /// Pop a shape value and emit it as a `DrawCommand::DrawShape`.
    Emit,
    /// Pop `value_count` values and emit a console message at the given level.
    /// level: 0 = log, 1 = warn, 2 = error.
    Print(u8, u8),

    // ── Optionals ────────────────────────────────────────────────────────────
    /// Null-coalescing short-circuit: if TOS is not None, jump (keeping TOS); if None, pop and fall through.
    CoalesceJump(i16),
    /// Optional chaining: if TOS is None, jump (keeping None); if not None, fall through.
    OptChainJump(i16),
    /// Push bool(TOS is None) without consuming TOS.
    IsNone,

    // ── Casts ────────────────────────────────────────────────────────────────
    /// Pop TOS, push `bool(is_truthy(TOS))`.
    Truthy,
    /// Pop TOS, push float conversion.
    CastFloat,
    /// Pop TOS, push string conversion (heap-allocated Str).
    CastString,

    // ── Iteration ────────────────────────────────────────────────────────────
    /// Pop a list, push a heap-allocated Iterator.
    IterInit,
    /// Advance the iterator on TOS; if exhausted, jump by offset, else push next element.
    IterNext(i16),

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
    /// Run `chunk_idx` in a protected frame; push ResOk(value) or ResErr(msg).
    TryCall(u16),
    /// Pop a closure from TOS, call it in a protected frame; push ResOk(value) or ResErr(msg).
    TryCallClosure,

    // ── Cancellation ─────────────────────────────────────────────────────────
    /// Check the cancellation flag; abort execution if set.
    CheckCancel,

    // ── Value construction for imports ───────────────────────────────────────
    /// Push a `HeapRef` to a NativeFnRef(idx). Used when importing native functions as values.
    MakeNativeFnRef(u16),
    /// Push a `HeapRef` to a `RenderMode`. 0=Sdf, 1=Fill, 2=Outline.
    MakeRenderMode(u8),

    /// No-operation.
    Nop,
}

const _: () = assert!(std::mem::size_of::<Op>() == 4);

impl std::fmt::Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Op::Const(idx)          => write!(f, "CONST {idx}"),
            Op::ConstStr(idx)       => write!(f, "CONST_STR {idx}"),
            Op::Pop                 => write!(f, "POP"),
            Op::Dup                 => write!(f, "DUP"),
            Op::DupAt(d)            => write!(f, "DUP_AT {d}"),
            Op::Swap                => write!(f, "SWAP"),
            Op::Rot(d)              => write!(f, "ROT {d}"),
            Op::LoadLocal(s)        => write!(f, "LOAD_LOCAL {s}"),
            Op::StoreLocal(s)       => write!(f, "STORE_LOCAL {s}"),
            Op::LoadUpvalue(s)      => write!(f, "LOAD_UPVAL {s}"),
            Op::StoreUpvalue(s)     => write!(f, "STORE_UPVAL {s}"),
            Op::LoadGlobal(s)       => write!(f, "LOAD_GLOBAL {s}"),
            Op::StoreGlobal(s)      => write!(f, "STORE_GLOBAL {s}"),
            Op::Add                 => write!(f, "ADD"),
            Op::Sub                 => write!(f, "SUB"),
            Op::Mul                 => write!(f, "MUL"),
            Op::Div                 => write!(f, "DIV"),
            Op::Mod                 => write!(f, "MOD"),
            Op::Neg                 => write!(f, "NEG"),
            Op::Gt                  => write!(f, "GT"),
            Op::Lt                  => write!(f, "LT"),
            Op::Gte                 => write!(f, "GTE"),
            Op::Lte                 => write!(f, "LTE"),
            Op::Eq                  => write!(f, "EQ"),
            Op::Neq                 => write!(f, "NEQ"),
            Op::Not                 => write!(f, "NOT"),
            Op::Jump(off)           => write!(f, "JUMP {off:+}"),
            Op::JumpIfFalse(off)    => write!(f, "JUMP_IF_FALSE {off:+}"),
            Op::JumpIfTrue(off)     => write!(f, "JUMP_IF_TRUE {off:+}"),
            Op::Loop(off)           => write!(f, "LOOP {off:+}"),
            Op::Call(c, a)          => write!(f, "CALL chunk={c} argc={a}"),
            Op::CallNative(n, a)    => write!(f, "CALL_NATIVE {n} argc={a}"),
            Op::CallClosure(a)      => write!(f, "CALL_CLOSURE argc={a}"),
            Op::Return              => write!(f, "RETURN"),
            Op::CallMethod(s, a)    => write!(f, "CALL_METHOD str={s} argc={a}"),
            Op::GetField(s)         => write!(f, "GET_FIELD str={s}"),
            Op::SetField(s)         => write!(f, "SET_FIELD str={s}"),
            Op::SetFieldRebuild(s)  => write!(f, "SET_FIELD_REBUILD str={s}"),
            Op::GetIndex            => write!(f, "GET_INDEX"),
            Op::SetIndex            => write!(f, "SET_INDEX"),
            Op::MakeVec2            => write!(f, "MAKE_VEC2"),
            Op::MakeVec3            => write!(f, "MAKE_VEC3"),
            Op::MakeVec4            => write!(f, "MAKE_VEC4"),
            Op::MakeColor(n)        => write!(f, "MAKE_COLOR {n}"),
            Op::MakeList(n)         => write!(f, "MAKE_LIST {n}"),
            Op::MakeClosure(c, u)   => write!(f, "MAKE_CLOSURE chunk={c} upvals={u}"),
            Op::MakeStruct(d, n)    => write!(f, "MAKE_STRUCT def={d} fields={n}"),
            Op::MakeEnum(d, v)      => write!(f, "MAKE_ENUM def={d} variant={v}"),
            Op::Emit                => write!(f, "EMIT"),
            Op::Print(lvl, n)       => write!(f, "PRINT level={lvl} count={n}"),
            Op::CoalesceJump(off)   => write!(f, "COALESCE_JUMP {off:+}"),
            Op::OptChainJump(off)   => write!(f, "OPT_CHAIN_JUMP {off:+}"),
            Op::IsNone              => write!(f, "IS_NONE"),
            Op::Truthy              => write!(f, "TRUTHY"),
            Op::CastFloat           => write!(f, "CAST_FLOAT"),
            Op::CastString          => write!(f, "CAST_STRING"),
            Op::IterInit            => write!(f, "ITER_INIT"),
            Op::IterNext(off)       => write!(f, "ITER_NEXT {off:+}"),
            Op::MatchEnum(s)        => write!(f, "MATCH_ENUM str={s}"),
            Op::GetEnumField(s)     => write!(f, "GET_ENUM_FIELD str={s}"),
            Op::Concat(n)           => write!(f, "CONCAT {n}"),
            Op::ApplyTransform(n)   => write!(f, "APPLY_TRANSFORM {n}"),
            Op::TryCall(c)          => write!(f, "TRY_CALL chunk={c}"),
            Op::TryCallClosure      => write!(f, "TRY_CALL_CLOSURE"),
            Op::CheckCancel         => write!(f, "CHECK_CANCEL"),
            Op::MakeNativeFnRef(i)  => write!(f, "MAKE_NATIVE_FN_REF {i}"),
            Op::MakeRenderMode(m)   => write!(f, "MAKE_RENDER_MODE {m}"),
            Op::Nop                 => write!(f, "NOP"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn op_size() {
        let size = std::mem::size_of::<Op>();
        eprintln!("size_of::<Op>() = {size} bytes");
        eprintln!("size_of::<Op>() * 1000 instructions = {} bytes", size * 1000);
    }

    #[test]
    fn op_is_copy() {
        let op = Op::Add;
        let _copy = op;
        let _still_original = op; // confirms Copy
    }

    #[test]
    fn display_parameterless_ops() {
        assert_eq!(format!("{}", Op::Add), "ADD");
        assert_eq!(format!("{}", Op::Return), "RETURN");
        assert_eq!(format!("{}", Op::Pop), "POP");
        assert_eq!(format!("{}", Op::Dup), "DUP");
        assert_eq!(format!("{}", Op::Swap), "SWAP");
        assert_eq!(format!("{}", Op::Nop), "NOP");
        assert_eq!(format!("{}", Op::GetIndex), "GET_INDEX");
        assert_eq!(format!("{}", Op::Emit), "EMIT");
    }

    #[test]
    fn display_ops_with_operands() {
        assert_eq!(format!("{}", Op::Const(42)), "CONST 42");
        assert_eq!(format!("{}", Op::LoadLocal(3)), "LOAD_LOCAL 3");
        assert_eq!(format!("{}", Op::Call(1, 2)), "CALL chunk=1 argc=2");
        assert_eq!(format!("{}", Op::MakeList(5)), "MAKE_LIST 5");
        assert_eq!(format!("{}", Op::MakeEnum(0, 1)), "MAKE_ENUM def=0 variant=1");
        assert_eq!(format!("{}", Op::Print(1, 2)), "PRINT level=1 count=2");
    }

    #[test]
    fn display_jump_ops_show_signed_offset() {
        assert_eq!(format!("{}", Op::Jump(10)), "JUMP +10");
        assert_eq!(format!("{}", Op::Jump(-5)), "JUMP -5");
        assert_eq!(format!("{}", Op::JumpIfFalse(0)), "JUMP_IF_FALSE +0");
        assert_eq!(format!("{}", Op::Loop(-8)), "LOOP -8");
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
        let _mk_enum    = Op::MakeEnum(0, 1);
        let _mk_color   = Op::MakeColor(4);
    }
}
