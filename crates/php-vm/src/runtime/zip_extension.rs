use super::context::RequestContext;
use super::extension::{Extension, ExtensionInfo, ExtensionResult};
use super::registry::ExtensionRegistry;
use crate::builtins::zip::register_zip_extension_to_registry;

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

    fn request_init(&self, _context: &mut RequestContext) -> ExtensionResult {
        ExtensionResult::Success
    }

    fn request_shutdown(&self, _context: &mut RequestContext) -> ExtensionResult {
        ExtensionResult::Success
    }
}
