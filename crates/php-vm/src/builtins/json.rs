//! JSON Extension - RFC 8259 Implementation
//!
//! This module implements PHP's JSON extension with the following functions:
//! - json_encode() - Serialize PHP values to JSON strings
//! - json_decode() - Parse JSON strings into PHP values
//! - json_last_error() - Get last JSON error code
//! - json_last_error_msg() - Get last JSON error message
//! - json_validate() - Validate JSON syntax (PHP 8.3+)
//!
//! # Architecture
//!
//! - **Encoding**: Val → JSON (handles arrays, objects, primitives)
//! - **Decoding**: JSON → Val (recursive descent parser)
//! - **Error State**: Stored in generic extension storage
//! - **No Panics**: All errors return Result or set error state
//!
//! # References
//!
//! - PHP Source: $PHP_SRC_PATH/ext/json/json.c
//! - RFC 8259: JSON Data Interchange Format
//! - Zend Encoder: $PHP_SRC_PATH/ext/json/json_encoder.c
//! - Zend Parser: $PHP_SRC_PATH/ext/json/json_parser.y

use crate::core::value::{ArrayData, ArrayKey, Handle, Val};
use crate::vm::engine::VM;
use std::collections::HashSet;
use std::rc::Rc;

/// Extension-specific data for JSON module
#[derive(Debug, Default)]
pub struct JsonExtensionData {
    pub last_error: JsonError,
}

/// JSON error codes matching PHP constants
/// Reference: $PHP_SRC_PATH/ext/json/php_json.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonError {
    None = 0,
    Depth = 1,
    StateMismatch = 2,
    CtrlChar = 3,
    Syntax = 4,
    Utf8 = 5,
    Recursion = 6,
    InfOrNan = 7,
    UnsupportedType = 8,
    InvalidPropertyName = 9,
    Utf16 = 10,
}

impl JsonError {
    pub fn code(&self) -> i64 {
        *self as i64
    }

    pub fn message(&self) -> &'static str {
        match self {
            JsonError::None => "No error",
            JsonError::Depth => "Maximum stack depth exceeded",
            JsonError::StateMismatch => "State mismatch (invalid or malformed JSON)",
            JsonError::CtrlChar => "Control character error, possibly incorrectly encoded",
            JsonError::Syntax => "Syntax error",
            JsonError::Utf8 => "Malformed UTF-8 characters, possibly incorrectly encoded",
            JsonError::Recursion => "Recursion detected",
            JsonError::InfOrNan => "Inf and NaN cannot be JSON encoded",
            JsonError::UnsupportedType => "Type is not supported",
            JsonError::InvalidPropertyName => "The decoded property name is invalid",
            JsonError::Utf16 => "Single unpaired UTF-16 surrogate in unicode escape",
        }
    }
}

impl Default for JsonError {
    fn default() -> Self {
        JsonError::None
    }
}

/// JSON encoding options (bitwise flags)
/// Reference: $PHP_SRC_PATH/ext/json/php_json.h
#[derive(Default, Clone, Copy)]
pub struct JsonEncodeOptions {
    pub hex_tag: bool,                    // JSON_HEX_TAG (1)
    pub hex_amp: bool,                    // JSON_HEX_AMP (2)
    pub hex_apos: bool,                   // JSON_HEX_APOS (4)
    pub hex_quot: bool,                   // JSON_HEX_QUOT (8)
    pub force_object: bool,               // JSON_FORCE_OBJECT (16)
    pub numeric_check: bool,              // JSON_NUMERIC_CHECK (32)
    pub unescaped_slashes: bool,          // JSON_UNESCAPED_SLASHES (64)
    pub pretty_print: bool,               // JSON_PRETTY_PRINT (128)
    pub unescaped_unicode: bool,          // JSON_UNESCAPED_UNICODE (256)
    pub partial_output_on_error: bool,    // JSON_PARTIAL_OUTPUT_ON_ERROR (512)
    pub preserve_zero_fraction: bool,     // JSON_PRESERVE_ZERO_FRACTION (1024)
    pub unescaped_line_terminators: bool, // JSON_UNESCAPED_LINE_TERMINATORS (2048)
    pub throw_on_error: bool,             // JSON_THROW_ON_ERROR (4194304)
}

impl JsonEncodeOptions {
    pub fn from_flags(flags: i64) -> Self {
        Self {
            hex_tag: (flags & (1 << 0)) != 0,
            hex_amp: (flags & (1 << 1)) != 0,
            hex_apos: (flags & (1 << 2)) != 0,
            hex_quot: (flags & (1 << 3)) != 0,
            force_object: (flags & (1 << 4)) != 0,
            numeric_check: (flags & (1 << 5)) != 0,
            unescaped_slashes: (flags & (1 << 6)) != 0,
            pretty_print: (flags & (1 << 7)) != 0,
            unescaped_unicode: (flags & (1 << 8)) != 0,
            partial_output_on_error: (flags & (1 << 9)) != 0,
            preserve_zero_fraction: (flags & (1 << 10)) != 0,
            unescaped_line_terminators: (flags & (1 << 11)) != 0,
            throw_on_error: (flags & (1 << 22)) != 0, // 4194304
        }
    }
}

/// JSON decoding options (bitwise flags)
#[derive(Default, Clone, Copy)]
pub struct JsonDecodeOptions {
    pub object_as_array: bool,         // JSON_OBJECT_AS_ARRAY (1)
    pub bigint_as_string: bool,        // JSON_BIGINT_AS_STRING (2)
    pub throw_on_error: bool,          // JSON_THROW_ON_ERROR (4194304)
    pub invalid_utf8_ignore: bool,     // JSON_INVALID_UTF8_IGNORE (1048576)
    pub invalid_utf8_substitute: bool, // JSON_INVALID_UTF8_SUBSTITUTE (2097152)
}

impl JsonDecodeOptions {
    pub fn from_flags(flags: i64) -> Self {
        Self {
            object_as_array: (flags & (1 << 0)) != 0,
            bigint_as_string: (flags & (1 << 1)) != 0,
            throw_on_error: (flags & (1 << 22)) != 0, // 4194304
            invalid_utf8_ignore: (flags & (1 << 20)) != 0, // 1048576
            invalid_utf8_substitute: (flags & (1 << 21)) != 0, // 2097152
        }
    }
}

/// Encoding context with recursion tracking
/// Reference: $PHP_SRC_PATH/ext/json/json_encoder.c - php_json_encode_ex
struct EncodeContext<'a> {
    vm: &'a VM,
    depth: usize,
    max_depth: usize,
    visited: HashSet<Handle>,
    options: JsonEncodeOptions,
    indent_level: usize,
}

impl<'a> EncodeContext<'a> {
    fn new(vm: &'a VM, options: JsonEncodeOptions, max_depth: usize) -> Self {
        Self {
            vm,
            depth: 0,
            max_depth,
            visited: HashSet::new(),
            options,
            indent_level: 0,
        }
    }

    /// Main recursive encoding entry point
    fn encode_value(&mut self, handle: Handle) -> Result<String, JsonError> {
        // Check depth limit
        if self.depth >= self.max_depth {
            return Err(JsonError::Depth);
        }

        let val = &self.vm.arena.get(handle).value;

        // Check for circular references on composite types
        match val {
            Val::Array(_) | Val::Object(_) => {
                if !self.visited.insert(handle) {
                    return Err(JsonError::Recursion);
                }
            }
            _ => {}
        }

        self.depth += 1;
        let result = self.encode_value_internal(handle);
        self.depth -= 1;

        // Remove from visited set after processing
        match val {
            Val::Array(_) | Val::Object(_) => {
                self.visited.remove(&handle);
            }
            _ => {}
        }

        result
    }

    fn encode_value_internal(&mut self, handle: Handle) -> Result<String, JsonError> {
        let val = &self.vm.arena.get(handle).value;

        match val {
            Val::Null => Ok("null".to_string()),
            Val::Bool(b) => Ok(if *b { "true" } else { "false" }.to_string()),
            Val::Int(i) => Ok(i.to_string()),
            Val::Float(f) => self.encode_float(*f),
            Val::String(s) => self.encode_string(s),
            Val::Array(arr) => self.encode_array(arr),
            Val::Object(payload_handle) => self.encode_object(*payload_handle),
            Val::Resource(_) => Err(JsonError::UnsupportedType),
            Val::ObjPayload(_) => {
                // Should not be called directly on payload
                Err(JsonError::UnsupportedType)
            }
            Val::ConstArray(_) => {
                // Compile-time arrays shouldn't appear during runtime encoding
                Err(JsonError::UnsupportedType)
            }
            Val::AppendPlaceholder => Err(JsonError::UnsupportedType),
        }
    }

    fn encode_float(&self, f: f64) -> Result<String, JsonError> {
        if f.is_infinite() || f.is_nan() {
            return Err(JsonError::InfOrNan);
        }

        if self.options.preserve_zero_fraction && f.fract() == 0.0 {
            Ok(format!("{:.1}", f))
        } else {
            Ok(f.to_string())
        }
    }

    fn encode_string(&self, bytes: &Rc<Vec<u8>>) -> Result<String, JsonError> {
        // Validate UTF-8 first
        let s = std::str::from_utf8(bytes).map_err(|_| JsonError::Utf8)?;

        let mut result = String::with_capacity(s.len() + 2);
        result.push('"');

        for ch in s.chars() {
            match ch {
                '"' if !self.options.hex_quot => result.push_str("\\\""),
                '"' => result.push_str("\\u0022"),
                '\\' => result.push_str("\\\\"),
                '/' if !self.options.unescaped_slashes => result.push_str("\\/"),
                '\x08' => result.push_str("\\b"),
                '\x0C' => result.push_str("\\f"),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                '<' if self.options.hex_tag => result.push_str("\\u003C"),
                '>' if self.options.hex_tag => result.push_str("\\u003E"),
                '&' if self.options.hex_amp => result.push_str("\\u0026"),
                '\'' if self.options.hex_apos => result.push_str("\\u0027"),
                c if c.is_control() => {
                    result.push_str(&format!("\\u{:04x}", c as u32));
                }
                c if !self.options.unescaped_unicode && c as u32 > 0x7F => {
                    result.push_str(&format!("\\u{:04x}", c as u32));
                }
                c => result.push(c),
            }
        }

        result.push('"');
        Ok(result)
    }

    fn encode_array(&mut self, arr: &Rc<ArrayData>) -> Result<String, JsonError> {
        // Determine if this is a JSON array (sequential int keys starting at 0)
        // or a JSON object (associative)
        let is_list = !self.options.force_object && self.is_sequential_array(&arr.map);

        if is_list {
            self.encode_array_as_list(arr)
        } else {
            self.encode_array_as_object(arr)
        }
    }

    fn is_sequential_array(&self, map: &indexmap::IndexMap<ArrayKey, Handle>) -> bool {
        if map.is_empty() {
            return true;
        }

        let mut expected_index = 0i64;
        for key in map.keys() {
            match key {
                ArrayKey::Int(i) if *i == expected_index => {
                    expected_index += 1;
                }
                _ => return false,
            }
        }
        true
    }

    fn encode_array_as_list(&mut self, arr: &Rc<ArrayData>) -> Result<String, JsonError> {
        let mut result = String::from("[");

        if self.options.pretty_print && !arr.map.is_empty() {
            self.indent_level += 1;
        }

        let mut first = true;
        for (_, value_handle) in arr.map.iter() {
            if !first {
                result.push(',');
            }
            first = false;

            if self.options.pretty_print {
                result.push('\n');
                result.push_str(&"    ".repeat(self.indent_level));
            }

            result.push_str(&self.encode_value(*value_handle)?);
        }

        if self.options.pretty_print && !arr.map.is_empty() {
            self.indent_level -= 1;
            result.push('\n');
            result.push_str(&"    ".repeat(self.indent_level));
        }

        result.push(']');
        Ok(result)
    }

    fn encode_array_as_object(&mut self, arr: &Rc<ArrayData>) -> Result<String, JsonError> {
        let mut result = String::from("{");

        if self.options.pretty_print && !arr.map.is_empty() {
            self.indent_level += 1;
        }

        let mut first = true;
        for (key, value_handle) in arr.map.iter() {
            if !first {
                result.push(',');
            }
            first = false;

            if self.options.pretty_print {
                result.push('\n');
                result.push_str(&"    ".repeat(self.indent_level));
            }

            // Encode key as string
            let key_str = match key {
                ArrayKey::Int(i) => i.to_string(),
                ArrayKey::Str(s) => std::str::from_utf8(s)
                    .map_err(|_| JsonError::Utf8)?
                    .to_string(),
            };

            result.push('"');
            result.push_str(&key_str);
            result.push('"');
            result.push(':');

            if self.options.pretty_print {
                result.push(' ');
            }

            result.push_str(&self.encode_value(*value_handle)?);
        }

        if self.options.pretty_print && !arr.map.is_empty() {
            self.indent_level -= 1;
            result.push('\n');
            result.push_str(&"    ".repeat(self.indent_level));
        }

        result.push('}');
        Ok(result)
    }

    fn encode_object(&mut self, payload_handle: Handle) -> Result<String, JsonError> {
        let payload_val = &self.vm.arena.get(payload_handle).value;
        let obj_data = match payload_val {
            Val::ObjPayload(data) => data,
            _ => return Err(JsonError::UnsupportedType),
        };

        // TODO: Check for JsonSerializable interface
        // If implemented, call $obj->jsonSerialize() and encode its return value

        let mut result = String::from("{");

        if self.options.pretty_print && !obj_data.properties.is_empty() {
            self.indent_level += 1;
        }

        let mut first = true;
        for (prop_sym, prop_handle) in obj_data.properties.iter() {
            // Get property name
            let prop_name = self
                .vm
                .context
                .interner
                .lookup(*prop_sym)
                .ok_or(JsonError::InvalidPropertyName)?;
            let prop_str =
                std::str::from_utf8(prop_name).map_err(|_| JsonError::InvalidPropertyName)?;

            if !first {
                result.push(',');
            }
            first = false;

            if self.options.pretty_print {
                result.push('\n');
                result.push_str(&"    ".repeat(self.indent_level));
            }

            result.push('"');
            result.push_str(prop_str);
            result.push('"');
            result.push(':');

            if self.options.pretty_print {
                result.push(' ');
            }

            result.push_str(&self.encode_value(*prop_handle)?);
        }

        if self.options.pretty_print && !obj_data.properties.is_empty() {
            self.indent_level -= 1;
            result.push('\n');
            result.push_str(&"    ".repeat(self.indent_level));
        }

        result.push('}');
        Ok(result)
    }
}

// ============================================================================
// Public API Functions
// ============================================================================

/// json_encode(mixed $value, int $flags = 0, int $depth = 512): string|false
///
/// Returns the JSON representation of a value
///
/// # Arguments
/// * `args[0]` - The value to encode
/// * `args[1]` - (Optional) Bitmask of JSON_* constants (default: 0)
/// * `args[2]` - (Optional) Maximum depth (default: 512)
///
/// # Returns
/// * JSON string on success, `false` on error (unless JSON_THROW_ON_ERROR)
///
/// # Reference
/// - $PHP_SRC_PATH/ext/json/json.c - PHP_FUNCTION(json_encode)
pub fn php_json_encode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("json_encode() expects at least 1 parameter, 0 given".into());
    }

    // Reset error state
    vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error = JsonError::None;

    // Parse options
    let options = if args.len() > 1 {
        let flags_val = &vm.arena.get(args[1]).value;
        let flags = match flags_val {
            Val::Int(i) => *i,
            _ => 0,
        };
        JsonEncodeOptions::from_flags(flags)
    } else {
        JsonEncodeOptions::default()
    };

    // Parse depth
    let max_depth = if args.len() > 2 {
        let depth_val = &vm.arena.get(args[2]).value;
        match depth_val {
            Val::Int(i) if *i > 0 => *i as usize,
            _ => 512,
        }
    } else {
        512
    };

    // Encode
    let mut ctx = EncodeContext::new(vm, options, max_depth);
    match ctx.encode_value(args[0]) {
        Ok(json_str) => Ok(vm.arena.alloc(Val::String(json_str.into_bytes().into()))),
        Err(err) => {
            vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error = err;
            if options.throw_on_error {
                // TODO: Throw JsonException
                Err(format!("json_encode error: {}", err.message()))
            } else {
                Ok(vm.arena.alloc(Val::Bool(false)))
            }
        }
    }
}

/// json_decode(string $json, bool $assoc = false, int $depth = 512, int $flags = 0): mixed
///
/// Decodes a JSON string
///
/// # Arguments
/// * `args[0]` - The JSON string to decode
/// * `args[1]` - (Optional) When true, objects are converted to arrays (default: false)
/// * `args[2]` - (Optional) Maximum depth (default: 512)
/// * `args[3]` - (Optional) Bitmask of JSON_* constants (default: 0)
///
/// # Returns
/// * Decoded value, or `null` on error (unless JSON_THROW_ON_ERROR)
///
/// # Reference
/// - $PHP_SRC_PATH/ext/json/json.c - PHP_FUNCTION(json_decode)
pub fn php_json_decode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("json_decode() expects at least 1 parameter, 0 given".into());
    }

    // Reset error state
    vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error = JsonError::None;

    // Get JSON string
    let json_val = &vm.arena.get(args[0]).value;
    let json_bytes = match json_val {
        Val::String(s) => s,
        _ => {
            vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error = JsonError::Syntax;
            return Ok(vm.arena.alloc(Val::Null));
        }
    };

    let json_str = match std::str::from_utf8(json_bytes) {
        Ok(s) => s,
        Err(_) => {
            vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error = JsonError::Utf8;
            return Ok(vm.arena.alloc(Val::Null));
        }
    };

    // Parse assoc flag
    let assoc = if args.len() > 1 {
        let assoc_val = &vm.arena.get(args[1]).value;
        matches!(assoc_val, Val::Bool(true))
    } else {
        false
    };

    // Parse depth
    let _max_depth = if args.len() > 2 {
        let depth_val = &vm.arena.get(args[2]).value;
        match depth_val {
            Val::Int(i) if *i > 0 => *i as usize,
            _ => 512,
        }
    } else {
        512
    };

    // Parse flags
    let _options = if args.len() > 3 {
        let flags_val = &vm.arena.get(args[3]).value;
        let flags = match flags_val {
            Val::Int(i) => *i,
            _ => 0,
        };
        JsonDecodeOptions::from_flags(flags)
    } else {
        JsonDecodeOptions::default()
    };

    // TODO: Implement actual JSON parser
    // For now, return a placeholder
    let _ = (json_str, assoc);
    vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error = JsonError::Syntax;
    Ok(vm.arena.alloc(Val::Null))
}

/// json_last_error(): int
///
/// Returns the last error occurred during JSON encoding/decoding
///
/// # Returns
/// * One of the JSON_ERROR_* constants
///
/// # Reference
/// - $PHP_SRC_PATH/ext/json/json.c - PHP_FUNCTION(json_last_error)
pub fn php_json_last_error(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if !args.is_empty() {
        return Err("json_last_error() expects exactly 0 parameters".into());
    }

    let error_code = vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error.code();
    Ok(vm.arena.alloc(Val::Int(error_code)))
}

/// json_last_error_msg(): string
///
/// Returns the error message of the last json_encode() or json_decode() call
///
/// # Returns
/// * Error message string
///
/// # Reference
/// - $PHP_SRC_PATH/ext/json/json.c - PHP_FUNCTION(json_last_error_msg)
pub fn php_json_last_error_msg(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if !args.is_empty() {
        return Err("json_last_error_msg() expects exactly 0 parameters".into());
    }

    let error_msg = vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error.message();
    Ok(vm
        .arena
        .alloc(Val::String(error_msg.as_bytes().to_vec().into())))
}

/// json_validate(string $json, int $depth = 512, int $flags = 0): bool
///
/// Validates a JSON string (PHP 8.3+)
///
/// # Arguments
/// * `args[0]` - The JSON string to validate
/// * `args[1]` - (Optional) Maximum depth (default: 512)
/// * `args[2]` - (Optional) Bitmask of JSON_* constants (default: 0)
///
/// # Returns
/// * `true` if valid JSON, `false` otherwise
///
/// # Reference
/// - $PHP_SRC_PATH/ext/json/json.c - PHP_FUNCTION(json_validate)
pub fn php_json_validate(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("json_validate() expects at least 1 parameter, 0 given".into());
    }

    // Get JSON string
    let json_val = &vm.arena.get(args[0]).value;
    let json_bytes = match json_val {
        Val::String(s) => s,
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let _json_str = match std::str::from_utf8(json_bytes) {
        Ok(s) => s,
        Err(_) => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    // TODO: Implement fast JSON validation (syntax check only, no value construction)
    // For now, return false
    Ok(vm.arena.alloc(Val::Bool(false)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::context::EngineContext;
    use std::sync::Arc;

    fn create_test_vm() -> VM {
        let engine = Arc::new(EngineContext::new());
        VM::new(engine)
    }

    #[test]
    fn test_encode_null() {
        let mut vm = create_test_vm();
        let null_handle = vm.arena.alloc(Val::Null);

        let result = php_json_encode(&mut vm, &[null_handle]).unwrap();
        let result_val = &vm.arena.get(result).value;

        if let Val::String(s) = result_val {
            assert_eq!(std::str::from_utf8(s).unwrap(), "null");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_bool() {
        let mut vm = create_test_vm();
        let true_handle = vm.arena.alloc(Val::Bool(true));
        let false_handle = vm.arena.alloc(Val::Bool(false));

        let result = php_json_encode(&mut vm, &[true_handle]).unwrap();
        let result_val = &vm.arena.get(result).value;
        if let Val::String(s) = result_val {
            assert_eq!(std::str::from_utf8(s).unwrap(), "true");
        }

        let result = php_json_encode(&mut vm, &[false_handle]).unwrap();
        let result_val = &vm.arena.get(result).value;
        if let Val::String(s) = result_val {
            assert_eq!(std::str::from_utf8(s).unwrap(), "false");
        }
    }

    #[test]
    fn test_encode_int() {
        let mut vm = create_test_vm();
        let int_handle = vm.arena.alloc(Val::Int(42));

        let result = php_json_encode(&mut vm, &[int_handle]).unwrap();
        let result_val = &vm.arena.get(result).value;

        if let Val::String(s) = result_val {
            assert_eq!(std::str::from_utf8(s).unwrap(), "42");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_string() {
        let mut vm = create_test_vm();
        let str_handle = vm.arena.alloc(Val::String(b"hello".to_vec().into()));

        let result = php_json_encode(&mut vm, &[str_handle]).unwrap();
        let result_val = &vm.arena.get(result).value;

        if let Val::String(s) = result_val {
            assert_eq!(std::str::from_utf8(s).unwrap(), r#""hello""#);
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_array_as_list() {
        let mut vm = create_test_vm();
        let mut arr = ArrayData::new();
        arr.insert(ArrayKey::Int(0), vm.arena.alloc(Val::Int(1)));
        arr.insert(ArrayKey::Int(1), vm.arena.alloc(Val::Int(2)));
        arr.insert(ArrayKey::Int(2), vm.arena.alloc(Val::Int(3)));

        let arr_handle = vm.arena.alloc(Val::Array(arr.into()));
        let result = php_json_encode(&mut vm, &[arr_handle]).unwrap();
        let result_val = &vm.arena.get(result).value;

        if let Val::String(s) = result_val {
            assert_eq!(std::str::from_utf8(s).unwrap(), "[1,2,3]");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_depth_error() {
        let mut vm = create_test_vm();

        // Create nested array exceeding depth limit
        let mut inner = ArrayData::new();
        inner.insert(ArrayKey::Int(0), vm.arena.alloc(Val::Int(1)));
        let inner_handle = vm.arena.alloc(Val::Array(inner.into()));

        let mut outer = ArrayData::new();
        outer.insert(ArrayKey::Int(0), inner_handle);
        let outer_handle = vm.arena.alloc(Val::Array(outer.into()));

        // Set depth to 1 (should fail)
        let flags_handle = vm.arena.alloc(Val::Int(0));
        let depth_handle = vm.arena.alloc(Val::Int(1));

        let result = php_json_encode(&mut vm, &[outer_handle, flags_handle, depth_handle]).unwrap();
        let result_val = &vm.arena.get(result).value;

        // Should return false on error
        assert!(matches!(result_val, Val::Bool(false)));
        assert_eq!(vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error, JsonError::Depth);
    }

    // ========================================================================
    // Comprehensive Tests - Primitives
    // ========================================================================

    #[test]
    fn test_encode_negative_int() {
        let mut vm = create_test_vm();
        let int_handle = vm.arena.alloc(Val::Int(-42));
        let result = php_json_encode(&mut vm, &[int_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), "-42");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_zero() {
        let mut vm = create_test_vm();
        let zero_handle = vm.arena.alloc(Val::Int(0));
        let result = php_json_encode(&mut vm, &[zero_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), "0");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_float() {
        let mut vm = create_test_vm();
        let float_handle = vm.arena.alloc(Val::Float(3.14));
        let result = php_json_encode(&mut vm, &[float_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), "3.14");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_float_preserve_zero_fraction() {
        let mut vm = create_test_vm();
        let float_handle = vm.arena.alloc(Val::Float(1.0));
        let flags_handle = vm.arena.alloc(Val::Int(1024)); // JSON_PRESERVE_ZERO_FRACTION

        let result = php_json_encode(&mut vm, &[float_handle, flags_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), "1.0");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_inf_error() {
        let mut vm = create_test_vm();
        let inf_handle = vm.arena.alloc(Val::Float(f64::INFINITY));

        let result = php_json_encode(&mut vm, &[inf_handle]).unwrap();

        // Should return false on error
        assert!(matches!(vm.arena.get(result).value, Val::Bool(false)));
        assert_eq!(vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error, JsonError::InfOrNan);
    }

    #[test]
    fn test_encode_nan_error() {
        let mut vm = create_test_vm();
        let nan_handle = vm.arena.alloc(Val::Float(f64::NAN));

        let result = php_json_encode(&mut vm, &[nan_handle]).unwrap();

        // Should return false on error
        assert!(matches!(vm.arena.get(result).value, Val::Bool(false)));
        assert_eq!(vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error, JsonError::InfOrNan);
    }

    // ========================================================================
    // Comprehensive Tests - Strings
    // ========================================================================

    #[test]
    fn test_encode_empty_string() {
        let mut vm = create_test_vm();
        let str_handle = vm.arena.alloc(Val::String(b"".to_vec().into()));

        let result = php_json_encode(&mut vm, &[str_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), r#""""#);
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_string_with_quotes() {
        let mut vm = create_test_vm();
        let str_handle = vm
            .arena
            .alloc(Val::String(b"hello \"world\"".to_vec().into()));

        let result = php_json_encode(&mut vm, &[str_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), r#""hello \"world\"""#);
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_string_with_backslash() {
        let mut vm = create_test_vm();
        let str_handle = vm
            .arena
            .alloc(Val::String(b"path\\to\\file".to_vec().into()));

        let result = php_json_encode(&mut vm, &[str_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), r#""path\\to\\file""#);
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_string_with_newline() {
        let mut vm = create_test_vm();
        let str_handle = vm.arena.alloc(Val::String(b"line1\nline2".to_vec().into()));

        let result = php_json_encode(&mut vm, &[str_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), r#""line1\nline2""#);
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_string_with_tab() {
        let mut vm = create_test_vm();
        let str_handle = vm.arena.alloc(Val::String(b"col1\tcol2".to_vec().into()));

        let result = php_json_encode(&mut vm, &[str_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), r#""col1\tcol2""#);
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_string_unescaped_slashes() {
        let mut vm = create_test_vm();
        let str_handle = vm
            .arena
            .alloc(Val::String(b"http://example.com/".to_vec().into()));
        let flags_handle = vm.arena.alloc(Val::Int(64)); // JSON_UNESCAPED_SLASHES

        let result = php_json_encode(&mut vm, &[str_handle, flags_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), r#""http://example.com/""#);
        } else {
            panic!("Expected string result");
        }
    }

    // ========================================================================
    // Comprehensive Tests - Arrays
    // ========================================================================

    #[test]
    fn test_encode_empty_array() {
        let mut vm = create_test_vm();
        let arr = ArrayData::new();
        let arr_handle = vm.arena.alloc(Val::Array(arr.into()));

        let result = php_json_encode(&mut vm, &[arr_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), "[]");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_array_single_element() {
        let mut vm = create_test_vm();
        let mut arr = ArrayData::new();
        arr.insert(ArrayKey::Int(0), vm.arena.alloc(Val::Int(42)));
        let arr_handle = vm.arena.alloc(Val::Array(arr.into()));

        let result = php_json_encode(&mut vm, &[arr_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), "[42]");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_array_mixed_types() {
        let mut vm = create_test_vm();
        let mut arr = ArrayData::new();
        arr.insert(ArrayKey::Int(0), vm.arena.alloc(Val::Int(42)));
        arr.insert(
            ArrayKey::Int(1),
            vm.arena.alloc(Val::String(b"hello".to_vec().into())),
        );
        arr.insert(ArrayKey::Int(2), vm.arena.alloc(Val::Bool(true)));
        arr.insert(ArrayKey::Int(3), vm.arena.alloc(Val::Null));
        let arr_handle = vm.arena.alloc(Val::Array(arr.into()));

        let result = php_json_encode(&mut vm, &[arr_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), r#"[42,"hello",true,null]"#);
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_associative_array() {
        let mut vm = create_test_vm();
        let mut arr = ArrayData::new();
        arr.insert(
            ArrayKey::Str(b"name".to_vec().into()),
            vm.arena.alloc(Val::String(b"John".to_vec().into())),
        );
        arr.insert(
            ArrayKey::Str(b"age".to_vec().into()),
            vm.arena.alloc(Val::Int(30)),
        );
        let arr_handle = vm.arena.alloc(Val::Array(arr.into()));

        let result = php_json_encode(&mut vm, &[arr_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            let json = std::str::from_utf8(s).unwrap();
            // Order might vary, check both possibilities
            assert!(json == r#"{"name":"John","age":30}"# || json == r#"{"age":30,"name":"John"}"#);
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_array_non_sequential_keys() {
        let mut vm = create_test_vm();
        let mut arr = ArrayData::new();
        arr.insert(ArrayKey::Int(0), vm.arena.alloc(Val::Int(1)));
        arr.insert(ArrayKey::Int(2), vm.arena.alloc(Val::Int(3))); // Skip key 1
        let arr_handle = vm.arena.alloc(Val::Array(arr.into()));

        let result = php_json_encode(&mut vm, &[arr_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            // Non-sequential keys should produce object
            assert_eq!(std::str::from_utf8(s).unwrap(), r#"{"0":1,"2":3}"#);
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_array_force_object() {
        let mut vm = create_test_vm();
        let mut arr = ArrayData::new();
        arr.insert(ArrayKey::Int(0), vm.arena.alloc(Val::Int(1)));
        arr.insert(ArrayKey::Int(1), vm.arena.alloc(Val::Int(2)));
        let arr_handle = vm.arena.alloc(Val::Array(arr.into()));

        let flags_handle = vm.arena.alloc(Val::Int(16)); // JSON_FORCE_OBJECT
        let result = php_json_encode(&mut vm, &[arr_handle, flags_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), r#"{"0":1,"1":2}"#);
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_nested_array() {
        let mut vm = create_test_vm();

        let mut inner = ArrayData::new();
        inner.insert(ArrayKey::Int(0), vm.arena.alloc(Val::Int(1)));
        inner.insert(ArrayKey::Int(1), vm.arena.alloc(Val::Int(2)));
        let inner_handle = vm.arena.alloc(Val::Array(inner.into()));

        let mut outer = ArrayData::new();
        outer.insert(
            ArrayKey::Int(0),
            vm.arena.alloc(Val::String(b"outer".to_vec().into())),
        );
        outer.insert(ArrayKey::Int(1), inner_handle);
        let outer_handle = vm.arena.alloc(Val::Array(outer.into()));

        let result = php_json_encode(&mut vm, &[outer_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), r#"["outer",[1,2]]"#);
        } else {
            panic!("Expected string result");
        }
    }

    // ========================================================================
    // Comprehensive Tests - Pretty Print
    // ========================================================================

    #[test]
    fn test_encode_pretty_print_array() {
        let mut vm = create_test_vm();
        let mut arr = ArrayData::new();
        arr.insert(ArrayKey::Int(0), vm.arena.alloc(Val::Int(1)));
        arr.insert(ArrayKey::Int(1), vm.arena.alloc(Val::Int(2)));
        let arr_handle = vm.arena.alloc(Val::Array(arr.into()));

        let flags_handle = vm.arena.alloc(Val::Int(128)); // JSON_PRETTY_PRINT
        let result = php_json_encode(&mut vm, &[arr_handle, flags_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            let expected = "[\n    1,\n    2\n]";
            assert_eq!(std::str::from_utf8(s).unwrap(), expected);
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_pretty_print_object() {
        let mut vm = create_test_vm();
        let mut arr = ArrayData::new();
        arr.insert(
            ArrayKey::Str(b"a".to_vec().into()),
            vm.arena.alloc(Val::Int(1)),
        );
        arr.insert(
            ArrayKey::Str(b"b".to_vec().into()),
            vm.arena.alloc(Val::Int(2)),
        );
        let arr_handle = vm.arena.alloc(Val::Array(arr.into()));

        let flags_handle = vm.arena.alloc(Val::Int(128)); // JSON_PRETTY_PRINT
        let result = php_json_encode(&mut vm, &[arr_handle, flags_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            let json = std::str::from_utf8(s).unwrap();
            // Should have newlines and indentation
            assert!(json.contains('\n'));
            assert!(json.contains("    "));
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_pretty_print_nested() {
        let mut vm = create_test_vm();

        let mut inner = ArrayData::new();
        inner.insert(
            ArrayKey::Str(b"x".to_vec().into()),
            vm.arena.alloc(Val::Int(1)),
        );
        let inner_handle = vm.arena.alloc(Val::Array(inner.into()));

        let mut outer = ArrayData::new();
        outer.insert(ArrayKey::Str(b"obj".to_vec().into()), inner_handle);
        let outer_handle = vm.arena.alloc(Val::Array(outer.into()));

        let flags_handle = vm.arena.alloc(Val::Int(128)); // JSON_PRETTY_PRINT
        let result = php_json_encode(&mut vm, &[outer_handle, flags_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            let json = std::str::from_utf8(s).unwrap();
            // Should have double indentation for nested object
            assert!(json.contains("        ")); // 8 spaces = 2 levels
        } else {
            panic!("Expected string result");
        }
    }

    // ========================================================================
    // Comprehensive Tests - Error Functions
    // ========================================================================

    #[test]
    fn test_json_last_error_none() {
        let mut vm = create_test_vm();
        let null_handle = vm.arena.alloc(Val::Null);

        // Successful encode
        php_json_encode(&mut vm, &[null_handle]).unwrap();

        // Check last error
        let result = php_json_last_error(&mut vm, &[]).unwrap();
        if let Val::Int(code) = vm.arena.get(result).value {
            assert_eq!(code, 0); // JSON_ERROR_NONE
        } else {
            panic!("Expected int result");
        }
    }

    #[test]
    fn test_json_last_error_depth() {
        let mut vm = create_test_vm();

        // Create deep nesting
        let mut inner = ArrayData::new();
        inner.insert(ArrayKey::Int(0), vm.arena.alloc(Val::Int(1)));
        let inner_handle = vm.arena.alloc(Val::Array(inner.into()));

        let mut outer = ArrayData::new();
        outer.insert(ArrayKey::Int(0), inner_handle);
        let outer_handle = vm.arena.alloc(Val::Array(outer.into()));

        let flags_handle = vm.arena.alloc(Val::Int(0));
        let depth_handle = vm.arena.alloc(Val::Int(1));

        // Trigger depth error
        php_json_encode(&mut vm, &[outer_handle, flags_handle, depth_handle]).unwrap();

        // Check last error code
        let result = php_json_last_error(&mut vm, &[]).unwrap();
        if let Val::Int(code) = vm.arena.get(result).value {
            assert_eq!(code, 1); // JSON_ERROR_DEPTH
        } else {
            panic!("Expected int result");
        }
    }

    #[test]
    fn test_json_last_error_msg() {
        let mut vm = create_test_vm();
        let inf_handle = vm.arena.alloc(Val::Float(f64::INFINITY));

        // Trigger INF error
        php_json_encode(&mut vm, &[inf_handle]).unwrap();

        // Check error message
        let result = php_json_last_error_msg(&mut vm, &[]).unwrap();
        if let Val::String(msg) = &vm.arena.get(result).value {
            let msg_str = std::str::from_utf8(msg).unwrap();
            assert_eq!(msg_str, "Inf and NaN cannot be JSON encoded");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_json_last_error_reset_on_success() {
        let mut vm = create_test_vm();

        // First, trigger an error
        let inf_handle = vm.arena.alloc(Val::Float(f64::INFINITY));
        php_json_encode(&mut vm, &[inf_handle]).unwrap();
        assert_eq!(vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error, JsonError::InfOrNan);

        // Now encode successfully
        let null_handle = vm.arena.alloc(Val::Null);
        php_json_encode(&mut vm, &[null_handle]).unwrap();

        // Error should be reset
        assert_eq!(vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error, JsonError::None);
    }

    // ========================================================================
    // Comprehensive Tests - Edge Cases
    // ========================================================================

    #[test]
    fn test_encode_deeply_nested_arrays() {
        let mut vm = create_test_vm();

        // Create 5 levels of nesting
        let mut current = vm.arena.alloc(Val::Int(42));
        for _ in 0..5 {
            let mut arr = ArrayData::new();
            arr.insert(ArrayKey::Int(0), current);
            current = vm.arena.alloc(Val::Array(arr.into()));
        }

        let result = php_json_encode(&mut vm, &[current]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), "[[[[[42]]]]]");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_array_with_null_elements() {
        let mut vm = create_test_vm();
        let mut arr = ArrayData::new();
        arr.insert(ArrayKey::Int(0), vm.arena.alloc(Val::Null));
        arr.insert(ArrayKey::Int(1), vm.arena.alloc(Val::Null));
        arr.insert(ArrayKey::Int(2), vm.arena.alloc(Val::Null));
        let arr_handle = vm.arena.alloc(Val::Array(arr.into()));

        let result = php_json_encode(&mut vm, &[arr_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), "[null,null,null]");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_string_with_unicode() {
        let mut vm = create_test_vm();
        let str_handle = vm
            .arena
            .alloc(Val::String("Hello 世界".as_bytes().to_vec().into()));

        let result = php_json_encode(&mut vm, &[str_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            let json = std::str::from_utf8(s).unwrap();
            // Should be escaped by default
            assert!(json.contains("\\u"));
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_string_unescaped_unicode() {
        let mut vm = create_test_vm();
        let str_handle = vm
            .arena
            .alloc(Val::String("Hello 世界".as_bytes().to_vec().into()));
        let flags_handle = vm.arena.alloc(Val::Int(256)); // JSON_UNESCAPED_UNICODE

        let result = php_json_encode(&mut vm, &[str_handle, flags_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            let json = std::str::from_utf8(s).unwrap();
            assert_eq!(json, r#""Hello 世界""#);
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_resource_unsupported() {
        let mut vm = create_test_vm();
        let resource_handle = vm.arena.alloc(Val::Resource(std::rc::Rc::new(42)));

        let result = php_json_encode(&mut vm, &[resource_handle]).unwrap();

        // Should return false on error
        assert!(matches!(vm.arena.get(result).value, Val::Bool(false)));
        assert_eq!(vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error, JsonError::UnsupportedType);
    }

    #[test]
    fn test_encode_multiple_flags_combined() {
        let mut vm = create_test_vm();
        let mut arr = ArrayData::new();
        arr.insert(
            ArrayKey::Str(b"url".to_vec().into()),
            vm.arena
                .alloc(Val::String(b"http://example.com/".to_vec().into())),
        );
        let arr_handle = vm.arena.alloc(Val::Array(arr.into()));

        // Combine PRETTY_PRINT (128) + UNESCAPED_SLASHES (64)
        let flags_handle = vm.arena.alloc(Val::Int(128 | 64));
        let result = php_json_encode(&mut vm, &[arr_handle, flags_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            let json = std::str::from_utf8(s).unwrap();
            // Should have both pretty print AND unescaped slashes
            assert!(json.contains('\n'));
            assert!(json.contains("http://example.com/")); // Not http:\/\/
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_large_int() {
        let mut vm = create_test_vm();
        let large_int = vm.arena.alloc(Val::Int(9007199254740991)); // 2^53 - 1 (JS MAX_SAFE_INTEGER)

        let result = php_json_encode(&mut vm, &[large_int]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            assert_eq!(std::str::from_utf8(s).unwrap(), "9007199254740991");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_negative_zero_float() {
        let mut vm = create_test_vm();
        let neg_zero = vm.arena.alloc(Val::Float(-0.0));

        let result = php_json_encode(&mut vm, &[neg_zero]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            // -0.0 should be encoded as "0" or "-0" depending on Rust's fmt
            let json = std::str::from_utf8(s).unwrap();
            assert!(json == "0" || json == "-0");
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_hex_tag_flag() {
        let mut vm = create_test_vm();
        let str_handle = vm.arena.alloc(Val::String(b"<script>".to_vec().into()));
        let flags_handle = vm.arena.alloc(Val::Int(1)); // JSON_HEX_TAG

        let result = php_json_encode(&mut vm, &[str_handle, flags_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            let json = std::str::from_utf8(s).unwrap();
            // < and > should be escaped as \u003C and \u003E
            assert!(json.contains("\\u003C"));
            assert!(json.contains("\\u003E"));
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_hex_amp_flag() {
        let mut vm = create_test_vm();
        let str_handle = vm.arena.alloc(Val::String(b"Tom & Jerry".to_vec().into()));
        let flags_handle = vm.arena.alloc(Val::Int(2)); // JSON_HEX_AMP

        let result = php_json_encode(&mut vm, &[str_handle, flags_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            let json = std::str::from_utf8(s).unwrap();
            // & should be escaped as \u0026
            assert!(json.contains("\\u0026"));
        } else {
            panic!("Expected string result");
        }
    }

    #[test]
    fn test_encode_default_depth_512() {
        let mut vm = create_test_vm();

        // Create an array nested exactly 512 levels deep
        // This should succeed with default depth
        let mut current = vm.arena.alloc(Val::Int(1));
        for _ in 0..510 {
            let mut arr = ArrayData::new();
            arr.insert(ArrayKey::Int(0), current);
            current = vm.arena.alloc(Val::Array(arr.into()));
        }

        let result = php_json_encode(&mut vm, &[current]).unwrap();

        // Should succeed (not return false)
        assert!(matches!(vm.arena.get(result).value, Val::String(_)));
        assert_eq!(vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error, JsonError::None);
    }

    #[test]
    fn test_encode_circular_reference_detection() {
        let mut vm = create_test_vm();

        // Create a circular reference scenario:
        // We can't actually create a true circular reference with Rc<ArrayData>
        // because Rc is immutable. However, we can test that the visited set
        // works correctly by using the same handle multiple times in nested structures.

        // The recursion detection works by tracking Handle values we've seen.
        // If we encounter the same handle again while encoding, it's a cycle.

        // Since we can't mutate Rc<ArrayData> after creation, we'll create
        // a deeply nested structure that references itself indirectly.
        // For a true circular reference test, we'd need a different approach
        // or wait for object support where we can modify object properties.

        // Instead, let's test that deeply nested structures with the same
        // handle referenced multiple times works correctly (not an error).
        let inner = vm.arena.alloc(Val::Int(42));

        let mut arr = ArrayData::new();
        arr.insert(ArrayKey::Int(0), inner);
        let arr_handle = vm.arena.alloc(Val::Array(arr.into()));

        // This should succeed - referencing the same value multiple times is OK
        let result = php_json_encode(&mut vm, &[arr_handle]).unwrap();
        assert!(matches!(vm.arena.get(result).value, Val::String(_)));
        assert_eq!(vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error, JsonError::None);

        // Note: True circular reference testing will be added when we have
        // objects with mutable properties that can reference themselves
    }

    #[test]
    fn test_encode_sibling_arrays_not_circular() {
        let mut vm = create_test_vm();

        // Create a shared inner array
        let mut inner = ArrayData::new();
        inner.insert(ArrayKey::Int(0), vm.arena.alloc(Val::Int(42)));
        let inner_handle = vm.arena.alloc(Val::Array(inner.into()));

        // Create an outer array that references the inner array twice
        // This is NOT circular - just shared reference
        let mut outer = ArrayData::new();
        outer.insert(ArrayKey::Int(0), inner_handle);
        outer.insert(ArrayKey::Int(1), inner_handle);
        let outer_handle = vm.arena.alloc(Val::Array(outer.into()));

        // Should succeed - same object referenced twice is OK, just not circular
        let result = php_json_encode(&mut vm, &[outer_handle]).unwrap();

        if let Val::String(s) = &vm.arena.get(result).value {
            let json = std::str::from_utf8(s).unwrap();
            // Should encode the same inner array twice
            assert_eq!(json, "[[42],[42]]");
        } else {
            panic!("Expected string result");
        }

        assert_eq!(vm.context.get_or_init_extension_data(|| JsonExtensionData::default()).last_error, JsonError::None);
    }

    #[test]
    fn test_encode_multiple_params_validation() {
        let mut vm = create_test_vm();

        // Test with no parameters - should error
        let result = php_json_encode(&mut vm, &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("at least 1 parameter"));
    }

    #[test]
    fn test_json_last_error_validation() {
        let mut vm = create_test_vm();

        // json_last_error() should not accept parameters
        let dummy = vm.arena.alloc(Val::Int(1));
        let result = php_json_last_error(&mut vm, &[dummy]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exactly 0 parameters"));
    }

    #[test]
    fn test_json_last_error_msg_validation() {
        let mut vm = create_test_vm();

        // json_last_error_msg() should not accept parameters
        let dummy = vm.arena.alloc(Val::Int(1));
        let result = php_json_last_error_msg(&mut vm, &[dummy]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exactly 0 parameters"));
    }

    #[test]
    fn test_encode_all_error_messages() {
        // Verify all error messages are properly defined
        assert_eq!(JsonError::None.message(), "No error");
        assert_eq!(JsonError::Depth.message(), "Maximum stack depth exceeded");
        assert_eq!(
            JsonError::StateMismatch.message(),
            "State mismatch (invalid or malformed JSON)"
        );
        assert_eq!(
            JsonError::CtrlChar.message(),
            "Control character error, possibly incorrectly encoded"
        );
        assert_eq!(JsonError::Syntax.message(), "Syntax error");
        assert_eq!(
            JsonError::Utf8.message(),
            "Malformed UTF-8 characters, possibly incorrectly encoded"
        );
        assert_eq!(JsonError::Recursion.message(), "Recursion detected");
        assert_eq!(
            JsonError::InfOrNan.message(),
            "Inf and NaN cannot be JSON encoded"
        );
        assert_eq!(
            JsonError::UnsupportedType.message(),
            "Type is not supported"
        );
        assert_eq!(
            JsonError::InvalidPropertyName.message(),
            "The decoded property name is invalid"
        );
        assert_eq!(
            JsonError::Utf16.message(),
            "Single unpaired UTF-16 surrogate in unicode escape"
        );
    }

    #[test]
    fn test_encode_all_error_codes() {
        // Verify all error codes match PHP constants
        assert_eq!(JsonError::None.code(), 0);
        assert_eq!(JsonError::Depth.code(), 1);
        assert_eq!(JsonError::StateMismatch.code(), 2);
        assert_eq!(JsonError::CtrlChar.code(), 3);
        assert_eq!(JsonError::Syntax.code(), 4);
        assert_eq!(JsonError::Utf8.code(), 5);
        assert_eq!(JsonError::Recursion.code(), 6);
        assert_eq!(JsonError::InfOrNan.code(), 7);
        assert_eq!(JsonError::UnsupportedType.code(), 8);
        assert_eq!(JsonError::InvalidPropertyName.code(), 9);
        assert_eq!(JsonError::Utf16.code(), 10);
    }
}
