use crate::builtins::pdo;
use crate::runtime::context::RequestContext;
use crate::runtime::extension::{Extension, ExtensionInfo, ExtensionResult};
use crate::runtime::registry::ExtensionRegistry;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
/// Extension-specific data for PDO module
#[derive(Debug, Default)]
pub struct PdoExtensionData {
    pub connections: HashMap<u64, Rc<RefCell<Box<dyn pdo::driver::PdoConnection>>>>,
    pub statements: HashMap<u64, Rc<RefCell<Box<dyn pdo::driver::PdoStatement>>>>,
}

/// PDO extension - PHP Data Objects
pub struct PdoExtension;

impl Extension for PdoExtension {
    fn info(&self) -> ExtensionInfo {
        ExtensionInfo {
            name: "pdo",
            version: "1.0.0",
            dependencies: &[],
        }
    }

    fn module_init(&self, registry: &mut ExtensionRegistry) -> ExtensionResult {
        pdo::register_pdo_extension_to_registry(registry);
        // PDO driver registry is now a global singleton, initialized on first use
        ExtensionResult::Success
    }

    fn module_shutdown(&self) -> ExtensionResult {
        ExtensionResult::Success
    }

    fn request_init(&self, context: &mut RequestContext) -> ExtensionResult {
        // Initialize per-request PDO connections and statements
        context.set_extension_data(PdoExtensionData::default());
        ExtensionResult::Success
    }

    fn request_shutdown(&self, _context: &mut RequestContext) -> ExtensionResult {
        ExtensionResult::Success
    }
}
