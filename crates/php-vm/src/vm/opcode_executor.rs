//! Opcode executor trait
//!
//! Provides a trait-based visitor pattern for opcode execution,
//! separating opcode definition from execution logic.
//!
//! ## Design Pattern
//!
//! This implements the Visitor pattern where:
//! - OpCode enum is the "visited" type
//! - VM is the visitor that executes operations
//! - The trait provides double-dispatch capability
//!
//! ## Benefits
//!
//! - **Separation of Concerns**: OpCode definition separate from execution
//! - **Extensibility**: Easy to add logging, profiling, or alternative executors
//! - **Testability**: Can mock executor for testing
//!
//! ## Performance
//!
//! The default `OpCode` executor is a thin wrapper over the VM's internal
//! dispatcher, so there is no duplicated opcode-to-handler mapping to maintain.
//!
//! ## References
//!
//! - Gang of Four: Visitor Pattern
//! - Rust Book: Trait Objects and Dynamic Dispatch

use crate::vm::engine::{VmError, VM};
use crate::vm::opcode::OpCode;

/// Trait for executing opcodes on a VM
///
/// Implementors can define custom execution behavior for opcodes,
/// enabling features like profiling, debugging, or alternative VMs.
pub trait OpcodeExecutor {
    /// Execute this opcode on the given VM
    ///
    /// # Errors
    ///
    /// Returns VmError if execution fails (stack underflow, type error, etc.)
    fn execute(&self, vm: &mut VM) -> Result<(), VmError>;
}

impl OpcodeExecutor for OpCode {
    fn execute(&self, vm: &mut VM) -> Result<(), VmError> {
        vm.execute_opcode_direct(*self, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::value::Val;
    use crate::runtime::context::EngineContext;
    use std::sync::Arc;

    #[test]
    fn test_opcode_executor_trait() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // Test arithmetic operation via trait
        let left = vm.arena.alloc(Val::Int(5));
        let right = vm.arena.alloc(Val::Int(3));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        // Execute Add via the trait
        let add_op = OpCode::Add;
        add_op.execute(&mut vm).unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);

        match result_val.value {
            Val::Int(n) => assert_eq!(n, 8),
            _ => panic!("Expected Int result"),
        }
    }

    #[test]
    fn test_stack_operations_via_trait() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let val = vm.arena.alloc(Val::Int(42));
        vm.operand_stack.push(val);

        // Dup via trait
        let dup_op = OpCode::Dup;
        dup_op.execute(&mut vm).unwrap();

        assert_eq!(vm.operand_stack.len(), 2);

        // Pop via trait
        let pop_op = OpCode::Pop;
        pop_op.execute(&mut vm).unwrap();

        assert_eq!(vm.operand_stack.len(), 1);
    }

    #[test]
    fn test_comparison_via_trait() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Int(10));
        let right = vm.arena.alloc(Val::Int(20));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        // IsLess via trait
        let lt_op = OpCode::IsLess;
        lt_op.execute(&mut vm).unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);

        match result_val.value {
            Val::Bool(b) => assert!(b), // 10 < 20
            _ => panic!("Expected Bool result"),
        }
    }
}
