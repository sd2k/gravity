use anyhow::Result;
use gravity_go::{quote, Go, GoType, Operand, Tokens};
use std::collections::HashMap;

pub struct GenerationContext {
    pub out: Tokens<Go>,
    pub body: Tokens<Go>,
    pub operands: Vec<Operand>,
    pub vars: HashMap<String, GoType>,
    tmp_counter: usize,
}

impl GenerationContext {
    pub fn new() -> Self {
        Self {
            out: Tokens::new(),
            body: Tokens::new(),
            operands: Vec::new(),
            vars: HashMap::new(),
            tmp_counter: 0,
        }
    }

    pub fn tmp(&mut self) -> usize {
        let current = self.tmp_counter;
        self.tmp_counter += 1;
        current
    }

    pub fn push_operand(&mut self, operand: Operand) {
        self.operands.push(operand);
    }

    pub fn pop_operands(&mut self, count: usize) -> Vec<Operand> {
        let start = self.operands.len() - count;
        self.operands.drain(start..).collect()
    }

    pub fn peek_operands(&self, count: usize) -> &[Operand] {
        let start = self.operands.len() - count;
        &self.operands[start..]
    }
}
