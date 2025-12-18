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
//! - **Type Safety**: Compiler ensures all opcodes are handled
//! - **Testability**: Can mock executor for testing
//!
//! ## Performance
//!
//! The trait dispatch adds minimal overhead:
//! - Single indirect call via vtable (or inlined if monomorphized)
//! - No heap allocations
//! - Comparable to match-based dispatch
//!
//! ## References
//!
//! - Gang of Four: Visitor Pattern
//! - Rust Book: Trait Objects and Dynamic Dispatch

use crate::vm::engine::{VM, VmError};
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
        match self {
            // Stack operations - use direct execution since exec_stack_op is private
            OpCode::Const(_) | OpCode::Pop | OpCode::Dup | OpCode::Nop => {
                vm.execute_opcode_direct(*self, 0)
            }
            
            // Arithmetic operations
            OpCode::Add => vm.exec_add(),
            OpCode::Sub => vm.exec_sub(),
            OpCode::Mul => vm.exec_mul(),
            OpCode::Div => vm.exec_div(),
            OpCode::Mod => vm.exec_mod(),
            OpCode::Pow => vm.exec_pow(),
            
            // Bitwise operations
            OpCode::BitwiseAnd => vm.exec_bitwise_and(),
            OpCode::BitwiseOr => vm.exec_bitwise_or(),
            OpCode::BitwiseXor => vm.exec_bitwise_xor(),
            OpCode::ShiftLeft => vm.exec_shift_left(),
            OpCode::ShiftRight => vm.exec_shift_right(),
            OpCode::BitwiseNot => vm.exec_bitwise_not(),
            OpCode::BoolNot => vm.exec_bool_not(),
            
            // Comparison operations
            OpCode::IsEqual => vm.exec_equal(),
            OpCode::IsNotEqual => vm.exec_not_equal(),
            OpCode::IsIdentical => vm.exec_identical(),
            OpCode::IsNotIdentical => vm.exec_not_identical(),
            OpCode::IsLess => vm.exec_less_than(),
            OpCode::IsLessOrEqual => vm.exec_less_than_or_equal(),
            OpCode::IsGreater => vm.exec_greater_than(),
            OpCode::IsGreaterOrEqual => vm.exec_greater_than_or_equal(),
            OpCode::Spaceship => vm.exec_spaceship(),
            
            // Control flow operations
            OpCode::Jmp(target) => vm.exec_jmp(*target as usize),
            OpCode::JmpIfFalse(target) => vm.exec_jmp_if_false(*target as usize),
            OpCode::JmpIfTrue(target) => vm.exec_jmp_if_true(*target as usize),
            OpCode::JmpZEx(target) => vm.exec_jmp_z_ex(*target as usize),
            OpCode::JmpNzEx(target) => vm.exec_jmp_nz_ex(*target as usize),
            
            // Array operations
            OpCode::InitArray(capacity) => vm.exec_init_array(*capacity),
            OpCode::StoreDim => vm.exec_store_dim(),
            
            // Special operations
            OpCode::Echo => vm.exec_echo(),
            
            // Variable operations - these are private, use direct execution
            OpCode::LoadVar(_) | OpCode::StoreVar(_) => {
                vm.execute_opcode_direct(*self, 0)
            }
            
            // For all other opcodes, delegate to the main execute_opcode
            // This allows gradual migration to the visitor pattern
            _ => vm.execute_opcode_direct(*self, 0),
        }
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
