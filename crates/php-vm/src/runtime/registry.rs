use super::context::{NativeHandler, RequestContext};
use super::extension::{Extension, ExtensionResult};
use crate::core::value::{Symbol, Val, Visibility};
use std::collections::HashMap;

/// Native class definition for extension-provided classes
#[derive(Debug, Clone)]
pub struct NativeClassDef {
    pub name: Vec<u8>,
    pub parent: Option<Vec<u8>>,
    pub interfaces: Vec<Vec<u8>>,
    pub methods: HashMap<Vec<u8>, NativeMethodEntry>,
    pub constants: HashMap<Vec<u8>, (Val, Visibility)>,
    pub constructor: Option<NativeHandler>,
}

/// Native method entry for extension-provided class methods
#[derive(Debug, Clone)]
pub struct NativeMethodEntry {
    pub handler: NativeHandler,
    pub visibility: Visibility,
    pub is_static: bool,
}

/// Extension registry - manages all loaded extensions and their registered components
///
/// This is stored in `EngineContext` and persists for the lifetime of the process
/// (or worker in FPM). It holds all extension-registered functions, classes, and constants.
pub struct ExtensionRegistry {
    /// Native function handlers (name -> handler)
    functions: HashMap<Vec<u8>, NativeHandler>,
    /// Native function by-ref argument indexes (name -> zero-based indexes)
    functions_by_ref: HashMap<Vec<u8>, Vec<usize>>,
    /// Native class definitions (name -> class def)
    classes: HashMap<Vec<u8>, NativeClassDef>,
    /// Registered extensions
    extensions: Vec<Box<dyn Extension>>,
    /// Extension name -> index mapping for fast lookup
    extension_map: HashMap<String, usize>,
    /// Engine-level constants (name -> value)
    constants: HashMap<Vec<u8>, Val>,
}

impl ExtensionRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            functions_by_ref: HashMap::new(),
            classes: HashMap::new(),
            extensions: Vec::new(),
            extension_map: HashMap::new(),
            constants: HashMap::new(),
        }
    }

    /// Register a native function handler
    ///
    /// Function names are stored as-is (case-sensitive in storage, but PHP lookups are case-insensitive)
    pub fn register_function(&mut self, name: &[u8], handler: NativeHandler) {
        self.functions.insert(name.to_vec(), handler);
    }

    /// Register a native function handler with by-ref argument positions.
    pub fn register_function_with_by_ref(
        &mut self,
        name: &[u8],
        handler: NativeHandler,
        by_ref: Vec<usize>,
    ) {
        self.functions.insert(name.to_vec(), handler);
        if !by_ref.is_empty() {
            self.functions_by_ref.insert(name.to_vec(), by_ref);
        }
    }

    /// Register a native class definition
    pub fn register_class(&mut self, class: NativeClassDef) {
        self.classes.insert(class.name.clone(), class);
    }

    /// Register an engine-level constant
    ///
    /// Constant names are stored as byte slices and later interned when needed.
    pub fn register_constant(&mut self, name: &[u8], value: Val) {
        self.constants.insert(name.to_vec(), value);
    }

    /// Get a function handler by name (case-insensitive lookup)
    pub fn get_function(&self, name: &[u8]) -> Option<NativeHandler> {
        // Try exact match first
        if let Some(&handler) = self.functions.get(name) {
            return Some(handler);
        }

        // Fallback to case-insensitive search
        let lower_name: Vec<u8> = name.iter().map(|b| b.to_ascii_lowercase()).collect();
        for (key, &handler) in &self.functions {
            let lower_key: Vec<u8> = key.iter().map(|b| b.to_ascii_lowercase()).collect();
            if lower_key == lower_name {
                return Some(handler);
            }
        }
        None
    }

    /// Get by-ref argument indexes for a function (case-insensitive lookup)
    pub fn get_function_by_ref(&self, name: &[u8]) -> Option<&[usize]> {
        if let Some(by_ref) = self.functions_by_ref.get(name) {
            return Some(by_ref.as_slice());
        }

        let lower_name: Vec<u8> = name.iter().map(|b| b.to_ascii_lowercase()).collect();
        for (key, by_ref) in &self.functions_by_ref {
            let lower_key: Vec<u8> = key.iter().map(|b| b.to_ascii_lowercase()).collect();
            if lower_key == lower_name {
                return Some(by_ref.as_slice());
            }
        }
        None
    }

    /// Get a class definition by name
    pub fn get_class(&self, name: &[u8]) -> Option<&NativeClassDef> {
        self.classes.get(name)
    }

    /// Get an engine-level constant by name (case-sensitive)
    pub fn get_constant(&self, name: &[u8]) -> Option<&Val> {
        self.constants.get(name)
    }

    /// Check if an extension is loaded
    pub fn extension_loaded(&self, name: &str) -> bool {
        self.extension_map.contains_key(name)
    }

    /// Get list of all loaded extension names
    pub fn get_extensions(&self) -> Vec<&str> {
        self.extension_map.keys().map(|s| s.as_str()).collect()
    }

    /// Register an extension and call its MINIT hook
    ///
    /// Returns an error if:
    /// - Extension with same name already registered
    /// - Dependencies are not satisfied
    /// - MINIT hook fails
    pub fn register_extension(&mut self, extension: Box<dyn Extension>) -> Result<(), String> {
        let info = extension.info();

        // Check if already registered
        if self.extension_map.contains_key(info.name) {
            return Err(format!("Extension '{}' is already registered", info.name));
        }

        // Check dependencies
        for &dep in info.dependencies {
            if !self.extension_map.contains_key(dep) {
                return Err(format!(
                    "Extension '{}' depends on '{}' which is not loaded",
                    info.name, dep
                ));
            }
        }

        // Call MINIT
        match extension.module_init(self) {
            ExtensionResult::Success => {
                let index = self.extensions.len();
                self.extension_map.insert(info.name.to_string(), index);
                self.extensions.push(extension);
                Ok(())
            }
            ExtensionResult::Failure(msg) => {
                Err(format!("Extension '{}' MINIT failed: {}", info.name, msg))
            }
        }
    }

    /// Invoke RINIT for all extensions
    pub fn invoke_request_init(&self, context: &mut RequestContext) -> Result<(), String> {
        for ext in &self.extensions {
            match ext.request_init(context) {
                ExtensionResult::Success => {}
                ExtensionResult::Failure(msg) => {
                    return Err(format!(
                        "Extension '{}' RINIT failed: {}",
                        ext.info().name,
                        msg
                    ));
                }
            }
        }
        Ok(())
    }

    /// Invoke RSHUTDOWN for all extensions (in reverse order)
    pub fn invoke_request_shutdown(&self, context: &mut RequestContext) -> Result<(), String> {
        for ext in self.extensions.iter().rev() {
            match ext.request_shutdown(context) {
                ExtensionResult::Success => {}
                ExtensionResult::Failure(msg) => {
                    return Err(format!(
                        "Extension '{}' RSHUTDOWN failed: {}",
                        ext.info().name,
                        msg
                    ));
                }
            }
        }
        Ok(())
    }

    /// Invoke MSHUTDOWN for all extensions (in reverse order)
    pub fn invoke_module_shutdown(&self) -> Result<(), String> {
        for ext in self.extensions.iter().rev() {
            match ext.module_shutdown() {
                ExtensionResult::Success => {}
                ExtensionResult::Failure(msg) => {
                    return Err(format!(
                        "Extension '{}' MSHUTDOWN failed: {}",
                        ext.info().name,
                        msg
                    ));
                }
            }
        }
        Ok(())
    }

    /// Get all registered functions (for backward compatibility)
    pub fn functions(&self) -> &HashMap<Vec<u8>, NativeHandler> {
        &self.functions
    }

    /// Get all registered constants
    pub fn constants(&self) -> &HashMap<Vec<u8>, Val> {
        &self.constants
    }

    /// Get all registered classes
    pub fn classes(&self) -> &HashMap<Vec<u8>, NativeClassDef> {
        &self.classes
    }
}

impl Default for ExtensionRegistry {
    fn default() -> Self {
        Self::new()
    }
}
