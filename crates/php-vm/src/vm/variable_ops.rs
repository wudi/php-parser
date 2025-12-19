//! Variable operations module
//!
//! Handles variable loading, storing, and reference management following PHP semantics.
//!
//! ## PHP Variable Semantics
//!
//! PHP variables are stored in symbol tables (local, global, superglobal):
//! - Variables are created on first assignment
//! - Undefined variables produce notices and return null
//! - References allow multiple names for same value
//! - Superglobals accessible from any scope
//!
//! ## Operations
//!
//! - **load_variable**: Fetch variable value (handles undefined gracefully)
//! - **load_variable_dynamic**: Runtime variable name resolution
//! - **load_variable_ref**: Create or fetch reference to variable
//! - **store_variable**: Assign value to variable (copy or reference)
//! - **unset_variable**: Remove variable from symbol table
//!
//! ## Reference Handling
//!
//! PHP references allow multiple variables to share the same value:
//! ```php
//! $a = &$b;  // $a and $b point to same zval
//! $a = 5;    // Both $a and $b now equal 5
//! ```
//!
//! Implementation:
//! - References marked with `is_ref=true` on the handle
//! - Copy-on-write for non-reference assignments
//! - Reference counting handled by arena
//!
//! ## Superglobals
//!
//! Special variables always in scope: $_GET, $_POST, $GLOBALS, etc.
//! Lazily initialized on first access.
//!
//! ## Performance
//!
//! - Load/Store: O(1) hash table lookup
//! - Dynamic load: O(n) string conversion + O(1) lookup
//! - Unset: O(1) removal
//!
//! ## References
//!
//! - Zend: `$PHP_SRC_PATH/Zend/zend_execute.c` - ZEND_FETCH_*/ZEND_ASSIGN_*
//! - Zend: `$PHP_SRC_PATH/Zend/zend_variables.c` - Variable management

use crate::core::value::{Handle, Symbol};
use crate::vm::engine::{ErrorLevel, VmError, VM};

impl VM {
    /// Load variable by symbol, handling superglobals and undefined variables
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_FETCH_*
    pub(crate) fn load_variable(&mut self, sym: Symbol) -> Result<Handle, VmError> {
        // Check local scope first
        let existing = self
            .frames
            .last()
            .and_then(|frame| frame.locals.get(&sym).copied());

        if let Some(handle) = existing {
            return Ok(handle);
        }

        // Check for superglobals
        if self.is_superglobal(sym) {
            if let Some(handle) = self.ensure_superglobal_handle(sym) {
                if let Some(frame) = self.frames.last_mut() {
                    frame.locals.entry(sym).or_insert(handle);
                }
                return Ok(handle);
            }
        }

        // Undefined variable - emit notice and return null
        self.report_undefined_variable(sym);
        Ok(self.new_null_handle())
    }

    /// Load variable dynamically (name computed at runtime)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_FETCH_*_VAR
    pub(crate) fn load_variable_dynamic(&mut self, name_handle: Handle) -> Result<Handle, VmError> {
        let name_bytes = self.value_to_string_bytes(name_handle)?;
        let sym = self.context.interner.intern(&name_bytes);
        self.load_variable(sym)
    }

    /// Load variable as reference, creating if undefined
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_FETCH_*_REF
    pub(crate) fn load_variable_ref(&mut self, sym: Symbol) -> Result<Handle, VmError> {
        // Bind superglobal if needed
        if self.is_superglobal(sym) {
            if let Some(handle) = self.ensure_superglobal_handle(sym) {
                if let Some(frame) = self.frames.last_mut() {
                    frame.locals.entry(sym).or_insert(handle);
                }
            }
        }

        let frame = self
            .frames
            .last_mut()
            .ok_or_else(|| VmError::RuntimeError("No active frame".into()))?;

        if let Some(&handle) = frame.locals.get(&sym) {
            if self.arena.get(handle).is_ref {
                Ok(handle)
            } else {
                // Convert to reference - clone for uniqueness
                let val = self.arena.get(handle).value.clone();
                let new_handle = self.arena.alloc(val);
                self.arena.get_mut(new_handle).is_ref = true;
                frame.locals.insert(sym, new_handle);
                Ok(new_handle)
            }
        } else {
            // Create undefined variable as null reference
            // Must create handle before getting frame again
            let handle = self.arena.alloc(crate::core::value::Val::Null);
            self.arena.get_mut(handle).is_ref = true;

            // Now we can safely insert into frame
            let frame = self
                .frames
                .last_mut()
                .ok_or_else(|| VmError::RuntimeError("No active frame".into()))?;
            frame.locals.insert(sym, handle);
            Ok(handle)
        }
    }

    /// Store value to variable
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_ASSIGN
    pub(crate) fn store_variable(
        &mut self,
        sym: Symbol,
        val_handle: Handle,
    ) -> Result<(), VmError> {
        // PHP 8.1+: Disallow writing to entire $GLOBALS array
        // Reference: https://www.php.net/manual/en/reserved.variables.globals.php
        if self.is_globals_symbol(sym) {
            return Err(VmError::RuntimeError(
                "Cannot re-assign $GLOBALS".into()
            ));
        }

        // Bind superglobal if needed
        if self.is_superglobal(sym) {
            if let Some(handle) = self.ensure_superglobal_handle(sym) {
                let frame = self
                    .frames
                    .last_mut()
                    .ok_or_else(|| VmError::RuntimeError("No active frame".into()))?;
                frame.locals.entry(sym).or_insert(handle);
            }
        }

        let frame = self
            .frames
            .last_mut()
            .ok_or_else(|| VmError::RuntimeError("No active frame".into()))?;

        // Check if target is a reference
        if let Some(&old_handle) = frame.locals.get(&sym) {
            if self.arena.get(old_handle).is_ref {
                // Assign to reference - update value in place
                let new_val = self.arena.get(val_handle).value.clone();
                self.arena.get_mut(old_handle).value = new_val;
                return Ok(());
            }
        }

        // Normal assignment - clone value to ensure value semantics
        let val = self.arena.get(val_handle).value.clone();
        let final_handle = self.arena.alloc(val);
        frame.locals.insert(sym, final_handle);

        Ok(())
    }

    /// Store value to dynamically named variable
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - variable variables
    pub(crate) fn store_variable_dynamic(
        &mut self,
        name_handle: Handle,
        val_handle: Handle,
    ) -> Result<(), VmError> {
        let name_bytes = self.value_to_string_bytes(name_handle)?;
        let sym = self.context.interner.intern(&name_bytes);
        self.store_variable(sym, val_handle)
    }

    /// Check if variable exists in current scope
    pub(crate) fn variable_exists(&self, sym: Symbol) -> bool {
        self.frames
            .last()
            .and_then(|frame| frame.locals.get(&sym))
            .is_some()
    }

    /// Unset a variable (remove from local scope)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_UNSET_VAR
    pub(crate) fn unset_variable(&mut self, sym: Symbol) -> Result<(), VmError> {
        // PHP 8.1+: Disallow unsetting $GLOBALS
        if self.is_globals_symbol(sym) {
            return Err(VmError::RuntimeError(
                "Cannot unset $GLOBALS".into()
            ));
        }

        let frame = self
            .frames
            .last_mut()
            .ok_or_else(|| VmError::RuntimeError("No active frame".into()))?;
        frame.locals.remove(&sym);
        Ok(())
    }

    /// Report undefined variable notice
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - undefined variable notice
    fn report_undefined_variable(&mut self, sym: Symbol) {
        if let Some(var_bytes) = self.context.interner.lookup(sym) {
            let var_name = String::from_utf8_lossy(var_bytes);
            let msg = format!("Undefined variable: ${}", var_name);
            self.report_error(ErrorLevel::Notice, &msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::chunk::CodeChunk;
    use crate::core::value::Val;
    use crate::runtime::context::EngineContext;
    use crate::vm::frame::CallFrame;
    use std::rc::Rc;
    use std::sync::Arc;

    fn setup_vm() -> VM {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // Push a frame to have an active scope
        let chunk = Rc::new(CodeChunk::default());
        let frame = CallFrame::new(chunk);
        vm.frames.push(frame);

        vm
    }

    #[test]
    fn test_load_store_variable() {
        let mut vm = setup_vm();
        let sym = vm.context.interner.intern(b"test_var");

        // Store a value
        let value = vm.new_int_handle(42);
        vm.store_variable(sym, value).unwrap();

        // Load it back
        let loaded = vm.load_variable(sym).unwrap();
        assert_eq!(vm.value_to_int(loaded), 42);
    }

    #[test]
    fn test_undefined_variable_returns_null() {
        let mut vm = setup_vm();
        let sym = vm.context.interner.intern(b"undefined_var");

        let result = vm.load_variable(sym).unwrap();
        let val = &vm.arena.get(result).value;
        assert!(matches!(val, Val::Null));
    }

    #[test]
    fn test_reference_variable() {
        let mut vm = setup_vm();
        let sym = vm.context.interner.intern(b"ref_var");

        // Load as reference (creates if undefined)
        let ref_handle = vm.load_variable_ref(sym).unwrap();
        assert!(vm.arena.get(ref_handle).is_ref);

        // Assign to the reference
        let new_value = vm.new_int_handle(99);
        vm.store_variable(sym, new_value).unwrap();

        // Original reference should be updated
        let val = &vm.arena.get(ref_handle).value;
        assert_eq!(val.to_int(), 99);
    }

    #[test]
    fn test_dynamic_variable() {
        let mut vm = setup_vm();

        // Create variable name at runtime
        let name_handle = vm.new_string_handle(b"dynamic".to_vec());
        let value_handle = vm.new_int_handle(123);

        vm.store_variable_dynamic(name_handle, value_handle)
            .unwrap();

        // Load it back
        let loaded = vm.load_variable_dynamic(name_handle).unwrap();
        assert_eq!(vm.value_to_int(loaded), 123);
    }

    #[test]
    fn test_variable_exists() {
        let mut vm = setup_vm();
        let sym = vm.context.interner.intern(b"exists_test");

        assert!(!vm.variable_exists(sym));

        let value = vm.new_int_handle(1);
        vm.store_variable(sym, value).unwrap();

        assert!(vm.variable_exists(sym));
    }

    #[test]
    fn test_unset_variable() {
        let mut vm = setup_vm();
        let sym = vm.context.interner.intern(b"to_unset");

        let value = vm.new_int_handle(1);
        vm.store_variable(sym, value).unwrap();
        assert!(vm.variable_exists(sym));

        vm.unset_variable(sym).unwrap();
        assert!(!vm.variable_exists(sym));
    }
}
