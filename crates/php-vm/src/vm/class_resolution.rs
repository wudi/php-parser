//! Class and object resolution utilities
//!
//! Provides efficient lookup and resolution of class members following inheritance chains.
//! Reference: $PHP_SRC_PATH/Zend/zend_inheritance.c, Zend/zend_API.c

use crate::compiler::chunk::UserFunc;
use crate::core::value::{Symbol, Visibility};
use crate::runtime::context::ClassDef;
use crate::vm::engine::{VmError, VM};
use std::rc::Rc;

/// Result of method lookup in inheritance chain
#[derive(Debug, Clone)]
pub(crate) struct MethodLookupResult {
    pub func: Rc<UserFunc>,
    pub visibility: Visibility,
    pub is_static: bool,
    pub defining_class: Symbol,
}

/// Result of property lookup in inheritance chain
#[derive(Debug, Clone)]
pub(crate) struct PropertyLookupResult {
    pub visibility: Visibility,
    pub defining_class: Symbol,
}

/// Result of constant lookup in inheritance chain
#[derive(Debug, Clone)]
pub(crate) struct ConstantLookupResult {
    pub value: crate::core::value::Val,
    pub visibility: Visibility,
    pub defining_class: Symbol,
}

impl VM {
    /// Walk inheritance chain and find first match
    /// Generic helper that reduces code duplication
    /// Reference: $PHP_SRC_PATH/Zend/zend_inheritance.c - do_inheritance
    pub(crate) fn walk_class_hierarchy<F, T>(&self, start_class: Symbol, predicate: F) -> Option<T>
    where
        F: FnMut(&ClassDef, Symbol) -> Option<T>,
    {
        self.walk_inheritance_chain(start_class, predicate)
    }

    /// Find method in class hierarchy with detailed result
    /// Reference: $PHP_SRC_PATH/Zend/zend_API.c - zend_std_get_method
    pub(crate) fn lookup_method(
        &self,
        class_name: Symbol,
        method_name: Symbol,
    ) -> Option<MethodLookupResult> {
        let (func, vis, is_static, defining_class) = self.find_method(class_name, method_name)?;
        Some(MethodLookupResult {
            func,
            visibility: vis,
            is_static,
            defining_class,
        })
    }

    /// Find property in class hierarchy
    /// Reference: $PHP_SRC_PATH/Zend/zend_object_handlers.c - zend_std_get_property_ptr_ptr
    pub(crate) fn lookup_property(
        &self,
        class_name: Symbol,
        prop_name: Symbol,
    ) -> Option<PropertyLookupResult> {
        self.walk_class_hierarchy(class_name, |def, defining_class| {
            def.properties
                .get(&prop_name)
                .map(|entry| PropertyLookupResult {
                    visibility: entry.visibility,
                    defining_class,
                })
        })
    }

    /// Find static property in class hierarchy
    /// Reference: $PHP_SRC_PATH/Zend/zend_compile.c - static property access
    pub(crate) fn lookup_static_property(
        &self,
        class_name: Symbol,
        prop_name: Symbol,
    ) -> Result<(crate::core::value::Val, Visibility, Symbol), VmError> {
        self.find_static_prop(class_name, prop_name)
    }

    /// Find class constant in hierarchy
    /// Reference: $PHP_SRC_PATH/Zend/zend_compile.c - constant access
    pub(crate) fn lookup_class_constant(
        &self,
        class_name: Symbol,
        const_name: Symbol,
    ) -> Result<ConstantLookupResult, VmError> {
        let (value, visibility, defining_class) =
            self.find_class_constant(class_name, const_name)?;
        Ok(ConstantLookupResult {
            value,
            visibility,
            defining_class,
        })
    }

    /// Check if a class exists
    #[inline]
    pub(crate) fn class_exists(&self, class_name: Symbol) -> bool {
        self.context.classes.contains_key(&class_name)
    }

    /// Get class definition
    #[inline]
    pub(crate) fn get_class_def(&self, class_name: Symbol) -> Option<&ClassDef> {
        self.context.classes.get(&class_name)
    }

    /// Resolve special class names (self, parent, static)
    /// Reference: $PHP_SRC_PATH/Zend/zend_compile.c - class name resolution
    pub(crate) fn resolve_special_class_name(&self, class_name: Symbol) -> Result<Symbol, VmError> {
        self.resolve_class_name(class_name)
    }

    /// Check if child is subclass of parent (including same class)
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - instanceof_function
    #[inline]
    pub(crate) fn is_subclass(&self, child: Symbol, parent: Symbol) -> bool {
        self.is_subclass_of(child, parent)
    }

    /// Get all parent classes in order (immediate parent first)
    /// Reference: Useful for reflection and debugging
    pub(crate) fn get_parent_chain(&self, class_name: Symbol) -> Vec<Symbol> {
        let mut chain = Vec::new();
        let mut current = self.get_class_def(class_name).and_then(|def| def.parent);

        while let Some(parent) = current {
            chain.push(parent);
            current = self.get_class_def(parent).and_then(|def| def.parent);
        }

        chain
    }

    /// Get all interfaces implemented by a class
    /// Reference: $PHP_SRC_PATH/Zend/zend_inheritance.c - interface checks
    pub(crate) fn get_implemented_interfaces(&self, class_name: Symbol) -> Vec<Symbol> {
        let mut interfaces = Vec::new();

        if let Some(def) = self.get_class_def(class_name) {
            interfaces.extend(def.interfaces.iter().copied());

            // Recursively collect from parent
            if let Some(parent) = def.parent {
                interfaces.extend(self.get_implemented_interfaces(parent));
            }
        }

        interfaces.dedup();
        interfaces
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::context::EngineContext;
    use std::sync::Arc;

    #[test]
    fn test_parent_chain() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        // Create simple class hierarchy: GrandParent -> Parent -> Child
        let grandparent_sym = vm.context.interner.intern(b"GrandParent");
        let parent_sym = vm.context.interner.intern(b"Parent");
        let child_sym = vm.context.interner.intern(b"Child");

        let grandparent_def = ClassDef {
            name: grandparent_sym,
            parent: None,
            is_interface: false,
            is_trait: false,
            is_abstract: false,
            is_enum: false,
            enum_backed_type: None,
            interfaces: Vec::new(),
            traits: Vec::new(),
            methods: std::collections::HashMap::new(),
            properties: indexmap::IndexMap::new(),
            constants: std::collections::HashMap::new(),
            static_properties: std::collections::HashMap::new(),
            abstract_methods: std::collections::HashSet::new(),
            allows_dynamic_properties: false,
        };
        vm.context.classes.insert(grandparent_sym, grandparent_def);

        let parent_def = ClassDef {
            name: parent_sym,
            parent: Some(grandparent_sym),
            is_interface: false,
            is_trait: false,
            is_abstract: false,
            is_enum: false,
            enum_backed_type: None,
            interfaces: Vec::new(),
            traits: Vec::new(),
            methods: std::collections::HashMap::new(),
            properties: indexmap::IndexMap::new(),
            constants: std::collections::HashMap::new(),
            static_properties: std::collections::HashMap::new(),
            abstract_methods: std::collections::HashSet::new(),
            allows_dynamic_properties: false,
        };
        vm.context.classes.insert(parent_sym, parent_def);

        let child_def = ClassDef {
            name: child_sym,
            parent: Some(parent_sym),
            is_interface: false,
            is_trait: false,
            is_abstract: false,
            is_enum: false,
            enum_backed_type: None,
            interfaces: Vec::new(),
            traits: Vec::new(),
            methods: std::collections::HashMap::new(),
            properties: indexmap::IndexMap::new(),
            constants: std::collections::HashMap::new(),
            static_properties: std::collections::HashMap::new(),
            abstract_methods: std::collections::HashSet::new(),
            allows_dynamic_properties: false,
        };
        vm.context.classes.insert(child_sym, child_def);

        let chain = vm.get_parent_chain(child_sym);
        assert_eq!(chain, vec![parent_sym, grandparent_sym]);
    }

    #[test]
    fn test_is_subclass() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);

        let parent_sym = vm.context.interner.intern(b"Parent");
        let child_sym = vm.context.interner.intern(b"Child");

        let parent_def = ClassDef {
            name: parent_sym,
            parent: None,
            is_interface: false,
            is_trait: false,
            is_abstract: false,
            is_enum: false,
            enum_backed_type: None,
            interfaces: Vec::new(),
            traits: Vec::new(),
            methods: std::collections::HashMap::new(),
            properties: indexmap::IndexMap::new(),
            constants: std::collections::HashMap::new(),
            static_properties: std::collections::HashMap::new(),
            abstract_methods: std::collections::HashSet::new(),
            allows_dynamic_properties: false,
        };
        vm.context.classes.insert(parent_sym, parent_def);

        let child_def = ClassDef {
            name: child_sym,
            parent: Some(parent_sym),
            is_interface: false,
            is_trait: false,
            is_abstract: false,
            is_enum: false,
            enum_backed_type: None,
            interfaces: Vec::new(),
            traits: Vec::new(),
            methods: std::collections::HashMap::new(),
            properties: indexmap::IndexMap::new(),
            constants: std::collections::HashMap::new(),
            static_properties: std::collections::HashMap::new(),
            abstract_methods: std::collections::HashSet::new(),
            allows_dynamic_properties: false,
        };
        vm.context.classes.insert(child_sym, child_def);

        assert!(vm.is_subclass(child_sym, parent_sym));
        assert!(vm.is_subclass(child_sym, child_sym)); // Class is subclass of itself
        assert!(!vm.is_subclass(parent_sym, child_sym));
    }
}
