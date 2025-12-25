use crate::builtins::{
    array, bcmath, class, datetime, exception, exec, filesystem, function, hash, http, json, math,
    output_control, pcre, spl, string, url, variable,
};
use crate::core::value::{Symbol, Val, Visibility};
use crate::runtime::extension::{Extension, ExtensionInfo, ExtensionResult};
use crate::runtime::registry::{ExtensionRegistry, NativeClassDef, NativeMethodEntry};
use std::collections::HashMap;

/// Core extension providing built-in PHP functions
pub struct CoreExtension;

impl Extension for CoreExtension {
    fn info(&self) -> ExtensionInfo {
        ExtensionInfo {
            name: "Core",
            version: "8.3.0",
            dependencies: &[],
        }
    }

    fn module_init(&self, registry: &mut ExtensionRegistry) -> ExtensionResult {
        // String functions
        registry.register_function(b"strlen", string::php_strlen);
        registry.register_function(b"str_repeat", string::php_str_repeat);
        registry.register_function(b"substr", string::php_substr);
        registry.register_function(b"substr_replace", string::php_substr_replace);
        registry.register_function(b"strpos", string::php_strpos);
        registry.register_function(b"strtr", string::php_strtr);
        registry.register_function(b"trim", string::php_trim);
        registry.register_function(b"ltrim", string::php_ltrim);
        registry.register_function(b"rtrim", string::php_rtrim);
        registry.register_function(b"chr", string::php_chr);
        registry.register_function(b"ord", string::php_ord);
        registry.register_function(b"bin2hex", string::php_bin2hex);
        registry.register_function(b"hex2bin", string::php_hex2bin);
        registry.register_function(b"addslashes", string::php_addslashes);
        registry.register_function(b"stripslashes", string::php_stripslashes);
        registry.register_function(b"addcslashes", string::php_addcslashes);
        registry.register_function(b"stripcslashes", string::php_stripcslashes);
        registry.register_function(b"str_pad", string::php_str_pad);
        registry.register_function(b"str_rot13", string::php_str_rot13);
        registry.register_function(b"str_shuffle", string::php_str_shuffle);
        registry.register_function(b"str_split", string::php_str_split);
        registry.register_function(b"strrev", string::php_strrev);
        registry.register_function(b"strcmp", string::php_strcmp);
        registry.register_function(b"strcasecmp", string::php_strcasecmp);
        registry.register_function(b"strncmp", string::php_strncmp);
        registry.register_function(b"strncasecmp", string::php_strncasecmp);
        registry.register_function(b"strstr", string::php_strstr);
        registry.register_function(b"stristr", string::php_stristr);
        registry.register_function(b"substr_count", string::php_substr_count);
        registry.register_function(b"ucfirst", string::php_ucfirst);
        registry.register_function(b"lcfirst", string::php_lcfirst);
        registry.register_function(b"ucwords", string::php_ucwords);
        registry.register_function(b"wordwrap", string::php_wordwrap);
        registry.register_function(b"strtok", string::php_strtok);
        registry.register_function(b"str_contains", string::php_str_contains);
        registry.register_function(b"str_starts_with", string::php_str_starts_with);
        registry.register_function(b"str_ends_with", string::php_str_ends_with);
        registry.register_function_with_by_ref(b"str_replace", string::php_str_replace, vec![3]);
        registry.register_function_with_by_ref(b"str_ireplace", string::php_str_ireplace, vec![3]);
        registry.register_function(b"strtolower", string::php_strtolower);
        registry.register_function(b"strtoupper", string::php_strtoupper);
        registry.register_function(b"version_compare", string::php_version_compare);
        registry.register_function(b"implode", string::php_implode);
        registry.register_function(b"explode", string::php_explode);
        registry.register_function(b"sprintf", string::php_sprintf);
        registry.register_function(b"printf", string::php_printf);

        // Array functions
        registry.register_function(b"array_merge", array::php_array_merge);
        registry.register_function(b"array_keys", array::php_array_keys);
        registry.register_function(b"array_values", array::php_array_values);
        registry.register_function(b"in_array", array::php_in_array);
        registry.register_function(b"ksort", array::php_ksort);
        registry.register_function(b"array_unshift", array::php_array_unshift);
        registry.register_function(b"current", array::php_current);
        registry.register_function(b"next", array::php_next);
        registry.register_function(b"reset", array::php_reset);
        registry.register_function(b"end", array::php_end);
        registry.register_function(b"array_key_exists", array::php_array_key_exists);
        registry.register_function(b"count", array::php_count);

        // Variable functions
        registry.register_function(b"var_dump", variable::php_var_dump);
        registry.register_function(b"print_r", variable::php_print_r);
        registry.register_function(b"is_string", variable::php_is_string);
        registry.register_function(b"is_int", variable::php_is_int);
        registry.register_function(b"is_array", variable::php_is_array);
        registry.register_function(b"is_bool", variable::php_is_bool);
        registry.register_function(b"is_null", variable::php_is_null);
        registry.register_function(b"is_object", variable::php_is_object);
        registry.register_function(b"is_float", variable::php_is_float);
        registry.register_function(b"is_numeric", variable::php_is_numeric);
        registry.register_function(b"is_scalar", variable::php_is_scalar);
        registry.register_function(b"define", variable::php_define);
        registry.register_function(b"defined", variable::php_defined);
        registry.register_function(b"constant", variable::php_constant);
        registry.register_function(b"gettype", variable::php_gettype);
        registry.register_function(b"var_export", variable::php_var_export);
        registry.register_function(b"getenv", variable::php_getenv);
        registry.register_function(b"putenv", variable::php_putenv);
        registry.register_function(b"getopt", variable::php_getopt);
        registry.register_function(b"ini_get", variable::php_ini_get);
        registry.register_function(b"ini_set", variable::php_ini_set);
        registry.register_function(b"error_reporting", variable::php_error_reporting);
        registry.register_function(b"error_get_last", variable::php_error_get_last);

        // HTTP functions
        registry.register_function(b"header", http::php_header);
        registry.register_function(b"headers_sent", http::php_headers_sent);
        registry.register_function(b"header_remove", http::php_header_remove);

        // URL functions
        registry.register_function(b"urlencode", url::php_urlencode);
        registry.register_function(b"urldecode", url::php_urldecode);
        registry.register_function(b"rawurlencode", url::php_rawurlencode);
        registry.register_function(b"rawurldecode", url::php_rawurldecode);
        registry.register_function(b"base64_encode", url::php_base64_encode);
        registry.register_function(b"base64_decode", url::php_base64_decode);
        registry.register_function(b"parse_url", url::php_parse_url);
        registry.register_function(b"http_build_query", url::php_http_build_query);
        registry.register_function(b"get_headers", url::php_get_headers);
        registry.register_function(b"get_meta_tags", url::php_get_meta_tags);

        // Math functions
        registry.register_function(b"abs", math::php_abs);
        registry.register_function(b"max", math::php_max);
        registry.register_function(b"min", math::php_min);

        // BCMath functions
        registry.register_function(b"bcadd", bcmath::bcadd);
        registry.register_function(b"bcsub", bcmath::bcsub);
        registry.register_function(b"bcmul", bcmath::bcmul);
        registry.register_function(b"bcdiv", bcmath::bcdiv);

        // Class functions
        registry.register_function(b"get_object_vars", class::php_get_object_vars);
        registry.register_function(b"get_class", class::php_get_class);
        registry.register_function(b"get_parent_class", class::php_get_parent_class);
        registry.register_function(b"is_subclass_of", class::php_is_subclass_of);
        registry.register_function(b"is_a", class::php_is_a);
        registry.register_function(b"class_exists", class::php_class_exists);
        registry.register_function(b"interface_exists", class::php_interface_exists);
        registry.register_function(b"trait_exists", class::php_trait_exists);
        registry.register_function(b"method_exists", class::php_method_exists);
        registry.register_function(b"property_exists", class::php_property_exists);
        registry.register_function(b"get_class_methods", class::php_get_class_methods);
        registry.register_function(b"get_class_vars", class::php_get_class_vars);
        registry.register_function(b"get_called_class", class::php_get_called_class);

        // PCRE functions
        registry.register_function(b"preg_match", pcre::preg_match);
        registry.register_function(b"preg_replace", pcre::preg_replace);
        registry.register_function(b"preg_split", pcre::preg_split);
        registry.register_function(b"preg_quote", pcre::preg_quote);

        // Function handling functions
        registry.register_function(b"func_get_args", function::php_func_get_args);
        registry.register_function(b"func_num_args", function::php_func_num_args);
        registry.register_function(b"func_get_arg", function::php_func_get_arg);
        registry.register_function(b"function_exists", function::php_function_exists);
        registry.register_function(b"is_callable", function::php_is_callable);
        registry.register_function(b"call_user_func", function::php_call_user_func);
        registry.register_function(b"call_user_func_array", function::php_call_user_func_array);
        registry.register_function(b"extension_loaded", function::php_extension_loaded);
        registry.register_function(b"spl_autoload_register", spl::php_spl_autoload_register);
        registry.register_function(b"spl_object_hash", spl::php_spl_object_hash);
        registry.register_function(b"assert", function::php_assert);

        // Filesystem functions - File I/O
        registry.register_function(b"fopen", filesystem::php_fopen);
        registry.register_function(b"fclose", filesystem::php_fclose);
        registry.register_function(b"fread", filesystem::php_fread);
        registry.register_function(b"fwrite", filesystem::php_fwrite);
        registry.register_function(b"fputs", filesystem::php_fputs);
        registry.register_function(b"fgets", filesystem::php_fgets);
        registry.register_function(b"fgetc", filesystem::php_fgetc);
        registry.register_function(b"fseek", filesystem::php_fseek);
        registry.register_function(b"ftell", filesystem::php_ftell);
        registry.register_function(b"rewind", filesystem::php_rewind);
        registry.register_function(b"feof", filesystem::php_feof);
        registry.register_function(b"fflush", filesystem::php_fflush);

        // Filesystem functions - File content
        registry.register_function(b"file_get_contents", filesystem::php_file_get_contents);
        registry.register_function(b"file_put_contents", filesystem::php_file_put_contents);
        registry.register_function(b"file", filesystem::php_file);

        // Filesystem functions - File information
        registry.register_function(b"file_exists", filesystem::php_file_exists);
        registry.register_function(b"is_file", filesystem::php_is_file);
        registry.register_function(b"is_dir", filesystem::php_is_dir);
        registry.register_function(b"is_link", filesystem::php_is_link);
        registry.register_function(b"is_readable", filesystem::php_is_readable);
        registry.register_function(b"is_writable", filesystem::php_is_writable);
        registry.register_function(b"is_writeable", filesystem::php_is_writable); // Alias
        registry.register_function(b"is_executable", filesystem::php_is_executable);

        // Filesystem functions - File metadata
        registry.register_function(b"filesize", filesystem::php_filesize);
        registry.register_function(b"filemtime", filesystem::php_filemtime);
        registry.register_function(b"filectime", filesystem::php_filectime);
        registry.register_function(b"fileatime", filesystem::php_fileatime);
        registry.register_function(b"fileperms", filesystem::php_fileperms);
        registry.register_function(b"fileowner", filesystem::php_fileowner);
        registry.register_function(b"filegroup", filesystem::php_filegroup);
        registry.register_function(b"stat", filesystem::php_stat);
        registry.register_function(b"lstat", filesystem::php_lstat);

        // Filesystem functions - File operations
        registry.register_function(b"unlink", filesystem::php_unlink);
        registry.register_function(b"rename", filesystem::php_rename);
        registry.register_function(b"copy", filesystem::php_copy);
        registry.register_function(b"touch", filesystem::php_touch);
        registry.register_function(b"chmod", filesystem::php_chmod);
        registry.register_function(b"readlink", filesystem::php_readlink);
        registry.register_function(b"realpath", filesystem::php_realpath);

        // Filesystem functions - Directory operations
        registry.register_function(b"mkdir", filesystem::php_mkdir);
        registry.register_function(b"rmdir", filesystem::php_rmdir);
        registry.register_function(b"scandir", filesystem::php_scandir);
        registry.register_function(b"getcwd", filesystem::php_getcwd);
        registry.register_function(b"chdir", filesystem::php_chdir);

        // Filesystem functions - Path operations
        registry.register_function(b"basename", filesystem::php_basename);
        registry.register_function(b"dirname", filesystem::php_dirname);

        // Filesystem functions - Temporary files
        registry.register_function(b"sys_get_temp_dir", filesystem::php_sys_get_temp_dir);
        registry.register_function(b"tmpfile", filesystem::php_tmpfile);
        registry.register_function(b"tempnam", filesystem::php_tempnam);

        // Filesystem functions - Disk space
        registry.register_function(b"disk_free_space", filesystem::php_disk_free_space);
        registry.register_function(b"disk_total_space", filesystem::php_disk_total_space);

        // Execution functions
        registry.register_function(b"escapeshellarg", exec::php_escapeshellarg);
        registry.register_function(b"escapeshellcmd", exec::php_escapeshellcmd);
        registry.register_function(b"exec", exec::php_exec);
        registry.register_function(b"passthru", exec::php_passthru);
        registry.register_function(b"shell_exec", exec::php_shell_exec);
        registry.register_function(b"system", exec::php_system);
        registry.register_function(b"proc_open", exec::php_proc_open);
        registry.register_function(b"proc_close", exec::php_proc_close);
        registry.register_function(b"proc_terminate", exec::php_proc_terminate);
        registry.register_function(b"proc_nice", exec::php_proc_nice);
        registry.register_function(b"proc_get_status", exec::php_proc_get_status);
        registry.register_function(b"set_time_limit", exec::php_set_time_limit);

        // Date/Time functions
        registry.register_function(b"date_default_timezone_get", datetime::php_date_default_timezone_get);
        registry.register_function(b"date", datetime::php_date);
        registry.register_function(b"gmdate", datetime::php_gmdate);
        registry.register_function(b"time", datetime::php_time);
        registry.register_function(b"microtime", datetime::php_microtime);
        registry.register_function(b"gettimeofday", datetime::php_gettimeofday);
        registry.register_function(b"localtime", datetime::php_localtime);
        registry.register_function(b"strtotime", datetime::php_strtotime);
        registry.register_function(b"mktime", datetime::php_mktime);
        registry.register_function(b"gmmktime", datetime::php_gmmktime);
        registry.register_function(b"getdate", datetime::php_getdate);
        registry.register_function(b"idate", datetime::php_idate);
        registry.register_function(b"date_parse", datetime::php_date_parse);
        registry.register_function(b"date_parse_from_format", datetime::php_date_parse_from_format);
        registry.register_function(b"date_create", datetime::php_date_create);
        registry.register_function(b"date_create_from_format", datetime::php_datetime_create_from_format);
        registry.register_function(b"date_format", datetime::php_date_format);
        registry.register_function(b"date_modify", datetime::php_date_modify);
        registry.register_function(b"date_add", datetime::php_date_add);
        registry.register_function(b"date_sub", datetime::php_date_sub);

        registry.register_function(b"date_diff", datetime::php_date_diff);
        registry.register_function(b"date_interval_create_from_date_string", datetime::php_date_interval_create_from_date_string);
        registry.register_function(b"date_interval_format", datetime::php_dateinterval_format);
        registry.register_function(b"checkdate", datetime::php_checkdate);
        registry.register_function(b"timezone_open", datetime::php_timezone_open);

        // Register DateTime classes
        
        // DateTime class
        let mut datetime_methods = HashMap::new();
        datetime_methods.insert(
            b"__construct".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetime_construct,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        datetime_methods.insert(
            b"format".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetime_format,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        datetime_methods.insert(
            b"setTimezone".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetime_set_timezone,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        datetime_methods.insert(
            b"add".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetime_add,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        datetime_methods.insert(
            b"sub".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetime_sub,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        datetime_methods.insert(
            b"diff".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetime_diff,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        registry.register_class(NativeClassDef {
            name: b"DateTime".to_vec(),
            parent: None,
            interfaces: vec![],
            methods: datetime_methods,
            constants: HashMap::new(),
            constructor: Some(datetime::php_datetime_construct),
        });

        // DateTimeZone class
        let mut datetimezone_methods = HashMap::new();
        datetimezone_methods.insert(
            b"__construct".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetimezone_construct,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        datetimezone_methods.insert(
            b"getName".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetimezone_get_name,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        registry.register_class(NativeClassDef {
            name: b"DateTimeZone".to_vec(),
            parent: None,
            interfaces: vec![],
            methods: datetimezone_methods,
            constants: HashMap::new(),
            constructor: Some(datetime::php_datetimezone_construct),
        });

        // DateInterval class
        let mut dateinterval_methods = HashMap::new();
        dateinterval_methods.insert(
            b"__construct".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_dateinterval_construct,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        dateinterval_methods.insert(
            b"format".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_dateinterval_format,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        registry.register_class(NativeClassDef {
            name: b"DateInterval".to_vec(),
            parent: None,
            interfaces: vec![],
            methods: dateinterval_methods,
            constants: HashMap::new(),
            constructor: Some(datetime::php_dateinterval_construct),
        });

        // DatePeriod class
        let mut dateperiod_methods = HashMap::new();
        dateperiod_methods.insert(
            b"__construct".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_dateperiod_construct,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        // Iterator methods
        dateperiod_methods.insert(
            b"rewind".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_dateperiod_rewind,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        dateperiod_methods.insert(
            b"valid".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_dateperiod_valid,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        dateperiod_methods.insert(
            b"current".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_dateperiod_current,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        dateperiod_methods.insert(
            b"key".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_dateperiod_key,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        dateperiod_methods.insert(
            b"next".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_dateperiod_next,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        registry.register_class(NativeClassDef {
            name: b"DatePeriod".to_vec(),
            parent: None,
            interfaces: vec![b"Iterator".to_vec()],
            methods: dateperiod_methods,
            constants: HashMap::new(),
            constructor: Some(datetime::php_dateperiod_construct),
        });

        // Output Control functions
        registry.register_function(b"ob_start", output_control::php_ob_start);
        registry.register_function(b"ob_end_clean", output_control::php_ob_end_clean);
        registry.register_function(b"ob_end_flush", output_control::php_ob_end_flush);
        registry.register_function(b"ob_clean", output_control::php_ob_clean);
        registry.register_function(b"ob_flush", output_control::php_ob_flush);
        registry.register_function(b"ob_get_contents", output_control::php_ob_get_contents);
        registry.register_function(b"ob_get_clean", output_control::php_ob_get_clean);
        registry.register_function(b"ob_get_flush", output_control::php_ob_get_flush);
        registry.register_function(b"ob_get_length", output_control::php_ob_get_length);
        registry.register_function(b"ob_get_level", output_control::php_ob_get_level);
        registry.register_function(b"ob_get_status", output_control::php_ob_get_status);
        registry.register_function(b"ob_implicit_flush", output_control::php_ob_implicit_flush);
        registry.register_function(b"ob_list_handlers", output_control::php_ob_list_handlers);
        registry.register_function(b"output_add_rewrite_var", output_control::php_output_add_rewrite_var);
        registry.register_function(b"output_reset_rewrite_vars", output_control::php_output_reset_rewrite_vars);

        // Register core string constants
        registry.register_constant(b"STR_PAD_LEFT", Val::Int(0));
        registry.register_constant(b"STR_PAD_RIGHT", Val::Int(1));
        registry.register_constant(b"STR_PAD_BOTH", Val::Int(2));

        ExtensionResult::Success
    }
}
