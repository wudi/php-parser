use crate::builtins::hash;
use crate::core::value::Val;
use crate::runtime::context::RequestContext;
use crate::runtime::extension::{Extension, ExtensionInfo, ExtensionResult};
use crate::runtime::registry::ExtensionRegistry;
use std::collections::HashMap;
use std::sync::Arc;

/// Extension-specific data for Hash module
pub struct HashExtensionData {
    pub registry: Arc<hash::HashRegistry>,
    pub states: HashMap<u64, Box<dyn hash::HashState>>,  // Use u64 for resource IDs
}

impl std::fmt::Debug for HashExtensionData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HashExtensionData")
            .field("registry", &"<HashRegistry>")
            .field("states", &format!("{} states", self.states.len()))
            .finish()
    }
}

impl Default for HashExtensionData {
    fn default() -> Self {
        Self {
            registry: Arc::new(hash::HashRegistry::new()),
            states: HashMap::new(),
        }
    }
}

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
        registry.register_function(b"hash_update_file", hash::php_hash_update_file);
        registry.register_function(b"hash_update_stream", hash::php_hash_update_stream);
        registry.register_function(b"hash_final", hash::php_hash_final);
        registry.register_function(b"hash_copy", hash::php_hash_copy);
        registry.register_function(b"hash_hmac", hash::hmac::php_hash_hmac);
        registry.register_function(b"hash_hmac_file", hash::hmac::php_hash_hmac_file);
        registry.register_function(b"hash_hmac_algos", hash::hmac::php_hash_hmac_algos);
        registry.register_function(b"hash_equals", hash::php_hash_equals);
        registry.register_function(b"hash_pbkdf2", hash::kdf::php_hash_pbkdf2);
        registry.register_function(b"hash_hkdf", hash::kdf::php_hash_hkdf);

        // Register Hash constants
        registry.register_constant(b"HASH_HMAC", Val::Int(1));

        ExtensionResult::Success
    }

    fn module_shutdown(&self) -> ExtensionResult {
        ExtensionResult::Success
    }

    fn request_init(&self, context: &mut RequestContext) -> ExtensionResult {
        // Initialize hash registry and states for new request
        context.set_extension_data(HashExtensionData {
            registry: Arc::new(hash::HashRegistry::new()),
            states: HashMap::new(),
        });
        ExtensionResult::Success
    }

    fn request_shutdown(&self, _context: &mut RequestContext) -> ExtensionResult {
        ExtensionResult::Success
    }
}
