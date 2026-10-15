//! Tiny label-based assembler for hand-written Boing VM templates (used by token v2 / pool v8).
//!
//! Jumps are `Push32 <dest>; Jump|JumpI` with destinations patched in [`Asm::finish`].
//! [`Asm::fault`] jumps to an out-of-range pc, which is a VM fault (`InvalidJump`): the whole
//! transaction fails and the block executor reverts all of its state changes.

use crate::bytecode::Opcode;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Label(usize);

pub(crate) struct Asm {
    c: Vec<u8>,
    labels: Vec<Option<usize>>,
    fixups: Vec<(usize, Label)>,
}

pub(crate) fn w_u64(n: u64) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[24..32].copy_from_slice(&n.to_be_bytes());
    w
}

pub(crate) fn w_sel(s: u8) -> [u8; 32] {
    let mut w = [0u8; 32];
    w[31] = s;
    w
}

/// `2^64` as a word (u64 ledger bound).
pub(crate) fn w_two_pow_64() -> [u8; 32] {
    let mut w = [0u8; 32];
    w[23] = 1;
    w
}

impl Asm {
    pub(crate) fn new() -> Self {
        Self {
            c: Vec::new(),
            labels: Vec::new(),
            fixups: Vec::new(),
        }
    }
    pub(crate) fn op(&mut self, o: Opcode) -> &mut Self {
        self.c.push(o as u8);
        self
    }
    pub(crate) fn push(&mut self, w: &[u8; 32]) -> &mut Self {
        self.c.push(Opcode::Push32 as u8);
        self.c.extend_from_slice(w);
        self
    }
    pub(crate) fn push_u(&mut self, n: u64) -> &mut Self {
        self.push(&w_u64(n))
    }
    pub(crate) fn label(&mut self) -> Label {
        self.labels.push(None);
        Label(self.labels.len() - 1)
    }
    pub(crate) fn mark(&mut self, l: Label) -> &mut Self {
        assert!(self.labels[l.0].is_none(), "label marked twice");
        self.labels[l.0] = Some(self.c.len());
        self
    }
    fn dest(&mut self, l: Label) {
        self.fixups.push((self.c.len(), l));
        self.push(&[0u8; 32]);
    }
    pub(crate) fn jump(&mut self, l: Label) -> &mut Self {
        self.dest(l);
        self.op(Opcode::Jump)
    }
    /// Pops the condition already on the stack; jumps when non-zero.
    pub(crate) fn jumpi(&mut self, l: Label) -> &mut Self {
        self.dest(l);
        self.op(Opcode::JumpI)
    }
    /// VM fault (out-of-range jump): the transaction fails and is reverted.
    pub(crate) fn fault(&mut self) -> &mut Self {
        self.push_u(u64::MAX);
        self.op(Opcode::Jump)
    }
    pub(crate) fn mload(&mut self, off: u64) -> &mut Self {
        self.push_u(off);
        self.op(Opcode::MLoad)
    }
    /// Stores the stack top at memory `off`.
    pub(crate) fn mstore(&mut self, off: u64) -> &mut Self {
        self.push_u(off);
        self.op(Opcode::MStore)
    }
    /// `mem[dst] = mem[src]`.
    pub(crate) fn mcopy(&mut self, src: u64, dst: u64) -> &mut Self {
        self.mload(src);
        self.mstore(dst)
    }
    pub(crate) fn sload_key(&mut self, k: &[u8; 32]) -> &mut Self {
        self.push(k);
        self.op(Opcode::SLoad)
    }
    /// Stores the stack top at storage key `k`.
    pub(crate) fn sstore_key(&mut self, k: &[u8; 32]) -> &mut Self {
        self.push(k);
        self.op(Opcode::SStore)
    }
    /// Return the 32-byte word at memory `off`.
    pub(crate) fn return_word(&mut self, off: u64) -> &mut Self {
        self.push_u(32);
        self.push_u(off);
        self.op(Opcode::Return)
    }
    pub(crate) fn finish(mut self) -> Vec<u8> {
        for (at, l) in std::mem::take(&mut self.fixups) {
            let dest = self.labels[l.0].expect("unmarked label") as u64;
            self.c[at + 1..at + 33].copy_from_slice(&w_u64(dest));
        }
        self.c
    }
}
