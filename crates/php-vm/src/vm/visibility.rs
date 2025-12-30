//! Visibility checking and access control
//!
//! Implements PHP visibility rules for class members (properties, methods, constants).
//! Following Zend engine semantics for public, protected, and private access.
//!
//! ## PHP Visibility Rules
//!
//! - **Public**: Accessible from anywhere
//! - **Protected**: Accessible from same class or subclass
//! - **Private**: Accessible only from defining class
//!
//! ## References
//!
//! - Zend: `$PHP_SRC_PATH/Zend/zend_compile.c` - zend_check_visibility
//! - PHP Manual: https://www.php.net/manual/en/language.oop5.visibility.php

use crate::core::value::{Symbol, Visibility};
use crate::vm::engine::{VmError, VM};
use crate::vm::error_formatting::MemberKind;

impl VM {
    /// Unified visibility check following Zend rules
    /// Reference: $PHP_SRC_PATH/Zend/zend_compile.c - zend_check_visibility
    #[inline(always)]
    pub(crate) fn is_visible_from(
        &self,
        defining_class: Symbol,
        visibility: Visibility,
        caller_scope: Option<Symbol>,
    ) -> bool {
        match visibility {
            Visibility::Public => true,
            Visibility::Protected => caller_scope
                .map(|scope| self.is_subclass_of(scope, defining_class))
                .unwrap_or(false),
            Visibility::Private => Some(defining_class) == caller_scope,
        }
    }

    /// Unified visibility checker for class members
    /// Reference: $PHP_SRC_PATH/Zend/zend_compile.c - visibility rules
    pub(crate) fn check_member_visibility(
        &self,
        defining_class: Symbol,
        visibility: Visibility,
        member_kind: MemberKind,
        member_name: Option<Symbol>,
    ) -> Result<(), VmError> {
        match visibility {
            Visibility::Public => Ok(()),
            Visibility::Protected | Visibility::Private => {
                let caller_scope = self.get_current_class();
                if self.is_visible_from(defining_class, visibility, caller_scope) {
                    Ok(())
                } else {
                    self.build_visibility_error(
                        defining_class,
                        visibility,
                        member_kind,
                        member_name,
                    )
                }
            }
        }
    }

    /// Build visibility error message
    fn build_visibility_error(
        &self,
        defining_class: Symbol,
        visibility: Visibility,
        member_kind: MemberKind,
        member_name: Option<Symbol>,
    ) -> Result<(), VmError> {
        let message =
            self.format_visibility_error(defining_class, visibility, member_kind, member_name);
        Err(VmError::RuntimeError(message))
    }

    /// Check if a constant is visible
    /// Reference: $PHP_SRC_PATH/Zend/zend_compile.c - constant access
    #[inline]
    pub(crate) fn check_const_visibility(
        &self,
        defining_class: Symbol,
        visibility: Visibility,
    ) -> Result<(), VmError> {
        self.check_member_visibility(defining_class, visibility, MemberKind::Constant, None)
    }

    /// Check if a method is visible
    /// Reference: $PHP_SRC_PATH/Zend/zend_compile.c - method access
    #[inline]
    pub(crate) fn check_method_visibility(
        &self,
        defining_class: Symbol,
        visibility: Visibility,
        method_name: Option<Symbol>,
    ) -> Result<(), VmError> {
        self.check_member_visibility(defining_class, visibility, MemberKind::Method, method_name)
    }

    /// Check if a method is visible to caller (returns bool, no error)
    /// Used for method listing/introspection
    #[inline]
    pub(crate) fn method_visible_to(
        &self,
        defining_class: Symbol,
        visibility: Visibility,
        caller_scope: Option<Symbol>,
    ) -> bool {
        self.is_visible_from(defining_class, visibility, caller_scope)
    }

    /// Check if a property is visible to caller (returns bool, no error)
    /// Used for property listing/introspection
    #[inline]
    pub(crate) fn property_visible_to(
        &self,
        defining_class: Symbol,
        visibility: Visibility,
        caller_scope: Option<Symbol>,
    ) -> bool {
        self.is_visible_from(defining_class, visibility, caller_scope)
    }

    /// Check property visibility with error on failure
    /// Reference: $PHP_SRC_PATH/Zend/zend_object_handlers.c - property access
    pub(crate) fn check_prop_visibility(
        &self,
        class_name: Symbol,
        prop_name: Symbol,
        current_scope: Option<Symbol>,
    ) -> Result<(), VmError> {
        // Find property in inheritance chain
        let found = self.walk_inheritance_chain(class_name, |def, cls| {
            def.properties.get(&prop_name).map(|entry| (entry.visibility, cls))
        });

        if let Some((vis, defined_class)) = found {
            if !self.is_visible_from(defined_class, vis, current_scope) {
                let class_bytes = self.context.interner.lookup(class_name).unwrap_or(b"");
                let prop_bytes = self.context.interner.lookup(prop_name).unwrap_or(b"");
                let class_str = String::from_utf8_lossy(class_bytes);
                let prop_str = String::from_utf8_lossy(prop_bytes);

                let vis_str = match vis {
                    Visibility::Public => "public",
                    Visibility::Protected => "protected",
                    Visibility::Private => "private",
                };

                return Err(VmError::RuntimeError(format!(
                    "Cannot access {} property {}::${}",
                    vis_str, class_str, prop_str
                )));
            }
            Ok(())
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::context::EngineContext;
    use std::sync::Arc;

    #[test]
    fn test_public_visibility_always_accessible() {
        let engine = Arc::new(EngineContext::new());
        let vm = VM::new(engine);

        let class_sym = Symbol(1);
        // Public is accessible from any scope
        assert!(vm.is_visible_from(class_sym, Visibility::Public, None));
        assert!(vm.is_visible_from(class_sym, Visibility::Public, Some(Symbol(99))));
    }

    #[test]
    fn test_protected_visibility_same_class() {
        let engine = Arc::new(EngineContext::new());
        let vm = VM::new(engine);

        let class_sym = Symbol(1);
        // Protected is accessible from same class
        assert!(vm.is_visible_from(class_sym, Visibility::Protected, Some(class_sym)));
        // Not accessible from outside
        assert!(!vm.is_visible_from(class_sym, Visibility::Protected, None));
    }

    #[test]
    fn test_private_visibility_same_class_only() {
        let engine = Arc::new(EngineContext::new());
        let vm = VM::new(engine);

        let class_sym = Symbol(1);
        // Private is only accessible from same class
        assert!(vm.is_visible_from(class_sym, Visibility::Private, Some(class_sym)));
        // Not accessible from anywhere else
        assert!(!vm.is_visible_from(class_sym, Visibility::Private, Some(Symbol(99))));
        assert!(!vm.is_visible_from(class_sym, Visibility::Private, None));
    }

    #[test]
    fn test_check_const_visibility_public() {
        let engine = Arc::new(EngineContext::new());
        let vm = VM::new(engine);

        let class_sym = Symbol(1);
        // Public const is always accessible
        assert!(vm
            .check_const_visibility(class_sym, Visibility::Public)
            .is_ok());
    }

    #[test]
    fn test_check_method_visibility_protected_from_outside() {
        let engine = Arc::new(EngineContext::new());
        let vm = VM::new(engine);

        let class_sym = Symbol(1);
        let method_sym = Symbol(2);

        // Protected method not accessible when no current class
        let result = vm.check_method_visibility(class_sym, Visibility::Protected, Some(method_sym));
        assert!(result.is_err());
    }
}
