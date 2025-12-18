//! ArrayAccess interface support
//! 
//! Implements PHP's ArrayAccess interface operations following Zend engine semantics.
//! Reference: $PHP_SRC_PATH/Zend/zend_execute.c - array access handlers

use crate::core::value::Handle;
use crate::vm::engine::{VM, VmError};

impl VM {
    /// Generic ArrayAccess method invoker
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - array access handlers
    #[inline]
    pub(crate) fn call_array_access_method(
        &mut self,
        obj_handle: Handle,
        method_name: &[u8],
        args: Vec<Handle>,
    ) -> Result<Option<Handle>, VmError> {
        let method_sym = self.context.interner.intern(method_name);
        let class_name = self.extract_object_class(obj_handle)?;
        
        let (user_func, _, _, defined_class) = self.find_method(class_name, method_sym)
            .ok_or_else(|| VmError::RuntimeError(
                format!("ArrayAccess::{} not found", String::from_utf8_lossy(method_name))
            ))?;
        
        self.invoke_user_method(obj_handle, user_func, args, defined_class, class_name)?;
        Ok(self.last_return_value.take())
    }

    /// Call ArrayAccess::offsetExists($offset)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_call_method
    #[inline]
    pub(crate) fn call_array_access_offset_exists(
        &mut self,
        obj_handle: Handle,
        offset_handle: Handle,
    ) -> Result<bool, VmError> {
        let result = self.call_array_access_method(obj_handle, b"offsetExists", vec![offset_handle])?
            .unwrap_or_else(|| self.arena.alloc(crate::core::value::Val::Null));
        Ok(self.arena.get(result).value.to_bool())
    }

    /// Call ArrayAccess::offsetGet($offset)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c
    #[inline]
    pub(crate) fn call_array_access_offset_get(
        &mut self,
        obj_handle: Handle,
        offset_handle: Handle,
    ) -> Result<Handle, VmError> {
        self.call_array_access_method(obj_handle, b"offsetGet", vec![offset_handle])?
            .ok_or_else(|| VmError::RuntimeError("offsetGet returned void".into()))
    }

    /// Call ArrayAccess::offsetSet($offset, $value)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c
    #[inline]
    pub(crate) fn call_array_access_offset_set(
        &mut self,
        obj_handle: Handle,
        offset_handle: Handle,
        value_handle: Handle,
    ) -> Result<(), VmError> {
        self.call_array_access_method(obj_handle, b"offsetSet", vec![offset_handle, value_handle])?;
        Ok(())
    }

    /// Call ArrayAccess::offsetUnset($offset)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c
    #[inline]
    pub(crate) fn call_array_access_offset_unset(
        &mut self,
        obj_handle: Handle,
        offset_handle: Handle,
    ) -> Result<(), VmError> {
        self.call_array_access_method(obj_handle, b"offsetUnset", vec![offset_handle])?;
        Ok(())
    }
}
