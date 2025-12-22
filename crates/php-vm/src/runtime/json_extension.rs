use crate::builtins::json;
use crate::runtime::context::RequestContext;
use crate::runtime::extension::{Extension, ExtensionInfo, ExtensionResult};
use crate::runtime::registry::ExtensionRegistry;

/// JSON extension - RFC 8259 compliant JSON encoding/decoding
///
/// This extension provides PHP's core JSON functionality:
/// - `json_encode()` - Encode PHP values to JSON
/// - `json_decode()` - Decode JSON to PHP values
/// - `json_last_error()` - Get last error code
/// - `json_last_error_msg()` - Get last error message
/// - `json_validate()` - Fast syntax validation (PHP 8.3+)
///
/// # Constants
///
/// Error codes:
/// - `JSON_ERROR_NONE`, `JSON_ERROR_DEPTH`, `JSON_ERROR_STATE_MISMATCH`
/// - `JSON_ERROR_CTRL_CHAR`, `JSON_ERROR_SYNTAX`, `JSON_ERROR_UTF8`
/// - `JSON_ERROR_RECURSION`, `JSON_ERROR_INF_OR_NAN`, `JSON_ERROR_UNSUPPORTED_TYPE`
/// - `JSON_ERROR_INVALID_PROPERTY_NAME`, `JSON_ERROR_UTF16`
///
/// Options:
/// - `JSON_HEX_TAG`, `JSON_HEX_AMP`, `JSON_HEX_APOS`, `JSON_HEX_QUOT`
/// - `JSON_FORCE_OBJECT`, `JSON_NUMERIC_CHECK`, `JSON_UNESCAPED_SLASHES`
/// - `JSON_PRETTY_PRINT`, `JSON_UNESCAPED_UNICODE`, `JSON_PARTIAL_OUTPUT_ON_ERROR`
/// - `JSON_PRESERVE_ZERO_FRACTION`, `JSON_UNESCAPED_LINE_TERMINATORS`
/// - `JSON_THROW_ON_ERROR`, `JSON_INVALID_UTF8_IGNORE`
pub struct JsonExtension;

impl Extension for JsonExtension {
    fn info(&self) -> ExtensionInfo {
        ExtensionInfo {
            name: "json",
            version: "1.5.0",
            dependencies: &[],
        }
    }

    fn module_init(&self, registry: &mut ExtensionRegistry) -> ExtensionResult {
        // Register JSON functions
        registry.register_function(b"json_encode", json::php_json_encode);
        registry.register_function(b"json_decode", json::php_json_decode);
        registry.register_function(b"json_last_error", json::php_json_last_error);
        registry.register_function(b"json_last_error_msg", json::php_json_last_error_msg);
        registry.register_function(b"json_validate", json::php_json_validate);

        // Register JSON error constants
        use crate::core::value::Val;

        registry.register_constant(b"JSON_ERROR_NONE", Val::Int(0));
        registry.register_constant(b"JSON_ERROR_DEPTH", Val::Int(1));
        registry.register_constant(b"JSON_ERROR_STATE_MISMATCH", Val::Int(2));
        registry.register_constant(b"JSON_ERROR_CTRL_CHAR", Val::Int(3));
        registry.register_constant(b"JSON_ERROR_SYNTAX", Val::Int(4));
        registry.register_constant(b"JSON_ERROR_UTF8", Val::Int(5));
        registry.register_constant(b"JSON_ERROR_RECURSION", Val::Int(6));
        registry.register_constant(b"JSON_ERROR_INF_OR_NAN", Val::Int(7));
        registry.register_constant(b"JSON_ERROR_UNSUPPORTED_TYPE", Val::Int(8));
        registry.register_constant(b"JSON_ERROR_INVALID_PROPERTY_NAME", Val::Int(9));
        registry.register_constant(b"JSON_ERROR_UTF16", Val::Int(10));

        // Register JSON option constants
        registry.register_constant(b"JSON_HEX_TAG", Val::Int(1));
        registry.register_constant(b"JSON_HEX_AMP", Val::Int(2));
        registry.register_constant(b"JSON_HEX_APOS", Val::Int(4));
        registry.register_constant(b"JSON_HEX_QUOT", Val::Int(8));
        registry.register_constant(b"JSON_FORCE_OBJECT", Val::Int(16));
        registry.register_constant(b"JSON_NUMERIC_CHECK", Val::Int(32));
        registry.register_constant(b"JSON_UNESCAPED_SLASHES", Val::Int(64));
        registry.register_constant(b"JSON_PRETTY_PRINT", Val::Int(128));
        registry.register_constant(b"JSON_UNESCAPED_UNICODE", Val::Int(256));
        registry.register_constant(b"JSON_PARTIAL_OUTPUT_ON_ERROR", Val::Int(512));
        registry.register_constant(b"JSON_PRESERVE_ZERO_FRACTION", Val::Int(1024));
        registry.register_constant(b"JSON_UNESCAPED_LINE_TERMINATORS", Val::Int(2048));
        registry.register_constant(b"JSON_THROW_ON_ERROR", Val::Int(4096));
        registry.register_constant(b"JSON_INVALID_UTF8_IGNORE", Val::Int(1048576));

        ExtensionResult::Success
    }

    fn module_shutdown(&self) -> ExtensionResult {
        ExtensionResult::Success
    }

    fn request_init(&self, context: &mut RequestContext) -> ExtensionResult {
        // Reset JSON error state for new request
        context.json_last_error = json::JsonError::None;
        ExtensionResult::Success
    }

    fn request_shutdown(&self, _context: &mut RequestContext) -> ExtensionResult {
        ExtensionResult::Success
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::value::Val;
    use crate::runtime::context::EngineBuilder;

    #[test]
    fn test_json_extension_registration() {
        // Build engine with JSON extension
        let engine = EngineBuilder::new()
            .with_extension(JsonExtension)
            .build()
            .expect("Failed to build engine with JSON extension");

        // Verify extension is loaded
        assert!(
            engine.registry.extension_loaded("json"),
            "JSON extension should be loaded"
        );

        // Verify functions are registered
        assert!(
            engine.registry.get_function(b"json_encode").is_some(),
            "json_encode should be registered"
        );
        assert!(
            engine.registry.get_function(b"json_decode").is_some(),
            "json_decode should be registered"
        );
        assert!(
            engine.registry.get_function(b"json_last_error").is_some(),
            "json_last_error should be registered"
        );
        assert!(
            engine
                .registry
                .get_function(b"json_last_error_msg")
                .is_some(),
            "json_last_error_msg should be registered"
        );
        assert!(
            engine.registry.get_function(b"json_validate").is_some(),
            "json_validate should be registered"
        );
    }

    #[test]
    fn test_json_constants_registered() {
        let engine = EngineBuilder::new()
            .with_extension(JsonExtension)
            .build()
            .expect("Failed to build engine");

        // Test error constants
        assert_eq!(
            engine.registry.get_constant(b"JSON_ERROR_NONE"),
            Some(&Val::Int(0))
        );
        assert_eq!(
            engine.registry.get_constant(b"JSON_ERROR_DEPTH"),
            Some(&Val::Int(1))
        );
        assert_eq!(
            engine.registry.get_constant(b"JSON_ERROR_SYNTAX"),
            Some(&Val::Int(4))
        );

        // Test option constants
        assert_eq!(
            engine.registry.get_constant(b"JSON_HEX_TAG"),
            Some(&Val::Int(1))
        );
        assert_eq!(
            engine.registry.get_constant(b"JSON_PRETTY_PRINT"),
            Some(&Val::Int(128))
        );
        assert_eq!(
            engine.registry.get_constant(b"JSON_THROW_ON_ERROR"),
            Some(&Val::Int(4096))
        );
    }

    #[test]
    fn test_json_request_init_clears_error() {
        let engine = EngineBuilder::new()
            .with_extension(JsonExtension)
            .build()
            .expect("Failed to build engine");

        let mut request_ctx = RequestContext::new(engine.clone());

        // Set an error
        request_ctx.json_last_error = json::JsonError::Depth;

        // Invoke RINIT
        engine
            .registry
            .invoke_request_init(&mut request_ctx)
            .expect("RINIT should succeed");

        // Verify error was reset
        assert_eq!(
            request_ctx.json_last_error.code(),
            0,
            "JSON error should be reset to None on request init"
        );
    }
}
