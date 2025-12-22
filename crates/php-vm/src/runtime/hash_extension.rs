use crate::builtins::hash;
use crate::runtime::context::RequestContext;
use crate::runtime::extension::{Extension, ExtensionInfo, ExtensionResult};
use crate::runtime::registry::ExtensionRegistry;
use std::sync::Arc;

/// Hash extension - Cryptographic Hashing Functions
pub struct HashExtension;

impl Extension for HashExtension {
    fn info(&self) -> ExtensionInfo {
        ExtensionInfo {
            name: "hash",
            version: "1.0.0",
            dependencies: &[],
        }
    }

    fn module_init(&self, registry: &mut ExtensionRegistry) -> ExtensionResult {
        // Register Hash functions
        registry.register_function(b"hash", hash::php_hash);
        registry.register_function(b"hash_algos", hash::php_hash_algos);
        registry.register_function(b"hash_file", hash::php_hash_file);
        registry.register_function(b"hash_init", hash::php_hash_init);
        registry.register_function(b"hash_update", hash::php_hash_update);
        registry.register_function(b"hash_final", hash::php_hash_final);
        registry.register_function(b"hash_copy", hash::php_hash_copy);

        ExtensionResult::Success
    }

    fn module_shutdown(&self) -> ExtensionResult {
        ExtensionResult::Success
    }

    fn request_init(&self, context: &mut RequestContext) -> ExtensionResult {
        // Initialize hash registry and states for new request
        context.hash_registry = Some(Arc::new(hash::HashRegistry::new()));
        context.hash_states = Some(Default::default());
        ExtensionResult::Success
    }

    fn request_shutdown(&self, _context: &mut RequestContext) -> ExtensionResult {
        ExtensionResult::Success
    }
}
