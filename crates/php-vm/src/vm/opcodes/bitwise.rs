//! Bitwise operations
//! 
//! Implements PHP bitwise and logical operations following Zend semantics.
//! 
//! ## PHP Semantics
//! 
//! Bitwise operations work on integers:
//! - Operands are converted to integers via type juggling
//! - Results are always integers (or strings for string bitwise ops)
//! - Shift operations use modulo for shift amounts
//! 
//! ## Operations
//! 
//! - **BitwiseAnd**: `$a & $b` - Bitwise AND
//! - **BitwiseOr**: `$a | $b` - Bitwise OR
//! - **BitwiseXor**: `$a ^ $b` - Bitwise XOR
//! - **BitwiseNot**: `~$a` - Bitwise NOT (one's complement)
//! - **ShiftLeft**: `$a << $b` - Left shift
//! - **ShiftRight**: `$a >> $b` - Right shift (arithmetic)
//! - **BoolNot**: `!$a` - Logical NOT (boolean negation)
//! 
//! ## Special Cases
//! 
//! - String bitwise operations work character-by-character
//! - Shift amounts > 63 are reduced via modulo
//! - Negative shift amounts cause undefined behavior in PHP
//! 
//! ## Performance
//! 
//! All operations are O(1) on integers. String bitwise operations
//! are O(n) where n is the string length.
//! 
//! ## References
//! 
//! - Zend: `$PHP_SRC_PATH/Zend/zend_operators.c` - bitwise functions
//! - PHP Manual: https://www.php.net/manual/en/language.operators.bitwise.php

use crate::vm::engine::{VM, VmError};

impl VM {
    /// Execute BitwiseAnd operation: $result = $left & $right
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - bitwise_and_function
    #[inline]
    pub(crate) fn exec_bitwise_and(&mut self) -> Result<(), VmError> {
        self.bitwise_and()
    }

    /// Execute BitwiseOr operation: $result = $left | $right
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - bitwise_or_function
    #[inline]
    pub(crate) fn exec_bitwise_or(&mut self) -> Result<(), VmError> {
        self.bitwise_or()
    }

    /// Execute BitwiseXor operation: $result = $left ^ $right
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - bitwise_xor_function
    #[inline]
    pub(crate) fn exec_bitwise_xor(&mut self) -> Result<(), VmError> {
        self.bitwise_xor()
    }

    /// Execute ShiftLeft operation: $result = $left << $right
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - shift_left_function
    #[inline]
    pub(crate) fn exec_shift_left(&mut self) -> Result<(), VmError> {
        self.bitwise_shl()
    }

    /// Execute ShiftRight operation: $result = $left >> $right
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - shift_right_function
    #[inline]
    pub(crate) fn exec_shift_right(&mut self) -> Result<(), VmError> {
        self.bitwise_shr()
    }

    /// Execute BitwiseNot operation: $result = ~$value
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - bitwise_not_function
    #[inline]
    pub(crate) fn exec_bitwise_not(&mut self) -> Result<(), VmError> {
        self.bitwise_not()
    }

    /// Execute BoolNot operation: $result = !$value
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - boolean_not_function
    #[inline]
    pub(crate) fn exec_bool_not(&mut self) -> Result<(), VmError> {
        self.bool_not()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::value::Val;
    use crate::runtime::context::EngineContext;
    use std::sync::Arc;

    #[test]
    fn test_bitwise_and() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // 12 & 10 = 8 (binary: 1100 & 1010 = 1000)
        let left = vm.arena.alloc(Val::Int(12));
        let right = vm.arena.alloc(Val::Int(10));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_bitwise_and().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(8)));
    }

    #[test]
    fn test_bitwise_or() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // 12 | 10 = 14 (binary: 1100 | 1010 = 1110)
        let left = vm.arena.alloc(Val::Int(12));
        let right = vm.arena.alloc(Val::Int(10));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_bitwise_or().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(14)));
    }

    #[test]
    fn test_bitwise_xor() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // 12 ^ 10 = 6 (binary: 1100 ^ 1010 = 0110)
        let left = vm.arena.alloc(Val::Int(12));
        let right = vm.arena.alloc(Val::Int(10));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_bitwise_xor().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(6)));
    }

    #[test]
    fn test_bitwise_not() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // ~5 = -6 (two's complement: NOT 0000...0101 = 1111...1010 = -6)
        let value = vm.arena.alloc(Val::Int(5));
        vm.operand_stack.push(value);

        vm.exec_bitwise_not().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(-6)));
    }

    #[test]
    fn test_shift_left() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // 5 << 2 = 20 (binary: 101 << 2 = 10100)
        let left = vm.arena.alloc(Val::Int(5));
        let right = vm.arena.alloc(Val::Int(2));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_shift_left().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(20)));
    }

    #[test]
    fn test_shift_right() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // 20 >> 2 = 5 (binary: 10100 >> 2 = 101)
        let left = vm.arena.alloc(Val::Int(20));
        let right = vm.arena.alloc(Val::Int(2));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_shift_right().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(5)));
    }

    #[test]
    fn test_shift_right_negative() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // -20 >> 2 = -5 (arithmetic right shift preserves sign)
        let left = vm.arena.alloc(Val::Int(-20));
        let right = vm.arena.alloc(Val::Int(2));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_shift_right().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(-5)));
    }

    #[test]
    fn test_bool_not_true() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let value = vm.arena.alloc(Val::Bool(true));
        vm.operand_stack.push(value);

        vm.exec_bool_not().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Bool(false)));
    }

    #[test]
    fn test_bool_not_false() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let value = vm.arena.alloc(Val::Bool(false));
        vm.operand_stack.push(value);

        vm.exec_bool_not().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Bool(true)));
    }

    #[test]
    fn test_bool_not_integer_zero() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // !0 = true
        let value = vm.arena.alloc(Val::Int(0));
        vm.operand_stack.push(value);

        vm.exec_bool_not().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Bool(true)));
    }

    #[test]
    fn test_bool_not_integer_nonzero() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // !42 = false
        let value = vm.arena.alloc(Val::Int(42));
        vm.operand_stack.push(value);

        vm.exec_bool_not().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Bool(false)));
    }

    #[test]
    fn test_bitwise_with_type_conversion() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // "12" & "10" performs character-by-character bitwise AND on strings
        // '1' (0x31) & '1' (0x31) = 0x31 = '1'
        // '2' (0x32) & '0' (0x30) = 0x30 = '0'
        // Result: "10" (as string, not integer)
        let left = vm.arena.alloc(Val::String(b"12".to_vec().into()));
        let right = vm.arena.alloc(Val::String(b"10".to_vec().into()));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_bitwise_and().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        // String & String = String (character-wise operation)
        assert!(matches!(result_val.value, Val::String(ref s) if s.as_ref() == b"10"));
    }

    #[test]
    fn test_shift_left_large_amount() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // 1 << 10 = 1024
        let left = vm.arena.alloc(Val::Int(1));
        let right = vm.arena.alloc(Val::Int(10));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_shift_left().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(1024)));
    }

    #[test]
    fn test_bitwise_operations_with_zero() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // 0 & 5 = 0
        let left = vm.arena.alloc(Val::Int(0));
        let right = vm.arena.alloc(Val::Int(5));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_bitwise_and().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(0)));
    }

    #[test]
    fn test_bitwise_or_all_ones() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // 15 | 240 = 255 (binary: 00001111 | 11110000 = 11111111)
        let left = vm.arena.alloc(Val::Int(15));
        let right = vm.arena.alloc(Val::Int(240));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_bitwise_or().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(255)));
    }
}
