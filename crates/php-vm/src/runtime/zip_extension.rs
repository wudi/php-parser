use super::context::RequestContext;
use super::extension::{Extension, ExtensionInfo, ExtensionResult};
use super::registry::ExtensionRegistry;
use crate::builtins::zip::{register_zip_extension_to_registry, ZipArchiveWrapper};
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use std::any::Any;

/// Extension-specific data for Zip module
#[derive(Debug, Default)]
pub struct ZipExtensionData {
    pub archives: HashMap<u64, Rc<RefCell<ZipArchiveWrapper>>>,
    pub resources: HashMap<u64, Rc<RefCell<ZipArchiveWrapper>>>,
    pub entries: HashMap<u64, (u64, usize)>,  // Maps entry_id -> (resource_id, entry_index)
}

pub struct ZipExtension;

impl Extension for ZipExtension {
    fn info(&self) -> ExtensionInfo {
        ExtensionInfo {
            name: "zip",
            version: "1.22.4",
            dependencies: &[],
        }
    }

    fn module_init(&self, registry: &mut ExtensionRegistry) -> ExtensionResult {
        register_zip_extension_to_registry(registry);
        ExtensionResult::Success
    }

    fn request_init(&self, context: &mut RequestContext) -> ExtensionResult {
        context.set_extension_data(ZipExtensionData::default());
        ExtensionResult::Success
    }

    fn request_shutdown(&self, _context: &mut RequestContext) -> ExtensionResult {
        ExtensionResult::Success
    }
}
