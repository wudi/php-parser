//! Control flow operations
//!
//! Implements control flow opcodes for jumps, conditionals, and exceptions.
//!
//! ## PHP Semantics
//!
//! Jump operations modify the instruction pointer (IP) to enable:
//! - Conditional execution (if/else, ternary)
//! - Loops (for, while, foreach)
//! - Short-circuit evaluation (&&, ||, ??)
//! - Exception handling (try/catch)
//!
//! ## Operations
//!
//! - **Jmp**: Unconditional jump to target offset
//! - **JmpIfFalse**: Jump if operand is falsy (pops value)
//! - **JmpIfTrue**: Jump if operand is truthy (pops value)
//! - **JmpZEx**: Jump if falsy, else leave value on stack (peek)
//! - **JmpNzEx**: Jump if truthy, else leave value on stack (peek)
//!
//! ## Implementation Notes
//!
//! Jump targets are absolute offsets into the current function's bytecode.
//! The VM maintains separate instruction pointers per call frame.
//!
//! Conditional jumps use PHP's truthiness rules:
//! - Falsy: false, 0, 0.0, "", "0", null, empty arrays
//! - Truthy: everything else
//!
//! ## Performance
//!
//! All jump operations are O(1). No heap allocations.
//!
//! ## References
//!
//! - Zend: `$PHP_SRC_PATH/Zend/zend_vm_execute.h` - ZEND_JMP* handlers
//! - Zend: `$PHP_SRC_PATH/Zend/zend_vm_def.h` - jump opcode definitions

use crate::vm::engine::{VmError, VM};

impl VM {
    /// Execute unconditional jump
    #[inline]
    pub(crate) fn exec_jmp(&mut self, target: usize) -> Result<(), VmError> {
        self.set_ip(target)
    }

    /// Execute conditional jump if false
    #[inline]
    pub(crate) fn exec_jmp_if_false(&mut self, target: usize) -> Result<(), VmError> {
        self.jump_if(target, |v| !v.to_bool())
    }

    /// Execute conditional jump if true
    #[inline]
    pub(crate) fn exec_jmp_if_true(&mut self, target: usize) -> Result<(), VmError> {
        self.jump_if(target, |v| v.to_bool())
    }

    /// Execute jump with zero check (peek or pop)
    #[inline]
    pub(crate) fn exec_jmp_z_ex(&mut self, target: usize) -> Result<(), VmError> {
        self.jump_peek_or_pop(target, |v| !v.to_bool())
    }

    /// Execute jump with non-zero check (peek or pop)
    #[inline]
    pub(crate) fn exec_jmp_nz_ex(&mut self, target: usize) -> Result<(), VmError> {
        self.jump_peek_or_pop(target, |v| v.to_bool())
    }
}

// Note: Control flow operations require call frame setup for testing.
// These operations are comprehensively tested through integration tests
// that execute real bytecode sequences.
