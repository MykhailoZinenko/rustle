use super::opcode::Op;

/// A compiled function body: a flat sequence of opcodes with source-line metadata.
#[derive(Debug, Clone)]
pub struct Chunk {
    /// The encoded instructions.
    pub code: Vec<Op>,
    /// Source line number for each opcode (parallel to `code`).
    pub lines: Vec<u32>,
    /// Human-readable name: function name, `"<top_level>"`, `"<lambda>"`, etc.
    pub name: String,
    /// Total number of local variable slots (parameters + locals).
    pub local_count: u16,
    /// Number of parameters; they occupy the first `param_count` local slots.
    pub param_count: u8,
}

impl Chunk {
    /// Create an empty chunk with the given name.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            code: Vec::new(),
            lines: Vec::new(),
            name: name.into(),
            local_count: 0,
            param_count: 0,
        }
    }

    /// Append one opcode and return its index.
    pub fn emit(&mut self, op: Op, line: u32) -> usize {
        let idx = self.code.len();
        self.code.push(op);
        self.lines.push(line);
        idx
    }

    /// Overwrite the jump offset inside an existing jump instruction.
    ///
    /// # Panics
    /// Panics if the opcode at `idx` is not a jump variant.
    pub fn patch_jump(&mut self, idx: usize, offset: i32) {
        self.code[idx] = match self.code[idx] {
            Op::Jump(_)         => Op::Jump(offset),
            Op::JumpIfFalse(_)  => Op::JumpIfFalse(offset),
            Op::JumpIfTrue(_)   => Op::JumpIfTrue(offset),
            Op::Loop(_)         => Op::Loop(offset),
            Op::CoalesceJump(_) => Op::CoalesceJump(offset),
            Op::OptChainJump(_) => Op::OptChainJump(offset),
            Op::IterNext(_)     => Op::IterNext(offset),
            other => panic!(
                "patch_jump called on non-jump opcode {:?} at index {}",
                other, idx
            ),
        };
    }

    /// Number of emitted opcodes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.code.len()
    }

    /// True if no opcodes have been emitted.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.code.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_chunk_is_empty() {
        let c = Chunk::new("test");
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
        assert_eq!(c.name, "test");
    }

    #[test]
    fn emit_and_len() {
        let mut c = Chunk::new("f");
        let i0 = c.emit(Op::Const(0), 1);
        let i1 = c.emit(Op::Add, 2);
        let i2 = c.emit(Op::Return, 2);
        assert_eq!(i0, 0);
        assert_eq!(i1, 1);
        assert_eq!(i2, 2);
        assert_eq!(c.len(), 3);
        assert!(!c.is_empty());
        assert_eq!(c.lines, vec![1, 2, 2]);
    }

    #[test]
    fn patch_jump_variants() {
        let mut c = Chunk::new("f");
        let j  = c.emit(Op::Jump(0), 1);
        let jf = c.emit(Op::JumpIfFalse(0), 1);
        let jt = c.emit(Op::JumpIfTrue(0), 1);
        let lo = c.emit(Op::Loop(0), 1);
        let cj = c.emit(Op::CoalesceJump(0), 1);
        let oj = c.emit(Op::OptChainJump(0), 1);
        let it = c.emit(Op::IterNext(0), 1);

        c.patch_jump(j,  42);
        c.patch_jump(jf, -7);
        c.patch_jump(jt, 3);
        c.patch_jump(lo, -15);
        c.patch_jump(cj, 10);
        c.patch_jump(oj, 5);
        c.patch_jump(it, 99);

        assert_eq!(c.code[j],  Op::Jump(42));
        assert_eq!(c.code[jf], Op::JumpIfFalse(-7));
        assert_eq!(c.code[jt], Op::JumpIfTrue(3));
        assert_eq!(c.code[lo], Op::Loop(-15));
        assert_eq!(c.code[cj], Op::CoalesceJump(10));
        assert_eq!(c.code[oj], Op::OptChainJump(5));
        assert_eq!(c.code[it], Op::IterNext(99));
    }

    #[test]
    #[should_panic(expected = "patch_jump called on non-jump opcode")]
    fn patch_jump_panics_on_non_jump() {
        let mut c = Chunk::new("f");
        let idx = c.emit(Op::Add, 1);
        c.patch_jump(idx, 5);
    }

    #[test]
    #[should_panic(expected = "patch_jump called on non-jump opcode")]
    fn patch_jump_panics_on_return() {
        let mut c = Chunk::new("f");
        let idx = c.emit(Op::Return, 1);
        c.patch_jump(idx, 0);
    }
}
