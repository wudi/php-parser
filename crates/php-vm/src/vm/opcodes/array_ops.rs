//! Array operations
//!
//! Implements PHP array manipulation operations following Zend semantics.
//!
//! ## PHP Semantics
//!
//! PHP arrays are ordered hash maps supporting both integer and string keys:
//! - Automatic integer key assignment for append operations
//! - String keys can be numeric strings ("0", "123")
//! - References allow in-place modification
//! - Copy-on-write for value assignments
//!
//! ## Operations
//!
//! - **InitArray**: Create a new array with initial capacity
//! - **AssignDim**: `$arr[$key] = $val` - Assign to array element
//! - **StoreDim**: Store value at dimension (handles refs)
//! - **FetchDim**: `$arr[$key]` - Fetch array element
//! - **AppendArray**: `$arr[] = $val` - Append with auto-key
//!
//! ## Reference Handling
//!
//! When the array handle has `is_ref=true`:
//! - Modification is in-place
//! - Same handle is pushed back to stack
//!
//! When `is_ref=false`:
//! - Copy-on-write semantics apply
//! - New handle is created and pushed
//!
//! ## ArrayAccess Interface
//!
//! Objects implementing ArrayAccess are handled specially:
//! - Dimension operations call offsetGet/offsetSet methods
//! - Allows user-defined array-like behavior
//!
//! ## Performance
//!
//! - Init: O(1) allocation
//! - Assign/Fetch: O(1) hash map access
//! - Append: Amortized O(1) with occasional reallocation
//!
//! ## References
//!
//! - Zend: `$PHP_SRC_PATH/Zend/zend_vm_execute.h` - ZEND_ASSIGN_DIM handlers
//! - Zend: `$PHP_SRC_PATH/Zend/zend_hash.c` - Hash table implementation

use crate::core::value::{ArrayData, Val};
use crate::vm::engine::{VmError, VM};
use std::rc::Rc;

impl VM {
    /// Execute InitArray operation: Create new array with initial capacity
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_INIT_ARRAY
    #[inline]
    pub(crate) fn exec_init_array(&mut self, _capacity: u32) -> Result<(), VmError> {
        let array_handle = self.arena.alloc(Val::Array(Rc::new(ArrayData::new())));
        self.operand_stack.push(array_handle);
        Ok(())
    }

    /// Execute AssignDim operation: $array[$key] = $value
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_ASSIGN_DIM
    #[inline]
    pub(crate) fn exec_assign_dim(&mut self) -> Result<(), VmError> {
        // Stack: [value, key, array]
        let val_handle = self.pop_operand_required()?;
        let key_handle = self.pop_operand_required()?;
        let array_handle = self.pop_operand_required()?;

        self.assign_dim_value(array_handle, key_handle, val_handle)?;
        Ok(())
    }

    /// Execute StoreDim operation: Pop val, key, array and assign array[key] = val
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_ASSIGN_DIM (variant)
    #[inline]
    pub(crate) fn exec_store_dim(&mut self) -> Result<(), VmError> {
        // Pops (top-to-bottom): value, key, array
        let val_handle = self.pop_operand_required()?;
        let key_handle = self.pop_operand_required()?;
        let array_handle = self.pop_operand_required()?;

        // assign_dim pushes the result array to the stack
        self.assign_dim(array_handle, key_handle, val_handle)?;
        Ok(())
    }

    /// Execute AppendArray operation: $array[] = $value
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_ASSIGN_DIM (no key)
    #[inline]
    pub(crate) fn exec_append_array(&mut self) -> Result<(), VmError> {
        let val_handle = self.pop_operand_required()?;
        let array_handle = self.pop_operand_required()?;

        self.append_array(array_handle, val_handle)?;
        Ok(())
    }

    /// Execute FetchDim operation: $result = $array[$key]
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_FETCH_DIM_*
    #[inline]
    pub(crate) fn exec_fetch_dim(&mut self) -> Result<(), VmError> {
        let key_handle = self.pop_operand_required()?;
        let array_handle = self.pop_operand_required()?;

        let result = self.fetch_nested_dim(array_handle, &[key_handle])?;
        self.operand_stack.push(result);
        Ok(())
    }

    /// Execute AssignNestedDim operation: $array[$k1][$k2]..[$kN] = $value
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - nested array assignment
    #[inline]
    pub(crate) fn exec_assign_nested_dim(&mut self, key_count: u8) -> Result<(), VmError> {
        let val_handle = self.pop_operand_required()?;
        let keys = self.pop_n_operands(key_count as usize)?;
        let array_handle = self.pop_operand_required()?;

        self.assign_nested_dim(array_handle, &keys, val_handle)?;
        Ok(())
    }

    /// Execute FetchNestedDim operation: $result = $array[$k1][$k2]..[$kN]
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - nested array access
    #[inline]
    pub(crate) fn exec_fetch_nested_dim_op(&mut self, key_count: u8) -> Result<(), VmError> {
        // Stack: [array, key_n, ..., key_1] (top is key_1)
        // Array is at depth + 1 from top (0-indexed)

        let array_handle = self.peek_operand_at(key_count as usize)?;

        let mut keys = Vec::with_capacity(key_count as usize);
        for i in 0..key_count {
            // Peek keys from bottom to top to get them in order
            let key_handle = self.peek_operand_at((key_count - 1 - i) as usize)?;
            keys.push(key_handle);
        }

        let result = self.fetch_nested_dim(array_handle, &keys)?;
        self.operand_stack.push(result);
        Ok(())
    }

    /// Execute UnsetNestedDim operation: unset($array[$k1][$k2]..[$kN])
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - nested array unset
    #[inline]
    pub(crate) fn exec_unset_nested_dim(&mut self, key_count: u8) -> Result<(), VmError> {
        // Stack: [array, key_n, ..., key_1] (top is key_1)
        // Similar to FetchNestedDim but modifies the array
        let keys = self.pop_n_operands(key_count as usize)?;
        let array_handle = self.pop_operand_required()?;

        let new_handle = self.unset_nested_dim(array_handle, &keys)?;
        self.operand_stack.push(new_handle);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::value::ArrayKey;
    use crate::runtime::context::EngineContext;
    use std::sync::Arc;

    #[test]
    fn test_init_array() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        vm.exec_init_array(0).unwrap();

        let handle = vm.operand_stack.pop().unwrap();
        let val = vm.arena.get(handle);
        assert!(matches!(&val.value, Val::Array(data) if data.map.is_empty()));
    }

    #[test]
    fn test_assign_dim_simple() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // For exec_assign_dim to work, we need a proper frame for constants
        // Instead, test the lower-level functionality directly
        let array = vm.arena.alloc(Val::Array(Rc::new(ArrayData::new())));
        vm.arena.get_mut(array).is_ref = true; // Mark as reference

        let key = vm.arena.alloc(Val::String(b"name".to_vec().into()));
        let value = vm.arena.alloc(Val::String(b"Alice".to_vec().into()));

        // Use assign_dim directly
        vm.assign_dim(array, key, value).unwrap();

        // Should push the result
        let result = vm.operand_stack.pop().unwrap();
        assert_eq!(result, array); // Same handle since it's a reference

        // Verify the value was stored
        let array_val = vm.arena.get(result);
        if let Val::Array(data) = &array_val.value {
            let stored = data
                .map
                .get(&ArrayKey::Str(b"name".to_vec().into()))
                .unwrap();
            let stored_val = vm.arena.get(*stored);
            assert!(matches!(stored_val.value, Val::String(ref s) if s.as_ref() == b"Alice"));
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_store_dim_with_integer_key() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // Create array and mark as reference for in-place modification
        let array_handle = vm.arena.alloc(Val::Array(Rc::new(ArrayData::new())));
        vm.arena.get_mut(array_handle).is_ref = true;

        let key = vm.arena.alloc(Val::Int(0));
        let value = vm.arena.alloc(Val::Int(42));

        // Stack order for StoreDim: [array, key, val]
        vm.operand_stack.push(array_handle);
        vm.operand_stack.push(key);
        vm.operand_stack.push(value);

        vm.exec_store_dim().unwrap();

        // Should push the array handle back
        let result = vm.operand_stack.pop().unwrap();
        assert_eq!(result, array_handle);

        // Verify the value was stored
        let array_val = vm.arena.get(result);
        if let Val::Array(data) = &array_val.value {
            let stored = data.map.get(&ArrayKey::Int(0)).unwrap();
            let stored_val = vm.arena.get(*stored);
            assert!(matches!(stored_val.value, Val::Int(42)));
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_append_array() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // Create array and mark as reference
        let array_handle = vm.arena.alloc(Val::Array(Rc::new(ArrayData::new())));
        vm.arena.get_mut(array_handle).is_ref = true;

        let value1 = vm.arena.alloc(Val::String(b"first".to_vec().into()));
        let value2 = vm.arena.alloc(Val::String(b"second".to_vec().into()));

        // Append first value
        vm.operand_stack.push(array_handle);
        vm.operand_stack.push(value1);
        vm.exec_append_array().unwrap();

        // Append second value
        let result1 = vm.operand_stack.pop().unwrap();
        vm.operand_stack.push(result1);
        vm.operand_stack.push(value2);
        vm.exec_append_array().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let array_val = vm.arena.get(result);

        if let Val::Array(data) = &array_val.value {
            assert_eq!(data.map.len(), 2);

            // Check keys are 0 and 1
            assert!(data.map.contains_key(&ArrayKey::Int(0)));
            assert!(data.map.contains_key(&ArrayKey::Int(1)));

            // Check values
            let val0 = vm.arena.get(*data.map.get(&ArrayKey::Int(0)).unwrap());
            let val1 = vm.arena.get(*data.map.get(&ArrayKey::Int(1)).unwrap());

            assert!(matches!(val0.value, Val::String(ref s) if s.as_ref() == b"first"));
            assert!(matches!(val1.value, Val::String(ref s) if s.as_ref() == b"second"));
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_append_array_maintains_next_index() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // Create array with explicit key 10 - need to properly compute next_free
        let val10 = vm.arena.alloc(Val::Int(10));
        let mut map = indexmap::IndexMap::new();
        map.insert(ArrayKey::Int(10), val10);

        // Use From trait to properly compute next_free
        let array_data = Rc::new(ArrayData::from(map));
        let array_handle = vm.arena.alloc(Val::Array(array_data));
        vm.arena.get_mut(array_handle).is_ref = true;

        // Append should use key 11
        let append_val = vm.arena.alloc(Val::String(b"appended".to_vec().into()));
        vm.operand_stack.push(array_handle);
        vm.operand_stack.push(append_val);
        vm.exec_append_array().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let array_val = vm.arena.get(result);

        if let Val::Array(data) = &array_val.value {
            // Should have keys 10 and 11
            assert!(data.map.contains_key(&ArrayKey::Int(10)));
            assert!(data.map.contains_key(&ArrayKey::Int(11)));
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_fetch_dim_string_key() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // Create array with string key
        let value = vm.arena.alloc(Val::Int(123));
        let mut array_data = Rc::new(ArrayData::new());
        Rc::make_mut(&mut array_data)
            .map
            .insert(ArrayKey::Str(b"test".to_vec().into()), value);

        let array_handle = vm.arena.alloc(Val::Array(array_data));
        let key_handle = vm.arena.alloc(Val::String(b"test".to_vec().into()));

        vm.operand_stack.push(array_handle);
        vm.operand_stack.push(key_handle);
        vm.exec_fetch_dim().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Int(123)));
    }

    #[test]
    fn test_fetch_dim_undefined_key_returns_null() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let array_handle = vm.arena.alloc(Val::Array(Rc::new(ArrayData::new())));
        let key_handle = vm.arena.alloc(Val::String(b"missing".to_vec().into()));

        vm.operand_stack.push(array_handle);
        vm.operand_stack.push(key_handle);

        // Should not panic - returns Null for undefined keys
        vm.exec_fetch_dim().unwrap();

        let result = vm.operand_stack.pop().unwrap();
        let result_val = vm.arena.get(result);
        assert!(matches!(result_val.value, Val::Null));
    }

    #[test]
    fn test_nested_dim_assign_two_levels() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // Create array and mark as reference
        let array_handle = vm.arena.alloc(Val::Array(Rc::new(ArrayData::new())));
        vm.arena.get_mut(array_handle).is_ref = true;

        let key1 = vm.arena.alloc(Val::String(b"outer".to_vec().into()));
        let key2 = vm.arena.alloc(Val::String(b"inner".to_vec().into()));
        let value = vm.arena.alloc(Val::Int(999));

        // Stack order for exec_assign_nested_dim: array (bottom), key1, key2, value (top)
        // pops: value, then pop_n_operands(2) gets [key2, key1], then array
        vm.operand_stack.push(array_handle);
        vm.operand_stack.push(key1);
        vm.operand_stack.push(key2);
        vm.operand_stack.push(value);

        vm.exec_assign_nested_dim(2).unwrap();

        // Verify nested structure
        let array_val = vm.arena.get(array_handle);
        if let Val::Array(outer_data) = &array_val.value {
            let inner_handle = outer_data
                .map
                .get(&ArrayKey::Str(b"outer".to_vec().into()))
                .expect("outer key exists");

            let inner_val = vm.arena.get(*inner_handle);
            if let Val::Array(inner_data) = &inner_val.value {
                let value_handle = inner_data
                    .map
                    .get(&ArrayKey::Str(b"inner".to_vec().into()))
                    .expect("inner key exists");

                let stored_val = vm.arena.get(*value_handle);
                assert!(matches!(stored_val.value, Val::Int(999)));
            } else {
                panic!("Expected inner array");
            }
        } else {
            panic!("Expected outer array");
        }
    }

    #[test]
    fn test_copy_on_write_semantics() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // Create array WITHOUT is_ref (copy-on-write)
        let original_array = vm.arena.alloc(Val::Array(Rc::new(ArrayData::new())));
        let key = vm.arena.alloc(Val::Int(0));
        let value = vm.arena.alloc(Val::String(b"modified".to_vec().into()));

        // Stack order for exec_store_dim: array (bottom), key, value (top)
        vm.operand_stack.push(original_array);
        vm.operand_stack.push(key);
        vm.operand_stack.push(value);

        vm.exec_store_dim().unwrap();

        let modified_array = vm.operand_stack.pop().unwrap();

        // Should be different handles due to copy-on-write
        assert_ne!(original_array, modified_array);

        // Original should still be empty
        let orig_val = vm.arena.get(original_array);
        if let Val::Array(data) = &orig_val.value {
            assert_eq!(data.map.len(), 0);
        }

        // Modified should have the value
        let mod_val = vm.arena.get(modified_array);
        if let Val::Array(data) = &mod_val.value {
            assert_eq!(data.map.len(), 1);
        }
    }
}
