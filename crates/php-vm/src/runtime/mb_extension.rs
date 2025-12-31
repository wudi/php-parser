use crate::builtins::mbstring;
use crate::core::value::Val;
use crate::runtime::context::RequestContext;
use crate::runtime::extension::{Extension, ExtensionInfo, ExtensionResult};
use crate::runtime::registry::ExtensionRegistry;
use std::rc::Rc;

pub struct MbStringExtension;

impl Extension for MbStringExtension {
    fn info(&self) -> ExtensionInfo {
        ExtensionInfo {
            name: "mbstring",
            version: "8.5.0",
            dependencies: &[],
        }
    }

    fn module_init(&self, registry: &mut ExtensionRegistry) -> ExtensionResult {
        registry.register_function(b"mb_internal_encoding", mbstring::php_mb_internal_encoding);

        registry.register_constant(b"MB_CASE_UPPER", Val::Int(0));
        registry.register_constant(b"MB_CASE_LOWER", Val::Int(1));
        registry.register_constant(b"MB_CASE_TITLE", Val::Int(2));
        registry.register_constant(b"MB_CASE_FOLD", Val::Int(3));
        registry.register_constant(b"MB_CASE_LOWER_SIMPLE", Val::Int(4));
        registry.register_constant(b"MB_CASE_UPPER_SIMPLE", Val::Int(5));
        registry.register_constant(b"MB_CASE_TITLE_SIMPLE", Val::Int(6));
        registry.register_constant(b"MB_CASE_FOLD_SIMPLE", Val::Int(7));
        registry.register_constant(
            b"MB_ONIGURUMA_VERSION",
            Val::String(Rc::new(b"0.0.0".to_vec())),
        );

        ExtensionResult::Success
    }

    fn request_init(&self, context: &mut RequestContext) -> ExtensionResult {
        context.set_extension_data(crate::runtime::mb::state::MbStringState::default());
        ExtensionResult::Success
    }
}
