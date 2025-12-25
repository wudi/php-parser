/// Date/Time extension - PHP's date and time functionality
///
/// This extension provides PHP's core date/time classes and functions:
/// - DateTime, DateTimeImmutable, DateTimeZone
/// - DateInterval, DatePeriod
/// - Date-related exception hierarchy
/// - date(), time(), strtotime(), and other procedural functions
///
/// # Classes
/// - `DateTime` - Mutable date/time representation
/// - `DateTimeImmutable` - Immutable date/time representation
/// - `DateTimeZone` - Timezone representation
/// - `DateInterval` - Time interval specification
/// - `DatePeriod` - Iterator for recurring dates
/// - `DateTimeInterface` - Interface for DateTime objects
///
/// # Exceptions
/// - `DateError` - Base error class
/// - `DateException` - Base exception class
/// - `DateObjectError`, `DateRangeError` (errors)
/// - `DateInvalidOperationException`, `DateInvalidTimeZoneException`
/// - `DateMalformedIntervalStringException`, `DateMalformedPeriodStringException`
/// - `DateMalformedStringException`
///
/// # Functions (to be implemented)
/// - `date()`, `gmdate()`, `idate()`, `strtotime()`
/// - `mktime()`, `gmmktime()`, `time()`
/// - `getdate()`, `localtime()`, `checkdate()`
/// - `date_parse()`, `date_parse_from_format()`
/// - `timezone_open()`, `timezone_name_get()`, etc.

use crate::builtins::datetime;
use crate::core::value::{Val, Visibility};
use crate::runtime::context::RequestContext;
use crate::runtime::extension::{Extension, ExtensionInfo, ExtensionResult};
use crate::runtime::registry::{ExtensionRegistry, NativeClassDef, NativeMethodEntry};
use std::collections::HashMap;
use std::rc::Rc;

pub struct DateExtension;

impl Extension for DateExtension {
    fn info(&self) -> ExtensionInfo {
        ExtensionInfo {
            name: "date",
            version: "8.3.0",
            dependencies: &[],
        }
    }

    fn module_init(&self, registry: &mut ExtensionRegistry) -> ExtensionResult {
        // ========================================
        // DATE/TIME INTERFACES
        // ========================================

        // DateTimeInterface
        registry.register_class(NativeClassDef {
            name: b"DateTimeInterface".to_vec(),
            parent: None,
            is_interface: true,
            is_trait: false,
            interfaces: vec![],
            methods: HashMap::new(),
            constants: HashMap::new(),
            constructor: None,
        });

        // ========================================
        // DATE/TIME CLASSES
        // ========================================

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
        datetimezone_methods.insert(
            b"getOffset".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetimezone_get_offset,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        datetimezone_methods.insert(
            b"getLocation".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetimezone_get_location,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        datetimezone_methods.insert(
            b"listIdentifiers".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetimezone_list_identifiers,
                visibility: Visibility::Public,
                is_static: true,
            },
        );
        registry.register_class(NativeClassDef {
            name: b"DateTimeZone".to_vec(),
            parent: None,
            is_interface: false,
            is_trait: false,
            interfaces: vec![],
            methods: datetimezone_methods,
            constants: HashMap::new(),
            constructor: Some(datetime::php_datetimezone_construct),
        });

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
            b"getTimestamp".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetime_get_timestamp,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        datetime_methods.insert(
            b"setTimestamp".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetime_set_timestamp,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        datetime_methods.insert(
            b"getTimezone".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetime_get_timezone,
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
        datetime_methods.insert(
            b"modify".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetime_modify,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        datetime_methods.insert(
            b"createFromFormat".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_datetime_create_from_format,
                visibility: Visibility::Public,
                is_static: true,
            },
        );
        registry.register_class(NativeClassDef {
            name: b"DateTime".to_vec(),
            parent: None,
            is_interface: false,
            is_trait: false,
            interfaces: vec![b"DateTimeInterface".to_vec()],
            methods: datetime_methods.clone(),
            constants: HashMap::new(),
            constructor: Some(datetime::php_datetime_construct),
        });

        // DateTimeImmutable class (shares same methods as DateTime)
        registry.register_class(NativeClassDef {
            name: b"DateTimeImmutable".to_vec(),
            parent: None,
            is_interface: false,
            is_trait: false,
            interfaces: vec![b"DateTimeInterface".to_vec()],
            methods: datetime_methods,
            constants: HashMap::new(),
            constructor: Some(datetime::php_datetime_construct),
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
            is_interface: false,
            is_trait: false,
            interfaces: vec![],
            methods: dateinterval_methods,
            constants: HashMap::new(),
            constructor: Some(datetime::php_dateinterval_construct),
        });

        // DatePeriod class (implements Iterator)
        let mut dateperiod_methods = HashMap::new();
        dateperiod_methods.insert(
            b"__construct".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_dateperiod_construct,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        dateperiod_methods.insert(
            b"getStartDate".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_dateperiod_get_start_date,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        dateperiod_methods.insert(
            b"getEndDate".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_dateperiod_get_end_date,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        dateperiod_methods.insert(
            b"getInterval".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_dateperiod_get_interval,
                visibility: Visibility::Public,
                is_static: false,
            },
        );
        dateperiod_methods.insert(
            b"getRecurrences".to_vec(),
            NativeMethodEntry {
                handler: datetime::php_dateperiod_get_recurrences,
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
        
        let mut dateperiod_constants = HashMap::new();
        dateperiod_constants.insert(
            b"EXCLUDE_START_DATE".to_vec(),
            (Val::Int(1), Visibility::Public),
        );
        
        registry.register_class(NativeClassDef {
            name: b"DatePeriod".to_vec(),
            parent: None,
            is_interface: false,
            is_trait: false,
            interfaces: vec![b"Iterator".to_vec()],
            methods: dateperiod_methods,
            constants: dateperiod_constants,
            constructor: Some(datetime::php_dateperiod_construct),
        });

        // ========================================
        // DATE/TIME EXCEPTIONS
        // ========================================

        // DateError (extends Error)
        registry.register_class(NativeClassDef {
            name: b"DateError".to_vec(),
            parent: Some(b"Error".to_vec()),
            is_interface: false,
            is_trait: false,
            interfaces: vec![],
            methods: HashMap::new(),
            constants: HashMap::new(),
            constructor: None,
        });

        // DateObjectError (extends DateError)
        registry.register_class(NativeClassDef {
            name: b"DateObjectError".to_vec(),
            parent: Some(b"DateError".to_vec()),
            is_interface: false,
            is_trait: false,
            interfaces: vec![],
            methods: HashMap::new(),
            constants: HashMap::new(),
            constructor: None,
        });

        // DateRangeError (extends DateError)
        registry.register_class(NativeClassDef {
            name: b"DateRangeError".to_vec(),
            parent: Some(b"DateError".to_vec()),
            is_interface: false,
            is_trait: false,
            interfaces: vec![],
            methods: HashMap::new(),
            constants: HashMap::new(),
            constructor: None,
        });

        // DateException (extends Exception)
        registry.register_class(NativeClassDef {
            name: b"DateException".to_vec(),
            parent: Some(b"Exception".to_vec()),
            is_interface: false,
            is_trait: false,
            interfaces: vec![],
            methods: HashMap::new(),
            constants: HashMap::new(),
            constructor: None,
        });

        // DateInvalidOperationException (extends DateException)
        registry.register_class(NativeClassDef {
            name: b"DateInvalidOperationException".to_vec(),
            parent: Some(b"DateException".to_vec()),
            is_interface: false,
            is_trait: false,
            interfaces: vec![],
            methods: HashMap::new(),
            constants: HashMap::new(),
            constructor: None,
        });

        // DateInvalidTimeZoneException (extends DateException)
        registry.register_class(NativeClassDef {
            name: b"DateInvalidTimeZoneException".to_vec(),
            parent: Some(b"DateException".to_vec()),
            is_interface: false,
            is_trait: false,
            interfaces: vec![],
            methods: HashMap::new(),
            constants: HashMap::new(),
            constructor: None,
        });

        // DateMalformedIntervalStringException (extends DateException)
        registry.register_class(NativeClassDef {
            name: b"DateMalformedIntervalStringException".to_vec(),
            parent: Some(b"DateException".to_vec()),
            is_interface: false,
            is_trait: false,
            interfaces: vec![],
            methods: HashMap::new(),
            constants: HashMap::new(),
            constructor: None,
        });

        // DateMalformedPeriodStringException (extends DateException)
        registry.register_class(NativeClassDef {
            name: b"DateMalformedPeriodStringException".to_vec(),
            parent: Some(b"DateException".to_vec()),
            is_interface: false,
            is_trait: false,
            interfaces: vec![],
            methods: HashMap::new(),
            constants: HashMap::new(),
            constructor: None,
        });

        // DateMalformedStringException (extends DateException)
        registry.register_class(NativeClassDef {
            name: b"DateMalformedStringException".to_vec(),
            parent: Some(b"DateException".to_vec()),
            is_interface: false,
            is_trait: false,
            interfaces: vec![],
            methods: HashMap::new(),
            constants: HashMap::new(),
            constructor: None,
        });

        // ========================================
        // DATE/TIME CONSTANTS
        // ========================================

        // Register date format constants
        registry.register_constant(
            b"DATE_ATOM",
            Val::String(Rc::new(datetime::DATE_ATOM.as_bytes().to_vec())),
        );
        registry.register_constant(
            b"DATE_COOKIE",
            Val::String(Rc::new(datetime::DATE_COOKIE.as_bytes().to_vec())),
        );
        registry.register_constant(
            b"DATE_ISO8601",
            Val::String(Rc::new(datetime::DATE_ISO8601.as_bytes().to_vec())),
        );
        registry.register_constant(
            b"DATE_ISO8601_EXPANDED",
            Val::String(Rc::new(datetime::DATE_ISO8601_EXPANDED.as_bytes().to_vec())),
        );
        registry.register_constant(
            b"DATE_RFC822",
            Val::String(Rc::new(datetime::DATE_RFC822.as_bytes().to_vec())),
        );
        registry.register_constant(
            b"DATE_RFC850",
            Val::String(Rc::new(datetime::DATE_RFC850.as_bytes().to_vec())),
        );
        registry.register_constant(
            b"DATE_RFC1036",
            Val::String(Rc::new(datetime::DATE_RFC1036.as_bytes().to_vec())),
        );
        registry.register_constant(
            b"DATE_RFC1123",
            Val::String(Rc::new(datetime::DATE_RFC1123.as_bytes().to_vec())),
        );
        registry.register_constant(
            b"DATE_RFC7231",
            Val::String(Rc::new(datetime::DATE_RFC7231.as_bytes().to_vec())),
        );
        registry.register_constant(
            b"DATE_RFC2822",
            Val::String(Rc::new(datetime::DATE_RFC2822.as_bytes().to_vec())),
        );
        registry.register_constant(
            b"DATE_RFC3339",
            Val::String(Rc::new(datetime::DATE_RFC3339.as_bytes().to_vec())),
        );
        registry.register_constant(
            b"DATE_RFC3339_EXTENDED",
            Val::String(Rc::new(datetime::DATE_RFC3339_EXTENDED.as_bytes().to_vec())),
        );
        registry.register_constant(
            b"DATE_RSS",
            Val::String(Rc::new(datetime::DATE_RSS.as_bytes().to_vec())),
        );
        registry.register_constant(
            b"DATE_W3C",
            Val::String(Rc::new(datetime::DATE_W3C.as_bytes().to_vec())),
        );

        // Register sun function constants
        registry.register_constant(
            b"SUNFUNCS_RET_TIMESTAMP",
            Val::Int(datetime::SUNFUNCS_RET_TIMESTAMP),
        );
        registry.register_constant(
            b"SUNFUNCS_RET_STRING",
            Val::Int(datetime::SUNFUNCS_RET_STRING),
        );
        registry.register_constant(
            b"SUNFUNCS_RET_DOUBLE",
            Val::Int(datetime::SUNFUNCS_RET_DOUBLE),
        );

        // ========================================
        // DATE/TIME FUNCTIONS
        // ========================================

        // Register date/time functions that exist
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
                
        
        // TODO: Implement remaining date/time functions:
        // gmdate, microtime, gmmktime, date_create_immutable, date_create_from_format,
        // date_create_immutable_from_format, date_parse_from_format, date_timestamp_get,
        // date_timestamp_set, date_timezone_get, date_timezone_set, timezone_open,
        // timezone_name_get, timezone_identifiers_list, date_default_timezone_get,
        // date_default_timezone_set

        ExtensionResult::Success
    }

    fn request_init(&self, _ctx: &mut RequestContext) -> ExtensionResult {
        // Per-request initialization if needed
        ExtensionResult::Success
    }

    fn request_shutdown(&self, _ctx: &mut RequestContext) -> ExtensionResult {
        // Per-request cleanup if needed
        ExtensionResult::Success
    }

    fn module_shutdown(&self) -> ExtensionResult {
        // Module cleanup if needed
        ExtensionResult::Success
    }
}

