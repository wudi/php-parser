//! Value extraction helpers
//!
//! Provides convenient methods for extracting typed values from handles,
//! reducing boilerplate and improving error messages.
//!
//! ## Usage
//!
//! ```ignore
//! let class_sym = vm.extract_string_as_symbol(handle)?;
//! let (class, payload) = vm.extract_object_parts(obj_handle)?;
//! let key = vm.extract_array_key(key_handle)?;
//! ```

use crate::core::value::{Handle, Symbol, Val};
use crate::vm::engine::{VmError, VM};
use std::rc::Rc;

impl VM {
    /// Extract a string value and intern it as a symbol
    #[inline]
    pub(crate) fn extract_string_as_symbol(&mut self, handle: Handle) -> Result<Symbol, VmError> {
        match &self.arena.get(handle).value {
            Val::String(s) => Ok(self.context.interner.intern(s)),
            other => Err(VmError::type_error(
                "string",
                self.type_name(other),
                "symbol extraction",
            )),
        }
    }

    /// Extract object class and payload handle
    #[inline]
    pub(crate) fn extract_object_parts(&self, handle: Handle) -> Result<(Symbol, Handle), VmError> {
        match &self.arena.get(handle).value {
            Val::Object(payload_handle) => match &self.arena.get(*payload_handle).value {
                Val::ObjPayload(obj_data) => Ok((obj_data.class, *payload_handle)),
                _ => Err(VmError::runtime("Invalid object payload")),
            },
            _ => Err(VmError::runtime("Not an object")),
        }
    }

    /// Extract an array from a handle (for read-only operations)
    #[inline]
    pub(crate) fn extract_array(
        &self,
        handle: Handle,
    ) -> Result<&Rc<crate::core::value::ArrayData>, VmError> {
        match &self.arena.get(handle).value {
            Val::Array(arr) => Ok(arr),
            other => Err(VmError::type_error(
                "array",
                self.type_name(other),
                "array extraction",
            )),
        }
    }

    /// Extract integer value
    #[inline]
    pub(crate) fn extract_int(&self, handle: Handle) -> Result<i64, VmError> {
        match &self.arena.get(handle).value {
            Val::Int(i) => Ok(*i),
            other => Err(VmError::type_error(
                "int",
                self.type_name(other),
                "integer extraction",
            )),
        }
    }

    /// Extract string bytes
    #[inline]
    pub(crate) fn extract_string(&self, handle: Handle) -> Result<Rc<Vec<u8>>, VmError> {
        match &self.arena.get(handle).value {
            Val::String(s) => Ok(s.clone()),
            other => Err(VmError::type_error(
                "string",
                self.type_name(other),
                "string extraction",
            )),
        }
    }

    /// Get a human-readable type name for a value
    /// Note: For object class names, use the method from error_formatting.rs
    #[inline]
    pub(crate) fn type_name(&self, val: &Val) -> &'static str {
        match val {
            Val::Null => "null",
            Val::Bool(_) => "bool",
            Val::Int(_) => "int",
            Val::Float(_) => "float",
            Val::String(_) => "string",
            Val::Array(_) | Val::ConstArray(_) => "array",
            Val::Object(_) => "object",
            Val::ObjPayload(_) => "object",
            Val::Resource(_) => "resource",
            Val::AppendPlaceholder => "unknown",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::context::EngineContext;
    use std::sync::Arc;

    #[test]
    fn test_extract_int() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);
        let handle = vm.arena.alloc(Val::Int(42));

        assert_eq!(vm.extract_int(handle).unwrap(), 42);

        let str_handle = vm.arena.alloc(Val::String(b"not an int".to_vec().into()));
        assert!(vm.extract_int(str_handle).is_err());
    }

    #[test]
    fn test_type_name() {
        let engine = Arc::new(EngineContext::new());
        let vm = VM::new(engine);

        assert_eq!(vm.type_name(&Val::Null), "null");
        assert_eq!(vm.type_name(&Val::Int(42)), "int");
        assert_eq!(vm.type_name(&Val::Bool(true)), "bool");
        assert_eq!(
            vm.type_name(&Val::String(b"test".to_vec().into())),
            "string"
        );
    }
}
