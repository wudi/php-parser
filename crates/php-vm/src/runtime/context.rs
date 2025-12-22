use crate::builtins::spl;
use crate::builtins::{
    array, bcmath, class, datetime, exception, exec, filesystem, function, hash, http, json, math,
    output_control, pcre, string, url, variable,
};
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
pub struct NativeMethodEntry {
    pub name: Symbol,
    pub handler: NativeHandler,
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

#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub error_type: i64,
    pub message: String,
    pub file: String,
    pub line: i64,
}

pub struct EngineContext {
    pub registry: ExtensionRegistry,
    // Deprecated: use registry.functions() instead
    // Kept for backward compatibility
    pub functions: HashMap<Vec<u8>, NativeHandler>,
    // Deprecated: use registry.constants() instead
    // Kept for backward compatibility
    pub constants: HashMap<Symbol, Val>,
    /// PDO driver registry (set by PDO extension)
    pub pdo_driver_registry: Option<std::sync::Arc<crate::builtins::pdo::drivers::DriverRegistry>>,
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
        functions.insert(
            b"substr_replace".to_vec(),
            string::php_substr_replace as NativeHandler,
        );
        functions.insert(b"strpos".to_vec(), string::php_strpos as NativeHandler);
        functions.insert(b"strtr".to_vec(), string::php_strtr as NativeHandler);
        functions.insert(b"trim".to_vec(), string::php_trim as NativeHandler);
        functions.insert(b"ltrim".to_vec(), string::php_ltrim as NativeHandler);
        functions.insert(b"rtrim".to_vec(), string::php_rtrim as NativeHandler);
        functions.insert(b"chr".to_vec(), string::php_chr as NativeHandler);
        functions.insert(b"ord".to_vec(), string::php_ord as NativeHandler);
        functions.insert(b"bin2hex".to_vec(), string::php_bin2hex as NativeHandler);
        functions.insert(b"hex2bin".to_vec(), string::php_hex2bin as NativeHandler);
        functions.insert(
            b"addslashes".to_vec(),
            string::php_addslashes as NativeHandler,
        );
        functions.insert(
            b"stripslashes".to_vec(),
            string::php_stripslashes as NativeHandler,
        );
        functions.insert(
            b"addcslashes".to_vec(),
            string::php_addcslashes as NativeHandler,
        );
        functions.insert(
            b"stripcslashes".to_vec(),
            string::php_stripcslashes as NativeHandler,
        );
        functions.insert(b"str_pad".to_vec(), string::php_str_pad as NativeHandler);
        functions.insert(
            b"str_rot13".to_vec(),
            string::php_str_rot13 as NativeHandler,
        );
        functions.insert(
            b"str_shuffle".to_vec(),
            string::php_str_shuffle as NativeHandler,
        );
        functions.insert(
            b"str_split".to_vec(),
            string::php_str_split as NativeHandler,
        );
        functions.insert(b"strrev".to_vec(), string::php_strrev as NativeHandler);
        functions.insert(b"strcmp".to_vec(), string::php_strcmp as NativeHandler);
        functions.insert(
            b"strcasecmp".to_vec(),
            string::php_strcasecmp as NativeHandler,
        );
        functions.insert(b"strncmp".to_vec(), string::php_strncmp as NativeHandler);
        functions.insert(
            b"strncasecmp".to_vec(),
            string::php_strncasecmp as NativeHandler,
        );
        functions.insert(b"strstr".to_vec(), string::php_strstr as NativeHandler);
        functions.insert(b"stristr".to_vec(), string::php_stristr as NativeHandler);
        functions.insert(
            b"substr_count".to_vec(),
            string::php_substr_count as NativeHandler,
        );
        functions.insert(b"ucfirst".to_vec(), string::php_ucfirst as NativeHandler);
        functions.insert(b"lcfirst".to_vec(), string::php_lcfirst as NativeHandler);
        functions.insert(b"ucwords".to_vec(), string::php_ucwords as NativeHandler);
        functions.insert(b"wordwrap".to_vec(), string::php_wordwrap as NativeHandler);
        functions.insert(b"strtok".to_vec(), string::php_strtok as NativeHandler);

        functions.insert(
            b"str_contains".to_vec(),
            string::php_str_contains as NativeHandler,
        );
        functions.insert(
            b"str_starts_with".to_vec(),
            string::php_str_starts_with as NativeHandler,
        );
        functions.insert(
            b"str_ends_with".to_vec(),
            string::php_str_ends_with as NativeHandler,
        );
        functions.insert(
            b"str_replace".to_vec(),
            string::php_str_replace as NativeHandler,
        );
        functions.insert(
            b"str_ireplace".to_vec(),
            string::php_str_ireplace as NativeHandler,
        );
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
        functions.insert(b"ksort".to_vec(), array::php_ksort as NativeHandler);
        functions.insert(
            b"array_unshift".to_vec(),
            array::php_array_unshift as NativeHandler,
        );
        functions.insert(b"current".to_vec(), array::php_current as NativeHandler);
        functions.insert(b"next".to_vec(), array::php_next as NativeHandler);
        functions.insert(b"reset".to_vec(), array::php_reset as NativeHandler);
        functions.insert(b"end".to_vec(), array::php_end as NativeHandler);
        functions.insert(
            b"array_key_exists".to_vec(),
            array::php_array_key_exists as NativeHandler,
        );
        functions.insert(
            b"var_dump".to_vec(),
            variable::php_var_dump as NativeHandler,
        );
        functions.insert(b"print_r".to_vec(), variable::php_print_r as NativeHandler);
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
        functions.insert(
            b"headers_sent".to_vec(),
            http::php_headers_sent as NativeHandler,
        );
        functions.insert(
            b"header_remove".to_vec(),
            http::php_header_remove as NativeHandler,
        );

        // URL functions
        functions.insert(b"urlencode".to_vec(), url::php_urlencode as NativeHandler);
        functions.insert(b"urldecode".to_vec(), url::php_urldecode as NativeHandler);
        functions.insert(
            b"rawurlencode".to_vec(),
            url::php_rawurlencode as NativeHandler,
        );
        functions.insert(
            b"rawurldecode".to_vec(),
            url::php_rawurldecode as NativeHandler,
        );
        functions.insert(
            b"base64_encode".to_vec(),
            url::php_base64_encode as NativeHandler,
        );
        functions.insert(
            b"base64_decode".to_vec(),
            url::php_base64_decode as NativeHandler,
        );
        functions.insert(b"parse_url".to_vec(), url::php_parse_url as NativeHandler);
        functions.insert(
            b"http_build_query".to_vec(),
            url::php_http_build_query as NativeHandler,
        );
        functions.insert(
            b"get_headers".to_vec(),
            url::php_get_headers as NativeHandler,
        );
        functions.insert(
            b"get_meta_tags".to_vec(),
            url::php_get_meta_tags as NativeHandler,
        );

        functions.insert(b"abs".to_vec(), math::php_abs as NativeHandler);
        functions.insert(b"max".to_vec(), math::php_max as NativeHandler);
        functions.insert(b"min".to_vec(), math::php_min as NativeHandler);
        functions.insert(b"define".to_vec(), variable::php_define as NativeHandler);

        // BCMath functions
        functions.insert(b"bcadd".to_vec(), bcmath::bcadd as NativeHandler);
        functions.insert(b"bcsub".to_vec(), bcmath::bcsub as NativeHandler);
        functions.insert(b"bcmul".to_vec(), bcmath::bcmul as NativeHandler);
        functions.insert(b"bcdiv".to_vec(), bcmath::bcdiv as NativeHandler);

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
        functions.insert(b"ini_get".to_vec(), variable::php_ini_get as NativeHandler);
        functions.insert(b"ini_set".to_vec(), variable::php_ini_set as NativeHandler);
        functions.insert(
            b"error_reporting".to_vec(),
            variable::php_error_reporting as NativeHandler,
        );
        functions.insert(
            b"error_get_last".to_vec(),
            variable::php_error_get_last as NativeHandler,
        );
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
        functions.insert(b"preg_match".to_vec(), pcre::preg_match as NativeHandler);
        functions.insert(
            b"
        "
            .to_vec(),
            pcre::preg_replace as NativeHandler,
        );
        functions.insert(b"preg_split".to_vec(), pcre::preg_split as NativeHandler);
        functions.insert(b"preg_quote".to_vec(), pcre::preg_quote as NativeHandler);
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
            b"call_user_func".to_vec(),
            function::php_call_user_func as NativeHandler,
        );
        functions.insert(
            b"call_user_func_array".to_vec(),
            function::php_call_user_func_array as NativeHandler,
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
        functions.insert(
            b"set_time_limit".to_vec(),
            exec::php_set_time_limit as NativeHandler,
        );

        // Date/Time functions
        functions.insert(
            b"checkdate".to_vec(),
            datetime::php_checkdate as NativeHandler,
        );
        functions.insert(b"date".to_vec(), datetime::php_date as NativeHandler);
        functions.insert(b"gmdate".to_vec(), datetime::php_gmdate as NativeHandler);
        functions.insert(b"time".to_vec(), datetime::php_time as NativeHandler);
        functions.insert(
            b"date_create".to_vec(),
            datetime::php_date_create as NativeHandler,
        );
        functions.insert(
            b"date_create_immutable".to_vec(),
            datetime::php_date_create_immutable as NativeHandler,
        );
        functions.insert(
            b"date_format".to_vec(),
            datetime::php_date_format as NativeHandler,
        );
        functions.insert(
            b"microtime".to_vec(),
            datetime::php_microtime as NativeHandler,
        );
        functions.insert(b"mktime".to_vec(), datetime::php_mktime as NativeHandler);
        functions.insert(
            b"gmmktime".to_vec(),
            datetime::php_gmmktime as NativeHandler,
        );
        functions.insert(
            b"strtotime".to_vec(),
            datetime::php_strtotime as NativeHandler,
        );
        functions.insert(b"getdate".to_vec(), datetime::php_getdate as NativeHandler);
        functions.insert(b"idate".to_vec(), datetime::php_idate as NativeHandler);
        functions.insert(
            b"gettimeofday".to_vec(),
            datetime::php_gettimeofday as NativeHandler,
        );
        functions.insert(
            b"localtime".to_vec(),
            datetime::php_localtime as NativeHandler,
        );
        functions.insert(
            b"date_default_timezone_get".to_vec(),
            datetime::php_date_default_timezone_get as NativeHandler,
        );
        functions.insert(
            b"date_default_timezone_set".to_vec(),
            datetime::php_date_default_timezone_set as NativeHandler,
        );
        functions.insert(
            b"date_sunrise".to_vec(),
            datetime::php_date_sunrise as NativeHandler,
        );
        functions.insert(
            b"date_sunset".to_vec(),
            datetime::php_date_sunset as NativeHandler,
        );
        functions.insert(
            b"date_sun_info".to_vec(),
            datetime::php_date_sun_info as NativeHandler,
        );
        functions.insert(
            b"date_parse".to_vec(),
            datetime::php_date_parse as NativeHandler,
        );
        functions.insert(
            b"date_parse_from_format".to_vec(),
            datetime::php_date_parse_from_format as NativeHandler,
        );
        functions.insert(
            b"date_add".to_vec(),
            datetime::php_date_add as NativeHandler,
        );
        functions.insert(
            b"date_sub".to_vec(),
            datetime::php_date_sub as NativeHandler,
        );
        functions.insert(
            b"date_diff".to_vec(),
            datetime::php_date_diff as NativeHandler,
        );
        functions.insert(
            b"date_modify".to_vec(),
            datetime::php_date_modify as NativeHandler,
        );
        functions.insert(
            b"date_create_from_format".to_vec(),
            datetime::php_datetime_create_from_format as NativeHandler,
        );
        functions.insert(
            b"date_interval_create_from_date_string".to_vec(),
            datetime::php_date_interval_create_from_date_string as NativeHandler,
        );
        functions.insert(
            b"date_interval_format".to_vec(),
            datetime::php_dateinterval_format as NativeHandler,
        );
        functions.insert(
            b"timezone_open".to_vec(),
            datetime::php_timezone_open as NativeHandler,
        );

        // Output Control functions
        functions.insert(
            b"ob_start".to_vec(),
            output_control::php_ob_start as NativeHandler,
        );
        functions.insert(
            b"ob_clean".to_vec(),
            output_control::php_ob_clean as NativeHandler,
        );
        functions.insert(
            b"ob_flush".to_vec(),
            output_control::php_ob_flush as NativeHandler,
        );
        functions.insert(
            b"ob_end_clean".to_vec(),
            output_control::php_ob_end_clean as NativeHandler,
        );
        functions.insert(
            b"ob_end_flush".to_vec(),
            output_control::php_ob_end_flush as NativeHandler,
        );
        functions.insert(
            b"ob_get_clean".to_vec(),
            output_control::php_ob_get_clean as NativeHandler,
        );
        functions.insert(
            b"ob_get_contents".to_vec(),
            output_control::php_ob_get_contents as NativeHandler,
        );
        functions.insert(
            b"ob_get_flush".to_vec(),
            output_control::php_ob_get_flush as NativeHandler,
        );
        functions.insert(
            b"ob_get_length".to_vec(),
            output_control::php_ob_get_length as NativeHandler,
        );
        functions.insert(
            b"ob_get_level".to_vec(),
            output_control::php_ob_get_level as NativeHandler,
        );
        functions.insert(
            b"ob_get_status".to_vec(),
            output_control::php_ob_get_status as NativeHandler,
        );
        functions.insert(
            b"ob_implicit_flush".to_vec(),
            output_control::php_ob_implicit_flush as NativeHandler,
        );
        functions.insert(
            b"ob_list_handlers".to_vec(),
            output_control::php_ob_list_handlers as NativeHandler,
        );
        functions.insert(
            b"flush".to_vec(),
            output_control::php_flush as NativeHandler,
        );
        functions.insert(
            b"output_add_rewrite_var".to_vec(),
            output_control::php_output_add_rewrite_var as NativeHandler,
        );
        functions.insert(
            b"output_reset_rewrite_vars".to_vec(),
            output_control::php_output_reset_rewrite_vars as NativeHandler,
        );

        let mut registry = ExtensionRegistry::new();

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

        // Register core string functions with by-ref info
        registry.register_function_with_by_ref(b"str_replace", string::php_str_replace, vec![3]);
        registry.register_function_with_by_ref(b"str_ireplace", string::php_str_ireplace, vec![3]);

        // Register core string constants
        registry.register_constant(b"STR_PAD_LEFT", Val::Int(0));
        registry.register_constant(b"STR_PAD_RIGHT", Val::Int(1));
        registry.register_constant(b"STR_PAD_BOTH", Val::Int(2));

        Self {
            registry,
            functions,
            constants: HashMap::new(),
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
        };
        ctx.register_builtin_classes();
        ctx.materialize_extension_classes();
        ctx.register_builtin_constants();
        ctx
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
                    is_interface: false,
                    is_trait: false,
                    interfaces,
                    traits: Vec::new(),
                    methods: HashMap::new(),
                    properties: IndexMap::new(),
                    constants,
                    static_properties: HashMap::new(),
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
}

impl RequestContext {
    fn register_builtin_classes(&mut self) {
        // Helper to register a native method
        let register_native_method = |ctx: &mut RequestContext,
                                      class_sym: Symbol,
                                      name: &[u8],
                                      handler: NativeHandler,
                                      visibility: Visibility,
                                      is_static: bool| {
            let method_sym = ctx.interner.intern(name);
            ctx.native_methods.insert(
                (class_sym, method_sym),
                NativeMethodEntry {
                    name: method_sym,
                    handler,
                    visibility,
                    is_static,
                    declaring_class: class_sym,
                },
            );
        };

        //=====================================================================
        // Predefined Interfaces and Classes
        // Reference: $PHP_SRC_PATH/Zend/zend_interfaces.c
        //=====================================================================

        // Stringable interface (PHP 8.0+) - must be defined before Throwable
        let stringable_sym = self.interner.intern(b"Stringable");
        self.classes.insert(
            stringable_sym,
            ClassDef {
                name: stringable_sym,
                parent: None,
                is_interface: true,
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

        // Throwable interface (base for all exceptions/errors, extends Stringable)
        let throwable_sym = self.interner.intern(b"Throwable");
        self.classes.insert(
            throwable_sym,
            ClassDef {
                name: throwable_sym,
                parent: None,
                is_interface: true,
                is_trait: false,
                interfaces: vec![stringable_sym],
                traits: Vec::new(),
                methods: HashMap::new(),
                properties: IndexMap::new(),
                constants: HashMap::new(),
                static_properties: HashMap::new(),
                allows_dynamic_properties: false,
            },
        );

        // Traversable interface (root iterator interface)
        let traversable_sym = self.interner.intern(b"Traversable");
        self.classes.insert(
            traversable_sym,
            ClassDef {
                name: traversable_sym,
                parent: None,
                is_interface: true,
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

        // Iterator interface
        let iterator_sym = self.interner.intern(b"Iterator");
        self.classes.insert(
            iterator_sym,
            ClassDef {
                name: iterator_sym,
                parent: None,
                is_interface: true,
                is_trait: false,
                interfaces: vec![traversable_sym],
                traits: Vec::new(),
                methods: HashMap::new(),
                properties: IndexMap::new(),
                constants: HashMap::new(),
                static_properties: HashMap::new(),
                allows_dynamic_properties: false,
            },
        );

        // IteratorAggregate interface
        let iterator_aggregate_sym = self.interner.intern(b"IteratorAggregate");
        self.classes.insert(
            iterator_aggregate_sym,
            ClassDef {
                name: iterator_aggregate_sym,
                parent: None,
                is_interface: true,
                is_trait: false,
                interfaces: vec![traversable_sym],
                traits: Vec::new(),
                methods: HashMap::new(),
                properties: IndexMap::new(),
                constants: HashMap::new(),
                static_properties: HashMap::new(),
                allows_dynamic_properties: false,
            },
        );

        // Countable interface
        let countable_sym = self.interner.intern(b"Countable");
        self.classes.insert(
            countable_sym,
            ClassDef {
                name: countable_sym,
                parent: None,
                is_interface: true,
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

        // ArrayAccess interface (allows objects to be accessed like arrays)
        let array_access_sym = self.interner.intern(b"ArrayAccess");
        self.classes.insert(
            array_access_sym,
            ClassDef {
                name: array_access_sym,
                parent: None,
                is_interface: true,
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

        // Serializable interface (deprecated since PHP 8.1)
        let serializable_sym = self.interner.intern(b"Serializable");
        self.classes.insert(
            serializable_sym,
            ClassDef {
                name: serializable_sym,
                parent: None,
                is_interface: true,
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

        // UnitEnum interface (PHP 8.1+)
        let unit_enum_sym = self.interner.intern(b"UnitEnum");
        self.classes.insert(
            unit_enum_sym,
            ClassDef {
                name: unit_enum_sym,
                parent: None,
                is_interface: true,
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

        // BackedEnum interface (PHP 8.1+)
        let backed_enum_sym = self.interner.intern(b"BackedEnum");
        self.classes.insert(
            backed_enum_sym,
            ClassDef {
                name: backed_enum_sym,
                parent: None,
                is_interface: true,
                is_trait: false,
                interfaces: vec![unit_enum_sym],
                traits: Vec::new(),
                methods: HashMap::new(),
                properties: IndexMap::new(),
                constants: HashMap::new(),
                static_properties: HashMap::new(),
                allows_dynamic_properties: false,
            },
        );

        //=====================================================================
        // Internal Classes
        //=====================================================================

        // Closure class (final)
        let closure_sym = self.interner.intern(b"Closure");
        self.classes.insert(
            closure_sym,
            ClassDef {
                name: closure_sym,
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
        register_native_method(
            self,
            closure_sym,
            b"bind",
            class::closure_bind,
            Visibility::Public,
            true,
        );
        register_native_method(
            self,
            closure_sym,
            b"bindTo",
            class::closure_bind_to,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            closure_sym,
            b"call",
            class::closure_call,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            closure_sym,
            b"fromCallable",
            class::closure_from_callable,
            Visibility::Public,
            true,
        );

        // stdClass - empty class for generic objects
        let stdclass_sym = self.interner.intern(b"stdClass");
        self.classes.insert(
            stdclass_sym,
            ClassDef {
                name: stdclass_sym,
                parent: None,
                is_interface: false,
                is_trait: false,
                interfaces: Vec::new(),
                traits: Vec::new(),
                methods: HashMap::new(),
                properties: IndexMap::new(),
                constants: HashMap::new(),
                static_properties: HashMap::new(),
                allows_dynamic_properties: true, // stdClass always allows dynamic properties
            },
        );

        // Generator class (final, implements Iterator)
        let generator_sym = self.interner.intern(b"Generator");
        self.classes.insert(
            generator_sym,
            ClassDef {
                name: generator_sym,
                parent: None,
                is_interface: false,
                is_trait: false,
                interfaces: vec![iterator_sym],
                traits: Vec::new(),
                methods: HashMap::new(),
                properties: IndexMap::new(),
                constants: HashMap::new(),
                static_properties: HashMap::new(),
                allows_dynamic_properties: false,
            },
        );
        register_native_method(
            self,
            generator_sym,
            b"current",
            class::generator_current,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            generator_sym,
            b"key",
            class::generator_key,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            generator_sym,
            b"next",
            class::generator_next,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            generator_sym,
            b"rewind",
            class::generator_rewind,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            generator_sym,
            b"valid",
            class::generator_valid,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            generator_sym,
            b"send",
            class::generator_send,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            generator_sym,
            b"throw",
            class::generator_throw,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            generator_sym,
            b"getReturn",
            class::generator_get_return,
            Visibility::Public,
            false,
        );

        // Fiber class (final, PHP 8.1+)
        let fiber_sym = self.interner.intern(b"Fiber");
        self.classes.insert(
            fiber_sym,
            ClassDef {
                name: fiber_sym,
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
        register_native_method(
            self,
            fiber_sym,
            b"__construct",
            class::fiber_construct,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            fiber_sym,
            b"start",
            class::fiber_start,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            fiber_sym,
            b"resume",
            class::fiber_resume,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            fiber_sym,
            b"suspend",
            class::fiber_suspend,
            Visibility::Public,
            true,
        );
        register_native_method(
            self,
            fiber_sym,
            b"throw",
            class::fiber_throw,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            fiber_sym,
            b"isStarted",
            class::fiber_is_started,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            fiber_sym,
            b"isSuspended",
            class::fiber_is_suspended,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            fiber_sym,
            b"isRunning",
            class::fiber_is_running,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            fiber_sym,
            b"isTerminated",
            class::fiber_is_terminated,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            fiber_sym,
            b"getReturn",
            class::fiber_get_return,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            fiber_sym,
            b"getCurrent",
            class::fiber_get_current,
            Visibility::Public,
            true,
        );

        // WeakReference class (final, PHP 7.4+)
        let weak_reference_sym = self.interner.intern(b"WeakReference");
        self.classes.insert(
            weak_reference_sym,
            ClassDef {
                name: weak_reference_sym,
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
        register_native_method(
            self,
            weak_reference_sym,
            b"__construct",
            class::weak_reference_construct,
            Visibility::Private,
            false,
        );
        register_native_method(
            self,
            weak_reference_sym,
            b"create",
            class::weak_reference_create,
            Visibility::Public,
            true,
        );
        register_native_method(
            self,
            weak_reference_sym,
            b"get",
            class::weak_reference_get,
            Visibility::Public,
            false,
        );

        // WeakMap class (final, PHP 8.0+, implements ArrayAccess, Countable, IteratorAggregate)
        let weak_map_sym = self.interner.intern(b"WeakMap");
        self.classes.insert(
            weak_map_sym,
            ClassDef {
                name: weak_map_sym,
                parent: None,
                is_interface: false,
                is_trait: false,
                interfaces: vec![array_access_sym, countable_sym, iterator_aggregate_sym],
                traits: Vec::new(),
                methods: HashMap::new(),
                properties: IndexMap::new(),
                constants: HashMap::new(),
                static_properties: HashMap::new(),
                allows_dynamic_properties: false,
            },
        );
        register_native_method(
            self,
            weak_map_sym,
            b"__construct",
            class::weak_map_construct,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            weak_map_sym,
            b"offsetExists",
            class::weak_map_offset_exists,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            weak_map_sym,
            b"offsetGet",
            class::weak_map_offset_get,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            weak_map_sym,
            b"offsetSet",
            class::weak_map_offset_set,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            weak_map_sym,
            b"offsetUnset",
            class::weak_map_offset_unset,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            weak_map_sym,
            b"count",
            class::weak_map_count,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            weak_map_sym,
            b"getIterator",
            class::weak_map_get_iterator,
            Visibility::Public,
            false,
        );

        // SensitiveParameterValue class (final, PHP 8.2+)
        let sensitive_param_sym = self.interner.intern(b"SensitiveParameterValue");
        self.classes.insert(
            sensitive_param_sym,
            ClassDef {
                name: sensitive_param_sym,
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
        register_native_method(
            self,
            sensitive_param_sym,
            b"__construct",
            class::sensitive_parameter_value_construct,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            sensitive_param_sym,
            b"getValue",
            class::sensitive_parameter_value_get_value,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            sensitive_param_sym,
            b"__debugInfo",
            class::sensitive_parameter_value_debug_info,
            Visibility::Public,
            false,
        );

        // __PHP_Incomplete_Class (used during unserialization)
        let incomplete_class_sym = self.interner.intern(b"__PHP_Incomplete_Class");
        self.classes.insert(
            incomplete_class_sym,
            ClassDef {
                name: incomplete_class_sym,
                parent: None,
                is_interface: false,
                is_trait: false,
                interfaces: Vec::new(),
                traits: Vec::new(),
                methods: HashMap::new(),
                properties: IndexMap::new(),
                constants: HashMap::new(),
                static_properties: HashMap::new(),
                allows_dynamic_properties: true,
            },
        );

        //=====================================================================
        //=====================================================================
        // Date/Time Classes
        //=====================================================================

        let datetime_interface_sym = self.interner.intern(b"DateTimeInterface");
        let datetime_sym = self.interner.intern(b"DateTime");
        let datetime_immutable_sym = self.interner.intern(b"DateTimeImmutable");
        let datetimezone_sym = self.interner.intern(b"DateTimeZone");
        let dateinterval_sym = self.interner.intern(b"DateInterval");
        let dateperiod_sym = self.interner.intern(b"DatePeriod");

        // Date/Time Exceptions
        let date_error_sym = self.interner.intern(b"DateError");
        let date_object_error_sym = self.interner.intern(b"DateObjectError");
        let date_range_error_sym = self.interner.intern(b"DateRangeError");
        let date_exception_sym = self.interner.intern(b"DateException");
        let date_invalid_operation_exception_sym =
            self.interner.intern(b"DateInvalidOperationException");
        let date_invalid_timezone_exception_sym =
            self.interner.intern(b"DateInvalidTimeZoneException");
        let date_malformed_interval_string_exception_sym = self
            .interner
            .intern(b"DateMalformedIntervalStringException");
        let date_malformed_period_string_exception_sym =
            self.interner.intern(b"DateMalformedPeriodStringException");
        let date_malformed_string_exception_sym =
            self.interner.intern(b"DateMalformedStringException");

        // DateTimeInterface
        self.classes.insert(
            datetime_interface_sym,
            ClassDef {
                name: datetime_interface_sym,
                parent: None,
                is_interface: true,
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

        // DateTimeZone
        self.classes.insert(
            datetimezone_sym,
            ClassDef {
                name: datetimezone_sym,
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

        register_native_method(
            self,
            datetimezone_sym,
            b"__construct",
            datetime::php_datetimezone_construct,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetimezone_sym,
            b"getName",
            datetime::php_datetimezone_get_name,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetimezone_sym,
            b"getOffset",
            datetime::php_datetimezone_get_offset,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetimezone_sym,
            b"getLocation",
            datetime::php_datetimezone_get_location,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetimezone_sym,
            b"listIdentifiers",
            datetime::php_datetimezone_list_identifiers,
            Visibility::Public,
            true,
        );

        // DateTime
        self.classes.insert(
            datetime_sym,
            ClassDef {
                name: datetime_sym,
                parent: None,
                is_interface: false,
                is_trait: false,
                interfaces: vec![datetime_interface_sym],
                traits: Vec::new(),
                methods: HashMap::new(),
                properties: IndexMap::new(),
                constants: HashMap::new(),
                static_properties: HashMap::new(),
                allows_dynamic_properties: false,
            },
        );

        register_native_method(
            self,
            datetime_sym,
            b"__construct",
            datetime::php_datetime_construct,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetime_sym,
            b"format",
            datetime::php_datetime_format,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetime_sym,
            b"getTimestamp",
            datetime::php_datetime_get_timestamp,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetime_sym,
            b"setTimestamp",
            datetime::php_datetime_set_timestamp,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetime_sym,
            b"getTimezone",
            datetime::php_datetime_get_timezone,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetime_sym,
            b"setTimezone",
            datetime::php_datetime_set_timezone,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetime_sym,
            b"add",
            datetime::php_datetime_add,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetime_sym,
            b"sub",
            datetime::php_datetime_sub,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetime_sym,
            b"diff",
            datetime::php_datetime_diff,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetime_sym,
            b"modify",
            datetime::php_datetime_modify,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetime_sym,
            b"createFromFormat",
            datetime::php_datetime_create_from_format,
            Visibility::Public,
            true,
        );

        // DateTimeImmutable
        self.classes.insert(
            datetime_immutable_sym,
            ClassDef {
                name: datetime_immutable_sym,
                parent: None,
                is_interface: false,
                is_trait: false,
                interfaces: vec![datetime_interface_sym],
                traits: Vec::new(),
                methods: HashMap::new(),
                properties: IndexMap::new(),
                constants: HashMap::new(),
                static_properties: HashMap::new(),
                allows_dynamic_properties: false,
            },
        );

        register_native_method(
            self,
            datetime_immutable_sym,
            b"__construct",
            datetime::php_datetime_construct,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetime_immutable_sym,
            b"format",
            datetime::php_datetime_format,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetime_immutable_sym,
            b"getTimestamp",
            datetime::php_datetime_get_timestamp,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetime_immutable_sym,
            b"getTimezone",
            datetime::php_datetime_get_timezone,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            datetime_immutable_sym,
            b"createFromFormat",
            datetime::php_datetime_create_from_format,
            Visibility::Public,
            true,
        );

        // DateInterval
        self.classes.insert(
            dateinterval_sym,
            ClassDef {
                name: dateinterval_sym,
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

        register_native_method(
            self,
            dateinterval_sym,
            b"__construct",
            datetime::php_dateinterval_construct,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            dateinterval_sym,
            b"format",
            datetime::php_dateinterval_format,
            Visibility::Public,
            false,
        );

        let traversable_sym = self.interner.intern(b"Traversable");
        let iterator_sym = self.interner.intern(b"Iterator");

        // DatePeriod
        self.classes.insert(
            dateperiod_sym,
            ClassDef {
                name: dateperiod_sym,
                parent: None,
                is_interface: false,
                is_trait: false,
                interfaces: vec![traversable_sym, iterator_sym],
                traits: Vec::new(),
                methods: HashMap::new(),
                properties: IndexMap::new(),
                constants: HashMap::new(),
                static_properties: HashMap::new(),
                allows_dynamic_properties: false,
            },
        );

        register_native_method(
            self,
            dateperiod_sym,
            b"__construct",
            datetime::php_dateperiod_construct,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            dateperiod_sym,
            b"getStartDate",
            datetime::php_dateperiod_get_start_date,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            dateperiod_sym,
            b"getEndDate",
            datetime::php_dateperiod_get_end_date,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            dateperiod_sym,
            b"getInterval",
            datetime::php_dateperiod_get_interval,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            dateperiod_sym,
            b"getRecurrences",
            datetime::php_dateperiod_get_recurrences,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            dateperiod_sym,
            b"current",
            datetime::php_dateperiod_current,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            dateperiod_sym,
            b"key",
            datetime::php_dateperiod_key,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            dateperiod_sym,
            b"next",
            datetime::php_dateperiod_next,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            dateperiod_sym,
            b"rewind",
            datetime::php_dateperiod_rewind,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            dateperiod_sym,
            b"valid",
            datetime::php_dateperiod_valid,
            Visibility::Public,
            false,
        );

        self.classes
            .get_mut(&dateperiod_sym)
            .unwrap()
            .constants
            .insert(
                self.interner.intern(b"EXCLUDE_START_DATE"),
                (Val::Int(1), Visibility::Public),
            );

        // Date Exceptions
        let error_sym = self.interner.intern(b"Error");
        let exception_sym = self.interner.intern(b"Exception");
        let runtime_exception_sym = self.interner.intern(b"RuntimeException");

        self.classes.insert(
            date_error_sym,
            ClassDef {
                name: date_error_sym,
                parent: Some(error_sym),
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

        self.classes.insert(
            date_object_error_sym,
            ClassDef {
                name: date_object_error_sym,
                parent: Some(date_error_sym),
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

        self.classes.insert(
            date_range_error_sym,
            ClassDef {
                name: date_range_error_sym,
                parent: Some(date_error_sym),
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

        self.classes.insert(
            date_exception_sym,
            ClassDef {
                name: date_exception_sym,
                parent: Some(exception_sym),
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

        self.classes.insert(
            date_invalid_operation_exception_sym,
            ClassDef {
                name: date_invalid_operation_exception_sym,
                parent: Some(date_exception_sym),
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

        self.classes.insert(
            date_invalid_timezone_exception_sym,
            ClassDef {
                name: date_invalid_timezone_exception_sym,
                parent: Some(date_exception_sym),
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

        self.classes.insert(
            date_malformed_interval_string_exception_sym,
            ClassDef {
                name: date_malformed_interval_string_exception_sym,
                parent: Some(date_exception_sym),
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

        self.classes.insert(
            date_malformed_period_string_exception_sym,
            ClassDef {
                name: date_malformed_period_string_exception_sym,
                parent: Some(date_exception_sym),
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

        self.classes.insert(
            date_malformed_string_exception_sym,
            ClassDef {
                name: date_malformed_string_exception_sym,
                parent: Some(date_exception_sym),
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

        // Exception and Error Classes
        //=====================================================================

        // Exception class with methods
        let exception_sym = self.interner.intern(b"Exception");

        // Add default property values
        let mut exception_props = IndexMap::new();
        let message_prop_sym = self.interner.intern(b"message");
        let code_prop_sym = self.interner.intern(b"code");
        let file_prop_sym = self.interner.intern(b"file");
        let line_prop_sym = self.interner.intern(b"line");
        let trace_prop_sym = self.interner.intern(b"trace");
        let previous_prop_sym = self.interner.intern(b"previous");

        exception_props.insert(
            message_prop_sym,
            (Val::String(Rc::new(Vec::new())), Visibility::Protected),
        );
        exception_props.insert(code_prop_sym, (Val::Int(0), Visibility::Protected));
        exception_props.insert(
            file_prop_sym,
            (
                Val::String(Rc::new(b"unknown".to_vec())),
                Visibility::Protected,
            ),
        );
        exception_props.insert(line_prop_sym, (Val::Int(0), Visibility::Protected));
        exception_props.insert(
            trace_prop_sym,
            (
                Val::Array(crate::core::value::ArrayData::new().into()),
                Visibility::Private,
            ),
        );
        exception_props.insert(previous_prop_sym, (Val::Null, Visibility::Private));

        self.classes.insert(
            exception_sym,
            ClassDef {
                name: exception_sym,
                parent: None,
                is_interface: false,
                is_trait: false,
                interfaces: vec![throwable_sym],
                traits: Vec::new(),
                methods: HashMap::new(),
                properties: exception_props,
                constants: HashMap::new(),
                static_properties: HashMap::new(),
                allows_dynamic_properties: false,
            },
        );

        // Register exception native methods
        register_native_method(
            self,
            exception_sym,
            b"__construct",
            exception::exception_construct,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            exception_sym,
            b"getMessage",
            exception::exception_get_message,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            exception_sym,
            b"getCode",
            exception::exception_get_code,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            exception_sym,
            b"getFile",
            exception::exception_get_file,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            exception_sym,
            b"getLine",
            exception::exception_get_line,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            exception_sym,
            b"getTrace",
            exception::exception_get_trace,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            exception_sym,
            b"getTraceAsString",
            exception::exception_get_trace_as_string,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            exception_sym,
            b"getPrevious",
            exception::exception_get_previous,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            exception_sym,
            b"__toString",
            exception::exception_to_string,
            Visibility::Public,
            false,
        );

        // Error class (PHP 7+) - has same methods as Exception
        let error_sym = self.interner.intern(b"Error");

        // Error has same properties as Exception
        let mut error_props = IndexMap::new();
        error_props.insert(
            message_prop_sym,
            (Val::String(Rc::new(Vec::new())), Visibility::Protected),
        );
        error_props.insert(code_prop_sym, (Val::Int(0), Visibility::Protected));
        error_props.insert(
            file_prop_sym,
            (
                Val::String(Rc::new(b"unknown".to_vec())),
                Visibility::Protected,
            ),
        );
        error_props.insert(line_prop_sym, (Val::Int(0), Visibility::Protected));
        error_props.insert(
            trace_prop_sym,
            (
                Val::Array(crate::core::value::ArrayData::new().into()),
                Visibility::Private,
            ),
        );
        error_props.insert(previous_prop_sym, (Val::Null, Visibility::Private));

        self.classes.insert(
            error_sym,
            ClassDef {
                name: error_sym,
                parent: None,
                is_interface: false,
                is_trait: false,
                interfaces: vec![throwable_sym],
                traits: Vec::new(),
                methods: HashMap::new(),
                properties: error_props,
                constants: HashMap::new(),
                static_properties: HashMap::new(),
                allows_dynamic_properties: false,
            },
        );

        // Register Error native methods (same as Exception)
        register_native_method(
            self,
            error_sym,
            b"__construct",
            exception::exception_construct,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            error_sym,
            b"getMessage",
            exception::exception_get_message,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            error_sym,
            b"getCode",
            exception::exception_get_code,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            error_sym,
            b"getFile",
            exception::exception_get_file,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            error_sym,
            b"getLine",
            exception::exception_get_line,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            error_sym,
            b"getTrace",
            exception::exception_get_trace,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            error_sym,
            b"getTraceAsString",
            exception::exception_get_trace_as_string,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            error_sym,
            b"getPrevious",
            exception::exception_get_previous,
            Visibility::Public,
            false,
        );
        register_native_method(
            self,
            error_sym,
            b"__toString",
            exception::exception_to_string,
            Visibility::Public,
            false,
        );

        // RuntimeException
        let runtime_exception_sym = self.interner.intern(b"RuntimeException");
        self.classes.insert(
            runtime_exception_sym,
            ClassDef {
                name: runtime_exception_sym,
                parent: Some(exception_sym),
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

        // LogicException
        let logic_exception_sym = self.interner.intern(b"LogicException");
        self.classes.insert(
            logic_exception_sym,
            ClassDef {
                name: logic_exception_sym,
                parent: Some(exception_sym),
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

        // TypeError (extends Error)
        let type_error_sym = self.interner.intern(b"TypeError");
        self.classes.insert(
            type_error_sym,
            ClassDef {
                name: type_error_sym,
                parent: Some(error_sym),
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

        // ArithmeticError (extends Error)
        let arithmetic_error_sym = self.interner.intern(b"ArithmeticError");
        self.classes.insert(
            arithmetic_error_sym,
            ClassDef {
                name: arithmetic_error_sym,
                parent: Some(error_sym),
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

        // DivisionByZeroError (extends ArithmeticError)
        let division_by_zero_sym = self.interner.intern(b"DivisionByZeroError");
        self.classes.insert(
            division_by_zero_sym,
            ClassDef {
                name: division_by_zero_sym,
                parent: Some(arithmetic_error_sym),
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

        // Output Control constants - Phase flags
        self.insert_builtin_constant(
            b"PHP_OUTPUT_HANDLER_START",
            Val::Int(output_control::PHP_OUTPUT_HANDLER_START),
        );
        self.insert_builtin_constant(
            b"PHP_OUTPUT_HANDLER_WRITE",
            Val::Int(output_control::PHP_OUTPUT_HANDLER_WRITE),
        );
        self.insert_builtin_constant(
            b"PHP_OUTPUT_HANDLER_FLUSH",
            Val::Int(output_control::PHP_OUTPUT_HANDLER_FLUSH),
        );
        self.insert_builtin_constant(
            b"PHP_OUTPUT_HANDLER_CLEAN",
            Val::Int(output_control::PHP_OUTPUT_HANDLER_CLEAN),
        );
        self.insert_builtin_constant(
            b"PHP_OUTPUT_HANDLER_FINAL",
            Val::Int(output_control::PHP_OUTPUT_HANDLER_FINAL),
        );
        self.insert_builtin_constant(
            b"PHP_OUTPUT_HANDLER_CONT",
            Val::Int(output_control::PHP_OUTPUT_HANDLER_CONT),
        );
        self.insert_builtin_constant(
            b"PHP_OUTPUT_HANDLER_END",
            Val::Int(output_control::PHP_OUTPUT_HANDLER_END),
        );

        // Output Control constants - Control flags
        self.insert_builtin_constant(
            b"PHP_OUTPUT_HANDLER_CLEANABLE",
            Val::Int(output_control::PHP_OUTPUT_HANDLER_CLEANABLE),
        );
        self.insert_builtin_constant(
            b"PHP_OUTPUT_HANDLER_FLUSHABLE",
            Val::Int(output_control::PHP_OUTPUT_HANDLER_FLUSHABLE),
        );
        self.insert_builtin_constant(
            b"PHP_OUTPUT_HANDLER_REMOVABLE",
            Val::Int(output_control::PHP_OUTPUT_HANDLER_REMOVABLE),
        );
        self.insert_builtin_constant(
            b"PHP_OUTPUT_HANDLER_STDFLAGS",
            Val::Int(output_control::PHP_OUTPUT_HANDLER_STDFLAGS),
        );

        // Output Control constants - Status flags
        self.insert_builtin_constant(
            b"PHP_OUTPUT_HANDLER_STARTED",
            Val::Int(output_control::PHP_OUTPUT_HANDLER_STARTED),
        );
        self.insert_builtin_constant(
            b"PHP_OUTPUT_HANDLER_DISABLED",
            Val::Int(output_control::PHP_OUTPUT_HANDLER_DISABLED),
        );
        self.insert_builtin_constant(
            b"PHP_OUTPUT_HANDLER_PROCESSED",
            Val::Int(output_control::PHP_OUTPUT_HANDLER_PROCESSED),
        );

        // URL constants
        self.insert_builtin_constant(b"PHP_URL_SCHEME", Val::Int(url::PHP_URL_SCHEME));
        self.insert_builtin_constant(b"PHP_URL_HOST", Val::Int(url::PHP_URL_HOST));
        self.insert_builtin_constant(b"PHP_URL_PORT", Val::Int(url::PHP_URL_PORT));
        self.insert_builtin_constant(b"PHP_URL_USER", Val::Int(url::PHP_URL_USER));
        self.insert_builtin_constant(b"PHP_URL_PASS", Val::Int(url::PHP_URL_PASS));
        self.insert_builtin_constant(b"PHP_URL_PATH", Val::Int(url::PHP_URL_PATH));
        self.insert_builtin_constant(b"PHP_URL_QUERY", Val::Int(url::PHP_URL_QUERY));
        self.insert_builtin_constant(b"PHP_URL_FRAGMENT", Val::Int(url::PHP_URL_FRAGMENT));
        self.insert_builtin_constant(b"PHP_QUERY_RFC1738", Val::Int(url::PHP_QUERY_RFC1738));
        self.insert_builtin_constant(b"PHP_QUERY_RFC3986", Val::Int(url::PHP_QUERY_RFC3986));

        // Date constants
        self.insert_builtin_constant(
            b"DATE_ATOM",
            Val::String(Rc::new(datetime::DATE_ATOM.as_bytes().to_vec())),
        );
        self.insert_builtin_constant(
            b"DATE_COOKIE",
            Val::String(Rc::new(datetime::DATE_COOKIE.as_bytes().to_vec())),
        );
        self.insert_builtin_constant(
            b"DATE_ISO8601",
            Val::String(Rc::new(datetime::DATE_ISO8601.as_bytes().to_vec())),
        );
        self.insert_builtin_constant(
            b"DATE_ISO8601_EXPANDED",
            Val::String(Rc::new(datetime::DATE_ISO8601_EXPANDED.as_bytes().to_vec())),
        );
        self.insert_builtin_constant(
            b"DATE_RFC822",
            Val::String(Rc::new(datetime::DATE_RFC822.as_bytes().to_vec())),
        );
        self.insert_builtin_constant(
            b"DATE_RFC850",
            Val::String(Rc::new(datetime::DATE_RFC850.as_bytes().to_vec())),
        );
        self.insert_builtin_constant(
            b"DATE_RFC1036",
            Val::String(Rc::new(datetime::DATE_RFC1036.as_bytes().to_vec())),
        );
        self.insert_builtin_constant(
            b"DATE_RFC1123",
            Val::String(Rc::new(datetime::DATE_RFC1123.as_bytes().to_vec())),
        );
        self.insert_builtin_constant(
            b"DATE_RFC7231",
            Val::String(Rc::new(datetime::DATE_RFC7231.as_bytes().to_vec())),
        );
        self.insert_builtin_constant(
            b"DATE_RFC2822",
            Val::String(Rc::new(datetime::DATE_RFC2822.as_bytes().to_vec())),
        );
        self.insert_builtin_constant(
            b"DATE_RFC3339",
            Val::String(Rc::new(datetime::DATE_RFC3339.as_bytes().to_vec())),
        );
        self.insert_builtin_constant(
            b"DATE_RFC3339_EXTENDED",
            Val::String(Rc::new(datetime::DATE_RFC3339_EXTENDED.as_bytes().to_vec())),
        );
        self.insert_builtin_constant(
            b"DATE_RSS",
            Val::String(Rc::new(datetime::DATE_RSS.as_bytes().to_vec())),
        );
        self.insert_builtin_constant(
            b"DATE_W3C",
            Val::String(Rc::new(datetime::DATE_W3C.as_bytes().to_vec())),
        );

        self.insert_builtin_constant(
            b"SUNFUNCS_RET_TIMESTAMP",
            Val::Int(datetime::SUNFUNCS_RET_TIMESTAMP),
        );
        self.insert_builtin_constant(
            b"SUNFUNCS_RET_STRING",
            Val::Int(datetime::SUNFUNCS_RET_STRING),
        );
        self.insert_builtin_constant(
            b"SUNFUNCS_RET_DOUBLE",
            Val::Int(datetime::SUNFUNCS_RET_DOUBLE),
        );

        // Error reporting constants
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

        // Copy extension-registered constants from registry
        for (name_bytes, value) in self.engine.registry.constants() {
            let sym = self.interner.intern(name_bytes);
            self.constants.insert(sym, value.clone());
        }
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
            pdo_driver_registry: None,
        }))
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}
