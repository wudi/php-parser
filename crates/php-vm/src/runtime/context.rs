use crate::builtins::spl;
use crate::builtins::{array, class, exec, filesystem, function, http, string, variable};
use crate::compiler::chunk::UserFunc;
use crate::core::interner::Interner;
use crate::core::value::{Handle, Symbol, Val, Visibility};
use crate::runtime::extension::Extension;
use crate::runtime::registry::ExtensionRegistry;
use crate::vm::engine::VM;
use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

pub type NativeHandler = fn(&mut VM, args: &[Handle]) -> Result<Handle, String>;

#[derive(Debug, Clone)]
pub struct MethodEntry {
    pub name: Symbol,
    pub func: Rc<UserFunc>,
    pub visibility: Visibility,
    pub is_static: bool,
    pub declaring_class: Symbol,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub name: Symbol,
    pub parent: Option<Symbol>,
    pub is_interface: bool,
    pub is_trait: bool,
    pub interfaces: Vec<Symbol>,
    pub traits: Vec<Symbol>,
    pub methods: HashMap<Symbol, MethodEntry>,
    pub properties: IndexMap<Symbol, (Val, Visibility)>, // Default values
    pub constants: HashMap<Symbol, (Val, Visibility)>,
    pub static_properties: HashMap<Symbol, (Val, Visibility)>,
    pub allows_dynamic_properties: bool, // Set by #[AllowDynamicProperties] attribute
}

#[derive(Debug, Clone)]
pub struct HeaderEntry {
    pub key: Option<Vec<u8>>, // Normalized lowercase header name
    pub line: Vec<u8>,        // Original header line bytes
}

pub struct EngineContext {
    pub registry: ExtensionRegistry,
    // Deprecated: use registry.functions() instead
    // Kept for backward compatibility
    pub functions: HashMap<Vec<u8>, NativeHandler>,
    // Deprecated: use registry.constants() instead
    // Kept for backward compatibility
    pub constants: HashMap<Symbol, Val>,
}

impl EngineContext {
    pub fn new() -> Self {
        let mut functions = HashMap::new();
        functions.insert(b"strlen".to_vec(), string::php_strlen as NativeHandler);
        functions.insert(
            b"str_repeat".to_vec(),
            string::php_str_repeat as NativeHandler,
        );
        functions.insert(b"substr".to_vec(), string::php_substr as NativeHandler);
        functions.insert(b"strpos".to_vec(), string::php_strpos as NativeHandler);
        functions.insert(
            b"strtolower".to_vec(),
            string::php_strtolower as NativeHandler,
        );
        functions.insert(
            b"strtoupper".to_vec(),
            string::php_strtoupper as NativeHandler,
        );
        functions.insert(
            b"version_compare".to_vec(),
            string::php_version_compare as NativeHandler,
        );
        functions.insert(
            b"array_merge".to_vec(),
            array::php_array_merge as NativeHandler,
        );
        functions.insert(
            b"array_keys".to_vec(),
            array::php_array_keys as NativeHandler,
        );
        functions.insert(
            b"array_values".to_vec(),
            array::php_array_values as NativeHandler,
        );
        functions.insert(b"in_array".to_vec(), array::php_in_array as NativeHandler);
        functions.insert(
            b"var_dump".to_vec(),
            variable::php_var_dump as NativeHandler,
        );
        functions.insert(b"count".to_vec(), array::php_count as NativeHandler);
        functions.insert(
            b"is_string".to_vec(),
            variable::php_is_string as NativeHandler,
        );
        functions.insert(b"is_int".to_vec(), variable::php_is_int as NativeHandler);
        functions.insert(
            b"is_array".to_vec(),
            variable::php_is_array as NativeHandler,
        );
        functions.insert(b"is_bool".to_vec(), variable::php_is_bool as NativeHandler);
        functions.insert(b"is_null".to_vec(), variable::php_is_null as NativeHandler);
        functions.insert(
            b"is_object".to_vec(),
            variable::php_is_object as NativeHandler,
        );
        functions.insert(
            b"is_float".to_vec(),
            variable::php_is_float as NativeHandler,
        );
        functions.insert(
            b"is_numeric".to_vec(),
            variable::php_is_numeric as NativeHandler,
        );
        functions.insert(
            b"is_scalar".to_vec(),
            variable::php_is_scalar as NativeHandler,
        );
        functions.insert(b"implode".to_vec(), string::php_implode as NativeHandler);
        functions.insert(b"explode".to_vec(), string::php_explode as NativeHandler);
        functions.insert(b"sprintf".to_vec(), string::php_sprintf as NativeHandler);
        functions.insert(b"printf".to_vec(), string::php_printf as NativeHandler);
        functions.insert(b"header".to_vec(), http::php_header as NativeHandler);
        functions.insert(b"define".to_vec(), variable::php_define as NativeHandler);
        functions.insert(b"defined".to_vec(), variable::php_defined as NativeHandler);
        functions.insert(
            b"constant".to_vec(),
            variable::php_constant as NativeHandler,
        );
        functions.insert(
            b"get_object_vars".to_vec(),
            class::php_get_object_vars as NativeHandler,
        );
        functions.insert(b"get_class".to_vec(), class::php_get_class as NativeHandler);
        functions.insert(
            b"get_parent_class".to_vec(),
            class::php_get_parent_class as NativeHandler,
        );
        functions.insert(
            b"is_subclass_of".to_vec(),
            class::php_is_subclass_of as NativeHandler,
        );
        functions.insert(b"is_a".to_vec(), class::php_is_a as NativeHandler);
        functions.insert(
            b"class_exists".to_vec(),
            class::php_class_exists as NativeHandler,
        );
        functions.insert(
            b"interface_exists".to_vec(),
            class::php_interface_exists as NativeHandler,
        );
        functions.insert(
            b"trait_exists".to_vec(),
            class::php_trait_exists as NativeHandler,
        );
        functions.insert(
            b"method_exists".to_vec(),
            class::php_method_exists as NativeHandler,
        );
        functions.insert(
            b"property_exists".to_vec(),
            class::php_property_exists as NativeHandler,
        );
        functions.insert(
            b"get_class_methods".to_vec(),
            class::php_get_class_methods as NativeHandler,
        );
        functions.insert(
            b"get_class_vars".to_vec(),
            class::php_get_class_vars as NativeHandler,
        );
        functions.insert(
            b"get_called_class".to_vec(),
            class::php_get_called_class as NativeHandler,
        );
        functions.insert(b"gettype".to_vec(), variable::php_gettype as NativeHandler);
        functions.insert(
            b"var_export".to_vec(),
            variable::php_var_export as NativeHandler,
        );
        functions.insert(b"getenv".to_vec(), variable::php_getenv as NativeHandler);
        functions.insert(b"putenv".to_vec(), variable::php_putenv as NativeHandler);
        functions.insert(b"getopt".to_vec(), variable::php_getopt as NativeHandler);
        functions.insert(
            b"sys_get_temp_dir".to_vec(),
            filesystem::php_sys_get_temp_dir as NativeHandler,
        );
        functions.insert(
            b"tmpfile".to_vec(),
            filesystem::php_tmpfile as NativeHandler,
        );
        functions.insert(
            b"func_get_args".to_vec(),
            function::php_func_get_args as NativeHandler,
        );
        functions.insert(
            b"func_num_args".to_vec(),
            function::php_func_num_args as NativeHandler,
        );
        functions.insert(
            b"func_get_arg".to_vec(),
            function::php_func_get_arg as NativeHandler,
        );
        functions.insert(
            b"function_exists".to_vec(),
            function::php_function_exists as NativeHandler,
        );
        functions.insert(
            b"is_callable".to_vec(),
            function::php_is_callable as NativeHandler,
        );
        functions.insert(
            b"extension_loaded".to_vec(),
            function::php_extension_loaded as NativeHandler,
        );
        functions.insert(
            b"spl_autoload_register".to_vec(),
            spl::php_spl_autoload_register as NativeHandler,
        );
        functions.insert(
            b"spl_object_hash".to_vec(),
            spl::php_spl_object_hash as NativeHandler,
        );
        functions.insert(b"assert".to_vec(), function::php_assert as NativeHandler);

        // Filesystem functions - File I/O
        functions.insert(b"fopen".to_vec(), filesystem::php_fopen as NativeHandler);
        functions.insert(b"fclose".to_vec(), filesystem::php_fclose as NativeHandler);
        functions.insert(b"fread".to_vec(), filesystem::php_fread as NativeHandler);
        functions.insert(b"fwrite".to_vec(), filesystem::php_fwrite as NativeHandler);
        functions.insert(b"fputs".to_vec(), filesystem::php_fputs as NativeHandler);
        functions.insert(b"fgets".to_vec(), filesystem::php_fgets as NativeHandler);
        functions.insert(b"fgetc".to_vec(), filesystem::php_fgetc as NativeHandler);
        functions.insert(b"fseek".to_vec(), filesystem::php_fseek as NativeHandler);
        functions.insert(b"ftell".to_vec(), filesystem::php_ftell as NativeHandler);
        functions.insert(b"rewind".to_vec(), filesystem::php_rewind as NativeHandler);
        functions.insert(b"feof".to_vec(), filesystem::php_feof as NativeHandler);
        functions.insert(b"fflush".to_vec(), filesystem::php_fflush as NativeHandler);

        // Filesystem functions - File content
        functions.insert(
            b"file_get_contents".to_vec(),
            filesystem::php_file_get_contents as NativeHandler,
        );
        functions.insert(
            b"file_put_contents".to_vec(),
            filesystem::php_file_put_contents as NativeHandler,
        );
        functions.insert(b"file".to_vec(), filesystem::php_file as NativeHandler);

        // Filesystem functions - File information
        functions.insert(
            b"file_exists".to_vec(),
            filesystem::php_file_exists as NativeHandler,
        );
        functions.insert(
            b"is_file".to_vec(),
            filesystem::php_is_file as NativeHandler,
        );
        functions.insert(b"is_dir".to_vec(), filesystem::php_is_dir as NativeHandler);
        functions.insert(
            b"is_link".to_vec(),
            filesystem::php_is_link as NativeHandler,
        );
        functions.insert(
            b"filesize".to_vec(),
            filesystem::php_filesize as NativeHandler,
        );
        functions.insert(
            b"is_readable".to_vec(),
            filesystem::php_is_readable as NativeHandler,
        );
        functions.insert(
            b"is_writable".to_vec(),
            filesystem::php_is_writable as NativeHandler,
        );
        functions.insert(
            b"is_writeable".to_vec(),
            filesystem::php_is_writable as NativeHandler,
        ); // Alias
        functions.insert(
            b"is_executable".to_vec(),
            filesystem::php_is_executable as NativeHandler,
        );

        // Filesystem functions - File metadata
        functions.insert(
            b"filemtime".to_vec(),
            filesystem::php_filemtime as NativeHandler,
        );
        functions.insert(
            b"fileatime".to_vec(),
            filesystem::php_fileatime as NativeHandler,
        );
        functions.insert(
            b"filectime".to_vec(),
            filesystem::php_filectime as NativeHandler,
        );
        functions.insert(
            b"fileperms".to_vec(),
            filesystem::php_fileperms as NativeHandler,
        );
        functions.insert(
            b"fileowner".to_vec(),
            filesystem::php_fileowner as NativeHandler,
        );
        functions.insert(
            b"filegroup".to_vec(),
            filesystem::php_filegroup as NativeHandler,
        );
        functions.insert(b"stat".to_vec(), filesystem::php_stat as NativeHandler);
        functions.insert(b"lstat".to_vec(), filesystem::php_lstat as NativeHandler);

        // Filesystem functions - File operations
        functions.insert(b"unlink".to_vec(), filesystem::php_unlink as NativeHandler);
        functions.insert(b"rename".to_vec(), filesystem::php_rename as NativeHandler);
        functions.insert(b"copy".to_vec(), filesystem::php_copy as NativeHandler);
        functions.insert(b"touch".to_vec(), filesystem::php_touch as NativeHandler);
        functions.insert(b"chmod".to_vec(), filesystem::php_chmod as NativeHandler);
        functions.insert(
            b"readlink".to_vec(),
            filesystem::php_readlink as NativeHandler,
        );

        // Filesystem functions - Directory operations
        functions.insert(b"mkdir".to_vec(), filesystem::php_mkdir as NativeHandler);
        functions.insert(b"rmdir".to_vec(), filesystem::php_rmdir as NativeHandler);
        functions.insert(
            b"scandir".to_vec(),
            filesystem::php_scandir as NativeHandler,
        );
        functions.insert(b"getcwd".to_vec(), filesystem::php_getcwd as NativeHandler);
        functions.insert(b"chdir".to_vec(), filesystem::php_chdir as NativeHandler);

        // Filesystem functions - Path operations
        functions.insert(
            b"basename".to_vec(),
            filesystem::php_basename as NativeHandler,
        );
        functions.insert(
            b"dirname".to_vec(),
            filesystem::php_dirname as NativeHandler,
        );
        functions.insert(
            b"realpath".to_vec(),
            filesystem::php_realpath as NativeHandler,
        );

        // Filesystem functions - Temporary files
        functions.insert(
            b"tempnam".to_vec(),
            filesystem::php_tempnam as NativeHandler,
        );

        // Filesystem functions - Disk space (stubs)
        functions.insert(
            b"disk_free_space".to_vec(),
            filesystem::php_disk_free_space as NativeHandler,
        );
        functions.insert(
            b"disk_total_space".to_vec(),
            filesystem::php_disk_total_space as NativeHandler,
        );

        // Execution functions
        functions.insert(
            b"escapeshellarg".to_vec(),
            exec::php_escapeshellarg as NativeHandler,
        );
        functions.insert(
            b"escapeshellcmd".to_vec(),
            exec::php_escapeshellcmd as NativeHandler,
        );
        functions.insert(b"exec".to_vec(), exec::php_exec as NativeHandler);
        functions.insert(b"passthru".to_vec(), exec::php_passthru as NativeHandler);
        functions.insert(
            b"shell_exec".to_vec(),
            exec::php_shell_exec as NativeHandler,
        );
        functions.insert(b"system".to_vec(), exec::php_system as NativeHandler);
        functions.insert(b"proc_open".to_vec(), exec::php_proc_open as NativeHandler);
        functions.insert(
            b"proc_close".to_vec(),
            exec::php_proc_close as NativeHandler,
        );
        functions.insert(
            b"proc_get_status".to_vec(),
            exec::php_proc_get_status as NativeHandler,
        );
        functions.insert(b"proc_nice".to_vec(), exec::php_proc_nice as NativeHandler);
        functions.insert(
            b"proc_terminate".to_vec(),
            exec::php_proc_terminate as NativeHandler,
        );

        Self {
            registry: ExtensionRegistry::new(),
            functions,
            constants: HashMap::new(),
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
    pub headers: Vec<HeaderEntry>,
    pub http_status: Option<i64>,
}

impl RequestContext {
    pub fn new(engine: Arc<EngineContext>) -> Self {
        let mut ctx = Self {
            engine,
            globals: HashMap::new(),
            constants: HashMap::new(),
            user_functions: HashMap::new(),
            classes: HashMap::new(),
            included_files: HashSet::new(),
            autoloaders: Vec::new(),
            interner: Interner::new(),
            error_reporting: 32767, // E_ALL
            headers: Vec::new(),
            http_status: None,
        };
        ctx.register_builtin_classes();
        ctx.register_builtin_constants();
        ctx
    }
}

impl RequestContext {
    fn register_builtin_classes(&mut self) {
        let exception_sym = self.interner.intern(b"Exception");
        self.classes.insert(
            exception_sym,
            ClassDef {
                name: exception_sym,
                parent: None,
                is_interface: false,
                is_trait: false,
                interfaces: Vec::new(),
                traits: Vec::new(),
                methods: HashMap::new(),
                properties: IndexMap::new(),
                constants: HashMap::new(),
                static_properties: HashMap::new(),
                allows_dynamic_properties: false,
            },
        );
    }

    fn register_builtin_constants(&mut self) {
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
        self.insert_builtin_constant(b"PHP_OS", Val::String(Rc::new(b"Darwin".to_vec())));
        self.insert_builtin_constant(b"PHP_SAPI", Val::String(Rc::new(b"cli".to_vec())));
        self.insert_builtin_constant(b"PHP_EOL", Val::String(Rc::new(b"\n".to_vec())));

        let dir_sep = std::path::MAIN_SEPARATOR.to_string().into_bytes();
        self.insert_builtin_constant(b"DIRECTORY_SEPARATOR", Val::String(Rc::new(dir_sep)));

        let path_sep_byte = if cfg!(windows) { b';' } else { b':' };
        self.insert_builtin_constant(b"PATH_SEPARATOR", Val::String(Rc::new(vec![path_sep_byte])));
    }

    fn insert_builtin_constant(&mut self, name: &[u8], value: Val) {
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
    /// This includes all the functions currently hardcoded in EngineContext::new()
    pub fn with_core_extensions(self) -> Self {
        // TODO: Replace with actual CoreExtension once implemented
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
            functions: HashMap::new(), // Deprecated, kept for compatibility
            constants: HashMap::new(), // Deprecated, kept for compatibility
        }))
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}
