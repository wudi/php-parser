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
/// # Invocation Order
///
/// **Startup (EngineBuilder::build)**:
/// 1. For each extension: MINIT (forward order)
///
/// **Request Start (RequestContext::new)**:
/// 1. Initialize builtin classes, constants
/// 2. For each extension: RINIT (forward order)
///
/// **Request End (RequestContext::drop)**:
/// 1. For each extension: RSHUTDOWN (reverse order, LIFO)
///
/// **Shutdown (EngineContext::drop)**:
/// 1. For each extension: MSHUTDOWN (reverse order, LIFO)
///
/// # Example Extension
///
/// ```rust,ignore
/// use crate::runtime::extension::{Extension, ExtensionInfo, ExtensionResult};
/// use std::any::TypeId;
/// use std::collections::HashMap;
///
/// // Per-request extension data
/// #[derive(Default)]
/// struct MyExtensionData {
///     counter: i64,
///     cache: HashMap<String, String>,
/// }
///
/// pub struct MyExtension;
///
/// impl Extension for MyExtension {
///     fn info(&self) -> ExtensionInfo {
///         ExtensionInfo {
///             name: "myext",
///             version: "1.0.0",
///             dependencies: &[],
///         }
///     }
///
///     fn module_init(&self, registry: &mut ExtensionRegistry) -> ExtensionResult {
///         // Called once per worker - register functions/constants
///         registry.register_function("my_function", my_function_impl);
///         ExtensionResult::Success
///     }
///
///     fn request_init(&self, context: &mut RequestContext) -> ExtensionResult {
///         // Called per request - initialize extension data
///         context.set_extension_data::<MyExtensionData>(
///             TypeId::of::<MyExtensionData>(),
///             Box::new(MyExtensionData::default())
///         );
///         ExtensionResult::Success
///     }
///
///     fn request_shutdown(&self, context: &mut RequestContext) -> ExtensionResult {
///         // Called per request - cleanup resources (automatically dropped)
///         ExtensionResult::Success
///     }
///
///     fn module_shutdown(&self) -> ExtensionResult {
///         // Called once at worker shutdown - cleanup persistent resources
///         ExtensionResult::Success
///     }
/// }
/// ```
///
/// # Thread Safety
///
/// This trait does NOT require `Send + Sync`. Extensions are used within single-threaded
/// execution contexts:
/// - **CLI SAPI**: Extensions are created and used on the main thread
/// - **FPM SAPI**: Each worker thread owns its extensions; never shared across threads
/// - **Async Tasks**: tokio `spawn_local` ensures tasks run on the same thread
///
/// This allows extensions to safely use `Rc`, `RefCell`, and other !Send types for
/// internal state management.
///
pub trait Extension {
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
