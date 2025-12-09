use super::context::RequestContext;
use super::registry::ExtensionRegistry;

/// Extension metadata and version information
#[derive(Debug, Clone)]
pub struct ExtensionInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub dependencies: &'static [&'static str],
}

/// Lifecycle hook results
#[derive(Debug)]
pub enum ExtensionResult {
    Success,
    Failure(String),
}

impl ExtensionResult {
    pub fn is_success(&self) -> bool {
        matches!(self, ExtensionResult::Success)
    }

    pub fn is_failure(&self) -> bool {
        matches!(self, ExtensionResult::Failure(_))
    }
}

/// Core extension trait - mirrors PHP's zend_module_entry lifecycle
///
/// # Lifecycle Hooks
///
/// - **MINIT** (`module_init`): Called once when extension is loaded (per worker in FPM)
/// - **MSHUTDOWN** (`module_shutdown`): Called once when engine is destroyed
/// - **RINIT** (`request_init`): Called at start of each request
/// - **RSHUTDOWN** (`request_shutdown`): Called at end of each request
///
/// # SAPI Models
///
/// | SAPI | MINIT/MSHUTDOWN | RINIT/RSHUTDOWN |
/// |------|-----------------|-----------------|
/// | CLI  | Once per script | Once per script |
/// | FPM  | Once per worker | Every request   |
///
pub trait Extension: Send + Sync {
    /// Extension metadata
    fn info(&self) -> ExtensionInfo;

    /// Module initialization (MINIT) - called once when extension is loaded
    ///
    /// Use for: registering functions, classes, constants at engine level.
    /// In FPM, this is called once per worker process and persists across requests.
    fn module_init(&self, _registry: &mut ExtensionRegistry) -> ExtensionResult {
        ExtensionResult::Success
    }

    /// Module shutdown (MSHUTDOWN) - called once when engine is destroyed
    ///
    /// Use for: cleanup of persistent resources allocated in MINIT.
    fn module_shutdown(&self) -> ExtensionResult {
        ExtensionResult::Success
    }

    /// Request initialization (RINIT) - called at start of each request
    ///
    /// Use for: per-request setup, initializing request-specific state.
    /// In FPM, this is called for every HTTP request.
    fn request_init(&self, _context: &mut RequestContext) -> ExtensionResult {
        ExtensionResult::Success
    }

    /// Request shutdown (RSHUTDOWN) - called at end of each request
    ///
    /// Use for: cleanup of request-specific resources.
    fn request_shutdown(&self, _context: &mut RequestContext) -> ExtensionResult {
        ExtensionResult::Success
    }
}
