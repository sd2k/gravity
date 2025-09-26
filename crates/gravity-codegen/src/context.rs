use gravity_go::{Go, GoType, Operand, Tokens};
use std::collections::HashMap;

/// Context for code generation that tracks state during instruction processing.
///
/// This struct maintains the operand stack, variable mappings, temporary counter,
/// and output tokens during the code generation process.
pub struct GenerationContext {
    /// Output tokens for declarations and top-level code.
    pub out: Tokens<Go>,
    /// Body tokens for function implementations.
    pub body: Tokens<Go>,
    /// Stack of operands being processed.
    pub operands: Vec<Operand>,
    /// Mapping from variable names to their Go types.
    pub vars: HashMap<String, GoType>,
    /// Counter for generating unique temporary variable names.
    tmp_counter: usize,
}

impl Default for GenerationContext {
    fn default() -> Self {
        Self::new()
    }
}

impl GenerationContext {
    /// Creates a new empty generation context.
    ///
    /// Initializes all fields to their default empty states.
    pub fn new() -> Self {
        Self {
            out: Tokens::new(),
            body: Tokens::new(),
            operands: Vec::new(),
            vars: HashMap::new(),
            tmp_counter: 0,
        }
    }

    /// Generates a unique temporary variable identifier.
    ///
    /// Returns the current counter value and increments it for the next call.
    /// This ensures each temporary variable has a unique numeric identifier.
    ///
    /// # Returns
    /// A unique numeric identifier for a temporary variable.
    pub fn tmp(&mut self) -> usize {
        let current = self.tmp_counter;
        self.tmp_counter += 1;
        current
    }

    /// Pushes an operand onto the operand stack.
    ///
    /// # Arguments
    /// * `operand` - The operand to push onto the stack.
    pub fn push_operand(&mut self, operand: Operand) {
        self.operands.push(operand);
    }

    /// Pops multiple operands from the stack.
    ///
    /// Removes the specified number of operands from the top of the stack
    /// and returns them in the order they were popped (bottom to top of the removed segment).
    ///
    /// # Arguments
    /// * `count` - The number of operands to pop.
    ///
    /// # Returns
    /// A vector containing the popped operands.
    ///
    /// # Panics
    /// Panics if there are fewer operands on the stack than requested.
    pub fn pop_operands(&mut self, count: usize) -> Vec<Operand> {
        let start = self.operands.len() - count;
        self.operands.drain(start..).collect()
    }

    /// Peeks at multiple operands from the top of the stack without removing them.
    ///
    /// # Arguments
    /// * `count` - The number of operands to peek at.
    ///
    /// # Returns
    /// A slice containing the top `count` operands.
    ///
    /// # Panics
    /// Panics if there are fewer operands on the stack than requested.
    pub fn peek_operands(&self, count: usize) -> &[Operand] {
        let start = self.operands.len() - count;
        &self.operands[start..]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tmp_counter() {
        let mut ctx = GenerationContext::new();
        assert_eq!(ctx.tmp(), 0);
        assert_eq!(ctx.tmp(), 1);
        assert_eq!(ctx.tmp(), 2);
        assert_eq!(ctx.tmp(), 3);
    }

    #[test]
    fn test_operand_stack() {
        let mut ctx = GenerationContext::new();

        // Push some operands
        ctx.push_operand(Operand::Literal("0".to_string()));
        ctx.push_operand(Operand::SingleValue("var1".to_string()));
        ctx.push_operand(Operand::MultiValue(("a".to_string(), "b".to_string())));

        // Check stack size
        assert_eq!(ctx.operands.len(), 3);

        // Peek at top 2
        let peeked = ctx.peek_operands(2);
        assert_eq!(peeked.len(), 2);
        assert_eq!(peeked[0], Operand::SingleValue("var1".to_string()));
        assert_eq!(
            peeked[1],
            Operand::MultiValue(("a".to_string(), "b".to_string()))
        );

        // Pop 2 operands
        let popped = ctx.pop_operands(2);
        assert_eq!(popped.len(), 2);
        assert_eq!(popped[0], Operand::SingleValue("var1".to_string()));
        assert_eq!(
            popped[1],
            Operand::MultiValue(("a".to_string(), "b".to_string()))
        );

        // Check remaining stack
        assert_eq!(ctx.operands.len(), 1);
        assert_eq!(ctx.operands[0], Operand::Literal("0".to_string()));
    }

    #[test]
    fn test_variable_storage() {
        let mut ctx = GenerationContext::new();

        // Store some variables with types
        ctx.vars.insert("x".to_string(), GoType::Uint32);
        ctx.vars.insert("name".to_string(), GoType::String);
        ctx.vars
            .insert("data".to_string(), GoType::Slice(Box::new(GoType::Uint8)));

        assert_eq!(ctx.vars.get("x"), Some(&GoType::Uint32));
        assert_eq!(ctx.vars.get("name"), Some(&GoType::String));
        assert_eq!(
            ctx.vars.get("data"),
            Some(&GoType::Slice(Box::new(GoType::Uint8)))
        );
        assert_eq!(ctx.vars.get("unknown"), None);
    }

    #[test]
    #[should_panic]
    fn test_peek_too_many() {
        let ctx = GenerationContext::new();
        // This should panic because we're trying to peek more than available
        let _ = ctx.peek_operands(1);
    }

    #[test]
    #[should_panic]
    fn test_pop_too_many() {
        let mut ctx = GenerationContext::new();
        ctx.push_operand(Operand::Literal("0".to_string()));
        // This should panic because we're trying to pop more than available
        let _ = ctx.pop_operands(2);
    }
}
