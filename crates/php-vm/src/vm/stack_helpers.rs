//! Stack operation helpers to reduce boilerplate
//!
//! Provides convenient methods for common stack manipulation patterns,
//! reducing code duplication and improving error handling consistency.
//!
//! ## Common Patterns
//!
//! Most VM operations follow these patterns:
//! 1. Pop operands from stack
//! 2. Perform operation
//! 3. Push result back to stack
//!
//! This module standardizes step 1 with clear error messages.
//!
//! ## Error Handling
//!
//! All helpers use the specific `StackUnderflow` error variant,
//! providing operation context for better debugging.
//!
//! ## Performance
//!
//! Methods are marked `#[inline]` to eliminate overhead.
//! No heap allocations except for Vec in `pop_n_operands`.

use crate::core::value::Handle;
use crate::vm::engine::{VmError, VM};

impl VM {
    /// Pop a single operand with clear error message
    /// Reference: Reduces boilerplate for stack operations
    #[inline(always)]
    pub(crate) fn pop_operand_required(&mut self) -> Result<Handle, VmError> {
        self.operand_stack
            .pop()
            .ok_or(VmError::StackUnderflow { operation: "pop" })
    }

    /// Pop two operands for binary operations (returns in (left, right) order)
    /// Reference: Common pattern in arithmetic and comparison operations
    #[inline]
    pub(crate) fn pop_binary_operands(&mut self) -> Result<(Handle, Handle), VmError> {
        let right = self.pop_operand_required()?;
        let left = self.pop_operand_required()?;
        Ok((left, right))
    }

    /// Pop N operands and return them in reverse order (as pushed)
    /// Reference: Used in function calls and array initialization
    #[inline]
    pub(crate) fn pop_n_operands(&mut self, count: usize) -> Result<Vec<Handle>, VmError> {
        let mut operands = Vec::with_capacity(count);
        for _ in 0..count {
            operands.push(self.pop_operand_required()?);
        }
        operands.reverse();
        Ok(operands)
    }

    /// Peek at top of stack without removing
    /// Reference: Used in JmpPeekOr operations
    #[inline]
    pub(crate) fn peek_operand(&self) -> Result<Handle, VmError> {
        self.operand_stack
            .peek()
            .ok_or(VmError::StackUnderflow { operation: "peek" })
    }

    /// Peek at stack with an offset from the top (0 = top)
    /// Reference: Used in nested array operations
    #[inline]
    pub(crate) fn peek_operand_at(&self, offset: usize) -> Result<Handle, VmError> {
        self.operand_stack
            .peek_at(offset)
            .ok_or(VmError::StackUnderflow {
                operation: "peek_at",
            })
    }

    /// Push result and chain
    /// Reference: Allows method chaining for cleaner code
    #[inline]
    pub(crate) fn push_result(&mut self, handle: Handle) -> Result<(), VmError> {
        self.operand_stack.push(handle);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::value::Val;
    use crate::runtime::context::EngineContext;
    use std::sync::Arc;

    #[test]
    fn test_pop_binary_operands_order() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Int(1));
        let right = vm.arena.alloc(Val::Int(2));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        let (l, r) = vm.pop_binary_operands().unwrap();
        assert_eq!(l, left);
        assert_eq!(r, right);
    }

    #[test]
    fn test_pop_n_operands() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let h1 = vm.arena.alloc(Val::Int(1));
        let h2 = vm.arena.alloc(Val::Int(2));
        let h3 = vm.arena.alloc(Val::Int(3));

        vm.operand_stack.push(h1);
        vm.operand_stack.push(h2);
        vm.operand_stack.push(h3);

        let ops = vm.pop_n_operands(3).unwrap();
        assert_eq!(ops, vec![h1, h2, h3]);
    }

    #[test]
    fn test_stack_underflow_errors() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // Test that stack underflow produces specific error variant
        let err = vm.pop_operand_required().unwrap_err();
        match err {
            VmError::StackUnderflow { operation } => {
                assert_eq!(operation, "pop");
            }
            _ => panic!("Expected StackUnderflow error"),
        }

        assert!(vm.pop_binary_operands().is_err());

        let peek_err = vm.peek_operand().unwrap_err();
        match peek_err {
            VmError::StackUnderflow { operation } => {
                assert_eq!(operation, "peek");
            }
            _ => panic!("Expected StackUnderflow error"),
        }
    }
}
