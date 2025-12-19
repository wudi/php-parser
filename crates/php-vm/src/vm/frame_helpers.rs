//! Frame operation helpers
//!
//! Provides convenient methods for common frame operations,
//! ensuring consistency and reducing code duplication.

use crate::compiler::chunk::UserFunc;
use crate::vm::engine::VM;
use crate::vm::frame::{ArgList, CallFrame};
use crate::core::value::{Handle, Symbol};
use std::rc::Rc;

impl VM {
    /// Create and push a function frame (no class scope)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c
    #[inline]
    pub(crate) fn create_function_frame(&mut self, func: Rc<UserFunc>, args: ArgList) {
        let mut frame = CallFrame::new(func.chunk.clone());
        frame.func = Some(func);
        frame.args = args;
        self.push_frame(frame);
    }

    /// Create and push a method frame
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_execute_data initialization
    #[inline]
    pub(crate) fn create_method_frame(
        &mut self,
        func: Rc<UserFunc>,
        this: Option<Handle>,
        class_scope: Symbol,
        called_scope: Symbol,
        args: ArgList,
    ) {
        let mut frame = CallFrame::new(func.chunk.clone());
        frame.func = Some(func);
        frame.this = this;
        frame.class_scope = Some(class_scope);
        frame.called_scope = Some(called_scope);
        frame.args = args;
        self.push_frame(frame);
    }

    /// Get current class scope from active frame
    #[inline]
    pub(crate) fn current_class_scope(&self) -> Option<Symbol> {
        self.frames.last().and_then(|f| f.class_scope)
    }

    /// Get current called scope from active frame
    #[inline]
    pub(crate) fn current_called_scope(&self) -> Option<Symbol> {
        self.frames.last().and_then(|f| f.called_scope)
    }

    /// Get current $this handle from active frame
    #[inline]
    pub(crate) fn current_this(&self) -> Option<Handle> {
        self.frames.last().and_then(|f| f.this)
    }

    /// Get frame stack depth
    #[inline]
    pub(crate) fn frame_depth(&self) -> usize {
        self.frames.len()
    }

    /// Check if we're in the global scope (frame 0)
    #[inline]
    pub(crate) fn is_global_scope(&self) -> bool {
        self.frames.len() <= 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::chunk::CodeChunk;
    use crate::runtime::context::EngineContext;
    use std::sync::Arc;

    #[test]
    fn test_frame_depth() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);
        
        assert_eq!(vm.frame_depth(), 0);
        assert!(vm.is_global_scope()); // 0 frames = global scope
        
        let chunk = Rc::new(CodeChunk::default());
        let frame = CallFrame::new(chunk);
        vm.push_frame(frame);
        
        assert_eq!(vm.frame_depth(), 1);
        assert!(vm.is_global_scope()); // 1 frame = still global scope per implementation
        
        // Add another frame to exit global scope
        let chunk2 = Rc::new(CodeChunk::default());
        let frame2 = CallFrame::new(chunk2);
        vm.push_frame(frame2);
        
        assert_eq!(vm.frame_depth(), 2);
        assert!(!vm.is_global_scope()); // 2+ frames = not global scope
    }

    #[test]
    fn test_current_scopes() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);
        
        let chunk = Rc::new(CodeChunk::default());
        let class_sym = Symbol(1);
        let called_sym = Symbol(2);
        
        let mut frame = CallFrame::new(chunk);
        frame.class_scope = Some(class_sym);
        frame.called_scope = Some(called_sym);
        vm.push_frame(frame);
        
        assert_eq!(vm.current_class_scope(), Some(class_sym));
        assert_eq!(vm.current_called_scope(), Some(called_sym));
    }
}
