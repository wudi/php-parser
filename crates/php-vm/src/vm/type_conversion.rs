//! PHP type juggling and conversion
//!
//! Implements PHP's automatic type conversion following Zend semantics.
//!
//! ## PHP Type Juggling Rules
//!
//! PHP automatically converts between types based on context:
//!
//! ### To Integer
//! - `true` → 1, `false` → 0
//! - Floats truncated toward zero: 7.9 → 7, -7.9 → -7
//! - Numeric strings parsed: "123" → 123, "12.34" → 12
//! - Non-numeric strings → 0 (with notice)
//! - null → 0
//! - Arrays/Objects → implementation-specific
//!
//! ### To Float
//! - Integers promoted to float
//! - Numeric strings parsed: "12.34" → 12.34
//! - Booleans: true → 1.0, false → 0.0
//! - null → 0.0
//!
//! ### To Boolean
//! - Falsy: false, 0, 0.0, "", "0", null, empty arrays
//! - Truthy: everything else
//!
//! ### To String
//! - Integers/floats: standard representation
//! - true → "1", false → ""
//! - null → ""
//! - Arrays → "Array" (with notice)
//! - Objects → __toString() or error
//!
//! ## Performance
//!
//! Type conversions are generally O(1) except:
//! - String parsing: O(n) where n is string length
//! - Object __toString(): depends on implementation
//!
//! ## References
//!
//! - Zend: `$PHP_SRC_PATH/Zend/zend_operators.c` - conversion functions
//! - PHP Manual: https://www.php.net/manual/en/language.types.type-juggling.php

use crate::core::value::{Handle, Val};
use crate::vm::engine::{VmError, VM};
use std::rc::Rc;

impl VM {
    /// Convert any value to integer following PHP rules
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - _zval_get_long_func
    #[inline]
    pub(crate) fn value_to_int(&self, handle: Handle) -> i64 {
        self.arena.get(handle).value.to_int()
    }

    /// Convert any value to float following PHP rules
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - _zval_get_double_func
    #[inline]
    pub(crate) fn value_to_float(&self, handle: Handle) -> f64 {
        self.arena.get(handle).value.to_float()
    }

    /// Convert any value to boolean following PHP rules
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - zend_is_true
    #[inline]
    pub(crate) fn value_to_bool(&self, handle: Handle) -> bool {
        self.arena.get(handle).value.to_bool()
    }

    /// Convert value to string with full error handling
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - _zval_get_string_func
    pub(crate) fn value_to_string_bytes(&mut self, handle: Handle) -> Result<Vec<u8>, VmError> {
        self.convert_to_string(handle)
    }

    /// Create a string handle from bytes
    /// Reference: Common pattern for string allocation
    #[inline]
    pub(crate) fn new_string_handle(&mut self, bytes: Vec<u8>) -> Handle {
        self.arena.alloc(Val::String(Rc::new(bytes)))
    }

    /// Create an integer handle
    #[inline]
    pub(crate) fn new_int_handle(&mut self, value: i64) -> Handle {
        self.arena.alloc(Val::Int(value))
    }

    /// Create a float handle
    #[inline]
    pub(crate) fn new_float_handle(&mut self, value: f64) -> Handle {
        self.arena.alloc(Val::Float(value))
    }

    /// Create a boolean handle
    #[inline]
    pub(crate) fn new_bool_handle(&mut self, value: bool) -> Handle {
        self.arena.alloc(Val::Bool(value))
    }

    /// Create a null handle
    #[inline]
    pub(crate) fn new_null_handle(&mut self) -> Handle {
        self.arena.alloc(Val::Null)
    }
}

/// Type coercion utilities
pub(crate) trait TypeJuggling {
    /// Determine if two values should be compared numerically
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - compare_function
    fn should_compare_numerically(&self, other: &Self) -> bool;

    /// Get numeric comparison value
    fn numeric_value(&self) -> NumericValue;
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum NumericValue {
    Int(i64),
    Float(f64),
}

impl NumericValue {
    pub fn to_float(self) -> f64 {
        match self {
            NumericValue::Int(i) => i as f64,
            NumericValue::Float(f) => f,
        }
    }
}

impl TypeJuggling for Val {
    fn should_compare_numerically(&self, other: &Self) -> bool {
        use Val::*;
        matches!(
            (self, other),
            (Int(_), Int(_))
                | (Float(_), Float(_))
                | (Int(_), Float(_))
                | (Float(_), Int(_))
                | (Int(_), String(_))
                | (String(_), Int(_))
                | (Float(_), String(_))
                | (String(_), Float(_))
        )
    }

    fn numeric_value(&self) -> NumericValue {
        match self {
            Val::Int(i) => NumericValue::Int(*i),
            Val::Float(f) => NumericValue::Float(*f),
            _ => NumericValue::Int(self.to_int()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::context::EngineContext;
    use std::sync::Arc;

    #[test]
    fn test_value_to_int() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let int_handle = vm.arena.alloc(Val::Int(42));
        assert_eq!(vm.value_to_int(int_handle), 42);

        let float_handle = vm.arena.alloc(Val::Float(3.14));
        assert_eq!(vm.value_to_int(float_handle), 3);

        let bool_handle = vm.arena.alloc(Val::Bool(true));
        assert_eq!(vm.value_to_int(bool_handle), 1);
    }

    #[test]
    fn test_value_to_bool() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let zero = vm.arena.alloc(Val::Int(0));
        assert!(!vm.value_to_bool(zero));

        let one = vm.arena.alloc(Val::Int(1));
        assert!(vm.value_to_bool(one));

        let null = vm.arena.alloc(Val::Null);
        assert!(!vm.value_to_bool(null));
    }

    #[test]
    fn test_numeric_comparison() {
        let int_val = Val::Int(42);
        let float_val = Val::Float(3.14);
        let string_val = Val::String(Rc::new(b"hello".to_vec()));

        assert!(int_val.should_compare_numerically(&float_val));
        assert!(int_val.should_compare_numerically(&string_val));
        assert!(!string_val.should_compare_numerically(&Val::Null));
    }
}
