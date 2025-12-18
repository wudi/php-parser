//! Special language constructs
//! 
//! Implements PHP-specific language constructs that don't fit other categories.
//! 
//! ## PHP Semantics
//! 
//! These operations handle special PHP constructs:
//! - Output: echo, print
//! - Type checking: isset, empty, is_array, etc.
//! - Object operations: clone, instanceof
//! - Error control: @ operator (silence)
//! 
//! ## Operations
//! 
//! - **Echo**: Output value to stdout (no return value)
//! - **Print**: Output value and return 1 (always succeeds)
//! 
//! ## Echo vs Print
//! 
//! Both convert values to strings and output them:
//! - `echo` is a statement (no return value, can take multiple args)
//! - `print` is an expression (returns 1, takes one arg)
//! 
//! ## String Conversion
//! 
//! Values are converted to strings following PHP rules:
//! - Integers/floats: standard string representation
//! - Booleans: "1" for true, "" for false
//! - null: ""
//! - Arrays: "Array" (with notice in some contexts)
//! - Objects: __toString() method or "Object"
//! 
//! ## Performance
//! 
//! Output operations are I/O bound. String conversion is O(1) for
//! primitive types, O(n) for arrays/objects.
//! 
//! ## References
//! 
//! - Zend: `$PHP_SRC_PATH/Zend/zend_vm_execute.h` - ZEND_ECHO handler
//! - PHP Manual: https://www.php.net/manual/en/function.echo.php

use crate::vm::engine::{VM, VmError};

impl VM {
    /// Execute Echo operation: Output value to stdout
    /// Reference: $PHP_SRC_PATH/Zend/zend_vm_execute.h - ZEND_ECHO
    pub(crate) fn exec_echo(&mut self) -> Result<(), VmError> {
        let handle = self.pop_operand_required()?;
        let s = self.convert_to_string(handle)?;
        self.write_output(&s)?;
        Ok(())
    }

    /// Execute Print operation: Output value and push 1
    /// Reference: $PHP_SRC_PATH/Zend/zend_vm_execute.h - ZEND_PRINT
    pub(crate) fn exec_print(&mut self) -> Result<(), VmError> {
        let handle = self.pop_operand_required()?;
        let s = self.convert_to_string(handle)?;
        self.write_output(&s)?;
        let one = self.arena.alloc(crate::core::value::Val::Int(1));
        self.operand_stack.push(one);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::context::EngineContext;
    use crate::core::value::Val;
    use std::sync::{Arc, Mutex};

    /// Test output writer that captures output to a Vec
    struct TestOutputWriter {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl TestOutputWriter {
        fn new(buffer: Arc<Mutex<Vec<u8>>>) -> Self {
            Self { buffer }
        }
    }

    impl crate::vm::engine::OutputWriter for TestOutputWriter {
        fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
            self.buffer.lock().unwrap().extend_from_slice(bytes);
            Ok(())
        }
    }

    #[test]
    fn test_echo_integer() {
        let engine = Arc::new(EngineContext::new());
        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let mut vm = VM::new(engine);
        vm.set_output_writer(Box::new(TestOutputWriter::new(output_buffer.clone())));

        let val = vm.arena.alloc(Val::Int(42));
        vm.operand_stack.push(val);

        vm.exec_echo().unwrap();

        let output = output_buffer.lock().unwrap();
        assert_eq!(&*output, b"42");
        assert!(vm.operand_stack.is_empty());
    }

    #[test]
    fn test_echo_string() {
        let engine = Arc::new(EngineContext::new());
        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let mut vm = VM::new(engine);
        vm.set_output_writer(Box::new(TestOutputWriter::new(output_buffer.clone())));

        let val = vm.arena.alloc(Val::String(b"Hello, World!".to_vec().into()));
        vm.operand_stack.push(val);

        vm.exec_echo().unwrap();

        let output = output_buffer.lock().unwrap();
        assert_eq!(&*output, b"Hello, World!");
    }

    #[test]
    fn test_echo_float() {
        let engine = Arc::new(EngineContext::new());
        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let mut vm = VM::new(engine);
        vm.set_output_writer(Box::new(TestOutputWriter::new(output_buffer.clone())));

        let val = vm.arena.alloc(Val::Float(3.14));
        vm.operand_stack.push(val);

        vm.exec_echo().unwrap();

        let output = output_buffer.lock().unwrap();
        assert_eq!(&*output, b"3.14");
    }

    #[test]
    fn test_echo_boolean_true() {
        let engine = Arc::new(EngineContext::new());
        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let mut vm = VM::new(engine);
        vm.set_output_writer(Box::new(TestOutputWriter::new(output_buffer.clone())));

        let val = vm.arena.alloc(Val::Bool(true));
        vm.operand_stack.push(val);

        vm.exec_echo().unwrap();

        let output = output_buffer.lock().unwrap();
        assert_eq!(&*output, b"1");
    }

    #[test]
    fn test_echo_boolean_false() {
        let engine = Arc::new(EngineContext::new());
        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let mut vm = VM::new(engine);
        vm.set_output_writer(Box::new(TestOutputWriter::new(output_buffer.clone())));

        let val = vm.arena.alloc(Val::Bool(false));
        vm.operand_stack.push(val);

        vm.exec_echo().unwrap();

        let output = output_buffer.lock().unwrap();
        assert_eq!(&*output, b"");
    }

    #[test]
    fn test_echo_null() {
        let engine = Arc::new(EngineContext::new());
        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let mut vm = VM::new(engine);
        vm.set_output_writer(Box::new(TestOutputWriter::new(output_buffer.clone())));

        let val = vm.arena.alloc(Val::Null);
        vm.operand_stack.push(val);

        vm.exec_echo().unwrap();

        let output = output_buffer.lock().unwrap();
        assert_eq!(&*output, b"");
    }

    #[test]
    fn test_print_integer_returns_one() {
        let engine = Arc::new(EngineContext::new());
        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let mut vm = VM::new(engine);
        vm.set_output_writer(Box::new(TestOutputWriter::new(output_buffer.clone())));

        let val = vm.arena.alloc(Val::Int(42));
        vm.operand_stack.push(val);

        vm.exec_print().unwrap();

        let output = output_buffer.lock().unwrap();
        assert_eq!(&*output, b"42");

        // print always returns 1
        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(1)));
    }

    #[test]
    fn test_print_string_returns_one() {
        let engine = Arc::new(EngineContext::new());
        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let mut vm = VM::new(engine);
        vm.set_output_writer(Box::new(TestOutputWriter::new(output_buffer.clone())));

        let val = vm.arena.alloc(Val::String(b"test".to_vec().into()));
        vm.operand_stack.push(val);

        vm.exec_print().unwrap();

        let output = output_buffer.lock().unwrap();
        assert_eq!(&*output, b"test");

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(1)));
    }

    #[test]
    fn test_echo_multiple_values() {
        let engine = Arc::new(EngineContext::new());
        let output_buffer = Arc::new(Mutex::new(Vec::new()));
        let mut vm = VM::new(engine);
        vm.set_output_writer(Box::new(TestOutputWriter::new(output_buffer.clone())));

        // Echo two values in sequence
        let val1 = vm.arena.alloc(Val::String(b"Hello ".to_vec().into()));
        vm.operand_stack.push(val1);
        vm.exec_echo().unwrap();

        let val2 = vm.arena.alloc(Val::String(b"World".to_vec().into()));
        vm.operand_stack.push(val2);
        vm.exec_echo().unwrap();

        let output = output_buffer.lock().unwrap();
        assert_eq!(&*output, b"Hello World");
    }
}
