//! Error message formatting utilities
//! 
//! Provides consistent error message generation following PHP error conventions.
//! Reference: $PHP_SRC_PATH/Zend/zend_exceptions.c - error message formatting

use crate::core::value::{Handle, Symbol, Visibility};
use crate::vm::engine::VM;

/// Member kind for visibility error messages
/// Reference: $PHP_SRC_PATH/Zend/zend_compile.c
#[derive(Debug, Clone, Copy)]
pub(crate) enum MemberKind {
    Constant,
    Method,
    Property,
}

impl VM {
    /// Get human-readable string for a symbol (for error messages)
    /// Reference: $PHP_SRC_PATH/Zend/zend_string.h - ZSTR_VAL
    #[inline]
    pub(crate) fn symbol_to_string(&self, sym: Symbol) -> String {
        self.context
            .interner
            .lookup(sym)
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_else(|| "Unknown".to_string())
    }

    /// Get human-readable string for an optional symbol
    #[inline]
    pub(crate) fn optional_symbol_to_string(&self, sym: Option<Symbol>, default: &str) -> String {
        sym.and_then(|s| self.context.interner.lookup(s))
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_else(|| default.to_string())
    }

    /// Get a human-readable type name for a value
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - zend_get_type_by_const
    pub(crate) fn get_type_name(&self, val_handle: Handle) -> String {
        let val = &self.arena.get(val_handle).value;
        match val {
            crate::core::value::Val::Null => "null".into(),
            crate::core::value::Val::Bool(_) => "bool".into(),
            crate::core::value::Val::Int(_) => "int".into(),
            crate::core::value::Val::Float(_) => "float".into(),
            crate::core::value::Val::String(_) => "string".into(),
            crate::core::value::Val::Array(_) => "array".into(),
            crate::core::value::Val::Object(payload_handle) => {
                format!("object({})", self.describe_object_class(*payload_handle))
            }
            crate::core::value::Val::Resource(_) => "resource".into(),
            _ => "unknown".into(),
        }
    }

    /// Describe an object's class for error messages
    /// Reference: $PHP_SRC_PATH/Zend/zend_objects_API.c
    pub(crate) fn describe_object_class(&self, payload_handle: Handle) -> String {
        if let crate::core::value::Val::ObjPayload(obj_data) = &self.arena.get(payload_handle).value {
            self.context
                .interner
                .lookup(obj_data.class)
                .map(|b| String::from_utf8_lossy(b))
                .unwrap_or_else(|| "Unknown".into())
                .into_owned()
        } else {
            "Invalid".into()
        }
    }

    /// Describe a handle for error messages
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c
    pub(crate) fn describe_handle(&self, handle: Handle) -> String {
        let val = self.arena.get(handle);
        match &val.value {
            crate::core::value::Val::Null => "null".into(),
            crate::core::value::Val::Bool(b) => format!("bool({})", b),
            crate::core::value::Val::Int(i) => format!("int({})", i),
            crate::core::value::Val::Float(f) => format!("float({})", f),
            crate::core::value::Val::String(s) => {
                let preview = if s.len() > 20 {
                    format!("{}...", String::from_utf8_lossy(&s[..20]))
                } else {
                    String::from_utf8_lossy(s).into_owned()
                };
                format!("string(\"{}\")", preview)
            }
            crate::core::value::Val::Array(data) => format!("array({})", data.map.len()),
            crate::core::value::Val::Object(h) => {
                format!("object({})", self.describe_object_class(*h))
            }
            crate::core::value::Val::Resource(_) => "resource".into(),
            _ => "unknown".into(),
        }
    }

    /// Format visibility error message
    /// Reference: $PHP_SRC_PATH/Zend/zend_compile.c - visibility error messages
    pub(crate) fn format_visibility_error(
        &self,
        defining_class: Symbol,
        visibility: Visibility,
        member_kind: MemberKind,
        member_name: Option<Symbol>,
    ) -> String {
        let class_str = self.symbol_to_string(defining_class);
        let member_str = self.optional_symbol_to_string(member_name, "unknown");

        let vis_str = match visibility {
            Visibility::Private => "private",
            Visibility::Protected => "protected",
            Visibility::Public => unreachable!(),
        };

        let (kind_str, separator) = match member_kind {
            MemberKind::Constant => ("constant", "::"),
            MemberKind::Method => ("method", "::"),
            MemberKind::Property => ("property", "::$"),
        };

        format!(
            "Cannot access {} {} {}{}{}",
            vis_str, kind_str, class_str, separator, member_str
        )
    }

    /// Format undefined method error
    /// Reference: $PHP_SRC_PATH/Zend/zend_exceptions.c
    pub(crate) fn format_undefined_method_error(&self, class: Symbol, method: Symbol) -> String {
        let class_str = self.symbol_to_string(class);
        let method_str = self.symbol_to_string(method);
        format!("Call to undefined method {}::{}", class_str, method_str)
    }

    /// Format type error message
    /// Reference: $PHP_SRC_PATH/Zend/zend_type_error.c
    pub(crate) fn format_type_error(&self, expected: &str, got_handle: Handle) -> String {
        let got = self.get_type_name(got_handle);
        format!("Expected {}, got {}", expected, got)
    }
}
