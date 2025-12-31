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
        registry.register_function(b"mb_detect_order", mbstring::php_mb_detect_order);
        registry.register_function(b"mb_language", mbstring::php_mb_language);
        registry.register_function(b"mb_get_info", mbstring::php_mb_get_info);
        registry.register_function(b"mb_convert_encoding", mbstring::php_mb_convert_encoding);
        registry.register_function(b"mb_convert_variables", mbstring::php_mb_convert_variables);
        registry.register_function(b"mb_detect_encoding", mbstring::php_mb_detect_encoding);
        registry.register_function(b"mb_check_encoding", mbstring::php_mb_check_encoding);
        registry.register_function(b"mb_scrub", mbstring::php_mb_scrub);
        registry.register_function(b"mb_strlen", mbstring::php_mb_strlen);
        registry.register_function(b"mb_substr", mbstring::php_mb_substr);
        registry.register_function(b"mb_strpos", mbstring::php_mb_strpos);
        registry.register_function(b"mb_strrpos", mbstring::php_mb_strrpos);
        registry.register_function(b"mb_strtolower", mbstring::php_mb_strtolower);
        registry.register_function(b"mb_strtoupper", mbstring::php_mb_strtoupper);
        registry.register_function(b"mb_convert_case", mbstring::php_mb_convert_case);
        registry.register_function(b"mb_strwidth", mbstring::php_mb_strwidth);
        registry.register_function(b"mb_strimwidth", mbstring::php_mb_strimwidth);
        registry.register_function(b"mb_trim", mbstring::php_mb_trim);
        registry.register_function(b"mb_str_split", mbstring::php_mb_str_split);
        registry.register_function(b"mb_str_pad", mbstring::php_mb_str_pad);
        registry.register_function(b"mb_substr_count", mbstring::php_mb_substr_count);
        registry.register_function(b"mb_strstr", mbstring::php_mb_strstr);
        registry.register_function(b"mb_chr", mbstring::php_mb_chr);
        registry.register_function(b"mb_ord", mbstring::php_mb_ord);
        registry.register_function(b"mb_ucfirst", mbstring::php_mb_ucfirst);
        registry.register_function(b"mb_lcfirst", mbstring::php_mb_lcfirst);
        registry.register_function(b"mb_http_output", mbstring::php_mb_http_output);
        registry.register_function(b"mb_http_input", mbstring::php_mb_http_input);
        registry.register_function(b"mb_list_encodings", mbstring::php_mb_list_encodings);
        registry.register_function(b"mb_encoding_aliases", mbstring::php_mb_encoding_aliases);
        registry.register_function(b"mb_substitute_character", mbstring::php_mb_substitute_character);

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
