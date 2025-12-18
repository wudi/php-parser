//! Arithmetic operations
//!
//! Implements PHP arithmetic operations following Zend engine semantics.
//!
//! ## PHP Semantics
//!
//! PHP arithmetic operations perform automatic type juggling:
//! - Numeric strings are converted to integers/floats
//! - Booleans: true=1, false=0
//! - null converts to 0
//! - Arrays/Objects cause type errors or warnings
//!
//! ## Operations
//!
//! - **Add**: `$a + $b` - Addition with type coercion
//! - **Sub**: `$a - $b` - Subtraction
//! - **Mul**: `$a * $b` - Multiplication
//! - **Div**: `$a / $b` - Division (returns float or int)
//! - **Mod**: `$a % $b` - Modulo operation
//! - **Pow**: `$a ** $b` - Exponentiation
//!
//! ## Performance
//!
//! All operations are O(1) after type conversion. Type juggling may
//! allocate new values on the arena.
//!
//! ## References
//!
//! - Zend: `$PHP_SRC_PATH/Zend/zend_operators.c` - arithmetic functions
//! - PHP Manual: https://www.php.net/manual/en/language.operators.arithmetic.php

use crate::core::value::Val;
use crate::vm::engine::{ErrorLevel, VmError, VM};
use std::rc::Rc;

/// Arithmetic operation types
/// Reference: $PHP_SRC_PATH/Zend/zend_operators.c
#[derive(Debug, Clone, Copy)]
enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

impl ArithOp {
    fn apply_int(&self, a: i64, b: i64) -> Option<i64> {
        match self {
            ArithOp::Add => Some(a.wrapping_add(b)),
            ArithOp::Sub => Some(a.wrapping_sub(b)),
            ArithOp::Mul => Some(a.wrapping_mul(b)),
            ArithOp::Mod if b != 0 => Some(a % b),
            _ => None, // Div/Pow always use float, Mod checks zero
        }
    }

    fn apply_float(&self, a: f64, b: f64) -> f64 {
        match self {
            ArithOp::Add => a + b,
            ArithOp::Sub => a - b,
            ArithOp::Mul => a * b,
            ArithOp::Div => a / b,
            ArithOp::Pow => a.powf(b),
            ArithOp::Mod => unreachable!(), // Mod uses int only
        }
    }

    fn always_float(&self) -> bool {
        matches!(self, ArithOp::Div | ArithOp::Pow)
    }
}

impl VM {
    /// Generic binary arithmetic operation
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c
    fn binary_arithmetic(&mut self, op: ArithOp) -> Result<(), VmError> {
        let (a_handle, b_handle) = self.pop_binary_operands()?;
        let a_val = &self.arena.get(a_handle).value;
        let b_val = &self.arena.get(b_handle).value;

        // Special case: Array + Array = union (only for Add)
        if matches!(op, ArithOp::Add) {
            if let (Val::Array(a_arr), Val::Array(b_arr)) = (a_val, b_val) {
                let mut result = (**a_arr).clone();
                for (k, v) in b_arr.map.iter() {
                    result.map.entry(k.clone()).or_insert(*v);
                }
                self.operand_stack
                    .push(self.arena.alloc(Val::Array(Rc::new(result))));
                return Ok(());
            }
        }

        // Check for division/modulo by zero
        if matches!(op, ArithOp::Div) && b_val.to_float() == 0.0 {
            self.report_error(ErrorLevel::Warning, "Division by zero");
            self.operand_stack
                .push(self.arena.alloc(Val::Float(f64::INFINITY)));
            return Ok(());
        }
        if matches!(op, ArithOp::Mod) && b_val.to_int() == 0 {
            self.report_error(ErrorLevel::Warning, "Modulo by zero");
            self.operand_stack.push(self.arena.alloc(Val::Bool(false)));
            return Ok(());
        }

        // Determine result type and compute
        let needs_float =
            op.always_float() || matches!(a_val, Val::Float(_)) || matches!(b_val, Val::Float(_));

        let result = if needs_float {
            Val::Float(op.apply_float(a_val.to_float(), b_val.to_float()))
        } else if let Some(int_result) = op.apply_int(a_val.to_int(), b_val.to_int()) {
            Val::Int(int_result)
        } else {
            Val::Float(op.apply_float(a_val.to_float(), b_val.to_float()))
        };

        self.operand_stack.push(self.arena.alloc(result));
        Ok(())
    }

    /// Execute Add operation: $result = $left + $right
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - add_function
    #[inline]
    pub(crate) fn exec_add(&mut self) -> Result<(), VmError> {
        self.binary_arithmetic(ArithOp::Add)
    }

    /// Execute Sub operation: $result = $left - $right
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - sub_function
    #[inline]
    pub(crate) fn exec_sub(&mut self) -> Result<(), VmError> {
        self.binary_arithmetic(ArithOp::Sub)
    }

    /// Execute Mul operation: $result = $left * $right
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - mul_function
    #[inline]
    pub(crate) fn exec_mul(&mut self) -> Result<(), VmError> {
        self.binary_arithmetic(ArithOp::Mul)
    }

    /// Execute Div operation: $result = $left / $right
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - div_function
    #[inline]
    pub(crate) fn exec_div(&mut self) -> Result<(), VmError> {
        self.binary_arithmetic(ArithOp::Div)
    }

    /// Execute Mod operation: $result = $left % $right
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - mod_function
    #[inline]
    pub(crate) fn exec_mod(&mut self) -> Result<(), VmError> {
        self.binary_arithmetic(ArithOp::Mod)
    }

    /// Execute Pow operation: $result = $left ** $right
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - pow_function
    #[inline]
    pub(crate) fn exec_pow(&mut self) -> Result<(), VmError> {
        self.binary_arithmetic(ArithOp::Pow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::value::Val;
    use crate::runtime::context::EngineContext;
    use std::sync::Arc;

    #[test]
    fn test_add_integers() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Int(10));
        let right = vm.arena.alloc(Val::Int(32));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_add().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(42)));
    }

    #[test]
    fn test_add_floats() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Float(10.5));
        let right = vm.arena.alloc(Val::Float(20.7));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_add().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        if let Val::Float(f) = result_val.value {
            assert!((f - 31.2).abs() < 0.0001);
        } else {
            panic!("Expected float");
        }
    }

    #[test]
    fn test_add_int_and_float() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Int(10));
        let right = vm.arena.alloc(Val::Float(5.5));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_add().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        // Int + Float = Float
        if let Val::Float(f) = result_val.value {
            assert!((f - 15.5).abs() < 0.0001);
        } else {
            panic!("Expected float, got {:?}", result_val.value);
        }
    }

    #[test]
    fn test_subtract_integers() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Int(50));
        let right = vm.arena.alloc(Val::Int(8));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_sub().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(42)));
    }

    #[test]
    fn test_multiply_integers() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Int(6));
        let right = vm.arena.alloc(Val::Int(7));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_mul().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(42)));
    }

    #[test]
    fn test_divide_integers_exact() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Int(84));
        let right = vm.arena.alloc(Val::Int(2));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_div().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        // Division always returns float in PHP
        if let Val::Float(f) = result_val.value {
            assert!((f - 42.0).abs() < 0.0001);
        } else {
            panic!("Expected float for division");
        }
    }

    #[test]
    fn test_divide_integers_float_result() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Int(10));
        let right = vm.arena.alloc(Val::Int(3));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_div().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        // 10 / 3 = 3.333... (non-exact division returns float)
        if let Val::Float(f) = result_val.value {
            assert!((f - 3.333333).abs() < 0.001);
        } else {
            panic!("Expected float for non-exact division");
        }
    }

    #[test]
    fn test_modulo() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Int(10));
        let right = vm.arena.alloc(Val::Int(3));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_mod().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(1)));
    }

    #[test]
    fn test_power() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Int(2));
        let right = vm.arena.alloc(Val::Int(10));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_pow().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        // Power always returns float in PHP
        if let Val::Float(f) = result_val.value {
            assert!((f - 1024.0).abs() < 0.0001);
        } else {
            panic!("Expected float for power operation");
        }
    }

    #[test]
    fn test_add_with_numeric_string() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Int(10));
        let right = vm.arena.alloc(Val::String(b"32".to_vec().into()));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_add().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        // "32" converts to 32
        assert!(matches!(result_val.value, Val::Int(42)));
    }

    #[test]
    fn test_add_with_bool() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Int(41));
        let right = vm.arena.alloc(Val::Bool(true));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_add().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        // true converts to 1
        assert!(matches!(result_val.value, Val::Int(42)));
    }

    #[test]
    fn test_add_with_null() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Int(42));
        let right = vm.arena.alloc(Val::Null);

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_add().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        // null converts to 0
        assert!(matches!(result_val.value, Val::Int(42)));
    }

    #[test]
    fn test_negative_result() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let left = vm.arena.alloc(Val::Int(10));
        let right = vm.arena.alloc(Val::Int(52));

        vm.operand_stack.push(left);
        vm.operand_stack.push(right);

        vm.exec_sub().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(-42)));
    }
}
