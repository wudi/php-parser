use crate::builtins::mysqli;
use crate::runtime::context::RequestContext;
use crate::runtime::extension::{Extension, ExtensionInfo, ExtensionResult};
use crate::runtime::registry::ExtensionRegistry;
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

/// Extension-specific data for MySQLi module
///
/// Note: In the future, resources (connections, results) will be stored in
/// RequestContext::resource_manager for unified, type-safe resource handling.
#[derive(Debug, Default)]
pub struct MysqliExtensionData {
    pub connections: HashMap<u64, Rc<RefCell<mysqli::MysqliConnection>>>,
    pub results: HashMap<u64, Rc<RefCell<mysqli::result::MysqliResult>>>,
}

/// MySQLi extension - MySQL Improved Extension
pub struct MysqliExtension;

impl Extension for MysqliExtension {
    fn info(&self) -> ExtensionInfo {
        ExtensionInfo {
            name: "mysqli",
            version: "1.0.0",
            dependencies: &[],
        }
    }

    fn module_init(&self, registry: &mut ExtensionRegistry) -> ExtensionResult {
        // Register MySQLi functions
        registry.register_function(b"mysqli_connect", mysqli::php_mysqli_connect);
        registry.register_function(b"mysqli_close", mysqli::php_mysqli_close);
        registry.register_function(b"mysqli_query", mysqli::php_mysqli_query);
        registry.register_function(b"mysqli_fetch_assoc", mysqli::php_mysqli_fetch_assoc);
        registry.register_function(b"mysqli_fetch_row", mysqli::php_mysqli_fetch_row);
        registry.register_function(b"mysqli_num_rows", mysqli::php_mysqli_num_rows);
        registry.register_function(b"mysqli_affected_rows", mysqli::php_mysqli_affected_rows);
        registry.register_function(b"mysqli_error", mysqli::php_mysqli_error);
        registry.register_function(b"mysqli_errno", mysqli::php_mysqli_errno);

        ExtensionResult::Success
    }

    fn module_shutdown(&self) -> ExtensionResult {
        ExtensionResult::Success
    }

    fn request_init(&self, context: &mut RequestContext) -> ExtensionResult {
        // Initialize MySQLi connections and results for new request
        context.set_extension_data(MysqliExtensionData::default());
        ExtensionResult::Success
    }

    fn request_shutdown(&self, _context: &mut RequestContext) -> ExtensionResult {
        // Cleanup is handled automatically by Drop on RequestContext
        ExtensionResult::Success
    }
}
