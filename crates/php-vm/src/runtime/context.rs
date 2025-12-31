use crate::builtins::{hash, json};
use crate::compiler::chunk::UserFunc;
use crate::core::interner::Interner;
use crate::core::value::{Handle, Symbol, Val, Visibility};
use crate::runtime::extension::Extension;
use crate::runtime::registry::ExtensionRegistry;
use crate::runtime::resource_manager::ResourceManager;
use crate::vm::engine::VM;
use indexmap::IndexMap;
use std::any::{Any, TypeId};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

pub type NativeHandler = fn(&mut VM, args: &[Handle]) -> Result<Handle, String>;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeHint {
    Int,
    Float,
    String,
    Bool,
    Array,
    Object,
    Callable,
    Iterable,
    Mixed,
    Void,
    Never,
    Null,
    Class(Symbol),
    Union(Vec<TypeHint>),
    Intersection(Vec<TypeHint>),
}

#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub name: Symbol,
    pub type_hint: Option<TypeHint>,
    pub is_reference: bool,
    pub is_variadic: bool,
    pub default_value: Option<Val>,
}

#[derive(Debug, Clone)]
pub struct MethodSignature {
    pub parameters: Vec<ParameterInfo>,
    pub return_type: Option<TypeHint>,
}

#[derive(Debug, Clone)]
pub struct MethodEntry {
    pub name: Symbol,
    pub func: Rc<UserFunc>,
    pub visibility: Visibility,
    pub is_static: bool,
    pub declaring_class: Symbol,
    pub is_abstract: bool,
    pub signature: MethodSignature,
}

#[derive(Debug, Clone)]
pub struct NativeMethodEntry {
    pub name: Symbol,
    pub handler: NativeHandler,
    pub visibility: Visibility,
    pub is_static: bool,
    pub declaring_class: Symbol,
}

#[derive(Debug, Clone)]
pub struct PropertyEntry {
    pub default_value: Val,
    pub visibility: Visibility,
    pub type_hint: Option<TypeHint>,
    pub is_readonly: bool,
}

#[derive(Debug, Clone)]
pub struct StaticPropertyEntry {
    pub value: Val,
    pub visibility: Visibility,
    pub type_hint: Option<TypeHint>,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub name: Symbol,
    pub parent: Option<Symbol>,
    pub is_interface: bool,
    pub is_trait: bool,
    pub is_abstract: bool,
    pub is_enum: bool,
    pub enum_backed_type: Option<EnumBackedType>,
    pub interfaces: Vec<Symbol>,
    pub traits: Vec<Symbol>,
    pub methods: HashMap<Symbol, MethodEntry>,
    pub properties: IndexMap<Symbol, PropertyEntry>, // Instance properties with type hints
    pub constants: HashMap<Symbol, (Val, Visibility)>,
    pub static_properties: HashMap<Symbol, StaticPropertyEntry>, // Static properties with type hints
    pub abstract_methods: HashSet<Symbol>,
    pub allows_dynamic_properties: bool, // Set by #[AllowDynamicProperties] attribute
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnumBackedType {
    Int,
    String,
}

#[derive(Debug, Clone)]
pub struct HeaderEntry {
    pub key: Option<Vec<u8>>, // Normalized lowercase header name
    pub line: Vec<u8>,        // Original header line bytes
}

#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub error_type: i64,
    pub message: String,
    pub file: String,
    pub line: i64,
}

pub struct EngineContext {
    pub registry: ExtensionRegistry,
    /// PDO driver registry (set by PDO extension)
    pub pdo_driver_registry: Option<Arc<crate::builtins::pdo::drivers::DriverRegistry>>,
}

impl EngineContext {
    pub fn new() -> Self {
        let mut registry = ExtensionRegistry::new();

        // Register Core extension (MUST BE FIRST - contains all built-in functions)
        use crate::runtime::core_extension::CoreExtension;
        registry
            .register_extension(Box::new(CoreExtension))
            .expect("Failed to register Core extension");

        use crate::runtime::date_extension::DateExtension;
        registry
            .register_extension(Box::new(DateExtension))
            .expect("Failed to register Date extension");

        // Register Hash extension
        use crate::runtime::hash_extension::HashExtension;
        registry
            .register_extension(Box::new(HashExtension))
            .expect("Failed to register Hash extension");

        // Register JSON extension
        use crate::runtime::json_extension::JsonExtension;
        registry
            .register_extension(Box::new(JsonExtension))
            .expect("Failed to register JSON extension");

        // Register MySQLi extension
        use crate::runtime::mysqli_extension::MysqliExtension;
        registry
            .register_extension(Box::new(MysqliExtension))
            .expect("Failed to register MySQLi extension");

        // Register PDO extension
        use crate::runtime::pdo_extension::PdoExtension;
        registry
            .register_extension(Box::new(PdoExtension))
            .expect("Failed to register PDO extension");

        // Register Zlib extension
        use crate::runtime::zlib_extension::ZlibExtension;
        registry
            .register_extension(Box::new(ZlibExtension))
            .expect("Failed to register Zlib extension");

        // Register MBString extension
        use crate::runtime::mb_extension::MbStringExtension;
        registry
            .register_extension(Box::new(MbStringExtension))
            .expect("Failed to register mbstring extension");

        // Register Zip extension
        use crate::runtime::zip_extension::ZipExtension;
        registry
            .register_extension(Box::new(ZipExtension))
            .expect("Failed to register Zip extension");

        // Register OpenSSL extension
        use crate::runtime::openssl_extension::OpenSSLExtension;
        registry
            .register_extension(Box::new(OpenSSLExtension))
            .expect("Failed to register OpenSSL extension");

        Self {
            registry,
            pdo_driver_registry: Some(Arc::new(
                crate::builtins::pdo::drivers::DriverRegistry::new(),
            )),
        }
    }
}

pub struct RequestContext {
    pub engine: Arc<EngineContext>,
    pub globals: HashMap<Symbol, Handle>,
    pub constants: HashMap<Symbol, Val>,
    pub user_functions: HashMap<Symbol, Rc<UserFunc>>,
    pub classes: HashMap<Symbol, ClassDef>,
    pub included_files: HashSet<String>,
    pub autoloaders: Vec<Handle>,
    pub interner: Interner,
    pub error_reporting: u32,
    pub last_error: Option<ErrorInfo>,
    pub headers: Vec<HeaderEntry>,
    pub http_status: Option<i64>,
    pub max_execution_time: i64,
    pub native_methods: HashMap<(Symbol, Symbol), NativeMethodEntry>,
    pub json_last_error: json::JsonError,
    pub hash_registry: Option<Arc<hash::HashRegistry>>,
    pub hash_states: Option<HashMap<u64, Box<dyn hash::HashState>>>,
    pub next_resource_id: u64,
    pub mysqli_connections:
        HashMap<u64, Rc<std::cell::RefCell<crate::builtins::mysqli::MysqliConnection>>>,
    pub mysqli_results: HashMap<u64, Rc<std::cell::RefCell<crate::builtins::mysqli::MysqliResult>>>,
    pub pdo_connections:
        HashMap<u64, Rc<std::cell::RefCell<Box<dyn crate::builtins::pdo::driver::PdoConnection>>>>,
    pub pdo_statements:
        HashMap<u64, Rc<std::cell::RefCell<Box<dyn crate::builtins::pdo::driver::PdoStatement>>>>,
    pub zip_archives: HashMap<u64, Rc<std::cell::RefCell<crate::builtins::zip::ZipArchiveWrapper>>>,
    pub zip_resources:
        HashMap<u64, Rc<std::cell::RefCell<crate::builtins::zip::ZipArchiveWrapper>>>,
    pub zip_entries: HashMap<u64, (u64, usize)>,
    pub timezone: String,
    pub strtok_string: Option<Vec<u8>>,
    pub strtok_pos: usize,
    pub working_dir: Option<std::path::PathBuf>,
    /// Generic extension data storage keyed by TypeId
    pub extension_data: HashMap<TypeId, Box<dyn Any>>,
    /// Unified resource manager for type-safe resource handling
    pub resource_manager: ResourceManager,
}

impl RequestContext {
    pub fn new(engine: Arc<EngineContext>) -> Self {
        let mut ctx = Self {
            engine: Arc::clone(&engine),
            globals: HashMap::new(),
            constants: HashMap::new(),
            user_functions: HashMap::new(),
            classes: HashMap::new(),
            included_files: HashSet::new(),
            autoloaders: Vec::new(),
            interner: Interner::new(),
            error_reporting: 32767, // E_ALL
            last_error: None,
            headers: Vec::new(),
            http_status: None,
            max_execution_time: 30, // Default 30 seconds
            native_methods: HashMap::new(),
            json_last_error: json::JsonError::None, // Initialize JSON error state
            hash_registry: Some(Arc::new(hash::HashRegistry::new())), // Initialize hash registry
            hash_states: Some(HashMap::new()),      // Initialize hash states map
            next_resource_id: 1,                    // Start resource IDs from 1
            mysqli_connections: HashMap::new(),     // Initialize MySQLi connections
            mysqli_results: HashMap::new(),         // Initialize MySQLi results
            pdo_connections: HashMap::new(),        // Initialize PDO connections
            pdo_statements: HashMap::new(),         // Initialize PDO statements
            zip_archives: HashMap::new(),           // Initialize Zip archives
            zip_resources: HashMap::new(),          // Initialize Zip resources
            zip_entries: HashMap::new(),            // Initialize Zip entries
            timezone: "UTC".to_string(),            // Default timezone
            strtok_string: None,
            strtok_pos: 0,
            working_dir: None,
            extension_data: HashMap::new(), // Generic extension storage
            resource_manager: ResourceManager::new(), // Type-safe resource management
        };

        // OPTIMIZATION: Copy constants from extension registry in bulk
        // This is faster than calling register_builtin_constants() which re-interns
        // and re-inserts every constant individually.
        ctx.copy_engine_constants();

        // Materialize classes from extensions (all builtin classes now come from extensions)
        ctx.materialize_extension_classes();

        // Call RINIT for all extensions
        engine.registry.invoke_request_init(&mut ctx).ok();

        ctx
    }

    /// Copy constants from engine registry in bulk
    ///
    /// This is more efficient than calling `register_builtin_constants()` because:
    /// - Single iteration over engine.registry.constants()
    /// - Symbols are already interned in engine context
    /// - Values are already constructed (Rc-shared, cheap to clone)
    ///
    /// Performance: O(n) where n = number of engine constants
    fn copy_engine_constants(&mut self) {
        for (name, val) in self.engine.registry.constants() {
            let sym = self.interner.intern(name);
            self.constants.insert(sym, val.clone());
        }

        // Still need to register builtin constants that aren't in the registry
        self.register_builtin_constants();
    }

    fn materialize_extension_classes(&mut self) {
        let native_classes: Vec<_> = self.engine.registry.classes().values().cloned().collect();
        for native_class in native_classes {
            let class_sym = self.interner.intern(&native_class.name);
            let parent_sym = native_class
                .parent
                .as_ref()
                .map(|p| self.interner.intern(p));
            let mut interfaces = Vec::new();
            for iface in &native_class.interfaces {
                interfaces.push(self.interner.intern(iface));
            }

            let mut constants = HashMap::new();
            for (name, (val, visibility)) in &native_class.constants {
                constants.insert(self.interner.intern(name), (val.clone(), *visibility));
            }

            self.classes.insert(
                class_sym,
                ClassDef {
                    name: class_sym,
                    parent: parent_sym,
                    is_interface: native_class.is_interface,
                    is_trait: native_class.is_trait,
                    is_abstract: false,
                    is_enum: false,
                    enum_backed_type: None,
                    interfaces,
                    traits: Vec::new(),
                    methods: HashMap::new(),
                    properties: IndexMap::new(),
                    constants,
                    static_properties: HashMap::new(),
                    abstract_methods: HashSet::new(),
                    allows_dynamic_properties: true,
                },
            );

            for (name, native_method) in &native_class.methods {
                let method_sym = self.interner.intern(name);
                self.native_methods.insert(
                    (class_sym, method_sym),
                    NativeMethodEntry {
                        name: method_sym,
                        handler: native_method.handler,
                        visibility: native_method.visibility,
                        is_static: native_method.is_static,
                        declaring_class: class_sym,
                    },
                );
            }
        }
    }

    /// Get immutable reference to extension-specific data
    ///
    /// # Type Safety
    /// Each extension should use its own unique type (e.g., JsonExtensionData)
    /// to avoid collisions in the TypeId-based storage.
    ///
    /// # Returns
    /// - `Some(&T)` if data of type T exists
    /// - `None` if no data of type T has been stored
    pub fn get_extension_data<T: 'static>(&self) -> Option<&T> {
        self.extension_data
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<T>())
    }

    /// Get mutable reference to extension-specific data
    ///
    /// # Type Safety
    /// Each extension should use its own unique type (e.g., JsonExtensionData)
    /// to avoid collisions in the TypeId-based storage.
    ///
    /// # Returns
    /// - `Some(&mut T)` if data of type T exists
    /// - `None` if no data of type T has been stored
    pub fn get_extension_data_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.extension_data
            .get_mut(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_mut::<T>())
    }

    /// Store extension-specific data
    ///
    /// # Type Safety
    /// Each extension should use its own unique type (e.g., JsonExtensionData)
    /// to avoid collisions in the TypeId-based storage.
    ///
    /// # Example
    /// ```rust,ignore
    /// struct MyExtensionData {
    ///     counter: u32,
    /// }
    /// ctx.set_extension_data(MyExtensionData { counter: 0 });
    /// ```
    pub fn set_extension_data<T: 'static>(&mut self, data: T) {
        self.extension_data
            .insert(TypeId::of::<T>(), Box::new(data));
    }

    /// Get or initialize extension-specific data
    ///
    /// If data of type T does not exist, initialize it using the provided closure.
    /// Returns a mutable reference to the data (existing or newly initialized).
    ///
    /// # Type Safety
    /// Each extension should use its own unique type (e.g., JsonExtensionData)
    /// to avoid collisions in the TypeId-based storage.
    ///
    /// # Example
    /// ```rust,ignore
    /// let data = ctx.get_or_init_extension_data(|| MyExtensionData { counter: 0 });
    /// data.counter += 1;
    /// ```
    pub fn get_or_init_extension_data<T: 'static, F>(&mut self, init: F) -> &mut T
    where
        F: FnOnce() -> T,
    {
        self.extension_data
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(init()))
            .downcast_mut::<T>()
            .expect("TypeId mismatch in extension_data")
    }
}

impl RequestContext {
    /// Register core PHP constants that are not provided by extensions
    ///
    /// This method only registers fundamental PHP constants that must exist
    /// in every request context. Extension-specific constants (output control,
    /// URL parsing, date formats, string functions, etc.) are registered by
    /// their respective extensions via ExtensionRegistry.
    ///
    /// Core constants registered here:
    /// - PHP version info (PHP_VERSION, PHP_VERSION_ID, etc.)
    /// - System constants (PHP_OS, PHP_SAPI, PHP_EOL)
    /// - Path separators (DIRECTORY_SEPARATOR, PATH_SEPARATOR)
    /// - Error reporting levels (E_ERROR, E_WARNING, etc.)
    fn register_builtin_constants(&mut self) {
        // PHP version constants
        const PHP_VERSION_STR: &str = "8.2.0";
        const PHP_VERSION_ID_VALUE: i64 = 80200;
        const PHP_MAJOR: i64 = 8;
        const PHP_MINOR: i64 = 2;
        const PHP_RELEASE: i64 = 0;

        self.insert_builtin_constant(
            b"PHP_VERSION",
            Val::String(Rc::new(PHP_VERSION_STR.as_bytes().to_vec())),
        );
        self.insert_builtin_constant(b"PHP_VERSION_ID", Val::Int(PHP_VERSION_ID_VALUE));
        self.insert_builtin_constant(b"PHP_MAJOR_VERSION", Val::Int(PHP_MAJOR));
        self.insert_builtin_constant(b"PHP_MINOR_VERSION", Val::Int(PHP_MINOR));
        self.insert_builtin_constant(b"PHP_RELEASE_VERSION", Val::Int(PHP_RELEASE));
        self.insert_builtin_constant(b"PHP_EXTRA_VERSION", Val::String(Rc::new(Vec::new())));

        // System constants
        self.insert_builtin_constant(b"PHP_OS", Val::String(Rc::new(b"Darwin".to_vec())));
        self.insert_builtin_constant(b"PHP_SAPI", Val::String(Rc::new(b"cli".to_vec())));
        self.insert_builtin_constant(b"PHP_EOL", Val::String(Rc::new(b"\n".to_vec())));

        // Path separator constants
        let dir_sep = std::path::MAIN_SEPARATOR.to_string().into_bytes();
        self.insert_builtin_constant(b"DIRECTORY_SEPARATOR", Val::String(Rc::new(dir_sep)));

        let path_sep_byte = if cfg!(windows) { b';' } else { b':' };
        self.insert_builtin_constant(b"PATH_SEPARATOR", Val::String(Rc::new(vec![path_sep_byte])));

        // Error reporting level constants
        self.insert_builtin_constant(b"E_ERROR", Val::Int(1));
        self.insert_builtin_constant(b"E_WARNING", Val::Int(2));
        self.insert_builtin_constant(b"E_PARSE", Val::Int(4));
        self.insert_builtin_constant(b"E_NOTICE", Val::Int(8));
        self.insert_builtin_constant(b"E_CORE_ERROR", Val::Int(16));
        self.insert_builtin_constant(b"E_CORE_WARNING", Val::Int(32));
        self.insert_builtin_constant(b"E_COMPILE_ERROR", Val::Int(64));
        self.insert_builtin_constant(b"E_COMPILE_WARNING", Val::Int(128));
        self.insert_builtin_constant(b"E_USER_ERROR", Val::Int(256));
        self.insert_builtin_constant(b"E_USER_WARNING", Val::Int(512));
        self.insert_builtin_constant(b"E_USER_NOTICE", Val::Int(1024));
        self.insert_builtin_constant(b"E_STRICT", Val::Int(2048));
        self.insert_builtin_constant(b"E_RECOVERABLE_ERROR", Val::Int(4096));
        self.insert_builtin_constant(b"E_DEPRECATED", Val::Int(8192));
        self.insert_builtin_constant(b"E_USER_DEPRECATED", Val::Int(16384));
        self.insert_builtin_constant(b"E_ALL", Val::Int(32767));
    }

    pub fn insert_builtin_constant(&mut self, name: &[u8], value: Val) {
        let sym = self.interner.intern(name);
        self.constants.insert(sym, value);
    }
}

/// Builder for constructing EngineContext with extensions
///
/// # Example
/// ```ignore
/// let engine = EngineBuilder::new()
///     .with_core_extensions()
///     .build()?;
/// ```
pub struct EngineBuilder {
    extensions: Vec<Box<dyn Extension>>,
}

impl EngineBuilder {
    /// Create a new empty builder
    pub fn new() -> Self {
        Self {
            extensions: Vec::new(),
        }
    }

    /// Add an extension to the builder
    pub fn with_extension<E: Extension + 'static>(mut self, ext: E) -> Self {
        self.extensions.push(Box::new(ext));
        self
    }

    /// Add core extensions (standard builtins)
    ///
    /// This includes all core PHP functionality: core functions, classes, interfaces,
    /// exceptions, and the date/time extension.
    pub fn with_core_extensions(mut self) -> Self {
        self.extensions.push(Box::new(super::core_extension::CoreExtension));
        self.extensions.push(Box::new(super::date_extension::DateExtension));
        self.extensions.push(Box::new(super::hash_extension::HashExtension));
        self.extensions.push(Box::new(super::mysqli_extension::MysqliExtension));
        self.extensions.push(Box::new(super::json_extension::JsonExtension));
        self.extensions.push(Box::new(super::openssl_extension::OpenSSLExtension));
        self.extensions.push(Box::new(super::pdo_extension::PdoExtension));
        self.extensions.push(Box::new(super::pthreads_extension::PthreadsExtension));
        self.extensions.push(Box::new(super::zlib_extension::ZlibExtension));
        self.extensions.push(Box::new(super::mb_extension::MbStringExtension));
        self
    }

    /// Build the EngineContext
    ///
    /// This will:
    /// 1. Create an empty registry
    /// 2. Register all extensions (calling MINIT for each)
    /// 3. Return the configured EngineContext
    pub fn build(self) -> Result<Arc<EngineContext>, String> {
        let mut registry = ExtensionRegistry::new();

        // Register all extensions
        for ext in self.extensions {
            registry.register_extension(ext)?;
        }

        Ok(Arc::new(EngineContext {
            registry,
            pdo_driver_registry: None,
        }))
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Call RSHUTDOWN for all extensions when request ends
impl Drop for RequestContext {
    fn drop(&mut self) {
        // Call RSHUTDOWN for all extensions in reverse order (LIFO)
        // Clone Arc to separate lifetimes and avoid borrow checker conflict
        let engine = Arc::clone(&self.engine);
        engine.registry.request_shutdown_all(self);
    }
}

/// Call MSHUTDOWN for all extensions when engine shuts down
impl Drop for EngineContext {
    fn drop(&mut self) {
        // Call MSHUTDOWN for all extensions in reverse order (LIFO)
        self.registry.module_shutdown_all();
    }
}
