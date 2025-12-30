//! VM Engine Core
//!
//! This module contains the main VM execution loop and core state.
//! Production-grade, fault-tolerant PHP VM with zero-heap AST guarantees.
//!
//! ## Architecture
//!
//! The VM follows a stack-based execution model similar to Zend Engine:
//! - **Operand Stack**: Temporary value storage during expression evaluation
//! - **Call Frames**: Function/method execution contexts with local variables
//! - **Arena Allocator**: Zero-heap allocation using `bumpalo` for values
//!
//! ## Delegated Responsibilities
//!
//! To improve modularity and maintainability, functionality is organized across modules:
//!
//! - **Arithmetic operations**: [`opcodes::arithmetic`](crate::vm::opcodes::arithmetic) - Add, Sub, Mul, Div, Mod, Pow
//! - **Bitwise operations**: [`opcodes::bitwise`](crate::vm::opcodes::bitwise) - And, Or, Xor, Not, Shifts
//! - **Comparison operations**: [`opcodes::comparison`](crate::vm::opcodes::comparison) - Equality, relational, spaceship
//! - **Type conversions**: [`type_conversion`](crate::vm::type_conversion) - PHP type juggling
//! - **Class resolution**: [`class_resolution`](crate::vm::class_resolution) - Inheritance chain walking
//! - **Stack helpers**: [`stack_helpers`](crate::vm::stack_helpers) - Pop/push/peek operations
//! - **Visibility checks**: [`visibility`](crate::vm::visibility) - Access control for class members
//! - **Variable operations**: [`variable_ops`](crate::vm::variable_ops) - Load/store/unset variables
//!
//! ## Core Execution
//!
//! - [`VM::run`] - Top-level script execution
//! - [`VM::run_loop`] - Main opcode dispatch loop
//! - [`VM::execute_opcode`] - Single opcode execution (delegated to specialized handlers)
//!
//! ## Performance Characteristics
//!
//! - **Zero-Copy**: Values reference arena-allocated memory, no cloning
//! - **Zero-Heap in AST**: All AST nodes use arena allocation
//! - **Inlined Hot Paths**: Critical operations marked `#[inline]`
//! - **Timeout Checking**: Configurable execution time limits
//!
//! ## Error Handling
//!
//! - **No Panics**: All errors return [`VmError`], ensuring fault tolerance
//! - **Error Recovery**: Parse errors become `Error` nodes, execution continues
//! - **Error Reporting**: Configurable error levels (Notice, Warning, Error)
//!
//! ## References
//!
//! - Zend VM: `$PHP_SRC_PATH/Zend/zend_execute.c` - Main execution loop
//! - Zend Operators: `$PHP_SRC_PATH/Zend/zend_operators.c` - Type juggling
//! - Zend Compile: `$PHP_SRC_PATH/Zend/zend_compile.c` - Visibility rules

use crate::compiler::chunk::{ClosureData, CodeChunk, ReturnType, UserFunc};
use crate::core::heap::Arena;
use crate::core::value::{ArrayData, ArrayKey, Handle, ObjectData, Symbol, Val, Visibility};
use crate::runtime::context::{ClassDef, EngineContext, MethodEntry, MethodSignature, ParameterInfo, PropertyEntry, RequestContext, StaticPropertyEntry, TypeHint};
use crate::vm::frame::{
    ArgList, CallFrame, GeneratorData, GeneratorState, SubGenState, SubIterator,
};
use crate::vm::opcode::OpCode;
use crate::vm::stack::Stack;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub enum VmError {
    /// Stack underflow during operation
    StackUnderflow { operation: &'static str },
    /// Type error during operation
    TypeError {
        expected: String,
        got: String,
        operation: &'static str,
    },
    /// Undefined variable access
    UndefinedVariable { name: String },
    /// Undefined function call
    UndefinedFunction { name: String },
    /// Undefined method call
    UndefinedMethod { class: String, method: String },
    /// Division by zero
    DivisionByZero,
    /// Generic runtime error (for gradual migration)
    RuntimeError(String),
    /// PHP exception object
    Exception(Handle),
}

impl std::fmt::Display for VmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmError::StackUnderflow { operation } => {
                write!(f, "Stack underflow during {}", operation)
            }
            VmError::TypeError {
                expected,
                got,
                operation,
            } => {
                write!(
                    f,
                    "Type error in {}: expected {}, got {}",
                    operation, expected, got
                )
            }
            VmError::UndefinedVariable { name } => {
                write!(f, "Undefined variable: ${}", name)
            }
            VmError::UndefinedFunction { name } => {
                write!(f, "Call to undefined function {}()", name)
            }
            VmError::UndefinedMethod { class, method } => {
                write!(f, "Call to undefined method {}::{}", class, method)
            }
            VmError::DivisionByZero => {
                write!(f, "Division by zero")
            }
            VmError::RuntimeError(msg) => {
                write!(f, "{}", msg)
            }
            VmError::Exception(_) => {
                write!(f, "Uncaught exception")
            }
        }
    }
}

impl std::error::Error for VmError {}

/// PHP error levels matching Zend constants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorLevel {
    Notice,      // E_NOTICE
    Warning,     // E_WARNING
    Error,       // E_ERROR
    ParseError,  // E_PARSE
    UserNotice,  // E_USER_NOTICE
    UserWarning, // E_USER_WARNING
    UserError,   // E_USER_ERROR
    Deprecated,  // E_DEPRECATED
}

impl ErrorLevel {
    /// Convert error level to the corresponding bitmask value
    pub fn to_bitmask(self) -> u32 {
        match self {
            ErrorLevel::Error => 1,         // E_ERROR
            ErrorLevel::Warning => 2,       // E_WARNING
            ErrorLevel::ParseError => 4,    // E_PARSE
            ErrorLevel::Notice => 8,        // E_NOTICE
            ErrorLevel::UserError => 256,   // E_USER_ERROR
            ErrorLevel::UserWarning => 512, // E_USER_WARNING
            ErrorLevel::UserNotice => 1024, // E_USER_NOTICE
            ErrorLevel::Deprecated => 8192, // E_DEPRECATED
        }
    }
}

pub trait ErrorHandler {
    /// Report an error/warning/notice at runtime
    fn report(&mut self, level: ErrorLevel, message: &str);
}

/// Default error handler that writes to stderr
pub struct StderrErrorHandler {
    stderr: io::Stderr,
}

impl Default for StderrErrorHandler {
    fn default() -> Self {
        Self {
            stderr: io::stderr(),
        }
    }
}

impl ErrorHandler for StderrErrorHandler {
    fn report(&mut self, level: ErrorLevel, message: &str) {
        let level_str = match level {
            ErrorLevel::Notice => "Notice",
            ErrorLevel::Warning => "Warning",
            ErrorLevel::Error => "Error",
            ErrorLevel::ParseError => "Parse error",
            ErrorLevel::UserNotice => "User notice",
            ErrorLevel::UserWarning => "User warning",
            ErrorLevel::UserError => "User error",
            ErrorLevel::Deprecated => "Deprecated",
        };
        // Follow the same pattern as OutputWriter - write to stderr and handle errors gracefully
        let _ = writeln!(self.stderr, "{}: {}", level_str, message);
        let _ = self.stderr.flush();
    }
}

/// Capturing error handler for testing and output capture
pub struct CapturingErrorHandler<F: FnMut(ErrorLevel, &str)> {
    callback: F,
}

impl<F: FnMut(ErrorLevel, &str)> CapturingErrorHandler<F> {
    pub fn new(callback: F) -> Self {
        Self { callback }
    }
}

impl<F: FnMut(ErrorLevel, &str)> ErrorHandler for CapturingErrorHandler<F> {
    fn report(&mut self, level: ErrorLevel, message: &str) {
        (self.callback)(level, message);
    }
}

pub trait OutputWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), VmError>;
    fn flush(&mut self) -> Result<(), VmError> {
        Ok(())
    }
}

/// Buffered stdout writer to avoid excessive syscalls
pub struct StdoutWriter {
    stdout: io::Stdout,
}

impl Default for StdoutWriter {
    fn default() -> Self {
        Self {
            stdout: io::stdout(),
        }
    }
}

impl OutputWriter for StdoutWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        self.stdout
            .write_all(bytes)
            .map_err(|e| VmError::RuntimeError(format!("Failed to write output: {}", e)))
    }

    fn flush(&mut self) -> Result<(), VmError> {
        self.stdout
            .flush()
            .map_err(|e| VmError::RuntimeError(format!("Failed to flush output: {}", e)))
    }
}

/// Capturing output writer for testing
pub struct CapturingOutputWriter<F: FnMut(&[u8])> {
    callback: F,
}

impl<F: FnMut(&[u8])> CapturingOutputWriter<F> {
    pub fn new(callback: F) -> Self {
        Self { callback }
    }
}

impl<F: FnMut(&[u8])> OutputWriter for CapturingOutputWriter<F> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        (self.callback)(bytes);
        Ok(())
    }
}

pub struct PendingCall {
    pub func_name: Option<Symbol>,
    pub func_handle: Option<Handle>,
    pub args: ArgList,
    pub is_static: bool,
    pub class_name: Option<Symbol>,
    pub this_handle: Option<Handle>,
}

#[derive(Clone, Copy, Debug)]
pub enum PropertyCollectionMode {
    All,
    VisibleTo(Option<Symbol>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum SuperglobalKind {
    Server,
    Get,
    Post,
    Files,
    Cookie,
    Request,
    Env,
    Session,
    Globals,
}

const SUPERGLOBAL_SPECS: &[(SuperglobalKind, &[u8])] = &[
    (SuperglobalKind::Server, b"_SERVER"),
    (SuperglobalKind::Get, b"_GET"),
    (SuperglobalKind::Post, b"_POST"),
    (SuperglobalKind::Files, b"_FILES"),
    (SuperglobalKind::Cookie, b"_COOKIE"),
    (SuperglobalKind::Request, b"_REQUEST"),
    (SuperglobalKind::Env, b"_ENV"),
    (SuperglobalKind::Session, b"_SESSION"),
    (SuperglobalKind::Globals, b"GLOBALS"),
];

pub struct VM {
    pub arena: Arena,
    pub operand_stack: Stack,
    pub frames: Vec<CallFrame>,
    pub context: RequestContext,
    pub last_return_value: Option<Handle>,
    pub silence_stack: Vec<u32>,
    pub pending_calls: Vec<PendingCall>,
    pub output_writer: Box<dyn OutputWriter>,
    pub error_handler: Box<dyn ErrorHandler>,
    pub output_buffers: Vec<crate::builtins::output_control::OutputBuffer>,
    pub implicit_flush: bool,
    pub url_rewrite_vars: HashMap<Rc<Vec<u8>>, Rc<Vec<u8>>>,
    trace_includes: bool,
    superglobal_map: HashMap<Symbol, SuperglobalKind>,
    pub(crate) var_handle_map: HashMap<Handle, Symbol>,
    pending_undefined: HashMap<Handle, Symbol>,
    pub(crate) suppress_undefined_notice: bool,
    pub execution_start_time: SystemTime,
    /// Track if we're currently executing finally blocks to prevent recursion
    executing_finally: bool,
    /// Stores a return value from within a finally block to override the original return
    finally_return_value: Option<Handle>,
    /// Strict types mode of the current builtin call's caller (for parameter validation)
    /// Reference: $PHP_SRC_PATH/Zend/zend_compile.h - ZEND_ARG_USES_STRICT_TYPES()
    pub(crate) builtin_call_strict: bool,
    /// Profiling: count of opcodes executed
    pub(crate) opcodes_executed: u64,
    /// Profiling: count of function calls
    pub(crate) function_calls: u64,
    /// Memory limit in bytes (0 = unlimited)
    pub(crate) memory_limit: usize,
    /// Sandboxing: allow file I/O operations
    pub(crate) allow_file_io: bool,
    /// Sandboxing: allow network operations
    pub(crate) allow_network: bool,
    /// Sandboxing: allowed function names (None = all allowed)
    pub(crate) allowed_functions: Option<std::collections::HashSet<String>>,
    /// Sandboxing: disabled function names (blacklist)
    pub(crate) disable_functions: std::collections::HashSet<String>,
    /// Sandboxing: disabled class names (blacklist)
    pub(crate) disable_classes: std::collections::HashSet<String>,
}

impl VM {
    pub fn new(engine_context: Arc<EngineContext>) -> Self {
        let trace_includes = std::env::var_os("PHP_VM_TRACE_INCLUDE").is_some();
        if trace_includes {
            eprintln!("[php-vm] include tracing enabled");
        }
        let mut vm = Self {
            arena: Arena::new(),
            operand_stack: Stack::new(),
            frames: Vec::new(),
            context: RequestContext::new(engine_context),
            last_return_value: None,
            silence_stack: Vec::new(),
            pending_calls: Vec::new(),
            output_writer: Box::new(StdoutWriter::default()),
            error_handler: Box::new(StderrErrorHandler::default()),
            output_buffers: Vec::new(),
            implicit_flush: false,
            url_rewrite_vars: HashMap::new(),
            trace_includes,
            superglobal_map: HashMap::new(),
            var_handle_map: HashMap::new(),
            pending_undefined: HashMap::new(),
            suppress_undefined_notice: false,
            execution_start_time: SystemTime::now(),
            executing_finally: false,
            finally_return_value: None,
            builtin_call_strict: false,
            opcodes_executed: 0,
            function_calls: 0,
            memory_limit: 0,         // Unlimited by default
            allow_file_io: true,     // Allow by default
            allow_network: true,     // Allow by default
            allowed_functions: None, // All functions allowed by default
            disable_functions: std::collections::HashSet::new(),
            disable_classes: std::collections::HashSet::new(),
        };
        vm.initialize_superglobals();
        vm
    }

    /// Instantiate a class and call its constructor.
    pub fn instantiate_class(
        &mut self,
        class_name: Symbol,
        args: &[Handle],
    ) -> Result<Handle, String> {
        let resolved_class = self
            .resolve_class_name(class_name)
            .map_err(|e| format!("{:?}", e))?;

        if !self.context.classes.contains_key(&resolved_class) {
            self.trigger_autoload(resolved_class)
                .map_err(|e| format!("{:?}", e))?;
        }

        if let Some(_class_def) = self.context.classes.get(&resolved_class) {
            let properties = self.collect_properties(resolved_class, PropertyCollectionMode::All);

            let obj_data = ObjectData {
                class: resolved_class,
                properties,
                internal: None,
                dynamic_properties: std::collections::HashSet::new(),
            };

            let payload_handle = self.arena.alloc(Val::ObjPayload(obj_data));
            let obj_val = Val::Object(payload_handle);
            let obj_handle = self.arena.alloc(obj_val);

            // Check for constructor
            let constructor_name = self.context.interner.intern(b"__construct");
            let method_lookup = self.find_method(resolved_class, constructor_name);

            if let Some((constructor, vis, _, defined_class)) = method_lookup {
                // For internal instantiation, we might want to bypass visibility checks,
                // but let's keep them for now or assume internal calls are "public".

                // Collect args
                let mut frame = CallFrame::new(constructor.chunk.clone());
                frame.func = Some(constructor.clone());
                frame.this = Some(obj_handle);
                frame.is_constructor = true;
                frame.class_scope = Some(defined_class);
                frame.args = args.to_vec().into();
                self.push_frame(frame);

                // We need to execute the constructor.
                // This is tricky because we are already in a native function.
                // For now, let's just return the object and hope the caller knows what they are doing,
                // OR we can try to execute the frame if it's a native constructor.

                // If it's a native constructor, we can call it directly.
                let native_constructor = self.find_native_method(resolved_class, constructor_name);
                if let Some(native_entry) = native_constructor {
                    let saved_this = self.frames.last().and_then(|f| f.this);
                    if let Some(frame) = self.frames.last_mut() {
                        frame.this = Some(obj_handle);
                    }
                    (native_entry.handler)(self, args).map_err(|e| format!("{:?}", e))?;
                    if let Some(frame) = self.frames.last_mut() {
                        frame.this = saved_this;
                    }
                }
            } else {
                // Check for native constructor directly if no PHP method found
                let native_constructor = self.find_native_method(resolved_class, constructor_name);
                if let Some(native_entry) = native_constructor {
                    let saved_this = self.frames.last().and_then(|f| f.this);
                    if let Some(frame) = self.frames.last_mut() {
                        frame.this = Some(obj_handle);
                    }
                    (native_entry.handler)(self, args).map_err(|e| format!("{:?}", e))?;
                    if let Some(frame) = self.frames.last_mut() {
                        frame.this = saved_this;
                    }
                }
            }

            Ok(obj_handle)
        } else {
            Err(format!("Class {:?} not found", class_name))
        }
    }

    #[inline]
    fn method_lookup_key(&self, name: Symbol) -> Option<Symbol> {
        let name_bytes = self.context.interner.lookup(name)?;
        let lower = name_bytes.to_ascii_lowercase();
        self.context.interner.find(&lower)
    }

    #[inline]
    fn intern_lowercase_symbol(&mut self, name: Symbol) -> Result<Symbol, VmError> {
        let name_bytes = self
            .context
            .interner
            .lookup(name)
            .ok_or_else(|| VmError::RuntimeError("Invalid method symbol".into()))?;
        let lower = name_bytes.to_ascii_lowercase();
        Ok(self.context.interner.intern(&lower))
    }

    fn register_superglobal_symbols(&mut self) {
        for (kind, name) in SUPERGLOBAL_SPECS {
            let sym = self.context.interner.intern(name);
            self.superglobal_map.insert(sym, *kind);
        }
    }

    fn initialize_superglobals(&mut self) {
        self.register_superglobal_symbols();
        let entries: Vec<(Symbol, SuperglobalKind)> = self
            .superglobal_map
            .iter()
            .map(|(&sym, &kind)| (sym, kind))
            .collect();
        for (sym, kind) in entries {
            if !self.context.globals.contains_key(&sym) {
                let handle = self.create_superglobal_value(kind);
                self.arena.get_mut(handle).is_ref = true;
                self.context.globals.insert(sym, handle);
            }
        }
    }

    fn create_superglobal_value(&mut self, kind: SuperglobalKind) -> Handle {
        match kind {
            SuperglobalKind::Server => self.create_server_superglobal(),
            SuperglobalKind::Globals => self.create_globals_superglobal(),
            _ => self.arena.alloc(Val::Array(Rc::new(ArrayData::new()))),
        }
    }

    /// Create $GLOBALS superglobal - a read-only copy of the global symbol table (PHP 8.1+)
    /// In PHP 8.1+, $GLOBALS is a read-only copy. Modifications must be done via $GLOBALS['key'].
    fn create_globals_superglobal(&mut self) -> Handle {
        let mut map = IndexMap::new();
        // $GLOBALS elements must share handles with global variables for reference behavior
        // When you do $ref = &$GLOBALS['x'], it should reference the actual global $x

        // Include variables from context.globals (superglobals and 'global' keyword vars)
        for (sym, handle) in &self.context.globals {
            // Don't include $GLOBALS itself to avoid circular reference
            let key_bytes = self.context.interner.lookup(*sym).unwrap_or(b"");
            if key_bytes != b"GLOBALS" {
                // Use the exact same handle so references work correctly
                map.insert(ArrayKey::Str(Rc::new(key_bytes.to_vec())), *handle);
            }
        }

        // Include variables from the top-level frame (frame 0) if it exists
        // These are the actual global scope variables in PHP
        if let Some(frame) = self.frames.first() {
            for (sym, handle) in &frame.locals {
                let key_bytes = self.context.interner.lookup(*sym).unwrap_or(b"");
                if key_bytes != b"GLOBALS" {
                    // Only add if not already present (context.globals takes precedence)
                    let key = ArrayKey::Str(Rc::new(key_bytes.to_vec()));
                    map.entry(key).or_insert(*handle);
                }
            }
        }

        self.arena.alloc(Val::Array(ArrayData::from(map).into()))
    }

    fn create_server_superglobal(&mut self) -> Handle {
        let mut data = ArrayData::new();

        let insert_str = |vm: &mut Self, data: &mut ArrayData, key: &[u8], val: &[u8]| {
            let handle = vm.alloc_string_handle(val);
            Self::insert_array_value(data, key, handle);
        };

        insert_str(self, &mut data, b"SERVER_PROTOCOL", b"HTTP/1.1");
        insert_str(self, &mut data, b"REQUEST_METHOD", b"GET");
        insert_str(self, &mut data, b"HTTP_HOST", b"localhost");
        insert_str(self, &mut data, b"SERVER_NAME", b"localhost");
        insert_str(self, &mut data, b"SERVER_SOFTWARE", b"php-vm");
        insert_str(self, &mut data, b"SERVER_ADDR", b"127.0.0.1");
        insert_str(self, &mut data, b"REMOTE_ADDR", b"127.0.0.1");

        Self::insert_array_value(&mut data, b"REMOTE_PORT", self.arena.alloc(Val::Int(0)));
        Self::insert_array_value(&mut data, b"SERVER_PORT", self.arena.alloc(Val::Int(80)));

        insert_str(self, &mut data, b"REQUEST_SCHEME", b"http");
        insert_str(self, &mut data, b"HTTPS", b"off");
        insert_str(self, &mut data, b"QUERY_STRING", b"");
        insert_str(self, &mut data, b"REQUEST_URI", b"/");
        insert_str(self, &mut data, b"PATH_INFO", b"");
        insert_str(self, &mut data, b"ORIG_PATH_INFO", b"");

        let document_root = std::env::current_dir()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| ".".into());
        let normalized_root = if document_root == "/" {
            document_root.clone()
        } else {
            document_root.trim_end_matches('/').to_string()
        };
        let script_basename = "index.php";
        let script_name = format!("/{}", script_basename);
        let script_filename = if normalized_root.is_empty() {
            script_basename.to_string()
        } else if normalized_root == "/" {
            format!("/{}", script_basename)
        } else {
            format!("{}/{}", normalized_root, script_basename)
        };

        insert_str(self, &mut data, b"DOCUMENT_ROOT", document_root.as_bytes());
        insert_str(self, &mut data, b"SCRIPT_NAME", script_name.as_bytes());
        insert_str(self, &mut data, b"PHP_SELF", script_name.as_bytes());
        insert_str(
            self,
            &mut data,
            b"SCRIPT_FILENAME",
            script_filename.as_bytes(),
        );

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let request_time = now.as_secs() as i64;
        let request_time_float = now.as_secs_f64();

        Self::insert_array_value(
            &mut data,
            b"REQUEST_TIME",
            self.arena.alloc(Val::Int(request_time)),
        );
        Self::insert_array_value(
            &mut data,
            b"REQUEST_TIME_FLOAT",
            self.arena.alloc(Val::Float(request_time_float)),
        );

        self.arena.alloc(Val::Array(Rc::new(data)))
    }

    fn alloc_string_handle(&mut self, value: &[u8]) -> Handle {
        self.arena.alloc(Val::String(Rc::new(value.to_vec())))
    }

    fn insert_array_value(data: &mut ArrayData, key: &[u8], handle: Handle) {
        data.insert(ArrayKey::Str(Rc::new(key.to_vec())), handle);
    }

    pub(crate) fn ensure_superglobal_handle(&mut self, sym: Symbol) -> Option<Handle> {
        let kind = self.superglobal_map.get(&sym).copied()?;

        // Special handling for $GLOBALS - always refresh to ensure it's current
        if kind == SuperglobalKind::Globals {
            // Update the $GLOBALS array to reflect current global state
            let globals_sym = self.context.interner.intern(b"GLOBALS");
            return if let Some(&existing_handle) = self.context.globals.get(&globals_sym) {
                // Update the existing array in place, maintaining handle sharing
                let mut map = IndexMap::new();

                // Include variables from context.globals
                for (sym, handle) in &self.context.globals {
                    let key_bytes = self.context.interner.lookup(*sym).unwrap_or(b"");
                    if key_bytes != b"GLOBALS" {
                        // Use the exact same handle - this is critical for reference behavior
                        map.insert(ArrayKey::Str(Rc::new(key_bytes.to_vec())), *handle);
                    }
                }

                // Include variables from the top-level frame (frame 0) if it exists
                if let Some(frame) = self.frames.first() {
                    for (sym, handle) in &frame.locals {
                        let key_bytes = self.context.interner.lookup(*sym).unwrap_or(b"");
                        if key_bytes != b"GLOBALS" {
                            // Only add if not already present (context.globals takes precedence)
                            let key = ArrayKey::Str(Rc::new(key_bytes.to_vec()));
                            map.entry(key).or_insert(*handle);
                        }
                    }
                }

                // Update the array value in-place
                let array_val = Val::Array(ArrayData::from(map).into());
                self.arena.get_mut(existing_handle).value = array_val;
                Some(existing_handle)
            } else {
                // Create new $GLOBALS array
                let handle = self.create_globals_superglobal();
                self.arena.get_mut(handle).is_ref = true;
                self.context.globals.insert(sym, handle);
                Some(handle)
            };
        }

        let handle = if let Some(&existing) = self.context.globals.get(&sym) {
            existing
        } else {
            let new_handle = self.create_superglobal_value(kind);
            self.context.globals.insert(sym, new_handle);
            new_handle
        };
        self.arena.get_mut(handle).is_ref = true;
        Some(handle)
    }

    pub(crate) fn is_superglobal(&self, sym: Symbol) -> bool {
        self.superglobal_map.contains_key(&sym)
    }

    /// Check if a symbol refers to the $GLOBALS superglobal
    pub(crate) fn is_globals_symbol(&self, sym: Symbol) -> bool {
        if let Some(kind) = self.superglobal_map.get(&sym) {
            *kind == SuperglobalKind::Globals
        } else {
            false
        }
    }

    /// Sync a modification to $GLOBALS['key'] = value back to the global symbol table
    /// In PHP 8.1+, modifying $GLOBALS['key'] should update the actual global variable
    pub(crate) fn sync_globals_write(&mut self, key_bytes: &[u8], val_handle: Handle) {
        // Intern the key to get its symbol
        let sym = self.context.interner.intern(key_bytes);

        // Don't create circular reference by syncing GLOBALS itself
        if key_bytes != b"GLOBALS" {
            // Mark handle as ref so future operations on it (via $GLOBALS) work correctly
            self.arena.get_mut(val_handle).is_ref = true;

            // Always update the global symbol table with the same handle
            // This ensures references work correctly
            self.context.globals.insert(sym, val_handle);

            // Also update the top-level frame's locals so it picks up the change
            // This handles the case where we modify $GLOBALS from within a function
            // and then access the variable at the top level
            if let Some(frame) = self.frames.first_mut() {
                frame.locals.insert(sym, val_handle);
            }
        }
    }

    fn sync_globals_key(&mut self, key: &ArrayKey, val_handle: Handle) {
        match key {
            ArrayKey::Str(bytes) => self.sync_globals_write(bytes, val_handle),
            ArrayKey::Int(num) => {
                let key_str = num.to_string();
                self.sync_globals_write(key_str.as_bytes(), val_handle);
            }
        }
    }

    /// Check if a handle belongs to a global variable
    /// Used to determine if array operations should be in-place
    pub(crate) fn is_global_variable_handle(&self, handle: Handle) -> bool {
        // Check context.globals (superglobals and 'global' keyword vars)
        if self.context.globals.values().any(|&h| h == handle) {
            return true;
        }

        // Check top-level frame (global scope variables)
        if let Some(frame) = self.frames.first() {
            if frame.locals.values().any(|&h| h == handle) {
                return true;
            }
        }

        false
    }

    pub fn new_with_context(context: RequestContext) -> Self {
        let trace_includes = std::env::var_os("PHP_VM_TRACE_INCLUDE").is_some();
        if trace_includes {
            eprintln!("[php-vm] include tracing enabled");
        }
        let mut vm = Self {
            arena: Arena::new(),
            operand_stack: Stack::new(),
            frames: Vec::new(),
            context,
            last_return_value: None,
            silence_stack: Vec::new(),
            pending_calls: Vec::new(),
            output_writer: Box::new(StdoutWriter::default()),
            error_handler: Box::new(StderrErrorHandler::default()),
            output_buffers: Vec::new(),
            implicit_flush: false,
            url_rewrite_vars: HashMap::new(),
            trace_includes,
            superglobal_map: HashMap::new(),
            var_handle_map: HashMap::new(),
            pending_undefined: HashMap::new(),
            suppress_undefined_notice: false,
            execution_start_time: SystemTime::now(),
            executing_finally: false,
            finally_return_value: None,
            builtin_call_strict: false,
            opcodes_executed: 0,
            function_calls: 0,
            memory_limit: 0,         // Unlimited by default
            allow_file_io: true,     // Allow by default
            allow_network: true,     // Allow by default
            allowed_functions: None, // All functions allowed by default
            disable_functions: std::collections::HashSet::new(),
            disable_classes: std::collections::HashSet::new(),
        };
        vm.initialize_superglobals();
        vm
    }

    /// Check if execution time limit has been exceeded
    /// Returns an error if the time limit is exceeded and not unlimited (0)
    fn check_execution_timeout(&self) -> Result<(), VmError> {
        if self.context.max_execution_time <= 0 {
            // 0 or negative means unlimited
            return Ok(());
        }

        let elapsed = self
            .execution_start_time
            .elapsed()
            .map_err(|e| VmError::RuntimeError(format!("Time error: {}", e)))?;

        let elapsed_secs = elapsed.as_secs() as i64;

        if elapsed_secs >= self.context.max_execution_time {
            return Err(VmError::RuntimeError(format!(
                "Maximum execution time of {} second{} exceeded",
                self.context.max_execution_time,
                if self.context.max_execution_time == 1 {
                    ""
                } else {
                    "s"
                }
            )));
        }

        Ok(())
    }

    /// Get approximate memory usage in bytes
    /// This is a simplified estimate based on arena storage
    fn get_memory_usage(&self) -> usize {
        // Estimate: each Zval is approximately 64 bytes (rough estimate)
        // This includes the Val enum discriminant and typical payloads
        const ZVAL_SIZE: usize = 64;
        self.arena.len() * ZVAL_SIZE
    }

    /// Check if memory limit has been exceeded
    /// Returns an error if the limit is exceeded and not unlimited (0)
    fn check_memory_limit(&self) -> Result<(), VmError> {
        if self.memory_limit == 0 {
            // 0 means unlimited
            return Ok(());
        }

        let current_usage = self.get_memory_usage();

        if current_usage >= self.memory_limit {
            return Err(VmError::RuntimeError(format!(
                "Allowed memory size of {} bytes exhausted (tried to allocate {} bytes)",
                self.memory_limit, current_usage
            )));
        }

        Ok(())
    }

    /// Check if a function call is allowed based on sandboxing rules
    /// First checks whitelist (if present), then checks blacklist
    pub(crate) fn check_function_allowed(&self, function_name: &str) -> Result<(), VmError> {
        // If there's a whitelist, function must be in it
        if let Some(ref allowed) = self.allowed_functions {
            if !allowed.contains(function_name) {
                return Err(VmError::RuntimeError(format!(
                    "Call to '{}' has been disabled for security reasons",
                    function_name
                )));
            }
        }

        // Check blacklist
        if self.disable_functions.contains(function_name) {
            return Err(VmError::RuntimeError(format!(
                "Call to '{}' has been disabled for security reasons",
                function_name
            )));
        }

        Ok(())
    }

    /// Check if a class can be instantiated based on sandboxing rules
    pub(crate) fn check_class_allowed(&self, class_name: &str) -> Result<(), VmError> {
        if self.disable_classes.contains(class_name) {
            return Err(VmError::RuntimeError(format!(
                "Class '{}' has been disabled for security reasons",
                class_name
            )));
        }
        Ok(())
    }

    /// Check if file I/O is allowed based on sandboxing rules
    pub(crate) fn check_file_io_allowed(&self) -> Result<(), VmError> {
        if !self.allow_file_io {
            return Err(VmError::RuntimeError(
                "File operations have been disabled for security reasons".to_string(),
            ));
        }
        Ok(())
    }

    /// Check if network operations are allowed based on sandboxing rules
    pub(crate) fn check_network_allowed(&self) -> Result<(), VmError> {
        if !self.allow_network {
            return Err(VmError::RuntimeError(
                "Network operations have been disabled for security reasons".to_string(),
            ));
        }
        Ok(())
    }

    /// Report an error respecting the error_reporting level
    /// Also stores the error in context.last_error for error_get_last()
    pub(crate) fn report_error(&mut self, level: ErrorLevel, message: &str) {
        let level_bitmask = level.to_bitmask();

        // Store this as the last error regardless of error_reporting level
        self.context.last_error = Some(crate::runtime::context::ErrorInfo {
            error_type: level_bitmask as i64,
            message: message.to_string(),
            file: "Unknown".to_string(),
            line: 0,
        });

        // Only report if the error level is enabled in error_reporting
        if (self.context.error_reporting & level_bitmask) != 0 {
            self.error_handler.report(level, message);
        }
    }

    pub fn with_output_writer(mut self, writer: Box<dyn OutputWriter>) -> Self {
        self.output_writer = writer;
        self
    }

    pub fn set_output_writer(&mut self, writer: Box<dyn OutputWriter>) {
        self.output_writer = writer;
    }

    pub fn set_error_handler(&mut self, handler: Box<dyn ErrorHandler>) {
        self.error_handler = handler;
    }

    pub(crate) fn write_output(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        // If output buffering is active, write to the buffer
        if let Some(buffer) = self.output_buffers.last_mut() {
            buffer.content.extend_from_slice(bytes);

            // Check if we need to flush based on chunk_size
            if buffer.chunk_size > 0 && buffer.content.len() >= buffer.chunk_size {
                // Auto-flush when chunk size is reached
                if buffer.is_flushable() {
                    // This is tricky - we need to flush without recursion
                    // For now, just let it accumulate
                }
            }
            Ok(())
        } else {
            // No buffering, write directly
            self.output_writer.write(bytes)
        }
    }

    pub fn flush_output(&mut self) -> Result<(), VmError> {
        self.output_writer.flush()
    }

    /// Trigger an error/warning/notice
    pub fn trigger_error(&mut self, level: ErrorLevel, message: &str) {
        self.report_error(level, message);
    }

    /// Call a user-defined function
    pub fn call_user_function(
        &mut self,
        callable: Handle,
        args: &[Handle],
    ) -> Result<Handle, String> {
        // This is a simplified version - the actual implementation would need to handle
        // different callable types (closures, function names, arrays with [object, method], etc.)
        match &self.arena.get(callable).value {
            Val::String(name) => {
                // Function name as string
                let name_bytes = name.as_ref();
                if let Some(func) = self.context.engine.registry.get_function(name_bytes) {
                    func(self, args)
                } else {
                    Err(format!(
                        "Call to undefined function {}",
                        String::from_utf8_lossy(name_bytes)
                    ))
                }
            }
            _ => {
                // For now, simplified - would need full callable handling
                Err("Invalid callback".into())
            }
        }
    }

    /// Convert a value to string
    pub fn value_to_string(&self, handle: Handle) -> Result<Vec<u8>, String> {
        let val = self.arena.get(handle);
        Ok(val.value.to_php_string_bytes())
    }

    pub fn print_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.write_output(bytes).map_err(|err| match err {
            VmError::RuntimeError(msg) => msg,
            VmError::Exception(_) => "Output aborted by exception".into(),
            _ => format!("{}", err),
        })
    }

    // Safe frame access helpers (no-panic guarantee)
    #[inline(always)]
    fn current_frame(&self) -> Result<&CallFrame, VmError> {
        self.frames
            .last()
            .ok_or_else(|| VmError::RuntimeError("No active frame".into()))
    }

    #[inline(always)]
    fn current_frame_mut(&mut self) -> Result<&mut CallFrame, VmError> {
        self.frames
            .last_mut()
            .ok_or_else(|| VmError::RuntimeError("No active frame".into()))
    }

    #[inline]
    fn pop_frame(&mut self) -> Result<CallFrame, VmError> {
        self.frames
            .pop()
            .ok_or_else(|| VmError::RuntimeError("Frame stack empty".into()))
    }

    #[inline]
    pub(super) fn push_frame(&mut self, mut frame: CallFrame) {
        if frame.stack_base.is_none() {
            frame.stack_base = Some(self.operand_stack.len());
        }
        self.frames.push(frame);
    }

    #[inline]
    fn collect_call_args<T>(&mut self, arg_count: T) -> Result<ArgList, VmError>
    where
        T: Into<usize>,
    {
        let count = arg_count.into();
        let mut args = ArgList::with_capacity(count);
        let prev_suppress = self.suppress_undefined_notice;
        self.suppress_undefined_notice = true;
        for _ in 0..count {
            args.push(self.pop_operand_required()?);
        }
        self.suppress_undefined_notice = prev_suppress;
        args.reverse();
        Ok(args)
    }

    #[inline]
    pub(crate) fn handle_pending_undefined_for_call(
        &mut self,
        args: &ArgList,
        by_ref: Option<&[usize]>,
    ) {
        for (idx, handle) in args.iter().enumerate() {
            if self.pending_undefined.contains_key(handle) {
                let is_by_ref = by_ref.map_or(false, |list| list.contains(&idx));
                if is_by_ref {
                    self.pending_undefined.remove(handle);
                } else {
                    self.maybe_report_undefined(*handle);
                }
            }
        }
    }

    #[inline]
    pub(crate) fn maybe_report_undefined(&mut self, handle: Handle) {
        if let Some(sym) = self.pending_undefined.remove(&handle) {
            let var_name = self
                .context
                .interner
                .lookup(sym)
                .map(String::from_utf8_lossy)
                .unwrap_or_else(|| "unknown".into());
            let msg = format!("Undefined variable: ${}", var_name);
            self.report_error(ErrorLevel::Notice, &msg);
        }
    }

    fn resolve_script_path(&self, raw: &str) -> Result<PathBuf, VmError> {
        let candidate = PathBuf::from(raw);
        if candidate.is_absolute() {
            return Ok(candidate);
        }

        // 1. Try relative to the directory of the currently executing script
        if let Some(frame) = self.frames.last() {
            if let Some(file_path) = &frame.chunk.file_path {
                let current_dir = Path::new(file_path).parent();
                if let Some(dir) = current_dir {
                    let resolved = dir.join(&candidate);
                    if resolved.exists() {
                        return Ok(resolved);
                    }
                }
            }
        }

        // 2. Fallback to CWD
        let cwd = std::env::current_dir()
            .map_err(|e| VmError::RuntimeError(format!("Failed to resolve path {}: {}", raw, e)))?;
        Ok(cwd.join(candidate))
    }

    #[inline]
    fn canonical_path_string(path: &Path) -> String {
        std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .into_owned()
    }

    fn trigger_autoload(&mut self, class_name: Symbol) -> Result<(), VmError> {
        let callsite_strict_types = self
            .frames
            .last()
            .map(|frame| frame.chunk.strict_types)
            .unwrap_or(false);

        // Get class name bytes
        let class_name_bytes = self
            .context
            .interner
            .lookup(class_name)
            .ok_or_else(|| VmError::RuntimeError("Invalid class name".into()))?;

        // Create a string handle for the class name
        let class_name_handle = self
            .arena
            .alloc(Val::String(Rc::new(class_name_bytes.to_vec())));

        // Call each autoloader
        let autoloaders = self.context.autoloaders.clone();
        for autoloader_handle in autoloaders {
            let args = smallvec::smallvec![class_name_handle];
            // Try to invoke the autoloader
            if let Ok(()) =
                self.invoke_callable_value(autoloader_handle, args, callsite_strict_types)
            {
                // Run until the frame completes
                let depth = self.frames.len();
                if depth > 0 {
                    self.run_loop(depth - 1)?;
                }

                // Check if the class was loaded
                if self.context.classes.contains_key(&class_name) {
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    /// Walk the inheritance chain and apply a predicate
    /// Reference: $PHP_SRC_PATH/Zend/zend_inheritance.c
    pub(crate) fn walk_inheritance_chain<F, T>(
        &self,
        start_class: Symbol,
        mut predicate: F,
    ) -> Option<T>
    where
        F: FnMut(&ClassDef, Symbol) -> Option<T>,
    {
        let mut current = Some(start_class);
        while let Some(class_sym) = current {
            if let Some(class_def) = self.context.classes.get(&class_sym) {
                if let Some(result) = predicate(class_def, class_sym) {
                    return Some(result);
                }
                current = class_def.parent;
            } else {
                break;
            }
        }
        None
    }

    pub fn find_method(
        &self,
        class_name: Symbol,
        method_name: Symbol,
    ) -> Option<(Rc<UserFunc>, Visibility, bool, Symbol)> {
        // Walk the inheritance chain (class -> parent -> parent -> ...)
        // Reference: $PHP_SRC_PATH/Zend/zend_API.c - zend_std_get_method
        let lower_method_key = self.method_lookup_key(method_name);
        let search_name = self
            .context
            .interner
            .lookup(method_name);

        self.walk_inheritance_chain(class_name, |def, _cls_sym| {
            // Try direct lookup with case-insensitive key
            if let Some(key) = lower_method_key {
                if let Some(entry) = def.methods.get(&key) {
                    return Some((
                        entry.func.clone(),
                        entry.visibility,
                        entry.is_static,
                        entry.declaring_class,
                    ));
                }
            }

            // Fallback: scan all methods with case-insensitive comparison
            if let Some(ref search_bytes) = search_name {
                let search_lower = search_bytes.to_ascii_lowercase();
                for entry in def.methods.values() {
                    if let Some(stored_bytes) = self.context.interner.lookup(entry.name) {
                        if stored_bytes.to_ascii_lowercase() == *search_lower {
                            return Some((
                                entry.func.clone(),
                                entry.visibility,
                                entry.is_static,
                                entry.declaring_class,
                            ));
                        }
                    }
                }
            }

            None
        })
    }

    pub fn find_native_method(
        &self,
        class_name: Symbol,
        method_name: Symbol,
    ) -> Option<crate::runtime::context::NativeMethodEntry> {
        // Walk the inheritance chain to find native methods
        self.walk_inheritance_chain(class_name, |_def, cls| {
            self.context
                .native_methods
                .get(&(cls, method_name))
                .cloned()
        })
    }

    pub(crate) fn call_native_method_simple(
        &mut self,
        obj_handle: Handle,
        method_name: Symbol,
    ) -> Result<Handle, VmError> {
        let class_name = if let Val::Object(h) = self.arena.get(obj_handle).value {
            if let Val::ObjPayload(data) = &self.arena.get(h).value {
                data.class
            } else {
                return Err(VmError::RuntimeError("Invalid object payload".into()));
            }
        } else {
            return Err(VmError::RuntimeError("Not an object".into()));
        };

        if let Some(native_entry) = self.find_native_method(class_name, method_name) {
            let saved_this = self.frames.last().and_then(|f| f.this);
            if let Some(frame) = self.frames.last_mut() {
                frame.this = Some(obj_handle);
            }
            let result = (native_entry.handler)(self, &[]).map_err(VmError::RuntimeError)?;
            if let Some(frame) = self.frames.last_mut() {
                frame.this = saved_this;
            }
            Ok(result)
        } else {
            Err(VmError::RuntimeError(format!(
                "Native method not found: {}::{}",
                String::from_utf8_lossy(
                    self.context
                        .interner
                        .lookup(class_name)
                        .unwrap_or(b"unknown")
                ),
                String::from_utf8_lossy(
                    self.context
                        .interner
                        .lookup(method_name)
                        .unwrap_or(b"unknown")
                )
            )))
        }
    }

    pub fn collect_methods(&self, class_name: Symbol, caller_scope: Option<Symbol>) -> Vec<Symbol> {
        // Collect methods from entire inheritance chain
        // Reference: $PHP_SRC_PATH/Zend/zend_API.c - reflection functions
        let mut seen = std::collections::HashSet::new();
        let mut visible = Vec::new();
        let mut current_class = Some(class_name);

        // Walk from child to parent, tracking which methods we've seen
        // Child methods override parent methods
        while let Some(cls) = current_class {
            if let Some(def) = self.context.classes.get(&cls) {
                for entry in def.methods.values() {
                    // Only add if we haven't seen this method name yet (respect overrides)
                    let lower_name =
                        if let Some(name_bytes) = self.context.interner.lookup(entry.name) {
                            name_bytes.to_ascii_lowercase()
                        } else {
                            continue;
                        };

                    if !seen.contains(&lower_name) {
                        if self.method_visible_to(
                            entry.declaring_class,
                            entry.visibility,
                            caller_scope,
                        ) {
                            visible.push(entry.name);
                            seen.insert(lower_name);
                        }
                    }
                }
                current_class = def.parent;
            } else {
                break;
            }
        }

        visible.sort_by(|a, b| {
            let a_bytes = self.context.interner.lookup(*a).unwrap_or(b"");
            let b_bytes = self.context.interner.lookup(*b).unwrap_or(b"");
            a_bytes.cmp(b_bytes)
        });

        visible
    }

    pub fn has_property(&self, class_name: Symbol, prop_name: Symbol) -> bool {
        self.walk_inheritance_chain(class_name, |def, _cls| {
            if def.properties.contains_key(&prop_name) {
                Some(true)
            } else {
                None
            }
        })
        .is_some()
    }

    /// Deep clone a Val, allocating arrays and their contents into the arena
    /// This is needed for property defaults that contain arrays, since each
    /// object instance needs its own copy of the array
    fn deep_clone_val(&mut self, val: &Val) -> Handle {
        match val {
            Val::ConstArray(const_arr) => {
                use crate::core::value::{ArrayData, ArrayKey};
                let mut new_array = ArrayData::new();

                // Clone the const array data to avoid borrow conflicts
                let entries: Vec<_> = const_arr
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                // Deep clone each element, converting ConstArrayKey to ArrayKey
                for (key, val) in entries {
                    let runtime_key = match key {
                        crate::core::value::ConstArrayKey::Int(i) => ArrayKey::Int(i),
                        crate::core::value::ConstArrayKey::Str(s) => ArrayKey::Str(s),
                    };
                    let runtime_val_handle = self.deep_clone_val(&val);
                    new_array.insert(runtime_key, runtime_val_handle);
                }

                self.arena.alloc(Val::Array(Rc::new(new_array)))
            }
            Val::Array(arr) => {
                // Runtime array - needs deep cloning of all elements
                use crate::core::value::ArrayData;
                let mut new_array = ArrayData::new();

                // Clone entries to avoid borrow conflicts
                let entries: Vec<_> = arr.map.iter().map(|(k, v)| (k.clone(), *v)).collect();

                for (key, val_handle) in entries {
                    let val = &self.arena.get(val_handle).value;
                    let val_clone = val.clone(); // Clone to avoid borrow conflict
                    let new_val_handle = self.deep_clone_val(&val_clone);
                    new_array.insert(key, new_val_handle);
                }

                self.arena.alloc(Val::Array(Rc::new(new_array)))
            }
            other => {
                // For non-array values, shallow clone is fine (strings are Rc)
                self.arena.alloc(other.clone())
            }
        }
    }

    pub fn collect_properties(
        &mut self,
        class_name: Symbol,
        mode: PropertyCollectionMode,
    ) -> IndexMap<Symbol, Handle> {
        let mut properties = IndexMap::new();
        let mut chain = Vec::new();
        let mut current_class = Some(class_name);

        // Collect class definitions
        while let Some(name) = current_class {
            if let Some(def) = self.context.classes.get(&name) {
                chain.push(def);
                current_class = def.parent;
            } else {
                break;
            }
        }

        // Clone property data to avoid borrow conflicts
        let mut prop_data: Vec<(Symbol, Val, Visibility)> = Vec::new();
        for def in chain.iter().rev() {
            for (name, entry) in &def.properties {
                if let PropertyCollectionMode::VisibleTo(scope) = mode {
                    if self
                        .check_prop_visibility(class_name, *name, scope)
                        .is_err()
                    {
                        continue;
                    }
                }
                prop_data.push((*name, entry.default_value.clone(), entry.visibility));
            }
        }

        // Now deep clone property defaults
        for (name, default_val, _visibility) in prop_data {
            let handle = self.deep_clone_val(&default_val);
            properties.insert(name, handle);
        }

        properties
    }

    pub fn is_subclass_of(&self, child: Symbol, parent: Symbol) -> bool {
        if child == parent {
            return true;
        }

        if let Some(def) = self.context.classes.get(&child) {
            // Check parent class
            if let Some(p) = def.parent {
                if self.is_subclass_of(p, parent) {
                    return true;
                }
            }
            // Check interfaces
            for &interface in &def.interfaces {
                if self.is_subclass_of(interface, parent) {
                    return true;
                }
            }
        }
        false
    }

    /// Convert ReturnType to TypeHint for signature validation
    fn return_type_to_type_hint(&self, rt: &crate::compiler::chunk::ReturnType) -> Option<TypeHint> {
        use crate::compiler::chunk::ReturnType;
        
        match rt {
            ReturnType::Int => Some(TypeHint::Int),
            ReturnType::Float => Some(TypeHint::Float),
            ReturnType::String => Some(TypeHint::String),
            ReturnType::Bool => Some(TypeHint::Bool),
            ReturnType::Array => Some(TypeHint::Array),
            ReturnType::Object => Some(TypeHint::Object),
            ReturnType::Callable => Some(TypeHint::Callable),
            ReturnType::Iterable => Some(TypeHint::Iterable),
            ReturnType::Mixed => Some(TypeHint::Mixed),
            ReturnType::Void => Some(TypeHint::Void),
            ReturnType::Never => Some(TypeHint::Never),
            ReturnType::Null => Some(TypeHint::Null),
            ReturnType::Named(sym) => Some(TypeHint::Class(*sym)),
            ReturnType::Union(types) => {
                let hints: Vec<_> = types.iter().filter_map(|t| self.return_type_to_type_hint(t)).collect();
                if hints.is_empty() {
                    None
                } else {
                    Some(TypeHint::Union(hints))
                }
            }
            ReturnType::Intersection(types) => {
                let hints: Vec<_> = types.iter().filter_map(|t| self.return_type_to_type_hint(t)).collect();
                if hints.is_empty() {
                    None
                } else {
                    Some(TypeHint::Intersection(hints))
                }
            }
            ReturnType::Nullable(inner) => {
                if let Some(hint) = self.return_type_to_type_hint(inner) {
                    Some(TypeHint::Union(vec![hint, TypeHint::Null]))
                } else {
                    None
                }
            }
            _ => None, // For True, False, Static, etc. - treat as Mixed
        }
    }

    /// Check if an object implements the ArrayAccess interface
    /// Reference: $PHP_SRC_PATH/Zend/zend_interfaces.c - instanceof_function_ex
    fn implements_array_access(&mut self, class_name: Symbol) -> bool {
        let array_access_sym = self.context.interner.intern(b"ArrayAccess");
        self.is_subclass_of(class_name, array_access_sym)
    }

    /// Validate property type assignment with type coercion (PHP-style weak typing)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - i_zend_verify_property_type, zend_verify_weak_scalar_type_hint
    fn validate_property_type(
        &mut self,
        class_name: Symbol,
        prop_name: Symbol,
        val_handle: Handle,
    ) -> Result<(), VmError> {
        // Get property type hint from class definition
        let type_hint = self.walk_inheritance_chain(class_name, |def, _cls| {
            def.properties.get(&prop_name).and_then(|entry| entry.type_hint.clone())
        });

        if let Some(hint) = type_hint {
            // Try to coerce the value to match the type hint (weak typing mode)
            // PHP always uses weak typing for properties (no strict_types for property assignments in our current impl)
            let coerced_val = self.coerce_to_type_hint(val_handle, &hint)?;
            
            if let Some(new_val) = coerced_val {
                // Value was coerced, update the handle
                self.arena.get_mut(val_handle).value = new_val;
            }
            // If None, value already matches (no coercion needed)
        }
        
        Ok(())
    }

    /// Validate static property type assignment with type coercion
    fn validate_static_property_type(
        &mut self,
        class_name: Symbol,
        prop_name: Symbol,
        val_handle: Handle,
    ) -> Result<(), VmError> {
        // Get static property type hint from class definition
        let type_hint = self.walk_inheritance_chain(class_name, |def, _cls| {
            def.static_properties.get(&prop_name).and_then(|entry| entry.type_hint.clone())
        });

        if let Some(hint) = type_hint {
            // Try to coerce the value to match the type hint
            let coerced_val = self.coerce_to_type_hint(val_handle, &hint)?;
            
            if let Some(new_val) = coerced_val {
                // Value was coerced, update the handle
                self.arena.get_mut(val_handle).value = new_val;
            }
        }
        
        Ok(())
    }

    /// Coerce value to match type hint following PHP's weak scalar type conversion rules
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_verify_weak_scalar_type_hint
    /// 
    /// Type preference order: int -> float -> string -> bool
    /// Returns Some(new_val) if coercion happened, None if already matches, Err if cannot coerce
    fn coerce_to_type_hint(
        &mut self,
        val_handle: Handle,
        hint: &TypeHint,
    ) -> Result<Option<Val>, VmError> {
        let val = self.arena.get(val_handle).value.clone();

        match hint {
            TypeHint::Int => {
                match &val {
                    Val::Int(_) => Ok(None), // Already int
                    Val::Float(f) => Ok(Some(Val::Int(*f as i64))), // float -> int
                    Val::String(s) => {
                        // Try to parse as int
                        let s_str = String::from_utf8_lossy(s);
                        if let Ok(i) = s_str.trim().parse::<i64>() {
                            Ok(Some(Val::Int(i)))
                        } else if let Ok(f) = s_str.trim().parse::<f64>() {
                            Ok(Some(Val::Int(f as i64)))
                        } else {
                            Err(self.type_error_for_property(&val, hint))
                        }
                    }
                    Val::Bool(b) => Ok(Some(Val::Int(if *b { 1 } else { 0 }))), // bool -> int
                    _ => Err(self.type_error_for_property(&val, hint)),
                }
            }
            TypeHint::Float => {
                match &val {
                    Val::Float(_) => Ok(None), // Already float
                    Val::Int(i) => Ok(Some(Val::Float(*i as f64))), // int -> float (always allowed)
                    Val::String(s) => {
                        let s_str = String::from_utf8_lossy(s);
                        if let Ok(f) = s_str.trim().parse::<f64>() {
                            Ok(Some(Val::Float(f)))
                        } else {
                            Err(self.type_error_for_property(&val, hint))
                        }
                    }
                    Val::Bool(b) => Ok(Some(Val::Float(if *b { 1.0 } else { 0.0 }))),
                    _ => Err(self.type_error_for_property(&val, hint)),
                }
            }
            TypeHint::String => {
                match &val {
                    Val::String(_) => Ok(None), // Already string
                    Val::Int(i) => Ok(Some(Val::String(i.to_string().into_bytes().into()))),
                    Val::Float(f) => Ok(Some(Val::String(f.to_string().into_bytes().into()))),
                    Val::Bool(b) => Ok(Some(Val::String((if *b { "1" } else { "" }).as_bytes().to_vec().into()))),
                    Val::Null => Ok(Some(Val::String(b"".to_vec().into()))),
                    _ => Err(self.type_error_for_property(&val, hint)),
                }
            }
            TypeHint::Bool => {
                match &val {
                    Val::Bool(_) => Ok(None), // Already bool
                    Val::Int(i) => Ok(Some(Val::Bool(*i != 0))),
                    Val::Float(f) => Ok(Some(Val::Bool(*f != 0.0))),
                    Val::String(s) => Ok(Some(Val::Bool(!s.is_empty() && s.as_ref() != b"0"))),
                    Val::Null => Ok(Some(Val::Bool(false))),
                    Val::Array(arr_data) => {
                        // Array is truthy if it has elements
                        Ok(Some(Val::Bool(!arr_data.map.is_empty())))
                    }
                    _ => Ok(Some(Val::Bool(true))), // Most values are truthy
                }
            }
            TypeHint::Array => {
                match &val {
                    Val::Array(_) | Val::ConstArray(_) => Ok(None),
                    _ => Err(self.type_error_for_property(&val, hint)),
                }
            }
            TypeHint::Object => {
                match &val {
                    Val::Object(_) => Ok(None),
                    _ => Err(self.type_error_for_property(&val, hint)),
                }
            }
            TypeHint::Null => {
                match &val {
                    Val::Null => Ok(None),
                    _ => Err(self.type_error_for_property(&val, hint)),
                }
            }
            TypeHint::Mixed => Ok(None), // Mixed accepts anything, no coercion
            TypeHint::Callable => {
                if self.is_callable(val_handle) {
                    Ok(None)
                } else {
                    Err(self.type_error_for_property(&val, hint))
                }
            }
            TypeHint::Iterable => match &val {
                Val::Array(_) | Val::ConstArray(_) => Ok(None),
                Val::Object(_) => {
                    if let Ok(obj_class) = self.extract_object_class(val_handle) {
                        let traversable_sym = self.context.interner.intern(b"Traversable");
                        if self.is_subclass_of(obj_class, traversable_sym) {
                            Ok(None)
                        } else {
                            Err(self.type_error_for_property(&val, hint))
                        }
                    } else {
                        Err(self.type_error_for_property(&val, hint))
                    }
                }
                _ => Err(self.type_error_for_property(&val, hint)),
            },
            TypeHint::Class(class_sym) => {
                if let Val::Object(_) = &val {
                    if let Ok(obj_class) = self.extract_object_class(val_handle) {
                        if self.is_subclass_of(obj_class, *class_sym) {
                            return Ok(None);
                        }
                    }
                }
                Err(self.type_error_for_property(&val, hint))
            }
            TypeHint::Union(types) => {
                // Try each type in the union
                for t in types {
                    if let Ok(result) = self.coerce_to_type_hint(val_handle, t) {
                        return Ok(result);
                    }
                }
                Err(self.type_error_for_property(&val, hint))
            }
            TypeHint::Intersection(types) => {
                if types
                    .iter()
                    .all(|t| self.matches_type_hint_without_coercion(val_handle, t))
                {
                    Ok(None)
                } else {
                    Err(self.type_error_for_property(&val, hint))
                }
            }
            _ => Err(self.type_error_for_property(&val, hint)),
        }
    }

    /// Helper to create type error for property assignment
    fn type_error_for_property(&self, val: &Val, hint: &TypeHint) -> VmError {
        let type_str = self.type_hint_to_string(hint);
        let actual_type = self.get_val_type_name(val);
        VmError::RuntimeError(format!(
            "Cannot assign {} to property of type {}",
            actual_type, type_str
        ))
    }

    /// Convert type hint to string for error messages
    fn type_hint_to_string(&self, hint: &TypeHint) -> String {
        match hint {
            TypeHint::Int => "int".to_string(),
            TypeHint::Float => "float".to_string(),
            TypeHint::String => "string".to_string(),
            TypeHint::Bool => "bool".to_string(),
            TypeHint::Array => "array".to_string(),
            TypeHint::Object => "object".to_string(),
            TypeHint::Null => "null".to_string(),
            TypeHint::Mixed => "mixed".to_string(),
            TypeHint::Void => "void".to_string(),
            TypeHint::Never => "never".to_string(),
            TypeHint::Callable => "callable".to_string(),
            TypeHint::Iterable => "iterable".to_string(),
            TypeHint::Class(sym) => {
                let bytes = self.context.interner.lookup(*sym).unwrap_or(b"???");
                String::from_utf8_lossy(bytes).to_string()
            }
            TypeHint::Union(types) => {
                types.iter().map(|t| self.type_hint_to_string(t)).collect::<Vec<_>>().join("|")
            }
            TypeHint::Intersection(types) => {
                types.iter().map(|t| self.type_hint_to_string(t)).collect::<Vec<_>>().join("&")
            }
        }
    }

    /// Get type name of a value for error messages
    fn get_val_type_name(&self, val: &Val) -> &str {
        match val {
            Val::Null => "null",
            Val::Bool(_) => "bool",
            Val::Int(_) => "int",
            Val::Float(_) => "float",
            Val::String(_) => "string",
            Val::Array(_) => "array",
            Val::Object(_) => "object",
            Val::Resource(_) => "resource",
            Val::ObjPayload(_) => "object",
            Val::ConstArray(_) => "array",
            Val::AppendPlaceholder => "unknown",
            Val::Uninitialized => "uninitialized",
        }
    }

    fn matches_type_hint_without_coercion(&mut self, val_handle: Handle, hint: &TypeHint) -> bool {
        let val = self.arena.get(val_handle).value.clone();

        match hint {
            TypeHint::Int => matches!(val, Val::Int(_)),
            TypeHint::Float => matches!(val, Val::Float(_)),
            TypeHint::String => matches!(val, Val::String(_)),
            TypeHint::Bool => matches!(val, Val::Bool(_)),
            TypeHint::Null => matches!(val, Val::Null),
            TypeHint::Array => matches!(val, Val::Array(_) | Val::ConstArray(_)),
            TypeHint::Object => matches!(val, Val::Object(_)),
            TypeHint::Mixed => true,
            TypeHint::Callable => self.is_callable(val_handle),
            TypeHint::Iterable => match val {
                Val::Array(_) | Val::ConstArray(_) => true,
                Val::Object(_) => {
                    if let Ok(obj_class) = self.extract_object_class(val_handle) {
                        let traversable_sym = self.context.interner.intern(b"Traversable");
                        self.is_subclass_of(obj_class, traversable_sym)
                    } else {
                        false
                    }
                }
                _ => false,
            },
            TypeHint::Class(class_sym) => match val {
                Val::Object(_) => {
                    if let Ok(obj_class) = self.extract_object_class(val_handle) {
                        self.is_subclass_of(obj_class, *class_sym)
                    } else {
                        false
                    }
                }
                _ => false,
            },
            TypeHint::Union(types) => types
                .iter()
                .any(|t| self.matches_type_hint_without_coercion(val_handle, t)),
            TypeHint::Intersection(types) => types
                .iter()
                .all(|t| self.matches_type_hint_without_coercion(val_handle, t)),
            TypeHint::Void | TypeHint::Never => false,
        }
    }

    /// Extract class symbol from object handle
    /// Reference: $PHP_SRC_PATH/Zend/zend_object_handlers.c
    #[inline]
    pub(crate) fn extract_object_class(&self, obj_handle: Handle) -> Result<Symbol, VmError> {
        let obj_val = &self.arena.get(obj_handle).value;
        match obj_val {
            Val::Object(payload_handle) => {
                let payload = self.arena.get(*payload_handle);
                match &payload.value {
                    Val::ObjPayload(obj_data) => Ok(obj_data.class),
                    _ => Err(VmError::RuntimeError("Invalid object payload".into())),
                }
            }
            _ => Err(VmError::RuntimeError("Not an object".into())),
        }
    }

    /// Execute a user-defined method with given arguments
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_call_function
    pub(crate) fn invoke_user_method(
        &mut self,
        this_handle: Handle,
        func: Rc<UserFunc>,
        args: Vec<Handle>,
        scope: Symbol,
        called_scope: Symbol,
    ) -> Result<(), VmError> {
        let mut frame = CallFrame::new(func.chunk.clone());
        frame.func = Some(func);
        frame.this = Some(this_handle);
        frame.class_scope = Some(scope);
        frame.called_scope = Some(called_scope);
        frame.args = args.into();

        self.push_frame(frame);

        // Execute until this frame completes
        let target_depth = self.frames.len() - 1;
        self.run_loop(target_depth)
    }

    /// Call a magic method synchronously and return the result value
    /// This is used for property access magic methods (__get, __set, __isset, __unset)
    /// where we need the result immediately to continue execution
    /// Reference: $PHP_SRC_PATH/Zend/zend_object_handlers.c - zend_std_read_property
    fn call_magic_method_sync(
        &mut self,
        obj_handle: Handle,
        class_name: Symbol,
        magic_method: Symbol,
        args: Vec<Handle>,
    ) -> Result<Option<Handle>, VmError> {
        if let Some((method, _, _, defined_class)) = self.find_method(class_name, magic_method) {
            let mut frame = CallFrame::new(method.chunk.clone());
            frame.func = Some(method.clone());
            frame.this = Some(obj_handle);
            frame.class_scope = Some(defined_class);
            frame.called_scope = Some(class_name);

            // Set parameters
            for (i, arg_handle) in args.iter().enumerate() {
                if let Some(param) = method.params.get(i) {
                    frame.locals.insert(param.name, *arg_handle);
                }
            }

            self.push_frame(frame);

            // Execute synchronously until frame completes
            let target_depth = self.frames.len() - 1;
            self.run_loop(target_depth)?;

            // Return the last return value
            Ok(self.last_return_value)
        } else {
            Ok(None)
        }
    }

    pub(crate) fn resolve_class_name(&self, class_name: Symbol) -> Result<Symbol, VmError> {
        let name_bytes = self
            .context
            .interner
            .lookup(class_name)
            .ok_or(VmError::RuntimeError("Invalid class symbol".into()))?;
        if name_bytes.eq_ignore_ascii_case(b"self") {
            let frame = self
                .frames
                .last()
                .ok_or(VmError::RuntimeError("No active frame".into()))?;
            return frame.class_scope.ok_or(VmError::RuntimeError(
                "Cannot access self:: when no class scope is active".into(),
            ));
        }
        if name_bytes.eq_ignore_ascii_case(b"parent") {
            let frame = self
                .frames
                .last()
                .ok_or(VmError::RuntimeError("No active frame".into()))?;
            let scope = frame.class_scope.ok_or(VmError::RuntimeError(
                "Cannot access parent:: when no class scope is active".into(),
            ))?;
            let class_def = self
                .context
                .classes
                .get(&scope)
                .ok_or(VmError::RuntimeError("Class not found".into()))?;
            return class_def
                .parent
                .ok_or(VmError::RuntimeError("Parent not found".into()));
        }
        if name_bytes.eq_ignore_ascii_case(b"static") {
            let frame = self
                .frames
                .last()
                .ok_or(VmError::RuntimeError("No active frame".into()))?;
            return frame.called_scope.ok_or(VmError::RuntimeError(
                "Cannot access static:: when no called scope is active".into(),
            ));
        }
        Ok(class_name)
    }

    pub(crate) fn find_class_constant(
        &self,
        start_class: Symbol,
        const_name: Symbol,
    ) -> Result<(Val, Visibility, Symbol), VmError> {
        // Reference: $PHP_SRC_PATH/Zend/zend_compile.c - constant access
        let found = self.walk_inheritance_chain(start_class, |def, cls| {
            def.constants
                .get(&const_name)
                .map(|(val, vis)| (val.clone(), *vis, cls))
        });

        if let Some((val, vis, defining_class)) = found {
            self.check_const_visibility(defining_class, vis)?;
            Ok((val, vis, defining_class))
        } else {
            let const_str =
                String::from_utf8_lossy(self.context.interner.lookup(const_name).unwrap_or(b"???"));
            let class_str = String::from_utf8_lossy(
                self.context.interner.lookup(start_class).unwrap_or(b"???"),
            );
            Err(VmError::RuntimeError(format!(
                "Undefined class constant {}::{}",
                class_str, const_str
            )))
        }
    }

    /// Helper to extract class name and property name from stack for static property operations
    /// Returns (property_name_symbol, defining_class, current_value)
    fn prepare_static_prop_access(&mut self) -> Result<(Symbol, Symbol, Val), VmError> {
        let prop_name_handle = self
            .operand_stack
            .pop()
            .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
        let class_name_handle = self
            .operand_stack
            .pop()
            .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

        let class_name = match &self.arena.get(class_name_handle).value {
            Val::String(s) => self.context.interner.intern(s),
            _ => return Err(VmError::RuntimeError("Class name must be string".into())),
        };

        let prop_name = match &self.arena.get(prop_name_handle).value {
            Val::String(s) => self.context.interner.intern(s),
            _ => return Err(VmError::RuntimeError("Property name must be string".into())),
        };

        let resolved_class = self.resolve_class_name(class_name)?;
        let (current_val, visibility, defining_class) =
            self.find_static_prop(resolved_class, prop_name)?;
        self.check_const_visibility(defining_class, visibility)?;

        Ok((prop_name, defining_class, current_val))
    }

    pub(crate) fn find_static_prop(
        &self,
        start_class: Symbol,
        prop_name: Symbol,
    ) -> Result<(Val, Visibility, Symbol), VmError> {
        // Reference: $PHP_SRC_PATH/Zend/zend_compile.c - static property access
        let found = self.walk_inheritance_chain(start_class, |def, cls| {
            def.static_properties
                .get(&prop_name)
                .map(|entry| (entry.value.clone(), entry.visibility, cls))
        });

        if let Some((val, vis, defining_class)) = found {
            // Check visibility using same logic as instance properties
            let caller_scope = self.get_current_class();
            if !self.property_visible_to(defining_class, vis, caller_scope) {
                let prop_str = String::from_utf8_lossy(
                    self.context.interner.lookup(prop_name).unwrap_or(b"???"),
                );
                let class_str = String::from_utf8_lossy(
                    self.context
                        .interner
                        .lookup(defining_class)
                        .unwrap_or(b"???"),
                );
                let vis_str = match vis {
                    Visibility::Private => "private",
                    Visibility::Protected => "protected",
                    Visibility::Public => unreachable!(),
                };
                return Err(VmError::RuntimeError(format!(
                    "Cannot access {} property {}::${}",
                    vis_str, class_str, prop_str
                )));
            }
            Ok((val, vis, defining_class))
        } else {
            let prop_str =
                String::from_utf8_lossy(self.context.interner.lookup(prop_name).unwrap_or(b"???"));
            let class_str = String::from_utf8_lossy(
                self.context.interner.lookup(start_class).unwrap_or(b"???"),
            );
            Err(VmError::RuntimeError(format!(
                "Undefined static property {}::${}",
                class_str, prop_str
            )))
        }
    }

    pub(crate) fn get_current_class(&self) -> Option<Symbol> {
        self.frames.last().and_then(|f| f.class_scope)
    }

    /// Create and push a method frame
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_execute_data initialization
    #[inline]
    pub(super) fn push_method_frame(
        &mut self,
        func: Rc<UserFunc>,
        this: Option<Handle>,
        class_scope: Symbol,
        called_scope: Symbol,
        args: ArgList,
        callsite_strict_types: bool,
    ) {
        let mut frame = CallFrame::new(func.chunk.clone());
        frame.func = Some(func);
        frame.this = this;
        frame.class_scope = Some(class_scope);
        frame.called_scope = Some(called_scope);
        frame.args = args;
        frame.callsite_strict_types = callsite_strict_types;
        self.push_frame(frame);
    }

    /// Create and push a function frame (no class scope)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c
    #[inline]
    fn push_function_frame(&mut self, func: Rc<UserFunc>, args: ArgList, callsite_strict_types: bool) {
        let mut frame = CallFrame::new(func.chunk.clone());
        frame.func = Some(func);
        frame.args = args;
        frame.callsite_strict_types = callsite_strict_types;
        self.push_frame(frame);
    }

    /// Create and push a closure frame with captures
    /// Reference: $PHP_SRC_PATH/Zend/zend_closures.c
    #[inline]
    pub(super) fn push_closure_frame(
        &mut self,
        closure: &ClosureData,
        args: ArgList,
        callsite_strict_types: bool,
    ) {
        let mut frame = CallFrame::new(closure.func.chunk.clone());
        frame.func = Some(closure.func.clone());
        frame.args = args;
        frame.this = closure.this;
        frame.callsite_strict_types = callsite_strict_types;

        for (sym, handle) in &closure.captures {
            frame.locals.insert(*sym, *handle);
        }

        self.push_frame(frame);
    }

    /// Bind function/method parameters to frame locals, handling by-ref semantics
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_bind_args
    #[inline]
    pub(super) fn bind_params_to_frame(
        &mut self,
        frame: &mut CallFrame,
        params: &[crate::compiler::chunk::FuncParam],
    ) -> Result<(), VmError> {
        for (i, param) in params.iter().enumerate() {
            if i < frame.args.len() {
                let arg_handle = frame.args[i];
                if param.by_ref {
                    // For by-ref params, mark as reference and use directly
                    if !self.arena.get(arg_handle).is_ref {
                        self.arena.get_mut(arg_handle).is_ref = true;
                    }
                    frame.locals.insert(param.name, arg_handle);
                } else {
                    // For by-value params, clone the value
                    let val = self.arena.get(arg_handle).value.clone();
                    let final_handle = self.arena.alloc(val);
                    frame.locals.insert(param.name, final_handle);
                }
            }
            // Note: Default values are handled by OpCode::RecvInit, not here
        }
        Ok(())
    }

    /// Check if a class allows dynamic properties
    ///
    /// A class allows dynamic properties if:
    /// 1. It has the #[AllowDynamicProperties] attribute
    /// 2. It has __get or __set magic methods
    /// 3. It's stdClass or __PHP_Incomplete_Class (special cases)
    fn class_allows_dynamic_properties(&self, class_name: Symbol) -> bool {
        // Check for #[AllowDynamicProperties] attribute
        if let Some(class_def) = self.context.classes.get(&class_name) {
            if class_def.allows_dynamic_properties {
                return true;
            }
        }

        // Check for magic methods
        let get_sym = self.context.interner.find(b"__get");
        let set_sym = self.context.interner.find(b"__set");

        if let Some(get_sym) = get_sym {
            if self.find_method(class_name, get_sym).is_some() {
                return true;
            }
        }

        if let Some(set_sym) = set_sym {
            if self.find_method(class_name, set_sym).is_some() {
                return true;
            }
        }

        // Check for special classes
        if let Some(class_bytes) = self.context.interner.lookup(class_name) {
            if class_bytes == b"stdClass" || class_bytes == b"__PHP_Incomplete_Class" {
                return true;
            }
        }

        false
    }

    /// Check if writing a dynamic property should emit a deprecation warning
    /// Reference: $PHP_SRC_PATH/Zend/zend_object_handlers.c - zend_std_write_property
    pub(crate) fn check_dynamic_property_write(
        &mut self,
        obj_handle: Handle,
        prop_name: Symbol,
    ) -> bool {
        // Get object data
        let obj_val = self.arena.get(obj_handle);
        let payload_handle = if let Val::Object(h) = obj_val.value {
            h
        } else {
            return false; // Not an object
        };

        let payload_val = self.arena.get(payload_handle);
        let obj_data = if let Val::ObjPayload(data) = &payload_val.value {
            data
        } else {
            return false;
        };

        let class_name = obj_data.class;

        // Check if this property is already tracked as dynamic in this instance
        if obj_data.dynamic_properties.contains(&prop_name) {
            return false; // Already created, no warning needed
        }

        // Check if this is a declared property in the class hierarchy
        let mut is_declared = false;
        let mut current = Some(class_name);

        while let Some(name) = current {
            if let Some(def) = self.context.classes.get(&name) {
                if def.properties.contains_key(&prop_name) {
                    is_declared = true;
                    break;
                }
                current = def.parent;
            } else {
                break;
            }
        }

        if !is_declared && !self.class_allows_dynamic_properties(class_name) {
            // This is a new dynamic property creation - emit warning
            let class_str = self
                .context
                .interner
                .lookup(class_name)
                .map(|b| String::from_utf8_lossy(b).to_string())
                .unwrap_or_else(|| "Unknown".to_string());
            let prop_str = self
                .context
                .interner
                .lookup(prop_name)
                .map(|b| String::from_utf8_lossy(b).to_string())
                .unwrap_or_else(|| "unknown".to_string());

            self.report_error(
                ErrorLevel::Deprecated,
                &format!(
                    "Creation of dynamic property {}::${} is deprecated",
                    class_str, prop_str
                ),
            );

            // Mark this property as dynamic in the object instance
            let payload_val_mut = self.arena.get_mut(payload_handle);
            if let Val::ObjPayload(ref mut data) = payload_val_mut.value {
                data.dynamic_properties.insert(prop_name);
            }

            return true; // Warning was emitted
        }

        false
    }

    fn is_instance_of(&self, obj_handle: Handle, class_sym: Symbol) -> bool {
        let obj_val = self.arena.get(obj_handle);
        if let Val::Object(payload_handle) = obj_val.value {
            if let Val::ObjPayload(data) = &self.arena.get(payload_handle).value {
                let obj_class = data.class;
                if obj_class == class_sym {
                    return true;
                }
                return self.is_subclass_of(obj_class, class_sym);
            }
        }
        false
    }

    fn handle_exception(&mut self, ex_handle: Handle) -> bool {
        // Validate that the exception is a Throwable
        let throwable_sym = self.context.interner.intern(b"Throwable");
        if !self.is_instance_of(ex_handle, throwable_sym) {
            // Not a valid exception object - this shouldn't happen if Throw validates properly
            self.frames.clear();
            return false;
        }

        let mut frame_idx = self.frames.len();
        let mut finally_blocks = Vec::new(); // Track finally blocks to execute

        // Unwind stack, collecting finally blocks
        while frame_idx > 0 {
            frame_idx -= 1;

            let (ip, chunk) = {
                let frame = &self.frames[frame_idx];
                let ip = if frame.ip > 0 { frame.ip - 1 } else { 0 } as u32;
                (ip, frame.chunk.clone())
            };

            // Check for matching catch or finally blocks
            let mut found_catch = false;

            for entry in &chunk.catch_table {
                if ip >= entry.start && ip < entry.end {
                    // Check for finally-only entry (no catch type)
                    if entry.catch_type.is_none() {
                        // This is a finally-only entry - collect it
                        finally_blocks.push((
                            frame_idx,
                            chunk.clone(),
                            entry.target,
                            entry.finally_end,
                        ));
                        continue;
                    }

                    // Check for matching catch block
                    if let Some(type_sym) = entry.catch_type {
                        if self.is_instance_of(ex_handle, type_sym) {
                            // Execute any finally blocks collected so far before entering catch
                            self.execute_finally_blocks(&finally_blocks);
                            finally_blocks.clear();

                            // Found matching catch block
                            self.frames.truncate(frame_idx + 1);
                            let frame = &mut self.frames[frame_idx];
                            frame.ip = entry.target as usize;
                            self.operand_stack.push(ex_handle);

                            // Mark finally for execution after catch completes
                            if let Some(finally_tgt) = entry.finally_target {
                                frame.pending_finally = Some(finally_tgt as usize);
                            }

                            found_catch = true;
                            break;
                        }
                    }
                }
            }

            if found_catch {
                return true;
            }
        }

        // No catch found - execute finally blocks during unwinding
        // In PHP, finally blocks execute from innermost to outermost
        // We've already collected them in the correct order during iteration
        self.execute_finally_blocks(&finally_blocks);
        self.frames.clear();
        false
    }

    /// Execute finally blocks
    /// Blocks should be provided in the order they should execute (innermost to outermost)
    fn execute_finally_blocks(
        &mut self,
        finally_blocks: &[(usize, Rc<CodeChunk>, u32, Option<u32>)],
    ) {
        // Execute in the order provided (innermost to outermost)
        for (frame_idx, chunk, target, end) in finally_blocks.iter() {
            // Truncate frames to the finally's level
            self.frames.truncate(*frame_idx + 1);

            // Save the original frame state
            let saved_stack_base = self.frames[*frame_idx].stack_base;

            // Set up the frame to execute the finally block
            {
                let frame = &mut self.frames[*frame_idx];
                frame.chunk = chunk.clone();
                frame.ip = *target as usize;
                // Set stack_base to current operand stack length so return can work correctly
                frame.stack_base = Some(self.operand_stack.len());
            }

            // Execute only the finally block, not code after it
            if let Some(finally_end) = end {
                // Execute statements until IP reaches finally_end
                loop {
                    let should_continue = {
                        if *frame_idx >= self.frames.len() {
                            false
                        } else {
                            let frame = &self.frames[*frame_idx];
                            frame.ip < *finally_end as usize && frame.ip < frame.chunk.code.len()
                        }
                    };

                    if !should_continue {
                        break;
                    }

                    let op = {
                        let frame = &self.frames[*frame_idx];
                        frame.chunk.code[frame.ip]
                    };

                    self.frames[*frame_idx].ip += 1;

                    // Execute the opcode, ignoring errors from finally itself
                    let _ = self.execute_opcode(op, *frame_idx);

                    // If the frame was popped (return happened), break out
                    if *frame_idx >= self.frames.len() {
                        break;
                    }
                }
            } else {
                // Fallback: execute until frame is popped (old behavior)
                let _ = self.run_loop(*frame_idx + 1);
                if self.frames.len() > *frame_idx {
                    self.frames.truncate(*frame_idx);
                }
            }

            // Restore stack_base if frame still exists
            if *frame_idx < self.frames.len() {
                self.frames[*frame_idx].stack_base = saved_stack_base;
            }
        }
    }

    /// Collect finally blocks that need to execute before a return
    /// Returns a list of (frame_idx, chunk, target, end) tuples
    fn collect_finally_blocks_for_return(&self) -> Vec<(usize, Rc<CodeChunk>, u32, Option<u32>)> {
        let mut finally_blocks = Vec::new();

        if self.frames.is_empty() {
            return finally_blocks;
        }

        let current_frame_idx = self.frames.len() - 1;
        let frame = &self.frames[current_frame_idx];
        let ip = if frame.ip > 0 { frame.ip - 1 } else { 0 } as u32;

        // Collect all finally blocks that contain the current IP
        // We need to collect from innermost to outermost (will reverse later for execution)
        let mut entries_to_execute: Vec<_> = frame
            .chunk
            .catch_table
            .iter()
            .filter(|entry| ip >= entry.start && ip < entry.end && entry.catch_type.is_none())
            .collect();

        // Sort by range size (smaller = more nested = inner)
        // Execute from inner to outer
        entries_to_execute.sort_by_key(|entry| entry.end - entry.start);

        for entry in entries_to_execute {
            if let Some(end) = entry.finally_end {
                finally_blocks.push((
                    current_frame_idx,
                    frame.chunk.clone(),
                    entry.target,
                    Some(end),
                ));
            }
        }

        finally_blocks
    }

    /// Collect finally blocks for break/continue jumps
    /// Similar to collect_finally_blocks_for_return but used for break/continue
    fn collect_finally_blocks_for_jump(&self) -> Vec<(usize, Rc<CodeChunk>, u32, Option<u32>)> {
        let mut finally_blocks = Vec::new();

        if self.frames.is_empty() {
            return finally_blocks;
        }

        let current_frame_idx = self.frames.len() - 1;
        let frame = &self.frames[current_frame_idx];
        let ip = if frame.ip > 0 { frame.ip - 1 } else { 0 } as u32;

        // Collect all finally blocks that contain the current IP
        let mut entries_to_execute: Vec<_> = frame
            .chunk
            .catch_table
            .iter()
            .filter(|entry| ip >= entry.start && ip < entry.end && entry.catch_type.is_none())
            .collect();

        // Sort by range size (smaller = more nested = inner)
        // Execute from inner to outer
        entries_to_execute.sort_by_key(|entry| entry.end - entry.start);

        for entry in entries_to_execute {
            if let Some(end) = entry.finally_end {
                finally_blocks.push((
                    current_frame_idx,
                    frame.chunk.clone(),
                    entry.target,
                    Some(end),
                ));
            }
        }

        finally_blocks
    }

    /// Complete the return after finally blocks have executed
    fn complete_return(
        &mut self,
        mut ret_val: Handle,
        force_by_ref: bool,
        target_depth: usize,
    ) -> Result<(), VmError> {
        // Verify return type BEFORE popping the frame
        // Extract return type info AND the callee's strict_types flag
        let return_type_check = {
            let frame = self.current_frame()?;
            frame.func.as_ref().and_then(|f| {
                f.return_type.as_ref().map(|rt| {
                    let func_name = self
                        .context
                        .interner
                        .lookup(f.chunk.name)
                        .map(|b| String::from_utf8_lossy(b).to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let callee_strict = f.chunk.strict_types;
                    (rt.clone(), func_name, callee_strict)
                })
            })
        };

        if let Some((ret_type, func_name, callee_strict)) = return_type_check {
            // Check return type with callee's strictness (not caller's!)
            if !self.check_return_type(ret_val, &ret_type)? {
                // Type mismatch - attempt coercion in weak mode
                if !callee_strict {
                    // Weak mode: try coercion
                    if let Some(coerced_handle) = self.coerce_parameter_value(ret_val, &ret_type)? {
                        // Coercion succeeded, use coerced value
                        ret_val = coerced_handle;
                    } else {
                        // Coercion failed in weak mode - still throw error
                        let val_type = self.get_type_name(ret_val);
                        let expected_type = self.return_type_to_string(&ret_type);

                        return Err(VmError::RuntimeError(format!(
                            "{}(): Return value must be of type {}, {} returned",
                            func_name, expected_type, val_type
                        )));
                    }
                } else {
                    // Strict mode: throw TypeError
                    let val_type = self.get_type_name(ret_val);
                    let expected_type = self.return_type_to_string(&ret_type);

                    return Err(VmError::RuntimeError(format!(
                        "{}(): Return value must be of type {}, {} returned",
                        func_name, expected_type, val_type
                    )));
                }
            }
        }

        let frame_base = {
            let frame = self.current_frame()?;
            frame.stack_base.unwrap_or(0)
        };

        while self.operand_stack.len() > frame_base {
            self.operand_stack.pop();
        }

        let popped_frame = self.pop_frame()?;

        if let Some(gen_handle) = popped_frame.generator {
            let gen_val = self.arena.get(gen_handle);
            if let Val::Object(payload_handle) = &gen_val.value {
                let payload = self.arena.get(*payload_handle);
                if let Val::ObjPayload(obj_data) = &payload.value {
                    if let Some(internal) = &obj_data.internal {
                        if let Ok(gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>()
                        {
                            let mut data = gen_data.borrow_mut();
                            data.state = GeneratorState::Finished;
                        }
                    }
                }
            }
        }

        let returns_ref = force_by_ref || popped_frame.chunk.returns_ref;

        // Handle return by reference
        let final_ret_val = if returns_ref {
            if !self.arena.get(ret_val).is_ref {
                self.arena.get_mut(ret_val).is_ref = true;
            }
            ret_val
        } else {
            // Function returns by value: if ret_val is a ref, dereference (copy) it.
            if self.arena.get(ret_val).is_ref {
                let val = self.arena.get(ret_val).value.clone();
                self.arena.alloc(val)
            } else {
                ret_val
            }
        };

        if self.frames.len() == target_depth {
            self.last_return_value = Some(final_ret_val);
            return Ok(());
        }

        if popped_frame.discard_return {
            // Return value is discarded
        } else if popped_frame.is_constructor {
            if let Some(this_handle) = popped_frame.this {
                self.operand_stack.push(this_handle);
            } else {
                return Err(VmError::RuntimeError(
                    "Constructor frame missing 'this'".into(),
                ));
            }
        } else {
            self.operand_stack.push(final_ret_val);
        }

        Ok(())
    }

    pub fn run(&mut self, chunk: Rc<CodeChunk>) -> Result<(), VmError> {
        let mut initial_frame = CallFrame::new(chunk);

        // Inject globals into the top-level frame locals
        for (symbol, handle) in &self.context.globals {
            initial_frame.locals.insert(*symbol, *handle);
        }

        self.push_frame(initial_frame);
        self.run_loop(0)
    }

    pub fn run_frame(&mut self, frame: CallFrame) -> Result<Handle, VmError> {
        let depth = self.frames.len();
        self.push_frame(frame);
        self.run_loop(depth)?;
        self.last_return_value
            .ok_or(VmError::RuntimeError("No return value".into()))
    }

    /// Call a callable (function, closure, method) and return its result
    pub fn call_callable(
        &mut self,
        callable_handle: Handle,
        args: ArgList,
    ) -> Result<Handle, VmError> {
        let callsite_strict_types = self
            .frames
            .last()
            .map(|frame| frame.chunk.strict_types)
            .unwrap_or(false);
        let initial_depth = self.frames.len();
        let stack_before = self.operand_stack.len();

        self.invoke_callable_value(callable_handle, args, callsite_strict_types)?;

        if self.frames.len() > initial_depth {
            self.run_loop(initial_depth)?;
            // After running user function, result is in last_return_value
            Ok(self
                .last_return_value
                .unwrap_or_else(|| self.arena.alloc(Val::Null)))
        } else if self.operand_stack.len() > stack_before {
            // Native function call - result is on stack, pop and return it
            // Don't set last_return_value since we're not completing a frame
            Ok(self.operand_stack.pop().unwrap())
        } else {
            // No result was produced
            Ok(self.arena.alloc(Val::Null))
        }
    }

    pub(crate) fn convert_to_string(&mut self, handle: Handle) -> Result<Vec<u8>, VmError> {
        let val = self.arena.get(handle).value.clone();
        match val {
            Val::String(s) => Ok(s.to_vec()),
            Val::Int(i) => Ok(i.to_string().into_bytes()),
            Val::Float(f) => Ok(f.to_string().into_bytes()),
            Val::Bool(b) => Ok(if b { b"1".to_vec() } else { vec![] }),
            Val::Null => Ok(vec![]),
            Val::Object(h) => {
                let obj_zval = self.arena.get(h);
                if let Val::ObjPayload(obj_data) = &obj_zval.value {
                    let to_string_magic = self.context.interner.intern(b"__toString");
                    if let Some((magic_func, _, _, magic_class)) =
                        self.find_method(obj_data.class, to_string_magic)
                    {
                        // Save caller's return value ONLY if we're actually calling __toString
                        // (Zend allocates per-call zval to avoid corruption)
                        let saved_return_value = self.last_return_value.take();

                        let mut frame = CallFrame::new(magic_func.chunk.clone());
                        frame.func = Some(magic_func.clone());
                        frame.this = Some(handle); // Pass the object handle, not payload
                        frame.class_scope = Some(magic_class);
                        frame.called_scope = Some(obj_data.class);

                        let depth = self.frames.len();
                        self.push_frame(frame);
                        self.run_loop(depth)?;

                        let ret_handle = self.last_return_value.ok_or(VmError::RuntimeError(
                            "__toString must return a value".into(),
                        ))?;
                        let ret_val = self.arena.get(ret_handle).value.clone();

                        // Restore caller's return value
                        self.last_return_value = saved_return_value;

                        match ret_val {
                            Val::String(s) => Ok(s.to_vec()),
                            _ => Err(VmError::RuntimeError(
                                "__toString must return a string".into(),
                            )),
                        }
                    } else {
                        // No __toString method - cannot convert
                        let class_name = String::from_utf8_lossy(
                            self.context
                                .interner
                                .lookup(obj_data.class)
                                .unwrap_or(b"Unknown"),
                        );
                        Err(VmError::RuntimeError(format!(
                            "Object of class {} could not be converted to string",
                            class_name
                        )))
                    }
                } else {
                    Err(VmError::RuntimeError("Invalid object payload".into()))
                }
            }
            Val::Array(_) => {
                self.error_handler
                    .report(ErrorLevel::Notice, "Array to string conversion");
                Ok(b"Array".to_vec())
            }
            Val::Resource(_) => {
                self.error_handler
                    .report(ErrorLevel::Notice, "Resource to string conversion");
                // PHP outputs "Resource id #N" where N is the resource ID
                // For now, just return "Resource"
                Ok(b"Resource".to_vec())
            }
            _ => {
                // Other types (e.g., ObjPayload) should not occur here
                Err(VmError::RuntimeError(
                    "Cannot convert value to string".to_string(),
                ))
            }
        }
    }

    fn handle_return(&mut self, force_by_ref: bool, target_depth: usize) -> Result<(), VmError> {
        let frame_base = {
            let frame = self.current_frame()?;
            frame.stack_base.unwrap_or(0)
        };

        let ret_val = if self.operand_stack.len() > frame_base {
            self.pop_operand_required()?
        } else {
            self.arena.alloc(Val::Null)
        };

        // If we're already executing finally blocks, store the return value and return
        // This allows the finally block to override the original return value
        if self.executing_finally {
            // Store the return value from the finally block
            self.finally_return_value = Some(ret_val);
            // Don't pop the frame or complete the return yet
            // Just return Ok to let the finally execution continue
            return Ok(());
        }

        // Check if we need to execute finally blocks before returning
        let finally_blocks = self.collect_finally_blocks_for_return();

        // Execute finally blocks if any
        if !finally_blocks.is_empty() {
            // Save return value before executing finally
            let saved_ret_val = ret_val;

            // Mark that we're executing finally blocks
            self.executing_finally = true;
            self.finally_return_value = None;

            // Execute finally blocks
            self.execute_finally_blocks(&finally_blocks);

            // Clear the flag
            self.executing_finally = false;

            // Check if finally block set a return value (override)
            let final_ret_val = if let Some(finally_val) = self.finally_return_value.take() {
                // Finally block returned - use its value instead
                finally_val
            } else {
                // Finally didn't return - use original value
                saved_ret_val
            };

            // Continue with normal return handling using the final value
            return self.complete_return(final_ret_val, force_by_ref, target_depth);
        }

        // No finally blocks - proceed with normal return
        self.complete_return(ret_val, force_by_ref, target_depth)
    }

    fn run_loop(&mut self, target_depth: usize) -> Result<(), VmError> {
        const TIMEOUT_CHECK_INTERVAL: u64 = 1000; // Check every 1000 instructions
        let mut instructions_until_timeout_check = TIMEOUT_CHECK_INTERVAL;
        const MEMORY_CHECK_INTERVAL: u64 = 5000; // Check every 5000 instructions (less frequent)
        let mut instructions_until_memory_check = MEMORY_CHECK_INTERVAL;

        while self.frames.len() > target_depth {
            // Increment opcode counter for profiling
            self.opcodes_executed += 1;

            // Periodically check execution timeout (countdown is faster than modulo)
            instructions_until_timeout_check -= 1;
            if instructions_until_timeout_check == 0 {
                self.check_execution_timeout()?;
                instructions_until_timeout_check = TIMEOUT_CHECK_INTERVAL;
            }

            // Periodically check memory limit
            instructions_until_memory_check -= 1;
            if instructions_until_memory_check == 0 {
                self.check_memory_limit()?;
                instructions_until_memory_check = MEMORY_CHECK_INTERVAL;
            }

            let op = {
                let frame = self.current_frame_mut()?;
                if frame.ip >= frame.chunk.code.len() {
                    self.frames.pop();
                    continue;
                }
                let op = frame.chunk.code[frame.ip];
                frame.ip += 1;
                op
            };

            let res = self.execute_opcode(op, target_depth);

            if let Err(e) = res {
                match e {
                    VmError::Exception(h) => {
                        if !self.handle_exception(h) {
                            return Err(VmError::Exception(h));
                        }
                    }
                    _ => return Err(e),
                }
            }
        }
        // Flush output when script completes normally
        if target_depth == 0 {
            self.output_writer.flush()?;
        }
        Ok(())
    }

    fn exec_stack_op(&mut self, op: OpCode) -> Result<(), VmError> {
        match op {
            OpCode::Const(idx) => {
                let frame = self.current_frame()?;
                let val = frame.chunk.constants[idx as usize].clone();
                let handle = self.arena.alloc(val);
                self.operand_stack.push(handle);
            }
            OpCode::Pop => {
                let _ = self.pop_operand_required()?;
            }
            OpCode::Dup => {
                let handle = self.peek_operand()?;
                self.operand_stack.push(handle);
            }
            OpCode::Nop => {}
            _ => unreachable!("Not a stack op"),
        }
        Ok(())
    }

    fn exec_math_op(&mut self, op: OpCode) -> Result<(), VmError> {
        match op {
            OpCode::Add => self.exec_add()?,
            OpCode::Sub => self.exec_sub()?,
            OpCode::Mul => self.exec_mul()?,
            OpCode::Div => self.exec_div()?,
            OpCode::Mod => self.exec_mod()?,
            OpCode::Pow => self.exec_pow()?,
            OpCode::BitwiseAnd => self.exec_bitwise_and()?,
            OpCode::BitwiseOr => self.exec_bitwise_or()?,
            OpCode::BitwiseXor => self.exec_bitwise_xor()?,
            OpCode::ShiftLeft => self.exec_shift_left()?,
            OpCode::ShiftRight => self.exec_shift_right()?,
            OpCode::BitwiseNot => self.exec_bitwise_not()?,
            OpCode::BoolNot => self.exec_bool_not()?,
            _ => unreachable!("Not a math op"),
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn set_ip(&mut self, target: usize) -> Result<(), VmError> {
        let frame = self.current_frame_mut()?;
        frame.ip = target;
        Ok(())
    }

    pub(crate) fn jump_if<F>(&mut self, target: usize, condition: F) -> Result<(), VmError>
    where
        F: Fn(&Val) -> bool,
    {
        let handle = self.pop_operand_required()?;
        let val = &self.arena.get(handle).value;
        if condition(val) {
            self.set_ip(target)?;
        }
        Ok(())
    }

    pub(crate) fn jump_peek_or_pop<F>(&mut self, target: usize, condition: F) -> Result<(), VmError>
    where
        F: Fn(&Val) -> bool,
    {
        let handle = self.peek_operand()?;
        let val = &self.arena.get(handle).value;

        if condition(val) {
            self.set_ip(target)?;
        } else {
            self.operand_stack.pop();
        }
        Ok(())
    }

    fn exec_control_flow(&mut self, op: OpCode) -> Result<(), VmError> {
        match op {
            OpCode::Jmp(target) => self.set_ip(target as usize)?,
            OpCode::JmpIfFalse(target) => self.jump_if(target as usize, |v| !v.to_bool())?,
            OpCode::JmpIfTrue(target) => self.jump_if(target as usize, |v| v.to_bool())?,
            OpCode::JmpZEx(target) => self.jump_peek_or_pop(target as usize, |v| !v.to_bool())?,
            OpCode::JmpNzEx(target) => self.jump_peek_or_pop(target as usize, |v| v.to_bool())?,
            OpCode::Coalesce(target) => {
                self.jump_peek_or_pop(target as usize, |v| !matches!(v, Val::Null))?
            }
            OpCode::JmpFinally(target) => {
                // Execute finally blocks before jumping (for break/continue)
                let finally_blocks = self.collect_finally_blocks_for_jump();
                if !finally_blocks.is_empty() {
                    self.execute_finally_blocks(&finally_blocks);
                }
                self.set_ip(target as usize)?;
            }
            _ => unreachable!("Not a control flow op"),
        }
        Ok(())
    }

    fn exec_throw(&mut self) -> Result<(), VmError> {
        let ex_handle = self
            .operand_stack
            .pop()
            .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

        // Validate that the thrown value is an object
        let (is_object, payload_handle_opt) = {
            let ex_val = &self.arena.get(ex_handle).value;
            match ex_val {
                Val::Object(ph) => (true, Some(*ph)),
                _ => (false, None),
            }
        };

        if !is_object {
            return Err(VmError::RuntimeError("Can only throw objects".into()));
        }

        let payload_handle = payload_handle_opt.unwrap();

        // Validate that the object implements Throwable interface
        let throwable_sym = self.context.interner.intern(b"Throwable");
        if !self.is_instance_of(ex_handle, throwable_sym) {
            // Get the class name for error message
            let class_name =
                if let Val::ObjPayload(obj_data) = &self.arena.get(payload_handle).value {
                    String::from_utf8_lossy(
                        self.context
                            .interner
                            .lookup(obj_data.class)
                            .unwrap_or(b"Object"),
                    )
                    .to_string()
                } else {
                    "Object".to_string()
                };

            return Err(VmError::RuntimeError(format!(
                "Cannot throw objects that do not implement Throwable ({})",
                class_name
            )));
        }

        // Set exception properties (file, line, trace) at throw time
        // This mimics PHP's behavior of capturing context when exception is thrown
        let file_sym = self.context.interner.intern(b"file");
        let line_sym = self.context.interner.intern(b"line");

        // Get current file and line from frame
        let (file_path, line_no) = if let Some(frame) = self.frames.last() {
            let file = frame
                .chunk
                .file_path
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            let line = if frame.ip > 0 && frame.ip <= frame.chunk.lines.len() {
                frame.chunk.lines[frame.ip - 1]
            } else {
                0
            };
            (file, line)
        } else {
            ("unknown".to_string(), 0)
        };

        // Allocate property values first
        let file_val = self.arena.alloc(Val::String(file_path.into_bytes().into()));
        let line_val = self.arena.alloc(Val::Int(line_no as i64));

        // Now mutate the object to set file and line
        let payload = self.arena.get_mut(payload_handle);
        if let Val::ObjPayload(ref mut obj_data) = payload.value {
            obj_data.properties.insert(file_sym, file_val);
            obj_data.properties.insert(line_sym, line_val);
        }

        Err(VmError::Exception(ex_handle))
    }

    fn exec_load_var(&mut self, sym: Symbol) -> Result<(), VmError> {
        // Special handling for $GLOBALS
        if self.is_globals_symbol(sym) {
            if let Some(handle) = self.ensure_superglobal_handle(sym) {
                self.var_handle_map.insert(handle, sym);
                self.operand_stack.push(handle);
            }
            return Ok(());
        }

        // Try to get from locals
        if let Some(handle) = self.current_frame()?.locals.get(&sym).copied() {
            self.var_handle_map.insert(handle, sym);
            self.operand_stack.push(handle);
            return Ok(());
        }

        let name = self.context.interner.lookup(sym);
        if let Some(name_bytes) = name {
            if name_bytes == b"this" {
                let frame = self.current_frame()?;
                if let Some(this_val) = frame.this {
                    self.var_handle_map.insert(this_val, sym);
                    self.operand_stack.push(this_val);
                    return Ok(());
                }
                return Err(VmError::RuntimeError(
                    "Using $this when not in object context".into(),
                ));
            }
        }

        if self.is_superglobal(sym) {
            if let Some(handle) = self.ensure_superglobal_handle(sym) {
                self.current_frame_mut()?
                    .locals
                    .entry(sym)
                    .or_insert(handle);
                self.var_handle_map.insert(handle, sym);
                self.operand_stack.push(handle);
            } else {
                let null = self.arena.alloc(Val::Null);
                self.var_handle_map.insert(null, sym);
                self.operand_stack.push(null);
            }
            return Ok(());
        }

        // Undefined variable
        let null = self.arena.alloc(Val::Null);
        self.var_handle_map.insert(null, sym);
        self.pending_undefined.insert(null, sym);
        self.operand_stack.push(null);
        Ok(())
    }

    /// Direct opcode execution (for internal use and trait delegation)
    /// This is the actual implementation method that can be called directly
    pub(crate) fn execute_opcode_direct(
        &mut self,
        op: OpCode,
        target_depth: usize,
    ) -> Result<(), VmError> {
        self.execute_opcode(op, target_depth)
    }

    fn execute_opcode(&mut self, op: OpCode, target_depth: usize) -> Result<(), VmError> {
        match op {
            OpCode::Throw => self.exec_throw()?,
            OpCode::Catch => {
                // Exception object is already on the operand stack (pushed by handler); nothing else to do.
            }
            OpCode::Const(_) | OpCode::Pop | OpCode::Dup | OpCode::Nop => self.exec_stack_op(op)?,

            // Arithmetic operations - delegated to opcodes::arithmetic
            OpCode::Add => self.exec_add()?,
            OpCode::Sub => self.exec_sub()?,
            OpCode::Mul => self.exec_mul()?,
            OpCode::Div => self.exec_div()?,
            OpCode::Mod => self.exec_mod()?,
            OpCode::Pow => self.exec_pow()?,
            OpCode::BitwiseAnd => self.exec_bitwise_and()?,
            OpCode::BitwiseOr => self.exec_bitwise_or()?,
            OpCode::BitwiseXor => self.exec_bitwise_xor()?,
            OpCode::ShiftLeft => self.exec_shift_left()?,
            OpCode::ShiftRight => self.exec_shift_right()?,
            OpCode::BitwiseNot => self.exec_bitwise_not()?,
            OpCode::BoolNot => self.exec_bool_not()?,

            OpCode::LoadVar(sym) => self.exec_load_var(sym)?,
            OpCode::LoadVarDynamic => {
                let name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let name_bytes = self.convert_to_string(name_handle)?;
                let sym = self.context.interner.intern(&name_bytes);

                let existing = self
                    .frames
                    .last()
                    .and_then(|frame| frame.locals.get(&sym).copied());

                if let Some(handle) = existing {
                    self.var_handle_map.insert(handle, sym);
                    self.operand_stack.push(handle);
                } else if self.is_superglobal(sym) {
                    if let Some(handle) = self.ensure_superglobal_handle(sym) {
                        if let Some(frame) = self.frames.last_mut() {
                            frame.locals.entry(sym).or_insert(handle);
                        }
                        self.var_handle_map.insert(handle, sym);
                        self.operand_stack.push(handle);
                    } else {
                        let null = self.arena.alloc(Val::Null);
                        self.var_handle_map.insert(null, sym);
                        self.operand_stack.push(null);
                    }
                } else {
                    let null = self.arena.alloc(Val::Null);
                    self.var_handle_map.insert(null, sym);
                    self.pending_undefined.insert(null, sym);
                    self.operand_stack.push(null);
                }
            }
            OpCode::LoadRef(sym) => {
                let to_bind = if self.is_superglobal(sym) {
                    self.ensure_superglobal_handle(sym)
                } else {
                    None
                };

                if let Some(handle) = to_bind {
                    if let Some(frame) = self.frames.last_mut() {
                        frame.locals.entry(sym).or_insert(handle);
                    }
                }

                let frame = self.frames.last_mut().unwrap();
                if let Some(&handle) = frame.locals.get(&sym) {
                    if self.arena.get(handle).is_ref {
                        self.operand_stack.push(handle);
                    } else {
                        // Convert to ref. Clone to ensure uniqueness/safety.
                        let val = self.arena.get(handle).value.clone();
                        let new_handle = self.arena.alloc(val);
                        self.arena.get_mut(new_handle).is_ref = true;
                        frame.locals.insert(sym, new_handle);
                        self.operand_stack.push(new_handle);
                    }
                } else {
                    // Undefined variable, create as Null ref
                    let handle = self.arena.alloc(Val::Null);
                    self.arena.get_mut(handle).is_ref = true;
                    frame.locals.insert(sym, handle);
                    self.operand_stack.push(handle);
                }
            }
            OpCode::StoreVar(sym) => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                // PHP 8.1+: Disallow writing to entire $GLOBALS array
                // Exception: if we're "storing back" the same handle (e.g., after array modification),
                // that's fine - it's a no-op
                if self.is_globals_symbol(sym) {
                    let existing_handle = self
                        .frames
                        .last()
                        .and_then(|f| f.locals.get(&sym).copied())
                        .or_else(|| self.context.globals.get(&sym).copied());

                    if existing_handle == Some(val_handle) {
                        // Same handle - no-op, skip the rest
                    } else {
                        return Err(VmError::RuntimeError(
                            "$GLOBALS can only be modified using the $GLOBALS[$name] = $value syntax".into()
                        ));
                    }
                } else {
                    // Normal variable assignment
                    let to_bind = if self.is_superglobal(sym) {
                        self.ensure_superglobal_handle(sym)
                    } else {
                        None
                    };

                    // Check if we're at top-level (before borrowing frame)
                    let is_top_level = self.frames.len() == 1;

                    let mut ref_handle: Option<Handle> = None;
                    {
                        let frame = self.frames.last_mut().unwrap();

                        if let Some(handle) = to_bind {
                            frame.locals.entry(sym).or_insert(handle);
                        }

                        if let Some(&old_handle) = frame.locals.get(&sym) {
                            if self.arena.get(old_handle).is_ref {
                                let new_val = self.arena.get(val_handle).value.clone();
                                self.arena.get_mut(old_handle).value = new_val;
                                ref_handle = Some(old_handle);
                            }
                        }
                    }

                    let final_handle = if let Some(existing) = ref_handle {
                        existing
                    } else {
                        let val = self.clone_value_for_assignment(sym, val_handle);
                        let new_handle = self.arena.alloc(val);
                        self.frames
                            .last_mut()
                            .unwrap()
                            .locals
                            .insert(sym, new_handle);
                        new_handle
                    };

                    // If we're at the top-level (frame depth == 1), also store in globals
                    // This ensures $GLOBALS can access these variables
                    if is_top_level {
                        self.context.globals.insert(sym, final_handle);
                    }
                }
            }
            OpCode::StoreVarDynamic => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let name_bytes = self.convert_to_string(name_handle)?;
                let sym = self.context.interner.intern(&name_bytes);

                let to_bind = if self.is_superglobal(sym) {
                    self.ensure_superglobal_handle(sym)
                } else {
                    None
                };

                let mut ref_handle: Option<Handle> = None;
                {
                    let frame = self.frames.last_mut().unwrap();

                    if let Some(handle) = to_bind {
                        frame.locals.entry(sym).or_insert(handle);
                    }

                    if let Some(&old_handle) = frame.locals.get(&sym) {
                        if self.arena.get(old_handle).is_ref {
                            let new_val = self.arena.get(val_handle).value.clone();
                            self.arena.get_mut(old_handle).value = new_val;
                            ref_handle = Some(old_handle);
                        }
                    }
                }

                let result_handle = if let Some(existing) = ref_handle {
                    existing
                } else {
                    let val = self.clone_value_for_assignment(sym, val_handle);
                    let final_handle = self.arena.alloc(val);
                    self.frames
                        .last_mut()
                        .unwrap()
                        .locals
                        .insert(sym, final_handle);
                    final_handle
                };

                self.operand_stack.push(result_handle);
            }
            OpCode::AssignRef(sym) => {
                let ref_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                // Mark the handle as a reference (idempotent if already ref)
                self.arena.get_mut(ref_handle).is_ref = true;

                let frame = self.frames.last_mut().unwrap();
                // Overwrite the local slot with the reference handle
                frame.locals.insert(sym, ref_handle);
                if self.is_superglobal(sym) {
                    self.context.globals.insert(sym, ref_handle);
                }
            }
            OpCode::AssignOp(op) => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let var_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                if self.arena.get(var_handle).is_ref {
                    let current_val = self.arena.get(var_handle).value.clone();
                    let val = self.arena.get(val_handle).value.clone();

                    use crate::vm::assign_op::AssignOpType;
                    let op_type = AssignOpType::from_u8(op).ok_or_else(|| {
                        VmError::RuntimeError(format!("Invalid assign op: {}", op))
                    })?;

                    let res = op_type.apply(current_val, val)?;

                    self.arena.get_mut(var_handle).value = res.clone();
                    let res_handle = self.arena.alloc(res);
                    self.operand_stack.push(res_handle);
                } else {
                    return Err(VmError::RuntimeError("AssignOp on non-reference".into()));
                }
            }
            OpCode::PreInc => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                if self.arena.get(handle).is_ref {
                    let val = &self.arena.get(handle).value;
                    let new_val = match val {
                        Val::Int(i) => Val::Int(i + 1),
                        _ => Val::Null,
                    };
                    self.arena.get_mut(handle).value = new_val.clone();
                    let res_handle = self.arena.alloc(new_val);
                    self.operand_stack.push(res_handle);
                } else {
                    return Err(VmError::RuntimeError("PreInc on non-reference".into()));
                }
            }
            OpCode::PreDec => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                if self.arena.get(handle).is_ref {
                    let val = &self.arena.get(handle).value;
                    let new_val = match val {
                        Val::Int(i) => Val::Int(i - 1),
                        _ => Val::Null,
                    };
                    self.arena.get_mut(handle).value = new_val.clone();
                    let res_handle = self.arena.alloc(new_val);
                    self.operand_stack.push(res_handle);
                } else {
                    return Err(VmError::RuntimeError("PreDec on non-reference".into()));
                }
            }
            OpCode::PostInc => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                if self.arena.get(handle).is_ref {
                    let val = self.arena.get(handle).value.clone();
                    let new_val = match &val {
                        Val::Int(i) => Val::Int(i + 1),
                        _ => Val::Null,
                    };
                    self.arena.get_mut(handle).value = new_val;
                    let res_handle = self.arena.alloc(val); // Return OLD value
                    self.operand_stack.push(res_handle);
                } else {
                    return Err(VmError::RuntimeError("PostInc on non-reference".into()));
                }
            }
            OpCode::PostDec => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                if self.arena.get(handle).is_ref {
                    let val = self.arena.get(handle).value.clone();
                    let new_val = match &val {
                        Val::Int(i) => Val::Int(i - 1),
                        _ => Val::Null,
                    };
                    self.arena.get_mut(handle).value = new_val;
                    let res_handle = self.arena.alloc(val); // Return OLD value
                    self.operand_stack.push(res_handle);
                } else {
                    return Err(VmError::RuntimeError("PostDec on non-reference".into()));
                }
            }
            OpCode::MakeVarRef(sym) => {
                let frame = self.frames.last_mut().unwrap();

                // Get current handle or create NULL
                let handle = if let Some(&h) = frame.locals.get(&sym) {
                    h
                } else {
                    let null = self.arena.alloc(Val::Null);
                    frame.locals.insert(sym, null);
                    null
                };

                // Check if it is already a ref
                if self.arena.get(handle).is_ref {
                    self.operand_stack.push(handle);
                } else {
                    // Not a ref. We must upgrade it.
                    // To avoid affecting other variables sharing this handle, we MUST clone.
                    let val = self.arena.get(handle).value.clone();
                    let new_handle = self.arena.alloc(val);
                    self.arena.get_mut(new_handle).is_ref = true;

                    // Update the local variable to point to the new ref handle
                    let frame = self.frames.last_mut().unwrap();
                    frame.locals.insert(sym, new_handle);

                    self.operand_stack.push(new_handle);
                }
            }
            OpCode::UnsetVar(sym) => {
                // PHP 8.1+: Cannot unset $GLOBALS itself
                if self.is_globals_symbol(sym) {
                    return Err(VmError::RuntimeError(
                        "Cannot unset $GLOBALS variable".into(),
                    ));
                }

                if !self.is_superglobal(sym) {
                    let frame = self.frames.last_mut().unwrap();
                    frame.locals.remove(&sym);
                }
            }
            OpCode::UnsetVarDynamic => {
                let name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let name_bytes = self.convert_to_string(name_handle)?;
                let sym = self.context.interner.intern(&name_bytes);

                // PHP 8.1+: Cannot unset $GLOBALS itself
                if self.is_globals_symbol(sym) {
                    return Err(VmError::RuntimeError(
                        "Cannot unset $GLOBALS variable".into(),
                    ));
                }

                if !self.is_superglobal(sym) {
                    let frame = self.frames.last_mut().unwrap();
                    frame.locals.remove(&sym);
                }
            }
            OpCode::BindGlobal(sym) => {
                let global_handle = self.context.globals.get(&sym).copied();

                let handle = if let Some(h) = global_handle {
                    h
                } else {
                    // Check main frame (frame 0) for the variable
                    let main_handle = if !self.frames.is_empty() {
                        self.frames[0].locals.get(&sym).copied()
                    } else {
                        None
                    };

                    if let Some(h) = main_handle {
                        h
                    } else {
                        self.arena.alloc(Val::Null)
                    }
                };

                // Ensure it is in globals map
                self.context.globals.insert(sym, handle);

                // Mark as reference
                self.arena.get_mut(handle).is_ref = true;

                let frame = self.frames.last_mut().unwrap();
                frame.locals.insert(sym, handle);
            }
            OpCode::BindStatic(sym, default_idx) => {
                let frame = self.frames.last_mut().unwrap();

                if let Some(func) = &frame.func {
                    let mut statics = func.statics.borrow_mut();

                    let handle = if let Some(h) = statics.get(&sym) {
                        *h
                    } else {
                        // Initialize with default value
                        let val = frame.chunk.constants[default_idx as usize].clone();
                        let h = self.arena.alloc(val);
                        statics.insert(sym, h);
                        h
                    };

                    // Mark as reference so StoreVar updates it in place
                    self.arena.get_mut(handle).is_ref = true;

                    // Bind to local
                    frame.locals.insert(sym, handle);
                } else {
                    return Err(VmError::RuntimeError(
                        "BindStatic called outside of function".into(),
                    ));
                }
            }
            OpCode::MakeRef => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                // Mark the handle as a reference in-place
                // This is critical for $GLOBALS reference behavior: when you do
                // $ref = &$GLOBALS['x'], both $ref and the global $x must point to
                // the SAME handle. Cloning would break this sharing.
                self.arena.get_mut(handle).is_ref = true;
                self.operand_stack.push(handle);
            }

            OpCode::Jmp(_)
            | OpCode::JmpIfFalse(_)
            | OpCode::JmpIfTrue(_)
            | OpCode::JmpZEx(_)
            | OpCode::JmpNzEx(_)
            | OpCode::Coalesce(_)
            | OpCode::JmpFinally(_) => self.exec_control_flow(op)?,

            OpCode::Echo => self.exec_echo()?,
            OpCode::Exit => {
                if let Some(handle) = self.operand_stack.pop() {
                    let s = self.convert_to_string(handle)?;
                    self.write_output(&s)?;
                }
                self.output_writer.flush()?;
                self.frames.clear();
                return Ok(());
            }
            OpCode::Silence(flag) => {
                if flag {
                    let current_level = self.context.error_reporting;
                    self.silence_stack.push(current_level);
                    self.context.error_reporting = 0;
                } else if let Some(level) = self.silence_stack.pop() {
                    self.context.error_reporting = level;
                }
            }
            OpCode::BeginSilence => {
                let current_level = self.context.error_reporting;
                self.silence_stack.push(current_level);
                self.context.error_reporting = 0;
            }
            OpCode::EndSilence => {
                if let Some(level) = self.silence_stack.pop() {
                    self.context.error_reporting = level;
                }
            }
            OpCode::Ticks(_) => {
                // Tick handler not yet implemented; treat as no-op.
            }
            OpCode::Cast(kind) => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                if kind == 3 {
                    let s = self.convert_to_string(handle)?;
                    let res_handle = self.arena.alloc(Val::String(s.into()));
                    self.operand_stack.push(res_handle);
                    return Ok(());
                }

                let val = self.arena.get(handle).value.clone();

                let new_val = match kind {
                    0 => match val {
                        // Int
                        Val::Int(i) => Val::Int(i),
                        Val::Float(f) => Val::Int(f as i64),
                        Val::Bool(b) => Val::Int(if b { 1 } else { 0 }),
                        Val::String(s) => {
                            let s = String::from_utf8_lossy(&s);
                            Val::Int(s.parse().unwrap_or(0))
                        }
                        Val::Null => Val::Int(0),
                        _ => Val::Int(0),
                    },
                    1 => Val::Bool(val.to_bool()), // Bool
                    2 => match val {
                        // Float
                        Val::Float(f) => Val::Float(f),
                        Val::Int(i) => Val::Float(i as f64),
                        Val::String(s) => {
                            let s = String::from_utf8_lossy(&s);
                            Val::Float(s.parse().unwrap_or(0.0))
                        }
                        _ => Val::Float(0.0),
                    },
                    3 => match val {
                        // String
                        Val::String(s) => Val::String(s),
                        Val::Int(i) => Val::String(i.to_string().into_bytes().into()),
                        Val::Float(f) => Val::String(f.to_string().into_bytes().into()),
                        Val::Bool(b) => Val::String(if b {
                            b"1".to_vec().into()
                        } else {
                            b"".to_vec().into()
                        }),
                        Val::Null => Val::String(Vec::new().into()),
                        Val::Object(_) => unreachable!(), // Handled above
                        _ => Val::String(b"Array".to_vec().into()),
                    },
                    4 => match val {
                        // Array
                        Val::Array(a) => Val::Array(a),
                        Val::Null => Val::Array(ArrayData::new().into()),
                        _ => {
                            let mut map = IndexMap::new();
                            map.insert(ArrayKey::Int(0), self.arena.alloc(val));
                            Val::Array(ArrayData::from(map).into())
                        }
                    },
                    5 => match val {
                        // Object
                        Val::Object(h) => Val::Object(h),
                        Val::Array(a) => {
                            let mut props = IndexMap::new();
                            for (k, v) in a.map.iter() {
                                let key_sym = match k {
                                    ArrayKey::Int(i) => {
                                        self.context.interner.intern(i.to_string().as_bytes())
                                    }
                                    ArrayKey::Str(s) => self.context.interner.intern(&s),
                                };
                                props.insert(key_sym, *v);
                            }
                            let obj_data = ObjectData {
                                class: self.context.interner.intern(b"stdClass"),
                                properties: props,
                                internal: None,
                                dynamic_properties: std::collections::HashSet::new(),
                            };
                            let payload = self.arena.alloc(Val::ObjPayload(obj_data));
                            Val::Object(payload)
                        }
                        Val::Null => {
                            let obj_data = ObjectData {
                                class: self.context.interner.intern(b"stdClass"),
                                properties: IndexMap::new(),
                                internal: None,
                                dynamic_properties: std::collections::HashSet::new(),
                            };
                            let payload = self.arena.alloc(Val::ObjPayload(obj_data));
                            Val::Object(payload)
                        }
                        _ => {
                            let mut props = IndexMap::new();
                            let key_sym = self.context.interner.intern(b"scalar");
                            props.insert(key_sym, self.arena.alloc(val));
                            let obj_data = ObjectData {
                                class: self.context.interner.intern(b"stdClass"),
                                properties: props,
                                internal: None,
                                dynamic_properties: std::collections::HashSet::new(),
                            };
                            let payload = self.arena.alloc(Val::ObjPayload(obj_data));
                            Val::Object(payload)
                        }
                    },
                    6 => Val::Null, // Unset
                    _ => val,
                };
                let res_handle = self.arena.alloc(new_val);
                self.operand_stack.push(res_handle);
            }
            OpCode::TypeCheck => {}
            OpCode::CallableConvert => {
                // Minimal callable validation: ensure value is a string or a 2-element array [class/object, method].
                let handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val = &self.arena.get(handle).value;
                match val {
                    Val::String(_) => {}
                    Val::Array(map) => {
                        if map.map.len() != 2 {
                            return Err(VmError::RuntimeError(
                                "Callable expects array(class, method)".into(),
                            ));
                        }
                    }
                    _ => return Err(VmError::RuntimeError("Value is not callable".into())),
                }
            }
            OpCode::DeclareClass => {
                let parent_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let name_sym = match &self.arena.get(name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                };

                let parent_sym = match &self.arena.get(parent_handle).value {
                    Val::String(s) => Some(self.context.interner.intern(s)),
                    Val::Null => None,
                    _ => {
                        return Err(VmError::RuntimeError(
                            "Parent class name must be string or null".into(),
                        ))
                    }
                };

                let mut methods = HashMap::new();

                if let Some(parent) = parent_sym {
                    if let Some(parent_def) = self.context.classes.get(&parent) {
                        // Inherit methods, excluding private ones.
                        for (key, entry) in &parent_def.methods {
                            if entry.visibility != Visibility::Private {
                                methods.insert(*key, entry.clone());
                            }
                        }
                    } else {
                        let parent_name = self
                            .context
                            .interner
                            .lookup(parent)
                            .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
                            .unwrap_or_else(|| format!("{:?}", parent));
                        return Err(VmError::RuntimeError(format!(
                            "Parent class {} not found",
                            parent_name
                        )));
                    }
                }

                let class_def = ClassDef {
                    name: name_sym,
                    parent: parent_sym,
                    is_interface: false,
                    is_trait: false,
                    is_abstract: false,
                    is_enum: false,
                    enum_backed_type: None,
                    interfaces: Vec::new(),
                    traits: Vec::new(),
                    methods,
                    properties: IndexMap::new(),
                    constants: HashMap::new(),
                    static_properties: HashMap::new(),
                    abstract_methods: HashSet::new(),
                    allows_dynamic_properties: false,
                };
                self.context.classes.insert(name_sym, class_def);
            }
            OpCode::DeclareFunction => {
                let func_idx_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let name_sym = match &self.arena.get(name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Function name must be string".into())),
                };

                let func_idx = match &self.arena.get(func_idx_handle).value {
                    Val::Int(i) => *i as u32,
                    _ => return Err(VmError::RuntimeError("Function index must be int".into())),
                };

                let val = {
                    let frame = self.frames.last().unwrap();
                    frame.chunk.constants[func_idx as usize].clone()
                };
                if let Val::Resource(rc) = val {
                    if let Ok(func) = rc.downcast::<UserFunc>() {
                        self.context.user_functions.insert(name_sym, func);
                    }
                }
            }
            OpCode::DeclareConst => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let name_sym = match &self.arena.get(name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Constant name must be string".into())),
                };

                let val = self.arena.get(val_handle).value.clone();
                self.context.constants.insert(name_sym, val);
            }
            OpCode::CaseStrict => {
                let case_val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let switch_val_handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?; // Peek

                let case_val = &self.arena.get(case_val_handle).value;
                let switch_val = &self.arena.get(switch_val_handle).value;

                // Strict comparison
                let is_equal = match (switch_val, case_val) {
                    (Val::Int(a), Val::Int(b)) => a == b,
                    (Val::String(a), Val::String(b)) => a == b,
                    (Val::Bool(a), Val::Bool(b)) => a == b,
                    (Val::Float(a), Val::Float(b)) => a == b,
                    (Val::Null, Val::Null) => true,
                    _ => false,
                };

                let res_handle = self.arena.alloc(Val::Bool(is_equal));
                self.operand_stack.push(res_handle);
            }
            OpCode::SwitchLong | OpCode::SwitchString => {
                // No-op
            }
            OpCode::Match => {
                // Match condition is expected on stack top; leave it for following comparisons.
            }
            OpCode::MatchError => {
                return Err(VmError::RuntimeError("UnhandledMatchError".into()));
            }

            OpCode::HandleException => {
                // Exception handling is coordinated via Catch tables and VmError::Exception;
                // this opcode acts as a marker in Zend but is a no-op here.
            }
            OpCode::JmpSet => {
                // Placeholder: would jump based on isset/empty in Zend. No-op for now.
            }
            OpCode::AssertCheck => {
                // Assertions not implemented; treat as no-op.
            }

            OpCode::Closure(func_idx, num_captures) => {
                let val = {
                    let frame = self.frames.last().unwrap();
                    frame.chunk.constants[func_idx as usize].clone()
                };

                let user_func = if let Val::Resource(rc) = val {
                    if let Ok(func) = rc.downcast::<UserFunc>() {
                        func
                    } else {
                        return Err(VmError::RuntimeError(
                            "Invalid function constant for closure".into(),
                        ));
                    }
                } else {
                    return Err(VmError::RuntimeError(
                        "Invalid function constant for closure".into(),
                    ));
                };

                let mut captures = IndexMap::new();
                let mut captured_vals = Vec::with_capacity(num_captures as usize);
                for _ in 0..num_captures {
                    captured_vals.push(
                        self.operand_stack
                            .pop()
                            .ok_or(VmError::RuntimeError("Stack underflow".into()))?,
                    );
                }
                captured_vals.reverse();

                for (i, sym) in user_func.uses.iter().enumerate() {
                    if i < captured_vals.len() {
                        captures.insert(*sym, captured_vals[i]);
                    }
                }

                let this_handle = if user_func.is_static {
                    None
                } else {
                    let frame = self.frames.last().unwrap();
                    frame.this
                };

                let closure_data = ClosureData {
                    func: user_func,
                    captures,
                    this: this_handle,
                };

                let closure_class_sym = self.context.interner.intern(b"Closure");
                let obj_data = ObjectData {
                    class: closure_class_sym,
                    properties: IndexMap::new(),
                    internal: Some(Rc::new(closure_data)),
                    dynamic_properties: std::collections::HashSet::new(),
                };

                let payload_handle = self.arena.alloc(Val::ObjPayload(obj_data));
                let obj_handle = self.arena.alloc(Val::Object(payload_handle));
                self.operand_stack.push(obj_handle);
            }

            OpCode::Call(arg_count) => {
                // Increment function call counter for profiling
                self.function_calls += 1;

                let callsite_strict_types = self
                    .frames
                    .last()
                    .map(|frame| frame.chunk.strict_types)
                    .unwrap_or(false);

                let args = self.collect_call_args(arg_count)?;

                let func_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                self.invoke_callable_value(func_handle, args, callsite_strict_types)?;
            }

            OpCode::Return => self.handle_return(false, target_depth)?,
            OpCode::ReturnByRef => self.handle_return(true, target_depth)?,
            OpCode::VerifyReturnType => {
                // Return type verification is now handled in handle_return
                // This opcode is a no-op
            }
            OpCode::VerifyNeverType => {
                return Err(VmError::RuntimeError(
                    "Never-returning function must not return".into(),
                ));
            }
            OpCode::Recv(arg_idx) => {
                let (func_clone, callsite_strict, func_name_str) = {
                    let frame = self.frames.last().unwrap();
                    let name_str = String::from_utf8_lossy(
                        self.context.interner.lookup(frame.chunk.name).unwrap_or(b"?"),
                    ).to_string();
                    (
                        frame.func.clone(),
                        frame.callsite_strict_types,
                        name_str,
                    )
                };

                if let Some(func) = func_clone {
                    if (arg_idx as usize) < func.params.len() {
                        let param = func.params[arg_idx as usize].clone();
                        
                        // Get arg_handle first
                        let has_arg = {
                            let frame = self.frames.last().unwrap();
                            (arg_idx as usize) < frame.args.len()
                        };

                        if has_arg {
                            let arg_handle = self.frames.last().unwrap().args[arg_idx as usize];
                            
                            // Type check and coerce if needed (this may mutate self)
                            let checked_handle = if let Some(ref param_type) = param.param_type {
                                self.check_parameter_type(
                                    arg_handle,
                                    param_type,
                                    callsite_strict,
                                    param.name,
                                    &func_name_str,
                                )?
                            } else {
                                arg_handle
                            };

                            // Now insert into frame locals
                            let frame = self.frames.last_mut().unwrap();
                            if param.by_ref {
                                if !self.arena.get(checked_handle).is_ref {
                                    self.arena.get_mut(checked_handle).is_ref = true;
                                }
                                frame.locals.insert(param.name, checked_handle);
                            } else {
                                let val = self.arena.get(checked_handle).value.clone();
                                let final_handle = self.arena.alloc(val);
                                frame.locals.insert(param.name, final_handle);
                            }
                        }
                    }
                }
            }
            OpCode::RecvInit(arg_idx, default_val_idx) => {
                let (func_clone, callsite_strict, func_name_str) = {
                    let frame = self.frames.last().unwrap();
                    let name_str = String::from_utf8_lossy(
                        self.context.interner.lookup(frame.chunk.name).unwrap_or(b"?"),
                    ).to_string();
                    (
                        frame.func.clone(),
                        frame.callsite_strict_types,
                        name_str,
                    )
                };

                if let Some(func) = func_clone {
                    if (arg_idx as usize) < func.params.len() {
                        let param = func.params[arg_idx as usize].clone();
                        
                        // Check if arg was supplied
                        let has_arg = {
                            let frame = self.frames.last().unwrap();
                            (arg_idx as usize) < frame.args.len()
                        };

                        if has_arg {
                            let arg_handle = self.frames.last().unwrap().args[arg_idx as usize];
                            
                            // Type check and coerce if needed
                            let checked_handle = if let Some(ref param_type) = param.param_type {
                                self.check_parameter_type(
                                    arg_handle,
                                    param_type,
                                    callsite_strict,
                                    param.name,
                                    &func_name_str,
                                )?
                            } else {
                                arg_handle
                            };

                            // Insert into frame locals
                            let frame = self.frames.last_mut().unwrap();
                            if param.by_ref {
                                if !self.arena.get(checked_handle).is_ref {
                                    self.arena.get_mut(checked_handle).is_ref = true;
                                }
                                frame.locals.insert(param.name, checked_handle);
                            } else {
                                let val = self.arena.get(checked_handle).value.clone();
                                let final_handle = self.arena.alloc(val);
                                frame.locals.insert(param.name, final_handle);
                            }
                        } else {
                            // Use default value
                            let frame = self.frames.last_mut().unwrap();
                            let default_val =
                                frame.chunk.constants[default_val_idx as usize].clone();
                            let default_handle = self.arena.alloc(default_val);
                            frame.locals.insert(param.name, default_handle);
                        }
                    }
                }
            }
            OpCode::RecvVariadic(arg_idx) => {
                let (func_clone, callsite_strict, func_name_str, args_to_check) = {
                    let frame = self.frames.last().unwrap();
                    let name_str = String::from_utf8_lossy(
                        self.context.interner.lookup(frame.chunk.name).unwrap_or(b"?"),
                    ).to_string();
                    let args: Vec<Handle> = if frame.args.len() > arg_idx as usize {
                        frame.args[arg_idx as usize..].to_vec()
                    } else {
                        vec![]
                    };
                    (
                        frame.func.clone(),
                        frame.callsite_strict_types,
                        name_str,
                        args,
                    )
                };

                if let Some(func) = func_clone {
                    if (arg_idx as usize) < func.params.len() {
                        let param = func.params[arg_idx as usize].clone();
                        let mut arr = IndexMap::new();
                        
                        for (i, handle) in args_to_check.iter().enumerate() {
                            let mut arg_handle = *handle;

                            // Type check each variadic argument
                            if let Some(ref param_type) = param.param_type {
                                arg_handle = self.check_parameter_type(
                                    arg_handle,
                                    param_type,
                                    callsite_strict,
                                    param.name,
                                    &func_name_str,
                                )?;
                            }

                            if param.by_ref {
                                if !self.arena.get(arg_handle).is_ref {
                                    self.arena.get_mut(arg_handle).is_ref = true;
                                }
                                arr.insert(ArrayKey::Int(i as i64), arg_handle);
                            } else {
                                let val = self.arena.get(arg_handle).value.clone();
                                let h = self.arena.alloc(val);
                                arr.insert(ArrayKey::Int(i as i64), h);
                            }
                        }
                        let arr_handle = self.arena.alloc(Val::Array(ArrayData::from(arr).into()));
                        let frame = self.frames.last_mut().unwrap();
                        frame.locals.insert(param.name, arr_handle);
                    }
                }
            }
            OpCode::SendVal => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let call = self
                    .pending_calls
                    .last_mut()
                    .ok_or(VmError::RuntimeError("No pending call".into()))?;
                let cloned = {
                    let val = self.arena.get(val_handle).value.clone();
                    self.arena.alloc(val)
                };
                call.args.push(cloned);
            }
            OpCode::SendVar => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let call = self
                    .pending_calls
                    .last_mut()
                    .ok_or(VmError::RuntimeError("No pending call".into()))?;
                call.args.push(val_handle);
            }
            OpCode::SendRef => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                if !self.arena.get(val_handle).is_ref {
                    self.arena.get_mut(val_handle).is_ref = true;
                }
                let call = self
                    .pending_calls
                    .last_mut()
                    .ok_or(VmError::RuntimeError("No pending call".into()))?;
                call.args.push(val_handle);
            }
            OpCode::Yield(has_key) => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let key_handle = if has_key {
                    Some(
                        self.operand_stack
                            .pop()
                            .ok_or(VmError::RuntimeError("Stack underflow".into()))?,
                    )
                } else {
                    None
                };

                let frame = self
                    .frames
                    .pop()
                    .ok_or(VmError::RuntimeError("No frame to yield from".into()))?;
                let gen_handle = frame.generator.ok_or(VmError::RuntimeError(
                    "Yield outside of generator context".into(),
                ))?;

                let gen_val = self.arena.get(gen_handle);
                if let Val::Object(payload_handle) = &gen_val.value {
                    let payload = self.arena.get(*payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload.value {
                        if let Some(internal) = &obj_data.internal {
                            if let Ok(gen_data) =
                                internal.clone().downcast::<RefCell<GeneratorData>>()
                            {
                                let mut data = gen_data.borrow_mut();
                                data.current_val = Some(val_handle);

                                if let Some(k) = key_handle {
                                    data.current_key = Some(k);
                                    if let Val::Int(i) = self.arena.get(k).value {
                                        data.auto_key = i + 1;
                                    }
                                } else {
                                    let k = data.auto_key;
                                    data.auto_key += 1;
                                    let k_handle = self.arena.alloc(Val::Int(k));
                                    data.current_key = Some(k_handle);
                                }

                                data.state = GeneratorState::Suspended(frame);
                            }
                        }
                    }
                }

                // Yield pauses execution of this frame. The value is stored in GeneratorData.
                // We don't push anything to the stack here. The sent value will be retrieved
                // by OpCode::GetSentValue when the generator is resumed.
            }
            OpCode::YieldFrom => {
                let frame_idx = self.frames.len() - 1;
                let frame = &mut self.frames[frame_idx];
                let gen_handle = frame.generator.ok_or(VmError::RuntimeError(
                    "YieldFrom outside of generator context".into(),
                ))?;

                let (mut sub_iter, is_new) = {
                    let gen_val = self.arena.get(gen_handle);
                    if let Val::Object(payload_handle) = &gen_val.value {
                        let payload = self.arena.get(*payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload.value {
                            if let Some(internal) = &obj_data.internal {
                                if let Ok(gen_data) =
                                    internal.clone().downcast::<RefCell<GeneratorData>>()
                                {
                                    let mut data = gen_data.borrow_mut();
                                    if let Some(iter) = &data.sub_iter {
                                        (iter.clone(), false)
                                    } else {
                                        let iterable_handle = self.operand_stack.pop().ok_or(
                                            VmError::RuntimeError("Stack underflow".into()),
                                        )?;
                                        let iter = match &self.arena.get(iterable_handle).value {
                                            Val::Array(_) => SubIterator::Array {
                                                handle: iterable_handle,
                                                index: 0,
                                            },
                                            Val::Object(_) => SubIterator::Generator {
                                                handle: iterable_handle,
                                                state: SubGenState::Initial,
                                            },
                                            val => {
                                                return Err(VmError::RuntimeError(format!(
                                                "Yield from expects array or traversable, got {:?}",
                                                val
                                            )))
                                            }
                                        };
                                        data.sub_iter = Some(iter.clone());
                                        (iter, true)
                                    }
                                } else {
                                    return Err(VmError::RuntimeError(
                                        "Invalid generator data".into(),
                                    ));
                                }
                            } else {
                                return Err(VmError::RuntimeError("Invalid generator data".into()));
                            }
                        } else {
                            return Err(VmError::RuntimeError("Invalid generator data".into()));
                        }
                    } else {
                        return Err(VmError::RuntimeError("Invalid generator data".into()));
                    }
                };

                match &mut sub_iter {
                    SubIterator::Array { handle, index } => {
                        if !is_new {
                            // Pop sent value (ignored for array)
                            {
                                let gen_val = self.arena.get(gen_handle);
                                if let Val::Object(payload_handle) = &gen_val.value {
                                    let payload = self.arena.get(*payload_handle);
                                    if let Val::ObjPayload(obj_data) = &payload.value {
                                        if let Some(internal) = &obj_data.internal {
                                            if let Ok(gen_data) = internal
                                                .clone()
                                                .downcast::<RefCell<GeneratorData>>()
                                            {
                                                let mut data = gen_data.borrow_mut();
                                                data.sent_val.take();
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if let Val::Array(map) = &self.arena.get(*handle).value {
                            if let Some((k, v)) = map.map.get_index(*index) {
                                let val_handle = *v;
                                let key_handle = match k {
                                    ArrayKey::Int(i) => self.arena.alloc(Val::Int(*i)),
                                    ArrayKey::Str(s) => {
                                        self.arena.alloc(Val::String(s.as_ref().clone().into()))
                                    }
                                };

                                *index += 1;

                                let mut frame = self.frames.pop().unwrap();
                                frame.ip -= 1; // Stay on YieldFrom

                                {
                                    let gen_val = self.arena.get(gen_handle);
                                    if let Val::Object(payload_handle) = &gen_val.value {
                                        let payload = self.arena.get(*payload_handle);
                                        if let Val::ObjPayload(obj_data) = &payload.value {
                                            if let Some(internal) = &obj_data.internal {
                                                if let Ok(gen_data) = internal
                                                    .clone()
                                                    .downcast::<RefCell<GeneratorData>>()
                                                {
                                                    let mut data = gen_data.borrow_mut();
                                                    data.current_val = Some(val_handle);
                                                    data.current_key = Some(key_handle);
                                                    data.state = GeneratorState::Delegating(frame);
                                                    data.sub_iter = Some(sub_iter.clone());
                                                }
                                            }
                                        }
                                    }
                                }

                                // Do NOT push to caller stack
                                return Ok(());
                            } else {
                                // Finished
                                {
                                    let gen_val = self.arena.get(gen_handle);
                                    if let Val::Object(payload_handle) = &gen_val.value {
                                        let payload = self.arena.get(*payload_handle);
                                        if let Val::ObjPayload(obj_data) = &payload.value {
                                            if let Some(internal) = &obj_data.internal {
                                                if let Ok(gen_data) = internal
                                                    .clone()
                                                    .downcast::<RefCell<GeneratorData>>()
                                                {
                                                    let mut data = gen_data.borrow_mut();
                                                    data.state = GeneratorState::Running;
                                                    data.sub_iter = None;
                                                }
                                            }
                                        }
                                    }
                                }
                                let null_handle = self.arena.alloc(Val::Null);
                                self.operand_stack.push(null_handle);
                            }
                        }
                    }
                    SubIterator::Generator { handle, state } => {
                        match state {
                            SubGenState::Initial | SubGenState::Resuming => {
                                let gen_b_val = self.arena.get(*handle);
                                if let Val::Object(payload_handle) = &gen_b_val.value {
                                    let payload = self.arena.get(*payload_handle);
                                    if let Val::ObjPayload(obj_data) = &payload.value {
                                        if let Some(internal) = &obj_data.internal {
                                            if let Ok(gen_data) = internal
                                                .clone()
                                                .downcast::<RefCell<GeneratorData>>()
                                            {
                                                let mut data = gen_data.borrow_mut();

                                                let frame_to_push = match &data.state {
                                                    GeneratorState::Created(f)
                                                    | GeneratorState::Suspended(f) => {
                                                        let mut f = f.clone();
                                                        f.generator = Some(*handle);
                                                        Some(f)
                                                    }
                                                    _ => None,
                                                };

                                                if let Some(f) = frame_to_push {
                                                    data.state = GeneratorState::Running;

                                                    // Update state to Yielded
                                                    *state = SubGenState::Yielded;

                                                    // Decrement IP of current frame so we re-execute YieldFrom when we return
                                                    {
                                                        let frame = self.frames.last_mut().unwrap();
                                                        frame.ip -= 1;
                                                    }

                                                    // Update GenA state (set sub_iter, but keep Running)
                                                    {
                                                        let gen_val = self.arena.get(gen_handle);
                                                        if let Val::Object(payload_handle) =
                                                            &gen_val.value
                                                        {
                                                            let payload =
                                                                self.arena.get(*payload_handle);
                                                            if let Val::ObjPayload(obj_data) =
                                                                &payload.value
                                                            {
                                                                if let Some(internal) =
                                                                    &obj_data.internal
                                                                {
                                                                    if let Ok(parent_gen_data) = internal.clone().downcast::<RefCell<GeneratorData>>() {
                                                                        let mut parent_data = parent_gen_data.borrow_mut();
                                                                        parent_data.sub_iter = Some(sub_iter.clone());
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }

                                                    self.push_frame(f);

                                                    // If Resuming, we leave the sent value on stack for GenB
                                                    // If Initial, we push null (dummy sent value)
                                                    if is_new {
                                                        let null_handle =
                                                            self.arena.alloc(Val::Null);
                                                        // Set sent_val in child generator data
                                                        data.sent_val = Some(null_handle);
                                                    }
                                                    return Ok(());
                                                } else if let GeneratorState::Finished = data.state
                                                {
                                                    // Already finished?
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            SubGenState::Yielded => {
                                let mut gen_b_finished = false;
                                let mut yielded_val = None;
                                let mut yielded_key = None;

                                {
                                    let gen_b_val = self.arena.get(*handle);
                                    if let Val::Object(payload_handle) = &gen_b_val.value {
                                        let payload = self.arena.get(*payload_handle);
                                        if let Val::ObjPayload(obj_data) = &payload.value {
                                            if let Some(internal) = &obj_data.internal {
                                                if let Ok(gen_data) = internal
                                                    .clone()
                                                    .downcast::<RefCell<GeneratorData>>()
                                                {
                                                    let data = gen_data.borrow();
                                                    if let GeneratorState::Finished = data.state {
                                                        gen_b_finished = true;
                                                    } else {
                                                        yielded_val = data.current_val;
                                                        yielded_key = data.current_key;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                if gen_b_finished {
                                    // GenB finished, return value is on the stack (pushed by OpCode::Return)
                                    let result_handle = self
                                        .operand_stack
                                        .pop()
                                        .unwrap_or_else(|| self.arena.alloc(Val::Null));

                                    // GenB finished, result_handle is return value
                                    {
                                        let gen_val = self.arena.get(gen_handle);
                                        if let Val::Object(payload_handle) = &gen_val.value {
                                            let payload = self.arena.get(*payload_handle);
                                            if let Val::ObjPayload(obj_data) = &payload.value {
                                                if let Some(internal) = &obj_data.internal {
                                                    if let Ok(gen_data) = internal
                                                        .clone()
                                                        .downcast::<RefCell<GeneratorData>>()
                                                    {
                                                        let mut data = gen_data.borrow_mut();
                                                        data.state = GeneratorState::Running;
                                                        data.sub_iter = None;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    self.operand_stack.push(result_handle);
                                } else {
                                    // GenB yielded
                                    *state = SubGenState::Resuming;

                                    let mut frame = self.frames.pop().unwrap();
                                    frame.ip -= 1;

                                    {
                                        let gen_val = self.arena.get(gen_handle);
                                        if let Val::Object(payload_handle) = &gen_val.value {
                                            let payload = self.arena.get(*payload_handle);
                                            if let Val::ObjPayload(obj_data) = &payload.value {
                                                if let Some(internal) = &obj_data.internal {
                                                    if let Ok(gen_data) = internal
                                                        .clone()
                                                        .downcast::<RefCell<GeneratorData>>()
                                                    {
                                                        let mut data = gen_data.borrow_mut();
                                                        data.current_val = yielded_val;
                                                        data.current_key = yielded_key;
                                                        data.state =
                                                            GeneratorState::Delegating(frame);
                                                        data.sub_iter = Some(sub_iter.clone());
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Do NOT push to caller stack
                                    return Ok(());
                                }
                            }
                        }
                    }
                }
            }

            OpCode::GetSentValue => {
                let frame_idx = self.frames.len() - 1;
                let frame = &mut self.frames[frame_idx];
                let gen_handle = frame.generator.ok_or(VmError::RuntimeError(
                    "GetSentValue outside of generator context".into(),
                ))?;

                let sent_handle = {
                    let gen_val = self.arena.get(gen_handle);
                    if let Val::Object(payload_handle) = &gen_val.value {
                        let payload = self.arena.get(*payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload.value {
                            if let Some(internal) = &obj_data.internal {
                                if let Ok(gen_data) =
                                    internal.clone().downcast::<RefCell<GeneratorData>>()
                                {
                                    let mut data = gen_data.borrow_mut();
                                    // Get and clear sent_val
                                    data.sent_val
                                        .take()
                                        .unwrap_or_else(|| self.arena.alloc(Val::Null))
                                } else {
                                    return Err(VmError::RuntimeError(
                                        "Invalid generator data".into(),
                                    ));
                                }
                            } else {
                                return Err(VmError::RuntimeError("Invalid generator data".into()));
                            }
                        } else {
                            return Err(VmError::RuntimeError("Invalid generator data".into()));
                        }
                    } else {
                        return Err(VmError::RuntimeError("Invalid generator data".into()));
                    }
                };

                self.operand_stack.push(sent_handle);
            }

            OpCode::DefFunc(name, func_idx) => {
                let val = {
                    let frame = self.frames.last().unwrap();
                    frame.chunk.constants[func_idx as usize].clone()
                };
                if let Val::Resource(rc) = val {
                    if let Ok(func) = rc.downcast::<UserFunc>() {
                        self.context.user_functions.insert(name, func);
                    }
                }
            }

            OpCode::Include => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val = self.arena.get(handle);
                let filename = match &val.value {
                    Val::String(s) => String::from_utf8_lossy(s).to_string(),
                    _ => return Err(VmError::RuntimeError("Include expects string".into())),
                };

                let resolved_path = self.resolve_script_path(&filename)?;
                let source = std::fs::read(&resolved_path).map_err(|e| {
                    VmError::RuntimeError(format!("Could not read file {}: {}", filename, e))
                })?;
                let canonical_path = Self::canonical_path_string(&resolved_path);

                let arena = bumpalo::Bump::new();
                let lexer = php_parser::lexer::Lexer::new(&source);
                let mut parser = php_parser::parser::Parser::new(lexer, &arena);
                let program = parser.parse_program();

                if !program.errors.is_empty() {
                    return Err(VmError::RuntimeError(format!(
                        "Parse errors: {:?}",
                        program.errors
                    )));
                }

                let emitter =
                    crate::compiler::emitter::Emitter::new(&source, &mut self.context.interner)
                        .with_file_path(canonical_path.clone());
                let (chunk, _) = emitter.compile(program.statements);

                // PHP shares the same symbol_table between caller and included code (Zend VM ref).
                // We clone locals, run the include, then copy them back to persist changes.
                let caller_frame_idx = self.frames.len() - 1;
                let mut frame = CallFrame::new(Rc::new(chunk));

                // Include inherits full scope (this, class_scope, called_scope) and symbol table
                if let Some(caller) = self.frames.get(caller_frame_idx) {
                    frame.locals = caller.locals.clone();
                    frame.this = caller.this;
                    frame.class_scope = caller.class_scope;
                    frame.called_scope = caller.called_scope;
                }

                self.push_frame(frame);
                let depth = self.frames.len();

                // Execute the included file (inlining run_loop to capture locals before pop)
                let mut include_error = None;
                loop {
                    if self.frames.len() < depth {
                        break; // Frame was popped by return
                    }
                    if self.frames.len() == depth {
                        let frame = &self.frames[depth - 1];
                        if frame.ip >= frame.chunk.code.len() {
                            break; // Frame execution complete
                        }
                    }

                    // Execute one opcode (mimicking run_loop)
                    let op = {
                        let frame = self.current_frame_mut()?;
                        if frame.ip >= frame.chunk.code.len() {
                            self.frames.pop();
                            break;
                        }
                        let op = frame.chunk.code[frame.ip];
                        frame.ip += 1;
                        op
                    };

                    if let Err(e) = self.execute_opcode(op, depth) {
                        include_error = Some(e);
                        break;
                    }
                }

                // Capture the included frame's final locals before popping
                let final_locals = if self.frames.len() >= depth {
                    Some(self.frames[depth - 1].locals.clone())
                } else {
                    None
                };

                // Pop the include frame if it's still on the stack
                if self.frames.len() >= depth {
                    self.frames.pop();
                }

                // Copy modified locals back to caller (PHP's shared symbol_table behavior)
                if let Some(locals) = final_locals {
                    if let Some(caller) = self.frames.get_mut(caller_frame_idx) {
                        caller.locals = locals;
                    }
                }

                // Handle errors
                if let Some(err) = include_error {
                    // On error, return false and DON'T mark as included
                    self.operand_stack.push(self.arena.alloc(Val::Bool(false)));
                    return Err(err);
                }

                // Mark file as successfully included ONLY after successful execution
                self.context.included_files.insert(canonical_path);

                // Push return value: include uses last_return_value if available, else Int(1)
                let return_val = self
                    .last_return_value
                    .unwrap_or_else(|| self.arena.alloc(Val::Int(1)));
                self.last_return_value = None; // Clear it for next operation
                self.operand_stack.push(return_val);
            }

            // Array operations - delegated to opcodes::array_ops
            OpCode::InitArray(size) => self.exec_init_array(size)?,

            OpCode::FetchDim => {
                let key_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let array_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let array_val = &self.arena.get(array_handle).value;
                match array_val {
                    Val::Array(map) => {
                        let key_val = &self.arena.get(key_handle).value;
                        let key = self.array_key_from_value(key_val)?;

                        if let Some(val_handle) = map.map.get(&key) {
                            self.operand_stack.push(*val_handle);
                        } else {
                            // Emit notice for undefined array key
                            let key_str = match &key {
                                ArrayKey::Int(i) => i.to_string(),
                                ArrayKey::Str(s) => String::from_utf8_lossy(s).to_string(),
                            };
                            self.report_error(
                                ErrorLevel::Notice,
                                &format!("Undefined array key \"{}\"", key_str),
                            );
                            let null_handle = self.arena.alloc(Val::Null);
                            self.operand_stack.push(null_handle);
                        }
                    }
                    Val::String(s) => {
                        // String offset access
                        // Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_fetch_dimension_address_read_R
                        let dim_val = &self.arena.get(key_handle).value;

                        // Convert offset to integer (PHP coerces any type to int for string offsets)
                        let offset = dim_val.to_int();

                        // Handle negative offsets (count from end)
                        // Reference: PHP 7.1+ supports negative string offsets
                        let len = s.len() as i64;
                        let actual_offset = if offset < 0 {
                            // Negative offset: count from end
                            let adjusted = len + offset;
                            if adjusted < 0 {
                                // Still out of bounds even after adjustment
                                self.report_error(
                                    ErrorLevel::Warning,
                                    &format!("Uninitialized string offset {}", offset),
                                );
                                let empty = self.arena.alloc(Val::String(vec![].into()));
                                self.operand_stack.push(empty);
                                return Ok(());
                            }
                            adjusted as usize
                        } else {
                            offset as usize
                        };

                        if actual_offset < s.len() {
                            let char_str = vec![s[actual_offset]];
                            let val = self.arena.alloc(Val::String(char_str.into()));
                            self.operand_stack.push(val);
                        } else {
                            self.report_error(
                                ErrorLevel::Warning,
                                &format!("Uninitialized string offset {}", offset),
                            );
                            let empty = self.arena.alloc(Val::String(vec![].into()));
                            self.operand_stack.push(empty);
                        }
                    }
                    Val::Object(payload_handle) => {
                        // Check if object implements ArrayAccess
                        let payload_val = self.arena.get(*payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload_val.value {
                            let class_name = obj_data.class;

                            if self.implements_array_access(class_name) {
                                // Call offsetGet method
                                let result =
                                    self.call_array_access_offset_get(array_handle, key_handle)?;
                                self.operand_stack.push(result);
                            } else {
                                // Object doesn't implement ArrayAccess
                                self.report_error(
                                    ErrorLevel::Warning,
                                    "Trying to access array offset on value of type object",
                                );
                                let null_handle = self.arena.alloc(Val::Null);
                                self.operand_stack.push(null_handle);
                            }
                        } else {
                            // Shouldn't happen, but handle it
                            self.report_error(
                                ErrorLevel::Warning,
                                "Trying to access array offset on value of type object",
                            );
                            let null_handle = self.arena.alloc(Val::Null);
                            self.operand_stack.push(null_handle);
                        }
                    }
                    Val::ObjPayload(obj_data) => {
                        // Direct ObjPayload (shouldn't normally happen in FetchDim context)
                        let class_name = obj_data.class;

                        if self.implements_array_access(class_name) {
                            // Call offsetGet method
                            let result =
                                self.call_array_access_offset_get(array_handle, key_handle)?;
                            self.operand_stack.push(result);
                        } else {
                            // Object doesn't implement ArrayAccess
                            self.report_error(
                                ErrorLevel::Warning,
                                "Trying to access array offset on value of type object",
                            );
                            let null_handle = self.arena.alloc(Val::Null);
                            self.operand_stack.push(null_handle);
                        }
                    }
                    _ => {
                        let type_str = match array_val {
                            Val::Null => "null",
                            Val::Bool(_) => "bool",
                            Val::Int(_) => "int",
                            Val::Float(_) => "float",
                            Val::String(_) => "string",
                            _ => "value",
                        };
                        self.report_error(
                            ErrorLevel::Warning,
                            &format!(
                                "Trying to access array offset on value of type {}",
                                type_str
                            ),
                        );
                        let null_handle = self.arena.alloc(Val::Null);
                        self.operand_stack.push(null_handle);
                    }
                }
            }

            OpCode::AssignDim => self.exec_assign_dim()?,

            OpCode::AssignDimRef => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let key_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let array_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                self.assign_dim(array_handle, key_handle, val_handle)?;

                // assign_dim pushes the new array handle.
                let new_array_handle = self.operand_stack.pop().unwrap();

                // We want to return [Val, NewArray] so that we can StoreVar(NewArray) and leave Val.
                self.operand_stack.push(val_handle);
                self.operand_stack.push(new_array_handle);
            }

            OpCode::AssignDimOp(op) => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let key_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let array_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                // Get current value
                let current_val = {
                    let array_val = &self.arena.get(array_handle).value;
                    match array_val {
                        Val::Array(map) => {
                            let key_val = &self.arena.get(key_handle).value;
                            let key = self.array_key_from_value(key_val)?;
                            if let Some(val_handle) = map.map.get(&key) {
                                self.arena.get(*val_handle).value.clone()
                            } else {
                                Val::Null
                            }
                        }
                        Val::Object(payload_handle) => {
                            // Check if it's ArrayAccess
                            let payload = self.arena.get(*payload_handle);
                            if let Val::ObjPayload(obj_data) = &payload.value {
                                let class_name = obj_data.class;
                                if self.implements_array_access(class_name) {
                                    // Call offsetGet
                                    let result = self
                                        .call_array_access_offset_get(array_handle, key_handle)?;
                                    self.arena.get(result).value.clone()
                                } else {
                                    return Err(VmError::RuntimeError(
                                        "Trying to access offset on non-array".into(),
                                    ));
                                }
                            } else {
                                return Err(VmError::RuntimeError(
                                    "Trying to access offset on non-array".into(),
                                ));
                            }
                        }
                        _ => {
                            return Err(VmError::RuntimeError(
                                "Trying to access offset on non-array".into(),
                            ))
                        }
                    }
                };

                let val = self.arena.get(val_handle).value.clone();
                let res = match op {
                    0 => match (current_val, val) {
                        // Add
                        (Val::Int(a), Val::Int(b)) => Val::Int(a + b),
                        _ => Val::Null,
                    },
                    1 => match (current_val, val) {
                        // Sub
                        (Val::Int(a), Val::Int(b)) => Val::Int(a - b),
                        _ => Val::Null,
                    },
                    2 => match (current_val, val) {
                        // Mul
                        (Val::Int(a), Val::Int(b)) => Val::Int(a * b),
                        _ => Val::Null,
                    },
                    3 => match (current_val, val) {
                        // Div
                        (Val::Int(a), Val::Int(b)) => Val::Int(a / b),
                        _ => Val::Null,
                    },
                    4 => match (current_val, val) {
                        // Mod
                        (Val::Int(a), Val::Int(b)) => {
                            if b == 0 {
                                return Err(VmError::RuntimeError("Modulo by zero".into()));
                            }
                            Val::Int(a % b)
                        }
                        _ => Val::Null,
                    },
                    7 => match (current_val, val) {
                        // Concat
                        (Val::String(a), Val::String(b)) => {
                            let mut s = String::from_utf8_lossy(&a).to_string();
                            s.push_str(&String::from_utf8_lossy(&b));
                            Val::String(s.into_bytes().into())
                        }
                        _ => Val::Null,
                    },
                    _ => Val::Null,
                };

                let res_handle = self.arena.alloc(res);
                self.assign_dim_value(array_handle, key_handle, res_handle)?;
            }
            OpCode::AddArrayElement => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let key_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let array_handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let key_val = &self.arena.get(key_handle).value;
                let key = self.array_key_from_value(key_val)?;

                let array_zval = self.arena.get_mut(array_handle);
                if let Val::Array(map) = &mut array_zval.value {
                    Rc::make_mut(map).insert(key, val_handle);
                } else {
                    return Err(VmError::RuntimeError(
                        "AddArrayElement expects array".into(),
                    ));
                }
            }
            OpCode::StoreDim => self.exec_store_dim()?,

            OpCode::AppendArray => self.exec_append_array()?,
            OpCode::AddArrayUnpack => {
                let src_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let dest_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                {
                    let dest_zval = self.arena.get_mut(dest_handle);
                    if matches!(dest_zval.value, Val::Null | Val::Bool(false)) {
                        dest_zval.value = Val::Array(ArrayData::new().into());
                    } else if !matches!(dest_zval.value, Val::Array(_)) {
                        return Err(VmError::RuntimeError("Cannot unpack into non-array".into()));
                    }
                }

                let src_map = {
                    let src_val = self.arena.get(src_handle);
                    match &src_val.value {
                        Val::Array(m) => m.clone(),
                        _ => {
                            return Err(VmError::RuntimeError("Array unpack expects array".into()))
                        }
                    }
                };

                let dest_map = {
                    let dest_val = self.arena.get_mut(dest_handle);
                    match &mut dest_val.value {
                        Val::Array(m) => m,
                        _ => unreachable!(),
                    }
                };

                // Get the starting next_key from ArrayData (O(1))
                let mut next_key = dest_map.next_index();

                for (key, val_handle) in src_map.map.iter() {
                    match key {
                        ArrayKey::Int(_) => {
                            // Reindex numeric keys using ArrayData::insert (maintains next_free)
                            Rc::make_mut(dest_map).insert(ArrayKey::Int(next_key), *val_handle);
                            next_key += 1;
                        }
                        ArrayKey::Str(s) => {
                            // Preserve string keys
                            Rc::make_mut(dest_map).insert(ArrayKey::Str(s.clone()), *val_handle);
                        }
                    }
                }

                self.operand_stack.push(dest_handle);
            }

            OpCode::StoreAppend => {
                let array_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                self.append_array(array_handle, val_handle)?;

                // Check if we just appended to an element of $GLOBALS and sync it
                // This handles cases like: $GLOBALS['arr'][] = 4
                let is_globals_element = {
                    let globals_sym = self.context.interner.intern(b"GLOBALS");
                    if let Some(&globals_handle) = self.context.globals.get(&globals_sym) {
                        // Check if array_handle is an element within the $GLOBALS array
                        if let Val::Array(globals_data) = &self.arena.get(globals_handle).value {
                            globals_data.map.values().any(|&h| h == array_handle)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };

                if is_globals_element {
                    // The array was already modified in place, and since $GLOBALS elements
                    // share handles with global variables, the change is already synced
                    // No additional sync needed
                }
            }
            OpCode::UnsetDim => {
                let key_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let array_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                // Check if this is an ArrayAccess object
                // Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_UNSET_DIM_SPEC
                let array_val = &self.arena.get(array_handle).value;

                if let Val::Object(payload_handle) = array_val {
                    let payload = self.arena.get(*payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload.value {
                        let class_name = obj_data.class;
                        if self.implements_array_access(class_name) {
                            // Call ArrayAccess::offsetUnset($offset)
                            self.call_array_access_offset_unset(array_handle, key_handle)?;
                            return Ok(());
                        }
                    }
                }

                // Standard array unset logic
                let key_val = &self.arena.get(key_handle).value;
                let key = self.array_key_from_value(key_val)?;

                let array_zval_mut = self.arena.get_mut(array_handle);
                if let Val::Array(map) = &mut array_zval_mut.value {
                    Rc::make_mut(map).map.shift_remove(&key);

                    // Check if this is a write to $GLOBALS and sync it
                    let is_globals_write = {
                        let globals_sym = self.context.interner.intern(b"GLOBALS");
                        self.context.globals.get(&globals_sym).copied() == Some(array_handle)
                    };

                    if is_globals_write {
                        // Sync the deletion back to the global symbol table
                        if let ArrayKey::Str(key_bytes) = &key {
                            let sym = self.context.interner.intern(key_bytes);
                            if key_bytes.as_ref() != b"GLOBALS" {
                                self.context.globals.remove(&sym);
                                if let Some(frame) = self.frames.first_mut() {
                                    frame.locals.remove(&sym);
                                }
                            }
                        }
                    }
                }
            }
            OpCode::InArray => {
                let array_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let needle_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let array_val = &self.arena.get(array_handle).value;
                let needle_val = &self.arena.get(needle_handle).value;

                let found = if let Val::Array(map) = array_val {
                    map.map.values().any(|h| {
                        let v = &self.arena.get(*h).value;
                        v == needle_val
                    })
                } else {
                    false
                };

                let res_handle = self.arena.alloc(Val::Bool(found));
                self.operand_stack.push(res_handle);
            }
            OpCode::ArrayKeyExists => {
                let array_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let key_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let key_val = &self.arena.get(key_handle).value;
                let key = self.array_key_from_value(key_val)?;

                let array_val = &self.arena.get(array_handle).value;
                let found = if let Val::Array(map) = array_val {
                    map.map.contains_key(&key)
                } else {
                    false
                };

                let res_handle = self.arena.alloc(Val::Bool(found));
                self.operand_stack.push(res_handle);
            }

            OpCode::StoreNestedDim(depth) => self.exec_assign_nested_dim(depth)?,

            OpCode::FetchNestedDim(depth) => self.exec_fetch_nested_dim_op(depth)?,

            OpCode::UnsetNestedDim(depth) => self.exec_unset_nested_dim(depth)?,

            OpCode::IterInit(target) => {
                // Stack: [Array/Object]
                let iterable_handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let iterable_val = &self.arena.get(iterable_handle).value;

                match iterable_val {
                    Val::Array(map) => {
                        let len = map.map.len();
                        if len == 0 {
                            self.operand_stack.pop(); // Pop array
                            let frame = self.frames.last_mut().unwrap();
                            frame.ip = target as usize;
                        } else {
                            let idx_handle = self.arena.alloc(Val::Int(0));
                            self.operand_stack.push(idx_handle);
                        }
                    }
                    Val::Object(payload_handle) => {
                        let payload = self.arena.get(*payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload.value {
                            let mut handled = false;
                            if let Some(internal) = &obj_data.internal {
                                if let Ok(gen_data) =
                                    internal.clone().downcast::<RefCell<GeneratorData>>()
                                {
                                    let mut data = gen_data.borrow_mut();
                                    match &data.state {
                                        GeneratorState::Created(frame) => {
                                            let mut frame = frame.clone();
                                            frame.generator = Some(iterable_handle);
                                            self.push_frame(frame);
                                            data.state = GeneratorState::Running;

                                            // Push dummy index to maintain [Iterable, Index] stack shape
                                            let idx_handle = self.arena.alloc(Val::Int(0));
                                            self.operand_stack.push(idx_handle);
                                        }
                                        GeneratorState::Finished => {
                                            self.operand_stack.pop(); // Pop iterable
                                            let frame = self.frames.last_mut().unwrap();
                                            frame.ip = target as usize;
                                        }
                                        _ => {
                                            return Err(VmError::RuntimeError(
                                                "Cannot rewind generator".into(),
                                            ))
                                        }
                                    }
                                    handled = true;
                                }
                            }

                            if !handled {
                                let iterator_sym = self.context.interner.intern(b"Iterator");
                                if self.is_instance_of(iterable_handle, iterator_sym) {
                                    let rewind_sym = self.context.interner.intern(b"rewind");
                                    let valid_sym = self.context.interner.intern(b"valid");

                                    self.call_native_method_simple(iterable_handle, rewind_sym)?;
                                    let is_valid =
                                        self.call_native_method_simple(iterable_handle, valid_sym)?;

                                    if let Val::Bool(false) = self.arena.get(is_valid).value {
                                        self.operand_stack.pop(); // Pop object
                                        let frame = self.frames.last_mut().unwrap();
                                        frame.ip = target as usize;
                                    } else {
                                        // Push dummy index
                                        let idx_handle = self.arena.alloc(Val::Int(0));
                                        self.operand_stack.push(idx_handle);
                                    }
                                    handled = true;
                                }
                            }

                            if !handled {
                                return Err(VmError::RuntimeError("Object not iterable".into()));
                            }
                        } else {
                            return Err(VmError::RuntimeError("Object not iterable".into()));
                        }
                    }
                    _ => {
                        return Err(VmError::RuntimeError(
                            "Foreach expects array or object".into(),
                        ))
                    }
                }
            }

            OpCode::IterValid(target) => {
                // Stack: [Iterable, Index]
                // Or [Iterable, DummyIndex, ReturnValue] if generator returned

                let mut idx_handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let mut iterable_handle = self
                    .operand_stack
                    .peek_at(1)
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                // Check for generator return value on stack
                if let Val::Null = &self.arena.get(iterable_handle).value {
                    if let Some(real_iterable_handle) = self.operand_stack.peek_at(2) {
                        if let Val::Object(_) = &self.arena.get(real_iterable_handle).value {
                            // Found generator return value. Pop it.
                            self.operand_stack.pop();
                            // Re-fetch handles
                            idx_handle = self
                                .operand_stack
                                .peek()
                                .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                            iterable_handle = self
                                .operand_stack
                                .peek_at(1)
                                .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                        }
                    }
                }

                let iterable_val = &self.arena.get(iterable_handle).value;
                match iterable_val {
                    Val::Array(map) => {
                        let idx = match self.arena.get(idx_handle).value {
                            Val::Int(i) => i as usize,
                            _ => {
                                return Err(VmError::RuntimeError(
                                    "Iterator index must be int".into(),
                                ))
                            }
                        };
                        if idx >= map.map.len() {
                            self.operand_stack.pop(); // Pop Index
                            self.operand_stack.pop(); // Pop Array
                            let frame = self.frames.last_mut().unwrap();
                            frame.ip = target as usize;
                        }
                    }
                    Val::Object(payload_handle) => {
                        let payload = self.arena.get(*payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload.value {
                            let mut handled = false;
                            if let Some(internal) = &obj_data.internal {
                                if let Ok(gen_data) =
                                    internal.clone().downcast::<RefCell<GeneratorData>>()
                                {
                                    let data = gen_data.borrow();
                                    if let GeneratorState::Finished = data.state {
                                        self.operand_stack.pop(); // Pop Index
                                        self.operand_stack.pop(); // Pop Iterable
                                        let frame = self.frames.last_mut().unwrap();
                                        frame.ip = target as usize;
                                    }
                                    handled = true;
                                }
                            }

                            if !handled {
                                let iterator_sym = self.context.interner.intern(b"Iterator");
                                if self.is_instance_of(iterable_handle, iterator_sym) {
                                    let valid_sym = self.context.interner.intern(b"valid");
                                    let is_valid =
                                        self.call_native_method_simple(iterable_handle, valid_sym)?;

                                    if let Val::Bool(false) = self.arena.get(is_valid).value {
                                        self.operand_stack.pop(); // Pop Index
                                        self.operand_stack.pop(); // Pop Iterable
                                        let frame = self.frames.last_mut().unwrap();
                                        frame.ip = target as usize;
                                    }
                                    handled = true;
                                }
                            }
                        }
                    }
                    _ => {
                        return Err(VmError::RuntimeError(
                            "Foreach expects array or object".into(),
                        ))
                    }
                }
            }

            OpCode::IterNext => {
                // Stack: [Iterable, Index]
                let idx_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let iterable_handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let iterable_val = &self.arena.get(iterable_handle).value;
                match iterable_val {
                    Val::Array(_) => {
                        let idx = match self.arena.get(idx_handle).value {
                            Val::Int(i) => i,
                            _ => {
                                return Err(VmError::RuntimeError(
                                    "Iterator index must be int".into(),
                                ))
                            }
                        };
                        let new_idx_handle = self.arena.alloc(Val::Int(idx + 1));
                        self.operand_stack.push(new_idx_handle);
                    }
                    Val::Object(payload_handle) => {
                        let payload = self.arena.get(*payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload.value {
                            let mut handled = false;
                            if let Some(internal) = &obj_data.internal {
                                if let Ok(gen_data) =
                                    internal.clone().downcast::<RefCell<GeneratorData>>()
                                {
                                    let mut data = gen_data.borrow_mut();
                                    if let GeneratorState::Suspended(frame) = &data.state {
                                        let mut frame = frame.clone();
                                        frame.generator = Some(iterable_handle);
                                        self.push_frame(frame);
                                        data.state = GeneratorState::Running;
                                        // Push dummy index
                                        let idx_handle = self.arena.alloc(Val::Null);
                                        self.operand_stack.push(idx_handle);
                                        // Store sent value (null) for generator
                                        let sent_handle = self.arena.alloc(Val::Null);
                                        data.sent_val = Some(sent_handle);
                                    } else if let GeneratorState::Delegating(frame) = &data.state {
                                        let mut frame = frame.clone();
                                        frame.generator = Some(iterable_handle);
                                        self.push_frame(frame);
                                        data.state = GeneratorState::Running;
                                        // Push dummy index
                                        let idx_handle = self.arena.alloc(Val::Null);
                                        self.operand_stack.push(idx_handle);
                                        // Store sent value (null) for generator
                                        let sent_handle = self.arena.alloc(Val::Null);
                                        data.sent_val = Some(sent_handle);
                                    } else if let GeneratorState::Finished = data.state {
                                        let idx_handle = self.arena.alloc(Val::Null);
                                        self.operand_stack.push(idx_handle);
                                    } else {
                                        return Err(VmError::RuntimeError(
                                            "Cannot resume running generator".into(),
                                        ));
                                    }
                                    handled = true;
                                }
                            }

                            if !handled {
                                let iterator_sym = self.context.interner.intern(b"Iterator");
                                if self.is_instance_of(iterable_handle, iterator_sym) {
                                    let next_sym = self.context.interner.intern(b"next");
                                    self.call_native_method_simple(iterable_handle, next_sym)?;
                                    // Push dummy index back
                                    self.operand_stack.push(idx_handle);
                                    handled = true;
                                }
                            }

                            if !handled {
                                return Err(VmError::RuntimeError("Object not iterable".into()));
                            }
                        } else {
                            return Err(VmError::RuntimeError("Object not iterable".into()));
                        }
                    }
                    _ => {
                        return Err(VmError::RuntimeError(
                            "Foreach expects array or object".into(),
                        ))
                    }
                }
            }

            OpCode::IterGetVal(sym) => {
                // Stack: [Iterable, Index]
                let idx_handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let iterable_handle = self
                    .operand_stack
                    .peek_at(1)
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let iterable_val = &self.arena.get(iterable_handle).value;
                match iterable_val {
                    Val::Array(map) => {
                        let idx = match self.arena.get(idx_handle).value {
                            Val::Int(i) => i as usize,
                            _ => {
                                return Err(VmError::RuntimeError(
                                    "Iterator index must be int".into(),
                                ))
                            }
                        };
                        if let Some((_, val_handle)) = map.map.get_index(idx) {
                            let val_h = *val_handle;
                            let final_handle = if self.arena.get(val_h).is_ref {
                                let val = self.arena.get(val_h).value.clone();
                                self.arena.alloc(val)
                            } else {
                                val_h
                            };
                            let frame = self.frames.last_mut().unwrap();
                            frame.locals.insert(sym, final_handle);
                        } else {
                            return Err(VmError::RuntimeError(
                                "Iterator index out of bounds".into(),
                            ));
                        }
                    }
                    Val::Object(payload_handle) => {
                        let payload = self.arena.get(*payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload.value {
                            let mut handled = false;
                            if let Some(internal) = &obj_data.internal {
                                if let Ok(gen_data) =
                                    internal.clone().downcast::<RefCell<GeneratorData>>()
                                {
                                    let data = gen_data.borrow();
                                    if let Some(val_handle) = data.current_val {
                                        let frame = self.frames.last_mut().unwrap();
                                        frame.locals.insert(sym, val_handle);
                                    } else {
                                        return Err(VmError::RuntimeError(
                                            "Generator has no current value".into(),
                                        ));
                                    }
                                    handled = true;
                                }
                            }

                            if !handled {
                                let iterator_sym = self.context.interner.intern(b"Iterator");
                                if self.is_instance_of(iterable_handle, iterator_sym) {
                                    let current_sym = self.context.interner.intern(b"current");
                                    let val_handle = self
                                        .call_native_method_simple(iterable_handle, current_sym)?;
                                    let frame = self.frames.last_mut().unwrap();
                                    frame.locals.insert(sym, val_handle);
                                    handled = true;
                                }
                            }

                            if !handled {
                                return Err(VmError::RuntimeError("Object not iterable".into()));
                            }
                        } else {
                            return Err(VmError::RuntimeError("Object not iterable".into()));
                        }
                    }
                    _ => {
                        return Err(VmError::RuntimeError(
                            "Foreach expects array or object".into(),
                        ))
                    }
                }
            }

            OpCode::IterGetValRef(sym) => {
                // Stack: [Array, Index]
                let idx_handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let array_handle = self
                    .operand_stack
                    .peek_at(1)
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let idx = match self.arena.get(idx_handle).value {
                    Val::Int(i) => i as usize,
                    _ => return Err(VmError::RuntimeError("Iterator index must be int".into())),
                };

                // Check if we need to upgrade the element.
                let (needs_upgrade, val_handle) = {
                    let array_zval = self.arena.get(array_handle);
                    if let Val::Array(map) = &array_zval.value {
                        if let Some((_, h)) = map.map.get_index(idx) {
                            let is_ref = self.arena.get(*h).is_ref;
                            (!is_ref, *h)
                        } else {
                            return Err(VmError::RuntimeError(
                                "Iterator index out of bounds".into(),
                            ));
                        }
                    } else {
                        return Err(VmError::RuntimeError("IterGetValRef expects array".into()));
                    }
                };

                let final_handle = if needs_upgrade {
                    // Upgrade: Clone value, make ref, update array.
                    let val = self.arena.get(val_handle).value.clone();
                    let new_handle = self.arena.alloc(val);
                    self.arena.get_mut(new_handle).is_ref = true;

                    // Update array
                    let array_zval_mut = self.arena.get_mut(array_handle);
                    if let Val::Array(map) = &mut array_zval_mut.value {
                        if let Some((_, h_ref)) = Rc::make_mut(map).map.get_index_mut(idx) {
                            *h_ref = new_handle;
                        }
                    }
                    new_handle
                } else {
                    val_handle
                };

                let frame = self.frames.last_mut().unwrap();
                frame.locals.insert(sym, final_handle);
            }

            OpCode::IterGetKey(sym) => {
                // Stack: [Array, Index]
                let idx_handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let array_handle = self
                    .operand_stack
                    .peek_at(1)
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let idx = match self.arena.get(idx_handle).value {
                    Val::Int(i) => i as usize,
                    _ => return Err(VmError::RuntimeError("Iterator index must be int".into())),
                };

                let array_val = &self.arena.get(array_handle).value;
                match array_val {
                    Val::Array(map) => {
                        if let Some((key, _)) = map.map.get_index(idx) {
                            let key_val = match key {
                                ArrayKey::Int(i) => Val::Int(*i),
                                ArrayKey::Str(s) => Val::String(s.as_ref().clone().into()),
                            };
                            let key_handle = self.arena.alloc(key_val);

                            // Store in local
                            let frame = self.frames.last_mut().unwrap();
                            frame.locals.insert(sym, key_handle);
                        } else {
                            return Err(VmError::RuntimeError(
                                "Iterator index out of bounds".into(),
                            ));
                        }
                    }
                    Val::Object(payload_handle) => {
                        let payload = self.arena.get(*payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload.value {
                            let mut handled = false;
                            if let Some(internal) = &obj_data.internal {
                                if let Ok(gen_data) =
                                    internal.clone().downcast::<RefCell<GeneratorData>>()
                                {
                                    let data = gen_data.borrow();
                                    let key =
                                        data.current_key.unwrap_or(self.arena.alloc(Val::Null));
                                    let frame = self.frames.last_mut().unwrap();
                                    frame.locals.insert(sym, key);
                                    handled = true;
                                }
                            }

                            if !handled {
                                let iterator_sym = self.context.interner.intern(b"Iterator");
                                if self.is_instance_of(array_handle, iterator_sym) {
                                    let key_sym = self.context.interner.intern(b"key");
                                    let key_handle =
                                        self.call_native_method_simple(array_handle, key_sym)?;
                                    let frame = self.frames.last_mut().unwrap();
                                    frame.locals.insert(sym, key_handle);
                                    handled = true;
                                }
                            }

                            if !handled {
                                return Err(VmError::RuntimeError("Object not iterable".into()));
                            }
                        }
                    }
                    _ => {
                        return Err(VmError::RuntimeError(
                            "IterGetKey expects array or object".into(),
                        ))
                    }
                }
            }
            OpCode::FeResetR(target) => {
                let array_handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let array_val = &self.arena.get(array_handle).value;
                let len = match array_val {
                    Val::Array(map) => map.map.len(),
                    _ => return Err(VmError::RuntimeError("Foreach expects array".into())),
                };
                if len == 0 {
                    self.operand_stack.pop();
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = target as usize;
                } else {
                    let idx_handle = self.arena.alloc(Val::Int(0));
                    self.operand_stack.push(idx_handle);
                }
            }
            OpCode::FeFetchR(target) => {
                let idx_handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let array_handle = self
                    .operand_stack
                    .peek_at(1)
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let idx = match self.arena.get(idx_handle).value {
                    Val::Int(i) => i as usize,
                    _ => return Err(VmError::RuntimeError("Iterator index must be int".into())),
                };

                let array_val = &self.arena.get(array_handle).value;
                let len = match array_val {
                    Val::Array(map) => map.map.len(),
                    _ => return Err(VmError::RuntimeError("Foreach expects array".into())),
                };

                if idx >= len {
                    self.operand_stack.pop();
                    self.operand_stack.pop();
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = target as usize;
                } else {
                    if let Val::Array(map) = array_val {
                        if let Some((_, val_handle)) = map.map.get_index(idx) {
                            self.operand_stack.push(*val_handle);
                        }
                    }
                    self.arena.get_mut(idx_handle).value = Val::Int((idx + 1) as i64);
                }
            }
            OpCode::FeResetRw(target) => {
                // Same as FeResetR but intended for by-ref iteration. We share logic to avoid diverging behavior.
                let array_handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let array_val = &self.arena.get(array_handle).value;
                let len = match array_val {
                    Val::Array(map) => map.map.len(),
                    _ => return Err(VmError::RuntimeError("Foreach expects array".into())),
                };
                if len == 0 {
                    self.operand_stack.pop();
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = target as usize;
                } else {
                    let idx_handle = self.arena.alloc(Val::Int(0));
                    self.operand_stack.push(idx_handle);
                }
            }
            OpCode::FeFetchRw(target) => {
                // Mirrors FeFetchR but leaves the fetched handle intact for by-ref writes.
                let idx_handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let array_handle = self
                    .operand_stack
                    .peek_at(1)
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let idx = match self.arena.get(idx_handle).value {
                    Val::Int(i) => i as usize,
                    _ => return Err(VmError::RuntimeError("Iterator index must be int".into())),
                };

                let array_val = &self.arena.get(array_handle).value;
                let len = match array_val {
                    Val::Array(map) => map.map.len(),
                    _ => return Err(VmError::RuntimeError("Foreach expects array".into())),
                };

                if idx >= len {
                    self.operand_stack.pop();
                    self.operand_stack.pop();
                    let frame = self.frames.last_mut().unwrap();
                    frame.ip = target as usize;
                } else {
                    if let Val::Array(map) = array_val {
                        if let Some((_, val_handle)) = map.map.get_index(idx) {
                            self.operand_stack.push(*val_handle);
                        }
                    }
                    self.arena.get_mut(idx_handle).value = Val::Int((idx + 1) as i64);
                }
            }
            OpCode::FeFree => {
                self.operand_stack.pop();
                self.operand_stack.pop();
            }

            OpCode::DefClass(name, parent) => {
                let mut methods = HashMap::new();
                let mut abstract_methods = HashSet::new();

                if let Some(parent_sym) = parent {
                    if let Some(parent_def) = self.context.classes.get(&parent_sym) {
                        // Inherit methods, excluding private ones.
                        for (key, entry) in &parent_def.methods {
                            if entry.visibility != Visibility::Private {
                                methods.insert(*key, entry.clone());
                            }
                        }
                        
                        // Inherit abstract methods (excluding private ones)
                        for &abstract_method in &parent_def.abstract_methods {
                            if let Some(method_entry) = parent_def.methods.get(&abstract_method) {
                                if method_entry.visibility != Visibility::Private {
                                    abstract_methods.insert(abstract_method);
                                }
                            }
                        }
                    } else {
                        let parent_name = self
                            .context
                            .interner
                            .lookup(parent_sym)
                            .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
                            .unwrap_or_else(|| format!("{:?}", parent_sym));
                        return Err(VmError::RuntimeError(format!(
                            "Parent class {} not found",
                            parent_name
                        )));
                    }
                }

                let class_def = ClassDef {
                    name,
                    parent,
                    is_interface: false,
                    is_trait: false,
                    is_abstract: false,
                    is_enum: false,
                    enum_backed_type: None,
                    interfaces: Vec::new(),
                    traits: Vec::new(),
                    methods,
                    properties: IndexMap::new(),
                    constants: HashMap::new(),
                    static_properties: HashMap::new(),
                    abstract_methods,
                    allows_dynamic_properties: false,
                };
                self.context.classes.insert(name, class_def);
            }
            OpCode::DefInterface(name) => {
                let class_def = ClassDef {
                    name,
                    parent: None,
                    is_interface: true,
                    is_trait: false,
                    is_abstract: true,
                    is_enum: false,
                    enum_backed_type: None,
                    interfaces: Vec::new(),
                    traits: Vec::new(),
                    methods: HashMap::new(),
                    properties: IndexMap::new(),
                    constants: HashMap::new(),
                    static_properties: HashMap::new(),
                    abstract_methods: HashSet::new(),
                    allows_dynamic_properties: false,
                };
                self.context.classes.insert(name, class_def);
            }
            OpCode::DefTrait(name) => {
                let class_def = ClassDef {
                    name,
                    parent: None,
                    is_interface: false,
                    is_trait: true,
                    is_abstract: false,
                    is_enum: false,
                    enum_backed_type: None,
                    interfaces: Vec::new(),
                    traits: Vec::new(),
                    methods: HashMap::new(),
                    properties: IndexMap::new(),
                    constants: HashMap::new(),
                    static_properties: HashMap::new(),
                    abstract_methods: HashSet::new(),
                    allows_dynamic_properties: false,
                };
                self.context.classes.insert(name, class_def);
            }
            OpCode::AddInterface(class_name, interface_name) => {
                // Just add the interface - validation happens later in FinalizeClass
                if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                    class_def.interfaces.push(interface_name);
                }
            }
            OpCode::FinalizeClass(class_name) => {
                // Validate interface implementation after all methods are defined
                if let Some(class_def) = self.context.classes.get(&class_name) {
                    for &interface_name in &class_def.interfaces.clone() {
                        self.validate_interface_implementation(class_name, interface_name)?;
                    }
                    
                    // Validate abstract method implementation
                    if !class_def.is_abstract {
                        self.validate_abstract_methods_implemented(class_name)?;
                    }
                }
            }
            OpCode::AllowDynamicProperties(class_name) => {
                if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                    class_def.allows_dynamic_properties = true;
                }
            }
            OpCode::MarkAbstract(class_name) => {
                if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                    class_def.is_abstract = true;
                }
            }
            OpCode::UseTrait(class_name, trait_name) => {
                let trait_methods = if let Some(trait_def) = self.context.classes.get(&trait_name) {
                    if !trait_def.is_trait {
                        return Err(VmError::RuntimeError("Not a trait".into()));
                    }
                    trait_def.methods.clone()
                } else {
                    return Err(VmError::RuntimeError("Trait not found".into()));
                };

                // Collect information about already-used traits BEFORE the mutable borrow
                let existing_traits_and_methods: Vec<(Symbol, Vec<Symbol>)> = if let Some(class_def) = self.context.classes.get(&class_name) {
                    class_def.traits.iter()
                        .filter_map(|&used_trait| {
                            self.context.classes.get(&used_trait).map(|used_trait_def| {
                                let methods: Vec<Symbol> = used_trait_def.methods.keys().copied().collect();
                                (used_trait, methods)
                            })
                        })
                        .collect()
                } else {
                    Vec::new()
                };

                if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                    class_def.traits.push(trait_name);
                    
                    // Track conflicts for error reporting
                    let mut conflicts = Vec::new();
                    
                    for (key, mut entry) in trait_methods {
                        // Check for conflicts with existing methods from other traits
                        let mut is_from_other_trait = false;
                        let mut conflicting_traits = Vec::new();
                        
                        for (used_trait, methods) in &existing_traits_and_methods {
                            if methods.contains(&key) {
                                is_from_other_trait = true;
                                let used_trait_str = self.context.interner.lookup(*used_trait)
                                    .map(|b| String::from_utf8_lossy(b).to_string())
                                    .unwrap_or_else(|| format!("{:?}", used_trait));
                                conflicting_traits.push(used_trait_str);
                            }
                        }
                        
                        if is_from_other_trait {
                            // This is a conflict between traits
                            let method_name_str = self.context.interner.lookup(key)
                                .map(|b| String::from_utf8_lossy(b).to_string())
                                .unwrap_or_else(|| format!("{:?}", key));
                            let trait_name_str = self.context.interner.lookup(trait_name)
                                .map(|b| String::from_utf8_lossy(b).to_string())
                                .unwrap_or_else(|| format!("{:?}", trait_name));
                            
                            conflicts.push((method_name_str, conflicting_traits, trait_name_str));
                            continue; // Don't insert the conflicting method
                        }
                        
                        // When using a trait, the methods become part of the class.
                        // The declaring class becomes the class using the trait (effectively).
                        entry.declaring_class = class_name;
                        class_def.methods.entry(key).or_insert(entry);
                    }
                    
                    // Report conflicts if any
                    if !conflicts.is_empty() {
                        let class_name_str = self.context.interner.lookup(class_name)
                            .map(|b| String::from_utf8_lossy(b).to_string())
                            .unwrap_or_else(|| format!("{:?}", class_name));
                        
                        let conflict_msgs: Vec<String> = conflicts.iter()
                            .map(|(method, existing_traits, new_trait)| {
                                format!(
                                    "Trait method {}::{} has not been applied as {}::{} has the same name in {}",
                                    new_trait,
                                    method,
                                    existing_traits.join(" and "),
                                    method,
                                    class_name_str
                                )
                            })
                            .collect();
                        
                        return Err(VmError::RuntimeError(conflict_msgs.join("; ")));
                    }
                }
            }
            OpCode::DefMethod(class_name, method_name, func_idx, visibility, is_static, is_abstract) => {
                let val = {
                    let frame = self.frames.last().unwrap();
                    frame.chunk.constants[func_idx as usize].clone()
                };
                if let Val::Resource(rc) = val {
                    if let Ok(func) = rc.downcast::<UserFunc>() {
                        let lower_key = self.intern_lowercase_symbol(method_name)?;
                        
                        // Build signature from UserFunc data
                        let signature = MethodSignature {
                            parameters: func.params.iter().map(|p| ParameterInfo {
                                name: p.name,
                                type_hint: p.param_type.clone().and_then(|rt| self.return_type_to_type_hint(&rt)),
                                is_reference: p.by_ref,
                                is_variadic: p.is_variadic,
                                default_value: p.default_value.clone(),
                            }).collect(),
                            return_type: func.return_type.as_ref().and_then(|rt| self.return_type_to_type_hint(rt)),
                        };
                        
                        // Check if parent class has this method - validate override compatibility
                        if let Some(class_def) = self.context.classes.get(&class_name) {
                            if let Some(parent_sym) = class_def.parent {
                                if let Some((parent_method, parent_vis, parent_static, _)) = 
                                    self.find_method(parent_sym, lower_key)
                                {
                                    self.validate_method_override(
                                        class_name,
                                        method_name,
                                        &signature,
                                        is_static,
                                        visibility,
                                        &parent_method,
                                        parent_static,
                                        parent_vis,
                                    )?;
                                }
                            }
                        }
                        
                        if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                            let entry = MethodEntry {
                                name: method_name,
                                func,
                                visibility,
                                is_static,
                                declaring_class: class_name,
                                is_abstract,
                                signature,
                            };
                            class_def.methods.insert(lower_key, entry.clone());
                            
                            // Track abstract methods or remove from inherited abstract methods if implemented
                            if is_abstract {
                                class_def.abstract_methods.insert(lower_key);
                            } else {
                                // If this method implements an inherited abstract method, remove it
                                class_def.abstract_methods.remove(&lower_key);
                            }
                        }
                    }
                }
            }
            OpCode::DefProp(class_name, prop_name, default_idx, visibility, type_hint_idx, is_readonly) => {
                let val = {
                    let frame = self.frames.last().unwrap();
                    frame.chunk.constants[default_idx as usize].clone()
                };
                let type_hint = {
                    let frame = self.frames.last().unwrap();
                    let hint_val = &frame.chunk.constants[type_hint_idx as usize];
                    if let Val::Resource(rc) = hint_val {
                        rc.downcast_ref::<ReturnType>().and_then(|rt| self.return_type_to_type_hint(rt))
                    } else {
                        None
                    }
                };
                if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                    class_def.properties.insert(prop_name, PropertyEntry {
                        default_value: val,
                        visibility,
                        type_hint,
                        is_readonly,
                    });
                }
            }
            OpCode::DefClassConst(class_name, const_name, val_idx, visibility) => {
                let val = {
                    let frame = self.frames.last().unwrap();
                    frame.chunk.constants[val_idx as usize].clone()
                };
                if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                    class_def.constants.insert(const_name, (val, visibility));
                }
            }
            OpCode::DefGlobalConst(name, val_idx) => {
                let val = {
                    let frame = self.frames.last().unwrap();
                    frame.chunk.constants[val_idx as usize].clone()
                };
                self.context.constants.insert(name, val);
            }
            OpCode::FetchGlobalConst(name) => {
                if let Some(val) = self.context.constants.get(&name) {
                    let handle = self.arena.alloc(val.clone());
                    self.operand_stack.push(handle);
                } else {
                    // PHP 8.x: Undefined constant throws Error (not Warning)
                    let name_bytes = self.context.interner.lookup(name).unwrap_or(b"???");
                    let name_str = String::from_utf8_lossy(name_bytes);
                    return Err(VmError::RuntimeError(format!(
                        "Undefined constant \"{}\"",
                        name_str
                    )));
                }
            }
            OpCode::DefStaticProp(class_name, prop_name, default_idx, visibility, type_hint_idx) => {
                let val = {
                    let frame = self.frames.last().unwrap();
                    frame.chunk.constants[default_idx as usize].clone()
                };
                let type_hint = {
                    let frame = self.frames.last().unwrap();
                    let hint_val = &frame.chunk.constants[type_hint_idx as usize];
                    if let Val::Resource(rc) = hint_val {
                        rc.downcast_ref::<ReturnType>().and_then(|rt| self.return_type_to_type_hint(rt))
                    } else {
                        None
                    }
                };
                if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                    class_def.static_properties.insert(prop_name, StaticPropertyEntry {
                        value: val,
                        visibility,
                        type_hint,
                    });
                }
            }
            OpCode::FetchClassConst(class_name, const_name) => {
                let resolved_class = self.resolve_class_name(class_name)?;
                let (val, visibility, defining_class) =
                    self.find_class_constant(resolved_class, const_name)?;
                self.check_const_visibility(defining_class, visibility)?;
                let handle = self.arena.alloc(val);
                self.operand_stack.push(handle);
            }
            OpCode::FetchClassConstDynamic(const_name) => {
                let class_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let class_val = self.arena.get(class_handle).value.clone();

                let class_name_sym = match class_val {
                    Val::Object(h) => {
                        if let Val::ObjPayload(data) = &self.arena.get(h).value {
                            data.class
                        } else {
                            return Err(VmError::RuntimeError("Invalid object payload".into()));
                        }
                    }
                    Val::String(s) => self.context.interner.intern(&s),
                    _ => {
                        return Err(VmError::RuntimeError(
                            "Class constant fetch on non-class".into(),
                        ))
                    }
                };

                let resolved_class = self.resolve_class_name(class_name_sym)?;
                let (val, visibility, defining_class) =
                    self.find_class_constant(resolved_class, const_name)?;
                self.check_const_visibility(defining_class, visibility)?;
                let handle = self.arena.alloc(val);
                self.operand_stack.push(handle);
            }
            OpCode::FetchStaticProp(class_name, prop_name) => {
                let resolved_class = self.resolve_class_name(class_name)?;
                let (val, visibility, defining_class) =
                    self.find_static_prop(resolved_class, prop_name)?;
                self.check_const_visibility(defining_class, visibility)?;
                let handle = self.deep_clone_val(&val);
                self.operand_stack.push(handle);
            }
            OpCode::AssignStaticProp(class_name, prop_name) => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let resolved_class = self.resolve_class_name(class_name)?;
                let (_, visibility, defining_class) =
                    self.find_static_prop(resolved_class, prop_name)?;
                self.check_const_visibility(defining_class, visibility)?;

                // Validate and potentially coerce static property type
                self.validate_static_property_type(defining_class, prop_name, val_handle)?;

                // Get the (possibly coerced) value
                let val = self.arena.get(val_handle).value.clone();

                if let Some(class_def) = self.context.classes.get_mut(&defining_class) {
                    if let Some(entry) = class_def.static_properties.get_mut(&prop_name) {
                        entry.value = val.clone();
                    }
                }

                let res_handle = self.arena.alloc(val);
                self.operand_stack.push(res_handle);
            }
            OpCode::AssignStaticPropRef => {
                let ref_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let prop_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let class_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let class_name = match &self.arena.get(class_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                };

                let prop_name = match &self.arena.get(prop_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };

                // Ensure value is a reference
                self.arena.get_mut(ref_handle).is_ref = true;
                let val = self.arena.get(ref_handle).value.clone();

                let resolved_class = self.resolve_class_name(class_name)?;
                let (_, visibility, defining_class) =
                    self.find_static_prop(resolved_class, prop_name)?;
                self.check_const_visibility(defining_class, visibility)?;

                if let Some(class_def) = self.context.classes.get_mut(&defining_class) {
                    if let Some(entry) = class_def.static_properties.get_mut(&prop_name) {
                        entry.value = val.clone();
                    }
                }

                self.operand_stack.push(ref_handle);
            }
            OpCode::FetchStaticPropR
            | OpCode::FetchStaticPropW
            | OpCode::FetchStaticPropRw
            | OpCode::FetchStaticPropIs
            | OpCode::FetchStaticPropFuncArg
            | OpCode::FetchStaticPropUnset => {
                let prop_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let class_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let class_name = match &self.arena.get(class_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                };

                let prop_name = match &self.arena.get(prop_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };

                let resolved_class = self.resolve_class_name(class_name)?;
                let (val, visibility, defining_class) =
                    self.find_static_prop(resolved_class, prop_name)?;
                self.check_const_visibility(defining_class, visibility)?;

                let handle = self.arena.alloc(val);
                self.operand_stack.push(handle);
            }
            OpCode::New(class_name, arg_count) => {
                // Resolve special class names (self, parent, static)
                let resolved_class = self.resolve_class_name(class_name)?;

                // Try autoloading if class doesn't exist
                if !self.context.classes.contains_key(&resolved_class) {
                    self.trigger_autoload(resolved_class)?;
                }

                // Check if class is abstract or interface
                if let Some(class_def) = self.context.classes.get(&resolved_class) {
                    if class_def.is_abstract && !class_def.is_interface {
                        let class_name_str = self.context.interner.lookup(resolved_class)
                            .map(|b| String::from_utf8_lossy(b).to_string())
                            .unwrap_or_else(|| format!("{:?}", resolved_class));
                        return Err(VmError::RuntimeError(format!(
                            "Cannot instantiate abstract class {}",
                            class_name_str
                        )));
                    }
                    if class_def.is_interface {
                        let class_name_str = self.context.interner.lookup(resolved_class)
                            .map(|b| String::from_utf8_lossy(b).to_string())
                            .unwrap_or_else(|| format!("{:?}", resolved_class));
                        return Err(VmError::RuntimeError(format!(
                            "Cannot instantiate interface {}",
                            class_name_str
                        )));
                    }
                }

                if self.context.classes.contains_key(&resolved_class) {
                    let properties =
                        self.collect_properties(resolved_class, PropertyCollectionMode::All);

                    let obj_data = ObjectData {
                        class: resolved_class,
                        properties,
                        internal: None,
                        dynamic_properties: std::collections::HashSet::new(),
                    };

                    let payload_handle = self.arena.alloc(Val::ObjPayload(obj_data));
                    let obj_val = Val::Object(payload_handle);
                    let obj_handle = self.arena.alloc(obj_val);

                    // Check for constructor
                    let constructor_name = self.context.interner.intern(b"__construct");
                    let mut method_lookup = self.find_method(resolved_class, constructor_name);

                    if method_lookup.is_none() {
                        if let Some(scope) = self.get_current_class() {
                            if let Some((func, vis, is_static, decl_class)) =
                                self.find_method(scope, constructor_name)
                            {
                                if vis == Visibility::Private && decl_class == scope {
                                    method_lookup = Some((func, vis, is_static, decl_class));
                                }
                            }
                        }
                    }

                    if let Some((constructor, vis, _, defined_class)) = method_lookup {
                        // Check visibility
                        match vis {
                            Visibility::Public => {}
                            Visibility::Private => {
                                let current_class = self.get_current_class();
                                if current_class != Some(defined_class) {
                                    return Err(VmError::RuntimeError(
                                        "Cannot call private constructor".into(),
                                    ));
                                }
                            }
                            Visibility::Protected => {
                                let current_class = self.get_current_class();
                                if let Some(scope) = current_class {
                                    if !self.is_subclass_of(scope, defined_class)
                                        && !self.is_subclass_of(defined_class, scope)
                                    {
                                        return Err(VmError::RuntimeError(
                                            "Cannot call protected constructor".into(),
                                        ));
                                    }
                                } else {
                                    return Err(VmError::RuntimeError(
                                        "Cannot call protected constructor".into(),
                                    ));
                                }
                            }
                        }

                        // Collect args
                        let mut frame = CallFrame::new(constructor.chunk.clone());
                        frame.func = Some(constructor.clone());
                        frame.this = Some(obj_handle);
                        frame.is_constructor = true;
                        frame.class_scope = Some(defined_class);
                        frame.args = self.collect_call_args(arg_count)?;
                        self.push_frame(frame);
                    } else {
                        // Check for native constructor
                        let native_constructor =
                            self.find_native_method(resolved_class, constructor_name);
                        if let Some(native_entry) = native_constructor {
                            // Call native constructor
                            let args = self.collect_call_args(arg_count)?;

                            // Set this in current frame temporarily
                            let saved_this = self.frames.last().and_then(|f| f.this);
                            if let Some(frame) = self.frames.last_mut() {
                                frame.this = Some(obj_handle);
                            }

                            // Call native handler
                            let _result = (native_entry.handler)(self, &args)
                                .map_err(VmError::RuntimeError)?;

                            // Restore previous this
                            if let Some(frame) = self.frames.last_mut() {
                                frame.this = saved_this;
                            }

                            self.operand_stack.push(obj_handle);
                        } else {
                            // No constructor found
                            // For built-in exception/error classes, accept args silently (they have implicit constructors)
                            let is_builtin_exception = {
                                let class_name_bytes =
                                    self.context.interner.lookup(resolved_class).unwrap_or(b"");
                                matches!(
                                    class_name_bytes,
                                    b"Exception"
                                        | b"Error"
                                        | b"Throwable"
                                        | b"RuntimeException"
                                        | b"LogicException"
                                        | b"TypeError"
                                        | b"ArithmeticError"
                                        | b"DivisionByZeroError"
                                        | b"ParseError"
                                        | b"ArgumentCountError"
                                )
                            };

                            if arg_count > 0 && !is_builtin_exception {
                                let class_name_bytes = self
                                    .context
                                    .interner
                                    .lookup(resolved_class)
                                    .unwrap_or(b"<unknown>");
                                let class_name_str = String::from_utf8_lossy(class_name_bytes);
                                return Err(VmError::RuntimeError(format!("Class {} does not have a constructor, so you cannot pass any constructor arguments", class_name_str).into()));
                            }

                            // Discard constructor arguments for built-in exceptions
                            for _ in 0..arg_count {
                                self.operand_stack.pop();
                            }

                            self.operand_stack.push(obj_handle);
                        }
                    }
                } else {
                    return Err(VmError::RuntimeError("Class not found".into()));
                }
            }
            OpCode::NewDynamic(arg_count) => {
                // Collect args first
                let args = self.collect_call_args(arg_count)?;

                let class_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let class_name = match &self.arena.get(class_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                };

                if self.context.classes.contains_key(&class_name) {
                    let properties =
                        self.collect_properties(class_name, PropertyCollectionMode::All);

                    let obj_data = ObjectData {
                        class: class_name,
                        properties,
                        internal: None,
                        dynamic_properties: std::collections::HashSet::new(),
                    };

                    let payload_handle = self.arena.alloc(Val::ObjPayload(obj_data));
                    let obj_val = Val::Object(payload_handle);
                    let obj_handle = self.arena.alloc(obj_val);

                    // Check for constructor
                    let constructor_name = self.context.interner.intern(b"__construct");
                    let mut method_lookup = self.find_method(class_name, constructor_name);

                    if method_lookup.is_none() {
                        if let Some(scope) = self.get_current_class() {
                            if let Some((func, vis, is_static, decl_class)) =
                                self.find_method(scope, constructor_name)
                            {
                                if vis == Visibility::Private && decl_class == scope {
                                    method_lookup = Some((func, vis, is_static, decl_class));
                                }
                            }
                        }
                    }

                    if let Some((constructor, vis, _, defined_class)) = method_lookup {
                        // Check visibility
                        match vis {
                            Visibility::Public => {}
                            Visibility::Private => {
                                let current_class = self.get_current_class();
                                if current_class != Some(defined_class) {
                                    return Err(VmError::RuntimeError(
                                        "Cannot call private constructor".into(),
                                    ));
                                }
                            }
                            Visibility::Protected => {
                                let current_class = self.get_current_class();
                                if let Some(scope) = current_class {
                                    if !self.is_subclass_of(scope, defined_class)
                                        && !self.is_subclass_of(defined_class, scope)
                                    {
                                        return Err(VmError::RuntimeError(
                                            "Cannot call protected constructor".into(),
                                        ));
                                    }
                                } else {
                                    return Err(VmError::RuntimeError(
                                        "Cannot call protected constructor".into(),
                                    ));
                                }
                            }
                        }

                        let mut frame = CallFrame::new(constructor.chunk.clone());
                        frame.func = Some(constructor.clone());
                        frame.this = Some(obj_handle);
                        frame.is_constructor = true;
                        frame.class_scope = Some(defined_class);
                        frame.args = args;
                        self.push_frame(frame);
                    } else {
                        if arg_count > 0 {
                            let class_name_bytes = self
                                .context
                                .interner
                                .lookup(class_name)
                                .unwrap_or(b"<unknown>");
                            let class_name_str = String::from_utf8_lossy(class_name_bytes);
                            return Err(VmError::RuntimeError(format!("Class {} does not have a constructor, so you cannot pass any constructor arguments", class_name_str).into()));
                        }
                        self.operand_stack.push(obj_handle);
                    }
                } else {
                    return Err(VmError::RuntimeError("Class not found".into()));
                }
            }
            OpCode::FetchProp(prop_name) => {
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                // Extract needed data to avoid holding borrow
                let (class_name, prop_handle_opt) = {
                    let obj_zval = self.arena.get(obj_handle);
                    if let Val::Object(payload_handle) = obj_zval.value {
                        let payload_zval = self.arena.get(payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload_zval.value {
                            (obj_data.class, obj_data.properties.get(&prop_name).copied())
                        } else {
                            return Err(VmError::RuntimeError("Invalid object payload".into()));
                        }
                    } else {
                        return Err(VmError::RuntimeError(
                            "Attempt to fetch property on non-object".into(),
                        ));
                    }
                };

                // Check visibility
                let current_scope = self.get_current_class();
                let visibility_check =
                    self.check_prop_visibility(class_name, prop_name, current_scope);

                let mut use_magic = false;

                if let Some(prop_handle) = prop_handle_opt {
                    if visibility_check.is_ok() {
                        self.operand_stack.push(prop_handle);
                    } else {
                        use_magic = true;
                    }
                } else {
                    use_magic = true;
                }

                if use_magic {
                    let magic_get = self.context.interner.intern(b"__get");
                    if let Some((method, _, _, defined_class)) =
                        self.find_method(class_name, magic_get)
                    {
                        let prop_name_bytes = self
                            .context
                            .interner
                            .lookup(prop_name)
                            .unwrap_or(b"")
                            .to_vec();
                        let name_handle = self.arena.alloc(Val::String(prop_name_bytes.into()));

                        let mut frame = CallFrame::new(method.chunk.clone());
                        frame.func = Some(method.clone());
                        frame.this = Some(obj_handle);
                        frame.class_scope = Some(defined_class);
                        frame.called_scope = Some(class_name);

                        if let Some(param) = method.params.get(0) {
                            frame.locals.insert(param.name, name_handle);
                        }

                        self.push_frame(frame);
                    } else {
                        if let Err(e) = visibility_check {
                            return Err(e);
                        }
                        let null = self.arena.alloc(Val::Null);
                        self.operand_stack.push(null);
                    }
                }
            }
            OpCode::FetchPropDynamic => {
                let name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let name_val = &self.arena.get(name_handle).value;
                let prop_name = match name_val {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };

                // Extract needed data to avoid holding borrow
                let (class_name, prop_handle_opt) = {
                    let obj_zval = self.arena.get(obj_handle);
                    if let Val::Object(payload_handle) = obj_zval.value {
                        let payload_zval = self.arena.get(payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload_zval.value {
                            (obj_data.class, obj_data.properties.get(&prop_name).copied())
                        } else {
                            return Err(VmError::RuntimeError("Invalid object payload".into()));
                        }
                    } else {
                        return Err(VmError::RuntimeError(
                            "Attempt to fetch property on non-object".into(),
                        ));
                    }
                };

                // Check visibility
                let current_scope = self.get_current_class();
                let visibility_check =
                    self.check_prop_visibility(class_name, prop_name, current_scope);

                let mut use_magic = false;

                if let Some(prop_handle) = prop_handle_opt {
                    if visibility_check.is_ok() {
                        self.operand_stack.push(prop_handle);
                    } else {
                        use_magic = true;
                    }
                } else {
                    use_magic = true;
                }

                if use_magic {
                    let magic_get = self.context.interner.intern(b"__get");
                    if let Some((method, _, _, defined_class)) =
                        self.find_method(class_name, magic_get)
                    {
                        let prop_name_bytes = self
                            .context
                            .interner
                            .lookup(prop_name)
                            .unwrap_or(b"")
                            .to_vec();
                        let name_handle = self.arena.alloc(Val::String(prop_name_bytes.into()));

                        let mut frame = CallFrame::new(method.chunk.clone());
                        frame.func = Some(method.clone());
                        frame.this = Some(obj_handle);
                        frame.class_scope = Some(defined_class);
                        frame.called_scope = Some(class_name);

                        if let Some(param) = method.params.get(0) {
                            frame.locals.insert(param.name, name_handle);
                        }

                        self.push_frame(frame);
                    } else {
                        if let Err(e) = visibility_check {
                            return Err(e);
                        }
                        let null = self.arena.alloc(Val::Null);
                        self.operand_stack.push(null);
                    }
                }
            }
            OpCode::AssignProp(prop_name) => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let payload_handle = if let Val::Object(h) = self.arena.get(obj_handle).value {
                    h
                } else {
                    return Err(VmError::RuntimeError(
                        "Attempt to assign property on non-object".into(),
                    ));
                };

                // Extract data
                let (class_name, prop_exists) = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        (obj_data.class, obj_data.properties.contains_key(&prop_name))
                    } else {
                        return Err(VmError::RuntimeError("Invalid object payload".into()));
                    }
                };

                let current_scope = self.get_current_class();
                let visibility_check =
                    self.check_prop_visibility(class_name, prop_name, current_scope);

                let mut use_magic = false;

                if prop_exists {
                    if visibility_check.is_err() {
                        use_magic = true;
                    }
                } else {
                    use_magic = true;
                }

                if use_magic {
                    let magic_set = self.context.interner.intern(b"__set");
                    if let Some((method, _, _, defined_class)) =
                        self.find_method(class_name, magic_set)
                    {
                        let prop_name_bytes = self
                            .context
                            .interner
                            .lookup(prop_name)
                            .unwrap_or(b"")
                            .to_vec();
                        let name_handle = self.arena.alloc(Val::String(prop_name_bytes.into()));

                        let mut frame = CallFrame::new(method.chunk.clone());
                        frame.func = Some(method.clone());
                        frame.this = Some(obj_handle);
                        frame.class_scope = Some(defined_class);
                        frame.called_scope = Some(class_name);
                        frame.discard_return = true;

                        if let Some(param) = method.params.get(0) {
                            frame.locals.insert(param.name, name_handle);
                        }
                        if let Some(param) = method.params.get(1) {
                            frame.locals.insert(param.name, val_handle);
                        }

                        self.operand_stack.push(val_handle);
                        self.push_frame(frame);
                                    } else {
                                        if let Err(e) = visibility_check {
                                            return Err(e);
                                        }
                    
                                        // Check for dynamic property deprecation (PHP 8.2+)
                                        if !prop_exists {
                                            self.check_dynamic_property_write(obj_handle, prop_name);
                                        }
                    
                                        // Check readonly constraint
                                        let prop_info = self.walk_inheritance_chain(class_name, |def, cls| {
                                            def.properties.get(&prop_name).map(|entry| (entry.is_readonly, cls))
                                        });
                    
                                        if let Some((is_readonly, defining_class)) = prop_info {
                                            if is_readonly {
                                                // Check if already initialized in object
                                                let payload_zval = self.arena.get(payload_handle);
                                                if let Val::ObjPayload(obj_data) = &payload_zval.value {
                                                    if let Some(current_handle) = obj_data.properties.get(&prop_name) {
                                                        let current_val = &self.arena.get(*current_handle).value;
                                                        if !matches!(current_val, Val::Uninitialized) {
                                                            let class_str = String::from_utf8_lossy(
                                                                self.context.interner.lookup(defining_class).unwrap_or(b"???")
                                                            );
                                                            let prop_str = String::from_utf8_lossy(
                                                                self.context.interner.lookup(prop_name).unwrap_or(b"???")
                                                            );
                                                            return Err(VmError::RuntimeError(format!(
                                                                "Cannot modify readonly property {}::${}", 
                                                                class_str, prop_str
                                                            )));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                    
                                        // Validate property type (check class definition for type hint)
                                        self.validate_property_type(class_name, prop_name, val_handle)?;
                    
                                        let payload_zval = self.arena.get_mut(payload_handle);
                                        if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                                            obj_data.properties.insert(prop_name, val_handle);
                                        }
                                        self.operand_stack.push(val_handle);
                                    }                } else {
                    // Check for dynamic property deprecation (PHP 8.2+)
                    if !prop_exists {
                        self.check_dynamic_property_write(obj_handle, prop_name);
                    }

                    // Validate property type (check class definition for type hint)
                    self.validate_property_type(class_name, prop_name, val_handle)?;

                    let payload_zval = self.arena.get_mut(payload_handle);
                    if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                        obj_data.properties.insert(prop_name, val_handle);
                    } else {
                        return Err(VmError::RuntimeError("Invalid object payload".into()));
                    }
                    self.operand_stack.push(val_handle);
                }
            }
            OpCode::CallMethod(method_name, arg_count) => {
                self.exec_call_method(method_name, arg_count, false)?;
            }
            OpCode::CallMethodDynamic(arg_count) => {
                let method_name_handle = self
                    .operand_stack
                    .peek_at(arg_count as usize)
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let method_name_bytes = self.convert_to_string(method_name_handle)?;
                let method_name = self.context.interner.intern(&method_name_bytes);
                self.exec_call_method(method_name, arg_count, true)?;
            }
            OpCode::UnsetObj => {
                let prop_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let prop_name = match &self.arena.get(prop_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                // Extract data to avoid borrow issues
                let (class_name, should_unset) = {
                    let obj_zval = self.arena.get(obj_handle);
                    if let Val::Object(payload_handle) = obj_zval.value {
                        let payload_zval = self.arena.get(payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload_zval.value {
                            let current_scope = self.get_current_class();
                            if self
                                .check_prop_visibility(obj_data.class, prop_name, current_scope)
                                .is_ok()
                            {
                                if obj_data.properties.contains_key(&prop_name) {
                                    (obj_data.class, true)
                                } else {
                                    (obj_data.class, false) // Not found
                                }
                            } else {
                                (obj_data.class, false) // Not accessible
                            }
                        } else {
                            return Err(VmError::RuntimeError("Invalid object payload".into()));
                        }
                    } else {
                        return Err(VmError::RuntimeError(
                            "Attempt to unset property on non-object".into(),
                        ));
                    }
                };

                if should_unset {
                    let payload_handle = if let Val::Object(h) = self.arena.get(obj_handle).value {
                        h
                    } else {
                        unreachable!()
                    };
                    let payload_zval = self.arena.get_mut(payload_handle);
                    if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                        obj_data.properties.swap_remove(&prop_name);
                    }
                } else {
                    // Property not found or not accessible. Check for __unset.
                    let unset_magic = self.context.interner.intern(b"__unset");
                    if let Some((magic_func, _, _, magic_class)) =
                        self.find_method(class_name, unset_magic)
                    {
                        // Found __unset

                        // Create method name string (prop name)
                        let prop_name_str = self
                            .context
                            .interner
                            .lookup(prop_name)
                            .expect("Prop name should be interned")
                            .to_vec();
                        let name_handle = self.arena.alloc(Val::String(prop_name_str.into()));

                        // Prepare frame for __unset
                        let mut frame = CallFrame::new(magic_func.chunk.clone());
                        frame.func = Some(magic_func.clone());
                        frame.this = Some(obj_handle);
                        frame.class_scope = Some(magic_class);
                        frame.called_scope = Some(class_name);
                        frame.discard_return = true; // Discard return value

                        // Param 0: name
                        if let Some(param) = magic_func.params.get(0) {
                            frame.locals.insert(param.name, name_handle);
                        }

                        self.push_frame(frame);
                    }
                    // If no __unset, do nothing (standard PHP behavior)
                }
            }
            OpCode::UnsetStaticProp => {
                let prop_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let prop_name = match &self.arena.get(prop_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };
                let class_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let class_name = match &self.arena.get(class_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                };

                // We need to find where it is defined to unset it?
                // Or does unset static prop only work if it's accessible?
                // In PHP, `unset(Foo::$prop)` unsets it.
                // But static properties are shared. Unsetting it might mean setting it to NULL or removing it?
                // Actually, you cannot unset static properties in PHP.
                // `unset(Foo::$prop)` results in "Attempt to unset static property".
                // Wait, let me check PHP behavior.
                // `class A { public static $a = 1; } unset(A::$a);` -> Error: Attempt to unset static property
                // So this opcode might be for internal use or I should throw error?
                // But `ZEND_UNSET_STATIC_PROP` exists.
                // Maybe it is used for `unset($a::$b)`?
                // If PHP throws error, I should throw error.

                let class_str = String::from_utf8_lossy(
                    self.context.interner.lookup(class_name).unwrap_or(b"?"),
                );
                let prop_str = String::from_utf8_lossy(
                    self.context.interner.lookup(prop_name).unwrap_or(b"?"),
                );
                return Err(VmError::RuntimeError(format!(
                    "Attempt to unset static property {}::${}",
                    class_str, prop_str
                )));
            }
            OpCode::FetchThis => {
                let frame = self
                    .frames
                    .last()
                    .ok_or(VmError::RuntimeError("No active frame".into()))?;
                if let Some(this_handle) = frame.this {
                    self.operand_stack.push(this_handle);
                } else {
                    return Err(VmError::RuntimeError(
                        "Using $this when not in object context".into(),
                    ));
                }
            }
            OpCode::FetchGlobals => {
                let mut map = IndexMap::new();
                for (sym, handle) in &self.context.globals {
                    let key_bytes = self.context.interner.lookup(*sym).unwrap_or(b"").to_vec();
                    map.insert(ArrayKey::Str(Rc::new(key_bytes)), *handle);
                }
                let arr_handle = self.arena.alloc(Val::Array(ArrayData::from(map).into()));
                self.operand_stack.push(arr_handle);
            }
            OpCode::IncludeOrEval => {
                let type_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let path_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let path_val = &self.arena.get(path_handle).value;
                let path_str = match path_val {
                    Val::String(s) => String::from_utf8_lossy(s).to_string(),
                    _ => return Err(VmError::RuntimeError("Include path must be string".into())),
                };

                let type_val = &self.arena.get(type_handle).value;
                let include_type = match type_val {
                    Val::Int(i) => *i,
                    _ => return Err(VmError::RuntimeError("Include type must be int".into())),
                };

                // Zend constants (enum, not bit flags): ZEND_EVAL=1, ZEND_INCLUDE=2, ZEND_INCLUDE_ONCE=3, ZEND_REQUIRE=4, ZEND_REQUIRE_ONCE=5

                if include_type == 1 {
                    // Eval
                    // PHP's eval() assumes code is in PHP mode (no <?php tag required)
                    // Wrap the code in PHP tags for the parser
                    let mut wrapped_source = b"<?php ".to_vec();
                    wrapped_source.extend_from_slice(path_str.as_bytes());

                    let arena = bumpalo::Bump::new();
                    let lexer = php_parser::lexer::Lexer::new(&wrapped_source);
                    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
                    let program = parser.parse_program();

                    if !program.errors.is_empty() {
                        // Eval error: in PHP 7+ throws ParseError
                        return Err(VmError::RuntimeError(format!(
                            "Eval parse errors: {:?}",
                            program.errors
                        )));
                    }

                    // Get caller's strict_types to inherit
                    // The caller is the current frame that's executing this OpCode
                    let caller_strict = self.frames.last()
                        .map(|f| f.chunk.strict_types)
                        .unwrap_or(false);

                    let emitter = crate::compiler::emitter::Emitter::new(
                        &wrapped_source,
                        &mut self.context.interner,
                    )
                    .with_inherited_strict_types(caller_strict);
                    let (chunk, _) = emitter.compile(program.statements);

                    let caller_frame_idx = self.frames.len() - 1;
                    let mut frame = CallFrame::new(Rc::new(chunk));
                    if let Some(caller) = self.frames.get(caller_frame_idx) {
                        frame.locals = caller.locals.clone();
                        frame.this = caller.this;
                        frame.class_scope = caller.class_scope;
                        frame.called_scope = caller.called_scope;
                    }

                    self.push_frame(frame);
                    let depth = self.frames.len();
                    let stack_before_eval = self.operand_stack.len();

                    // Execute eval'd code (inline run_loop to capture locals before pop)
                    let mut eval_error = None;
                    let mut last_eval_locals = None;
                    loop {
                        // Capture locals on each iteration in case Return pops the frame
                        if self.frames.len() >= depth {
                            last_eval_locals = Some(self.frames[depth - 1].locals.clone());
                        }

                        if self.frames.len() < depth {
                            break;
                        }
                        if self.frames.len() == depth {
                            let frame = &self.frames[depth - 1];
                            if frame.ip >= frame.chunk.code.len() {
                                break;
                            }
                        }

                        let op = {
                            let frame = self.current_frame_mut()?;
                            if frame.ip >= frame.chunk.code.len() {
                                self.frames.pop();
                                break;
                            }
                            let op = frame.chunk.code[frame.ip];
                            frame.ip += 1;
                            op
                        };

                        if let Err(e) = self.execute_opcode(op, depth) {
                            eval_error = Some(e);
                            break;
                        }
                    }

                    // Use the last captured locals
                    let final_locals = last_eval_locals;

                    // Pop eval frame if still on stack
                    if self.frames.len() >= depth {
                        self.frames.pop();
                    }

                    // Copy modified locals back to caller (eval shares caller's symbol table)
                    if let Some(locals) = final_locals {
                        if let Some(caller) = self.frames.get_mut(caller_frame_idx) {
                            caller.locals = locals;
                        }
                    }

                    if let Some(err) = eval_error {
                        return Err(err);
                    }

                    // If eval code had an explicit return, handle_return pushed the value onto the stack.
                    // If not, we need to push NULL (PHP's eval() returns NULL when no explicit return).
                    if self.operand_stack.len() == stack_before_eval {
                        let null_val = self.arena.alloc(Val::Null);
                        self.operand_stack.push(null_val);
                    }
                } else {
                    // File include/require (types 2, 3, 4, 5)
                    let is_once = include_type == 3 || include_type == 5; // include_once/require_once
                    let is_require = include_type == 4 || include_type == 5; // require/require_once

                    let resolved_path = self.resolve_script_path(&path_str)?;
                    let canonical_path = Self::canonical_path_string(&resolved_path);
                    let already_included = self.context.included_files.contains(&canonical_path);

                    if self.trace_includes {
                        eprintln!(
                            "[php-vm] include {:?} -> {} (once={}, already_included={})",
                            path_str,
                            resolved_path.display(),
                            is_once,
                            already_included
                        );
                    }

                    if is_once && already_included {
                        // _once variant already included: return true
                        let true_val = self.arena.alloc(Val::Bool(true));
                        self.operand_stack.push(true_val);
                    } else {
                        let inserted_once_guard = if is_once && !already_included {
                            self.context.included_files.insert(canonical_path.clone());
                            true
                        } else {
                            false
                        };

                        let source_res = std::fs::read(&resolved_path);
                        match source_res {
                            Ok(source) => {
                                let arena = bumpalo::Bump::new();
                                let lexer = php_parser::lexer::Lexer::new(&source);
                                let mut parser = php_parser::parser::Parser::new(lexer, &arena);
                                let program = parser.parse_program();

                                if !program.errors.is_empty() {
                                    if inserted_once_guard {
                                        self.context.included_files.remove(&canonical_path);
                                    }
                                    return Err(VmError::RuntimeError(format!(
                                        "Parse errors in {}: {:?}",
                                        path_str, program.errors
                                    )));
                                }

                                let emitter = crate::compiler::emitter::Emitter::new(
                                    &source,
                                    &mut self.context.interner,
                                )
                                .with_file_path(canonical_path.clone());
                                let (chunk, _) = emitter.compile(program.statements);

                                let caller_frame_idx = self.frames.len() - 1;
                                let mut frame = CallFrame::new(Rc::new(chunk));
                                // Include inherits full scope
                                if let Some(caller) = self.frames.get(caller_frame_idx) {
                                    frame.locals = caller.locals.clone();
                                    frame.this = caller.this;
                                    frame.class_scope = caller.class_scope;
                                    frame.called_scope = caller.called_scope;
                                }

                                self.push_frame(frame);
                                let depth = self.frames.len();
                                let target_depth = depth - 1; // Target is caller's depth

                                // Execute included file (inline run_loop to capture locals before pop)
                                let mut include_error = None;
                                loop {
                                    if self.frames.len() < depth {
                                        break;
                                    }
                                    if self.frames.len() == depth {
                                        let frame = &self.frames[depth - 1];
                                        if frame.ip >= frame.chunk.code.len() {
                                            break;
                                        }
                                    }

                                    let op = {
                                        let frame = self.current_frame_mut()?;
                                        if frame.ip >= frame.chunk.code.len() {
                                            self.frames.pop();
                                            break;
                                        }
                                        let op = frame.chunk.code[frame.ip];
                                        frame.ip += 1;
                                        op
                                    };

                                    if let Err(e) = self.execute_opcode(op, target_depth) {
                                        include_error = Some(e);
                                        break;
                                    }
                                }

                                // Capture included frame's final locals before popping
                                let final_locals = if self.frames.len() >= depth {
                                    Some(self.frames[depth - 1].locals.clone())
                                } else {
                                    None
                                };

                                // Pop include frame if still on stack
                                if self.frames.len() >= depth {
                                    self.frames.pop();
                                }

                                // Copy modified locals back to caller
                                if let Some(locals) = final_locals {
                                    if let Some(caller) = self.frames.get_mut(caller_frame_idx) {
                                        caller.locals = locals;
                                    }
                                }

                                if let Some(err) = include_error {
                                    if inserted_once_guard {
                                        self.context.included_files.remove(&canonical_path);
                                    }
                                    return Err(err);
                                }

                                // Include returns explicit return value or 1
                                let return_val = self
                                    .last_return_value
                                    .unwrap_or_else(|| self.arena.alloc(Val::Int(1)));
                                self.last_return_value = None;
                                self.operand_stack.push(return_val);
                            }
                            Err(e) => {
                                if inserted_once_guard {
                                    self.context.included_files.remove(&canonical_path);
                                }
                                if is_require {
                                    return Err(VmError::RuntimeError(format!(
                                        "Require failed: {}",
                                        e
                                    )));
                                } else {
                                    let msg = format!(
                                        "include({}): Failed to open stream: {}",
                                        path_str, e
                                    );
                                    self.report_error(ErrorLevel::Warning, &msg);
                                    let false_val = self.arena.alloc(Val::Bool(false));
                                    self.operand_stack.push(false_val);
                                }
                            }
                        }
                    }
                }
            }
            OpCode::FetchR(sym) => {
                let frame = self
                    .frames
                    .last_mut()
                    .ok_or(VmError::RuntimeError("No active frame".into()))?;
                if let Some(handle) = frame.locals.get(&sym) {
                    self.operand_stack.push(*handle);
                } else {
                    let var_name = String::from_utf8_lossy(
                        self.context.interner.lookup(sym).unwrap_or(b"unknown"),
                    );
                    let msg = format!("Undefined variable: ${}", var_name);
                    self.report_error(ErrorLevel::Notice, &msg);
                    let null = self.arena.alloc(Val::Null);
                    self.operand_stack.push(null);
                }
            }
            OpCode::FetchW(sym) | OpCode::FetchFuncArg(sym) => {
                let frame = self
                    .frames
                    .last_mut()
                    .ok_or(VmError::RuntimeError("No active frame".into()))?;
                if let Some(handle) = frame.locals.get(&sym) {
                    self.operand_stack.push(*handle);
                } else {
                    let null = self.arena.alloc(Val::Null);
                    frame.locals.insert(sym, null);
                    self.operand_stack.push(null);
                }
            }
            OpCode::FetchRw(sym) => {
                let frame = self
                    .frames
                    .last_mut()
                    .ok_or(VmError::RuntimeError("No active frame".into()))?;
                if let Some(handle) = frame.locals.get(&sym) {
                    self.operand_stack.push(*handle);
                } else {
                    // Release the mutable borrow before calling report_error
                    let null = self.arena.alloc(Val::Null);
                    let var_name = String::from_utf8_lossy(
                        self.context.interner.lookup(sym).unwrap_or(b"unknown"),
                    );
                    let msg = format!("Undefined variable: ${}", var_name);
                    self.error_handler.report(ErrorLevel::Notice, &msg);
                    frame.locals.insert(sym, null);
                    self.operand_stack.push(null);
                }
            }
            OpCode::FetchIs(sym) | OpCode::FetchUnset(sym) | OpCode::CheckFuncArg(sym) => {
                let frame = self
                    .frames
                    .last_mut()
                    .ok_or(VmError::RuntimeError("No active frame".into()))?;
                if let Some(handle) = frame.locals.get(&sym) {
                    self.operand_stack.push(*handle);
                } else {
                    let null = self.arena.alloc(Val::Null);
                    self.operand_stack.push(null);
                }
            }
            OpCode::FetchConstant(sym) => {
                if let Some(val) = self.context.constants.get(&sym) {
                    let handle = self.arena.alloc(val.clone());
                    self.operand_stack.push(handle);
                } else {
                    let name =
                        String::from_utf8_lossy(self.context.interner.lookup(sym).unwrap_or(b""));
                    return Err(VmError::RuntimeError(format!(
                        "Undefined constant '{}'",
                        name
                    )));
                }
            }
            OpCode::InitNsFcallByName | OpCode::InitFcallByName => {
                let name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let name_val = self.arena.get(name_handle);
                let name_sym = match &name_val.value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Function name must be string".into())),
                };

                self.pending_calls.push(PendingCall {
                    func_name: Some(name_sym),
                    func_handle: None,
                    args: ArgList::new(),
                    is_static: false,
                    class_name: None,
                    this_handle: None,
                });
            }
            OpCode::InitFcall | OpCode::InitUserCall => {
                let name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let name_val = self.arena.get(name_handle);
                let name_sym = match &name_val.value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Function name must be string".into())),
                };

                self.pending_calls.push(PendingCall {
                    func_name: Some(name_sym),
                    func_handle: None,
                    args: ArgList::new(),
                    is_static: false,
                    class_name: None,
                    this_handle: None,
                });
            }
            OpCode::InitDynamicCall => {
                let callable_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let callable_val = self.arena.get(callable_handle).value.clone();
                match callable_val {
                    Val::String(s) => {
                        let sym = self.context.interner.intern(&s);
                        self.pending_calls.push(PendingCall {
                            func_name: Some(sym),
                            func_handle: Some(callable_handle),
                            args: ArgList::new(),
                            is_static: false,
                            class_name: None,
                            this_handle: None,
                        });
                    }
                    Val::Object(payload_handle) => {
                        let payload_val = self.arena.get(payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload_val.value {
                            let invoke = self.context.interner.intern(b"__invoke");
                            self.pending_calls.push(PendingCall {
                                func_name: Some(invoke),
                                func_handle: Some(callable_handle),
                                args: ArgList::new(),
                                is_static: false,
                                class_name: Some(obj_data.class),
                                this_handle: Some(callable_handle),
                            });
                        } else {
                            return Err(VmError::RuntimeError(
                                "Dynamic call expects callable object".into(),
                            ));
                        }
                    }
                    _ => {
                        return Err(VmError::RuntimeError(
                            "Dynamic call expects string or object".into(),
                        ))
                    }
                }
            }
            OpCode::SendVarEx
            | OpCode::SendVarNoRefEx
            | OpCode::SendVarNoRef
            | OpCode::SendValEx
            | OpCode::SendFuncArg => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let call = self
                    .pending_calls
                    .last_mut()
                    .ok_or(VmError::RuntimeError("No pending call".into()))?;
                call.args.push(val_handle);
            }
            OpCode::SendArray | OpCode::SendUser => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let call = self
                    .pending_calls
                    .last_mut()
                    .ok_or(VmError::RuntimeError("No pending call".into()))?;
                call.args.push(val_handle);
            }
            OpCode::SendUnpack => {
                let array_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let call = self
                    .pending_calls
                    .last_mut()
                    .ok_or(VmError::RuntimeError("No pending call".into()))?;
                let arr_val = self.arena.get(array_handle);
                if let Val::Array(map) = &arr_val.value {
                    for (_, handle) in map.map.iter() {
                        call.args.push(*handle);
                    }
                } else {
                    return Err(VmError::RuntimeError(
                        "Argument unpack expects array".into(),
                    ));
                }
            }
            OpCode::DoFcall | OpCode::DoFcallByName | OpCode::DoIcall | OpCode::DoUcall => {
                let call = self
                    .pending_calls
                    .pop()
                    .ok_or(VmError::RuntimeError("No pending call".into()))?;
                self.execute_pending_call(call)?;
            }
            OpCode::ExtStmt | OpCode::ExtFcallBegin | OpCode::ExtFcallEnd | OpCode::ExtNop => {
                // No-op for now
            }
            OpCode::FetchListW => {
                let dim = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let container_handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?; // Peek container

                // We need mutable access to container if we want to create references?
                // But we only peek.
                // If we want to return a reference to an element, we need to ensure the element exists and is a reference?
                // Or just return the handle.

                // For now, same as FetchListR but maybe we should ensure it's a reference?
                // In PHP, list(&$a) = $arr;
                // The element in $arr must be referenceable.

                let container = &self.arena.get(container_handle).value;

                match container {
                    Val::Array(map) => {
                        let key = match &self.arena.get(dim).value {
                            Val::Int(i) => ArrayKey::Int(*i),
                            Val::String(s) => ArrayKey::Str(s.clone()),
                            _ => ArrayKey::Str(Rc::new(Vec::<u8>::new())),
                        };

                        if let Some(val_handle) = map.map.get(&key) {
                            self.operand_stack.push(*val_handle);
                        } else {
                            let null = self.arena.alloc(Val::Null);
                            self.operand_stack.push(null);
                        }
                    }
                    _ => {
                        let null = self.arena.alloc(Val::Null);
                        self.operand_stack.push(null);
                    }
                }
            }
            OpCode::FetchListR => {
                let dim = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let container_handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?; // Peek container

                let container = &self.arena.get(container_handle).value;

                match container {
                    Val::Array(map) => {
                        let key = match &self.arena.get(dim).value {
                            Val::Int(i) => ArrayKey::Int(*i),
                            Val::String(s) => ArrayKey::Str(s.clone()),
                            _ => ArrayKey::Str(Rc::new(Vec::<u8>::new())),
                        };

                        if let Some(val_handle) = map.map.get(&key) {
                            self.operand_stack.push(*val_handle);
                        } else {
                            let null = self.arena.alloc(Val::Null);
                            self.operand_stack.push(null);
                        }
                    }
                    _ => {
                        let null = self.arena.alloc(Val::Null);
                        self.operand_stack.push(null);
                    }
                }
            }
            OpCode::FetchDimR | OpCode::FetchDimIs | OpCode::FetchDimUnset => {
                let dim = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let container_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let container = &self.arena.get(container_handle).value;
                let is_fetch_r = matches!(op, OpCode::FetchDimR);
                let _is_unset = matches!(op, OpCode::FetchDimUnset);

                match container {
                    Val::Array(map) => {
                        // Proper key conversion following PHP semantics
                        // Reference: $PHP_SRC_PATH/Zend/zend_operators.c - convert_to_array_key
                        let dim_val = &self.arena.get(dim).value;
                        let key = self.array_key_from_value(dim_val)?;

                        if let Some(val_handle) = map.map.get(&key) {
                            self.operand_stack.push(*val_handle);
                        } else {
                            // Emit notice for FetchDimR, but not for isset/empty (FetchDimIs) or unset
                            if is_fetch_r {
                                let key_str = match &key {
                                    ArrayKey::Int(i) => i.to_string(),
                                    ArrayKey::Str(s) => String::from_utf8_lossy(s).to_string(),
                                };
                                self.report_error(
                                    ErrorLevel::Notice,
                                    &format!("Undefined array key \"{}\"", key_str),
                                );
                            }
                            let null = self.arena.alloc(Val::Null);
                            self.operand_stack.push(null);
                        }
                    }
                    Val::String(s) => {
                        // String offset access
                        // Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_fetch_dimension_address_read_R
                        let dim_val = &self.arena.get(dim).value;

                        // Convert offset to integer (PHP coerces any type to int for string offsets)
                        let offset = dim_val.to_int();

                        // Handle negative offsets (count from end)
                        // Reference: PHP 7.1+ supports negative string offsets
                        let len = s.len() as i64;
                        let actual_offset = if offset < 0 {
                            // Negative offset: count from end
                            let adjusted = len + offset;
                            if adjusted < 0 {
                                // Still out of bounds even after adjustment
                                if is_fetch_r {
                                    self.report_error(
                                        ErrorLevel::Warning,
                                        &format!("Uninitialized string offset {}", offset),
                                    );
                                }
                                let empty = self.arena.alloc(Val::String(vec![].into()));
                                self.operand_stack.push(empty);
                                return Ok(());
                            }
                            adjusted as usize
                        } else {
                            offset as usize
                        };

                        if actual_offset < s.len() {
                            let char_str = vec![s[actual_offset]];
                            let val = self.arena.alloc(Val::String(char_str.into()));
                            self.operand_stack.push(val);
                        } else {
                            if is_fetch_r {
                                self.report_error(
                                    ErrorLevel::Warning,
                                    &format!("Uninitialized string offset {}", offset),
                                );
                            }
                            let empty = self.arena.alloc(Val::String(vec![].into()));
                            self.operand_stack.push(empty);
                        }
                    }
                    Val::Bool(_) | Val::Int(_) | Val::Float(_) | Val::Resource(_) => {
                        // PHP 7.4+: Trying to use scalar types as arrays produces a warning
                        // Reference: $PHP_SRC_PATH/Zend/zend_execute.c
                        if is_fetch_r {
                            let type_str = container.type_name();
                            self.report_error(
                                ErrorLevel::Warning,
                                &format!(
                                    "Trying to access array offset on value of type {}",
                                    type_str
                                ),
                            );
                        }
                        let null = self.arena.alloc(Val::Null);
                        self.operand_stack.push(null);
                    }
                    Val::Null => {
                        // Accessing offset on null: Warning in FetchDimR, silent for isset
                        if is_fetch_r {
                            self.report_error(
                                ErrorLevel::Warning,
                                "Trying to access array offset on value of type null",
                            );
                        }
                        let null = self.arena.alloc(Val::Null);
                        self.operand_stack.push(null);
                    }
                    &Val::Object(_) | &Val::ObjPayload(_) => {
                        // Check if object implements ArrayAccess interface
                        // Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_FETCH_DIM_R_SPEC
                        let class_name = match container {
                            &Val::Object(payload_handle) => {
                                let payload = self.arena.get(payload_handle);
                                if let Val::ObjPayload(obj_data) = &payload.value {
                                    Some(obj_data.class)
                                } else {
                                    None
                                }
                            }
                            &Val::ObjPayload(ref obj_data) => Some(obj_data.class),
                            _ => None,
                        };

                        if let Some(cls) = class_name {
                            if self.implements_array_access(cls) {
                                // Call ArrayAccess::offsetGet($offset)
                                match self.call_array_access_offset_get(container_handle, dim) {
                                    Ok(result) => {
                                        self.operand_stack.push(result);
                                    }
                                    Err(e) => return Err(e),
                                }
                            } else {
                                // Object doesn't implement ArrayAccess
                                if is_fetch_r {
                                    self.report_error(
                                        ErrorLevel::Warning,
                                        "Trying to access array offset on value of type object",
                                    );
                                }
                                let null = self.arena.alloc(Val::Null);
                                self.operand_stack.push(null);
                            }
                        } else {
                            // Invalid object structure
                            if is_fetch_r {
                                self.report_error(
                                    ErrorLevel::Warning,
                                    "Trying to access array offset on value of type object",
                                );
                            }
                            let null = self.arena.alloc(Val::Null);
                            self.operand_stack.push(null);
                        }
                    }
                    _ => {
                        if is_fetch_r {
                            let type_str = container.type_name();
                            self.report_error(
                                ErrorLevel::Warning,
                                &format!(
                                    "Trying to access array offset on value of type {}",
                                    type_str
                                ),
                            );
                        }
                        let null = self.arena.alloc(Val::Null);
                        self.operand_stack.push(null);
                    }
                }
            }
            OpCode::FetchDimW | OpCode::FetchDimRw | OpCode::FetchDimFuncArg => {
                let dim = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let container_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                // 1. Resolve key
                let key = match &self.arena.get(dim).value {
                    Val::Int(i) => ArrayKey::Int(*i),
                    Val::String(s) => ArrayKey::Str(s.clone()),
                    _ => ArrayKey::Str(Rc::new(Vec::<u8>::new())),
                };

                // 2. Check if we need to insert (Immutable check)
                let needs_insert = {
                    let container = &self.arena.get(container_handle).value;
                    match container {
                        Val::Null => true,
                        Val::Array(map) => !map.map.contains_key(&key),
                        _ => {
                            return Err(VmError::RuntimeError(
                                "Cannot use [] for reading/writing on non-array".into(),
                            ))
                        }
                    }
                };

                if needs_insert {
                    // 3. Alloc new value
                    let val_handle = self.arena.alloc(Val::Null);

                    // 4. Modify container
                    let container = &mut self.arena.get_mut(container_handle).value;
                    if let Val::Null = container {
                        *container = Val::Array(ArrayData::new().into());
                    }

                    if let Val::Array(map) = container {
                        Rc::make_mut(map).insert(key, val_handle);
                        self.operand_stack.push(val_handle);
                    } else {
                        // Should not happen due to check above
                        return Err(VmError::RuntimeError("Container is not an array".into()));
                    }
                } else {
                    // 5. Get existing value
                    let container = &self.arena.get(container_handle).value;
                    if let Val::Array(map) = container {
                        let val_handle = map.map.get(&key).unwrap();
                        self.operand_stack.push(*val_handle);
                    } else {
                        return Err(VmError::RuntimeError("Container is not an array".into()));
                    }
                }
            }
            OpCode::FetchObjR | OpCode::FetchObjIs | OpCode::FetchObjUnset => {
                let prop = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let prop_name = match &self.arena.get(prop).value {
                    Val::String(s) => s.clone(),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };

                let obj = &self.arena.get(obj_handle).value;
                if let Val::Object(obj_data_handle) = obj {
                    let sym = self.context.interner.intern(&prop_name);

                    // Extract class name and check property
                    let (class_name, prop_handle_opt, has_prop) = {
                        let payload = self.arena.get(*obj_data_handle);
                        if let Val::ObjPayload(data) = &payload.value {
                            (
                                data.class,
                                data.properties.get(&sym).copied(),
                                data.properties.contains_key(&sym),
                            )
                        } else {
                            let null = self.arena.alloc(Val::Null);
                            self.operand_stack.push(null);
                            return Ok(());
                        }
                    };

                    // Check visibility
                    let current_scope = self.get_current_class();
                    let visibility_ok = has_prop
                        && self
                            .check_prop_visibility(class_name, sym, current_scope)
                            .is_ok();

                    if let Some(val_handle) = prop_handle_opt {
                        if visibility_ok {
                            self.operand_stack.push(val_handle);
                        } else {
                            // Try __get for inaccessible property
                            let magic_get = self.context.interner.intern(b"__get");
                            if let Some((method, _, _, defined_class)) =
                                self.find_method(class_name, magic_get)
                            {
                                let name_handle = self.arena.alloc(Val::String(prop_name.clone()));

                                let mut frame = CallFrame::new(method.chunk.clone());
                                frame.func = Some(method.clone());
                                frame.this = Some(obj_handle);
                                frame.class_scope = Some(defined_class);
                                frame.called_scope = Some(class_name);

                                if let Some(param) = method.params.get(0) {
                                    frame.locals.insert(param.name, name_handle);
                                }

                                self.push_frame(frame);
                            } else {
                                let null = self.arena.alloc(Val::Null);
                                self.operand_stack.push(null);
                            }
                        }
                    } else {
                        // Property doesn't exist, try __get
                        let magic_get = self.context.interner.intern(b"__get");
                        if let Some((method, _, _, defined_class)) =
                            self.find_method(class_name, magic_get)
                        {
                            let name_handle = self.arena.alloc(Val::String(prop_name));

                            let mut frame = CallFrame::new(method.chunk.clone());
                            frame.func = Some(method.clone());
                            frame.this = Some(obj_handle);
                            frame.class_scope = Some(defined_class);
                            frame.called_scope = Some(class_name);

                            if let Some(param) = method.params.get(0) {
                                frame.locals.insert(param.name, name_handle);
                            }

                            self.push_frame(frame);
                        } else {
                            let null = self.arena.alloc(Val::Null);
                            self.operand_stack.push(null);
                        }
                    }
                } else {
                    let null = self.arena.alloc(Val::Null);
                    self.operand_stack.push(null);
                }
            }
            OpCode::FetchObjW | OpCode::FetchObjRw | OpCode::FetchObjFuncArg => {
                let prop = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let prop_name = match &self.arena.get(prop).value {
                    Val::String(s) => s.clone(),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };

                let sym = self.context.interner.intern(&prop_name);

                // 1. Check object handle (Immutable)
                let obj_data_handle_opt = {
                    let obj = &self.arena.get(obj_handle).value;
                    match obj {
                        Val::Object(h) => Some(*h),
                        Val::Null => None,
                        _ => {
                            return Err(VmError::RuntimeError(
                                "Attempt to assign property of non-object".into(),
                            ))
                        }
                    }
                };

                if let Some(handle) = obj_data_handle_opt {
                    // 2. Alloc new value (if needed, or just alloc null)
                    let null_handle = self.arena.alloc(Val::Null);

                    // 3. Modify payload
                    let payload = &mut self.arena.get_mut(handle).value;
                    if let Val::ObjPayload(data) = payload {
                        if !data.properties.contains_key(&sym) {
                            data.properties.insert(sym, null_handle);
                        }
                        let val_handle = data.properties.get(&sym).unwrap();
                        self.operand_stack.push(*val_handle);
                    } else {
                        return Err(VmError::RuntimeError("Invalid object payload".into()));
                    }
                } else {
                    // Auto-vivify
                    return Err(VmError::RuntimeError(
                        "Creating default object from empty value not fully implemented".into(),
                    ));
                }
            }
            OpCode::FuncNumArgs => {
                let frame = self
                    .frames
                    .last()
                    .ok_or(VmError::RuntimeError("No active frame".into()))?;
                let count = frame.args.len();
                let handle = self.arena.alloc(Val::Int(count as i64));
                self.operand_stack.push(handle);
            }
            OpCode::FuncGetArgs => {
                let frame = self
                    .frames
                    .last()
                    .ok_or(VmError::RuntimeError("No active frame".into()))?;
                let mut map = IndexMap::new();
                for (i, handle) in frame.args.iter().enumerate() {
                    map.insert(ArrayKey::Int(i as i64), *handle);
                }
                let handle = self.arena.alloc(Val::Array(ArrayData::from(map).into()));
                self.operand_stack.push(handle);
            }
            OpCode::InitMethodCall => {
                let name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let name_val = self.arena.get(name_handle);
                let name_sym = match &name_val.value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Method name must be string".into())),
                };

                self.pending_calls.push(PendingCall {
                    func_name: Some(name_sym),
                    func_handle: None,
                    args: ArgList::new(),
                    is_static: false,
                    class_name: None, // Will be resolved from object
                    this_handle: Some(obj_handle),
                });

                let obj_val = self.arena.get(obj_handle);
                if let Val::Object(payload_handle) = obj_val.value {
                    let payload = self.arena.get(payload_handle);
                    if let Val::ObjPayload(data) = &payload.value {
                        let class_name = data.class;
                        let call = self.pending_calls.last_mut().unwrap();
                        call.class_name = Some(class_name);
                    }
                } else {
                    return Err(VmError::RuntimeError(
                        "Call to a member function on a non-object".into(),
                    ));
                }
            }
            OpCode::InitStaticMethodCall => {
                let name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let class_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let name_val = self.arena.get(name_handle);
                let name_sym = match &name_val.value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Method name must be string".into())),
                };

                let class_val = self.arena.get(class_handle);
                let class_sym = match &class_val.value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                };

                let resolved_class = self.resolve_class_name(class_sym)?;

                self.pending_calls.push(PendingCall {
                    func_name: Some(name_sym),
                    func_handle: None,
                    args: ArgList::new(),
                    is_static: true,
                    class_name: Some(resolved_class),
                    this_handle: None,
                });
            }
            OpCode::IssetIsemptyVar => {
                let type_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let type_val = match self.arena.get(type_handle).value {
                    Val::Int(i) => i,
                    _ => 0, // Default to isset
                };

                let name_sym = match &self.arena.get(name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Variable name must be string".into())),
                };

                let frame = self
                    .frames
                    .last()
                    .ok_or(VmError::RuntimeError("No active frame".into()))?;
                let exists = frame.locals.contains_key(&name_sym);
                let val_handle = if exists {
                    frame.locals.get(&name_sym).cloned()
                } else {
                    None
                };

                let result = if type_val == 0 {
                    // isset returns true if var exists and is not null
                    if let Some(h) = val_handle {
                        !matches!(self.arena.get(h).value, Val::Null)
                    } else {
                        false
                    }
                } else {
                    // empty returns true if var does not exist or is falsey
                    if let Some(h) = val_handle {
                        let val = &self.arena.get(h).value;
                        match val {
                            Val::Null => true,
                            Val::Bool(b) => !b,
                            Val::Int(i) => *i == 0,
                            Val::Float(f) => *f == 0.0,
                            Val::String(s) => s.is_empty() || s.as_slice() == b"0",
                            Val::Array(a) => a.map.is_empty(),
                            _ => false,
                        }
                    } else {
                        true
                    }
                };

                let res_handle = self.arena.alloc(Val::Bool(result));
                self.operand_stack.push(res_handle);
            }
            OpCode::IssetIsemptyDimObj => {
                let type_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let dim_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let container_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let type_val = match self.arena.get(type_handle).value {
                    Val::Int(i) => i,
                    _ => 0,
                };

                // Pre-check: extract object class and check ArrayAccess
                // before doing any operation to avoid borrow issues
                let (is_object, is_array_access, class_name) = {
                    match &self.arena.get(container_handle).value {
                        Val::Object(payload_handle) => {
                            let payload = self.arena.get(*payload_handle);
                            if let Val::ObjPayload(obj_data) = &payload.value {
                                let cn = obj_data.class;
                                let is_aa = self.implements_array_access(cn);
                                (true, is_aa, cn)
                            } else {
                                // Invalid object payload - should not happen
                                return Err(VmError::RuntimeError("Invalid object payload".into()));
                            }
                        }
                        _ => (false, false, self.context.interner.intern(b"")),
                    }
                };

                // Check for ArrayAccess objects first
                // Reference: PHP Zend/zend_execute.c - ZEND_ISSET_ISEMPTY_DIM_OBJ handler
                // For objects: must implement ArrayAccess, otherwise fatal error
                let val_handle = if is_object {
                    if is_array_access {
                        // Handle ArrayAccess
                        // isset: only calls offsetExists
                        // empty: calls offsetExists, if true then calls offsetGet to check emptiness
                        match self.call_array_access_offset_exists(container_handle, dim_handle) {
                            Ok(exists) => {
                                if !exists {
                                    // offsetExists returned false
                                    None
                                } else if type_val == 0 {
                                    // isset: offsetExists returned true, so isset is true
                                    // BUT we still need to get the value to check if it's null
                                    match self
                                        .call_array_access_offset_get(container_handle, dim_handle)
                                    {
                                        Ok(h) => Some(h),
                                        Err(_) => None,
                                    }
                                } else {
                                    // empty: need to check the actual value via offsetGet
                                    match self
                                        .call_array_access_offset_get(container_handle, dim_handle)
                                    {
                                        Ok(h) => Some(h),
                                        Err(_) => None,
                                    }
                                }
                            }
                            Err(_) => None,
                        }
                    } else {
                        // Non-ArrayAccess object used as array - fatal error
                        let class_name_str = String::from_utf8_lossy(
                            self.context
                                .interner
                                .lookup(class_name)
                                .unwrap_or(b"Unknown"),
                        );
                        return Err(VmError::RuntimeError(format!(
                            "Cannot use object of type {} as array",
                            class_name_str
                        )));
                    }
                } else {
                    // Handle non-object types
                    let container = &self.arena.get(container_handle).value;
                    match container {
                        Val::Array(map) => {
                            let key = match &self.arena.get(dim_handle).value {
                                Val::Int(i) => ArrayKey::Int(*i),
                                Val::String(s) => ArrayKey::Str(s.clone()),
                                _ => ArrayKey::Str(Rc::new(Vec::<u8>::new())),
                            };
                            map.map.get(&key).cloned()
                        }
                        Val::String(s) => {
                            // String offset access for isset/empty
                            let offset = self.arena.get(dim_handle).value.to_int();
                            let len = s.len() as i64;

                            // Handle negative offsets (PHP 7.1+)
                            let actual_offset = if offset < 0 {
                                let adjusted = len + offset;
                                if adjusted < 0 {
                                    None // Out of bounds
                                } else {
                                    Some(adjusted as usize)
                                }
                            } else {
                                Some(offset as usize)
                            };

                            // For strings, if offset is valid, create a temp string value
                            if let Some(idx) = actual_offset {
                                if idx < s.len() {
                                    let char_val = vec![s[idx]];
                                    Some(self.arena.alloc(Val::String(char_val.into())))
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        }
                        Val::Null | Val::Bool(_) | Val::Int(_) | Val::Float(_) => {
                            // Trying to use isset/empty on scalar as array
                            // PHP returns false/true respectively without error (warning only in some cases)
                            None
                        }
                        _ => None,
                    }
                };

                let result = if type_val == 0 {
                    // Isset
                    if let Some(h) = val_handle {
                        !matches!(self.arena.get(h).value, Val::Null)
                    } else {
                        false
                    }
                } else {
                    // Empty
                    if let Some(h) = val_handle {
                        let val = &self.arena.get(h).value;
                        match val {
                            Val::Null => true,
                            Val::Bool(b) => !b,
                            Val::Int(i) => *i == 0,
                            Val::Float(f) => *f == 0.0,
                            Val::String(s) => s.is_empty() || s.as_slice() == b"0",
                            Val::Array(a) => a.map.is_empty(),
                            _ => false,
                        }
                    } else {
                        true
                    }
                };

                let res_handle = self.arena.alloc(Val::Bool(result));
                self.operand_stack.push(res_handle);
            }
            OpCode::IssetIsemptyPropObj => {
                // Same as DimObj but specifically for properties?
                // In Zend, ISSET_ISEMPTY_PROP_OBJ is for properties.
                // ISSET_ISEMPTY_DIM_OBJ is for dimensions (arrays/ArrayAccess).
                // But here I merged logic in DimObj above.
                // Let's just delegate to DimObj logic or copy it.
                // For now, I'll copy the logic but enforce Object check.

                let type_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let prop_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let container_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let type_val = match self.arena.get(type_handle).value {
                    Val::Int(i) => i,
                    _ => 0,
                };

                let container = &self.arena.get(container_handle).value;

                // Check for __isset first
                let (val_handle_opt, should_check_isset_magic) = match container {
                    Val::Object(obj_handle) => {
                        let prop_name = match &self.arena.get(prop_handle).value {
                            Val::String(s) => s.clone(),
                            _ => vec![].into(),
                        };
                        if prop_name.is_empty() {
                            (None, false)
                        } else {
                            let sym = self.context.interner.intern(&prop_name);
                            let (class_name, has_prop, prop_val_opt) = {
                                let payload = self.arena.get(*obj_handle);
                                if let Val::ObjPayload(data) = &payload.value {
                                    (
                                        data.class,
                                        data.properties.contains_key(&sym),
                                        data.properties.get(&sym).cloned(),
                                    )
                                } else {
                                    (self.context.interner.intern(b""), false, None)
                                }
                            };

                            let current_scope = self.get_current_class();
                            let visibility_ok = has_prop
                                && self
                                    .check_prop_visibility(class_name, sym, current_scope)
                                    .is_ok();

                            if has_prop && visibility_ok {
                                (prop_val_opt, false)
                            } else {
                                // Property doesn't exist or is inaccessible - check for __isset
                                (None, true)
                            }
                        }
                    }
                    _ => (None, false),
                };

                let val_handle = if should_check_isset_magic {
                    // Try __isset
                    if let Val::Object(obj_handle) = container {
                        let prop_name = match &self.arena.get(prop_handle).value {
                            Val::String(s) => s.clone(),
                            _ => vec![].into(),
                        };

                        let class_name = {
                            let payload = self.arena.get(*obj_handle);
                            if let Val::ObjPayload(data) = &payload.value {
                                data.class
                            } else {
                                self.context.interner.intern(b"")
                            }
                        };

                        let magic_isset = self.context.interner.intern(b"__isset");
                        let name_handle = self.arena.alloc(Val::String(prop_name.clone()));

                        // Save caller's return value to avoid corruption
                        let saved_return_value = self.last_return_value.take();

                        // Call __isset synchronously
                        let isset_result = self.call_magic_method_sync(
                            container_handle,
                            class_name,
                            magic_isset,
                            vec![name_handle],
                        )?;

                        // Restore caller's return value
                        self.last_return_value = saved_return_value;

                        // For isset (type_val==0): return __isset's boolean result directly
                        // For empty (type_val==1): call __get to get the actual value if __isset returns true
                        if let Some(result_handle) = isset_result {
                            let isset_bool = self.arena.get(result_handle).value.to_bool();
                            if type_val == 0 {
                                // isset(): just use __isset's result
                                // Create a dummy non-null value to make isset return isset_bool
                                if isset_bool {
                                    Some(self.arena.alloc(Val::Int(1))) // Any non-null value
                                } else {
                                    None
                                }
                            } else {
                                // empty(): need to call __get if __isset returned true
                                if isset_bool {
                                    let magic_get = self.context.interner.intern(b"__get");

                                    // Save and restore return value again for __get
                                    let saved_return_value2 = self.last_return_value.take();
                                    let get_result = self.call_magic_method_sync(
                                        container_handle,
                                        class_name,
                                        magic_get,
                                        vec![name_handle],
                                    )?;
                                    self.last_return_value = saved_return_value2;

                                    get_result
                                } else {
                                    None
                                }
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    val_handle_opt
                };

                let result = if type_val == 0 {
                    // Isset
                    if let Some(h) = val_handle {
                        !matches!(self.arena.get(h).value, Val::Null)
                    } else {
                        false
                    }
                } else {
                    // Empty
                    if let Some(h) = val_handle {
                        let val = &self.arena.get(h).value;
                        match val {
                            Val::Null => true,
                            Val::Bool(b) => !b,
                            Val::Int(i) => *i == 0,
                            Val::Float(f) => *f == 0.0,
                            Val::String(s) => s.is_empty() || s.as_slice() == b"0",
                            Val::Array(a) => a.map.is_empty(),
                            _ => false,
                        }
                    } else {
                        true
                    }
                };

                let res_handle = self.arena.alloc(Val::Bool(result));
                self.operand_stack.push(res_handle);
            }
            OpCode::IssetIsemptyStaticProp => {
                let type_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let prop_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let class_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let type_val = match self.arena.get(type_handle).value {
                    Val::Int(i) => i,
                    _ => 0,
                };

                let class_name = match &self.arena.get(class_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                };

                let prop_name = match &self.arena.get(prop_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };

                let val_opt = if let Ok(resolved_class) = self.resolve_class_name(class_name) {
                    if let Ok((val, _, _)) = self.find_static_prop(resolved_class, prop_name) {
                        Some(val)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let result = if type_val == 0 {
                    // Isset
                    if let Some(val) = val_opt {
                        !matches!(val, Val::Null)
                    } else {
                        false
                    }
                } else {
                    // Empty
                    if let Some(val) = val_opt {
                        match val {
                            Val::Null => true,
                            Val::Bool(b) => !b,
                            Val::Int(i) => i == 0,
                            Val::Float(f) => f == 0.0,
                            Val::String(s) => s.is_empty() || s.as_slice() == b"0",
                            Val::Array(a) => a.map.is_empty(),
                            _ => false,
                        }
                    } else {
                        true
                    }
                };

                let res_handle = self.arena.alloc(Val::Bool(result));
                self.operand_stack.push(res_handle);
            }
            OpCode::AssignStaticPropOp(op) => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let prop_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let class_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let class_name = match &self.arena.get(class_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                };

                let prop_name = match &self.arena.get(prop_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };

                let resolved_class = self.resolve_class_name(class_name)?;
                let (current_val, visibility, defining_class) =
                    self.find_static_prop(resolved_class, prop_name)?;
                self.check_const_visibility(defining_class, visibility)?;

                let val = self.arena.get(val_handle).value.clone();

                // Use AssignOpType to perform the operation
                use crate::vm::assign_op::AssignOpType;
                let op_type = AssignOpType::from_u8(op)
                    .ok_or_else(|| VmError::RuntimeError(format!("Invalid assign op: {}", op)))?;

                let res = op_type.apply(current_val.clone(), val)?;

                if let Some(class_def) = self.context.classes.get_mut(&defining_class) {
                    if let Some(entry) = class_def.static_properties.get_mut(&prop_name) {
                        entry.value = res.clone();
                    }
                }

                let res_handle = self.arena.alloc(res);
                self.operand_stack.push(res_handle);
            }
            OpCode::PreIncStaticProp => {
                let (prop_name, defining_class, current_val) = self.prepare_static_prop_access()?;

                // Use increment_value for proper PHP type handling
                use crate::vm::inc_dec::increment_value;
                let new_val = increment_value(current_val, &mut *self.error_handler)?;

                if let Some(class_def) = self.context.classes.get_mut(&defining_class) {
                    if let Some(entry) = class_def.static_properties.get_mut(&prop_name) {
                        entry.value = new_val.clone();
                    }
                }

                let res_handle = self.arena.alloc(new_val);
                self.operand_stack.push(res_handle);
            }
            OpCode::PreDecStaticProp => {
                let (prop_name, defining_class, current_val) = self.prepare_static_prop_access()?;

                // Use decrement_value for proper PHP type handling
                use crate::vm::inc_dec::decrement_value;
                let new_val = decrement_value(current_val, &mut *self.error_handler)?;

                if let Some(class_def) = self.context.classes.get_mut(&defining_class) {
                    if let Some(entry) = class_def.static_properties.get_mut(&prop_name) {
                        entry.value = new_val.clone();
                    }
                }

                let res_handle = self.arena.alloc(new_val);
                self.operand_stack.push(res_handle);
            }
            OpCode::PostIncStaticProp => {
                let (prop_name, defining_class, current_val) = self.prepare_static_prop_access()?;

                // Use increment_value for proper PHP type handling
                use crate::vm::inc_dec::increment_value;
                let new_val = increment_value(current_val.clone(), &mut *self.error_handler)?;

                if let Some(class_def) = self.context.classes.get_mut(&defining_class) {
                    if let Some(entry) = class_def.static_properties.get_mut(&prop_name) {
                        entry.value = new_val;
                    }
                }

                // Post-increment returns the OLD value
                let res_handle = self.arena.alloc(current_val);
                self.operand_stack.push(res_handle);
            }
            OpCode::PostDecStaticProp => {
                let (prop_name, defining_class, current_val) = self.prepare_static_prop_access()?;

                // Use decrement_value for proper PHP type handling
                use crate::vm::inc_dec::decrement_value;
                let new_val = decrement_value(current_val.clone(), &mut *self.error_handler)?;

                if let Some(class_def) = self.context.classes.get_mut(&defining_class) {
                    if let Some(entry) = class_def.static_properties.get_mut(&prop_name) {
                        entry.value = new_val;
                    }
                }

                // Post-decrement returns the OLD value
                let res_handle = self.arena.alloc(current_val);
                self.operand_stack.push(res_handle);
            }
            OpCode::InstanceOf => {
                let class_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let class_name = match &self.arena.get(class_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                };

                let is_instance = if let Val::Object(h) = self.arena.get(obj_handle).value {
                    if let Val::ObjPayload(data) = &self.arena.get(h).value {
                        self.is_subclass_of(data.class, class_name)
                    } else {
                        false
                    }
                } else {
                    false
                };

                let res_handle = self.arena.alloc(Val::Bool(is_instance));
                self.operand_stack.push(res_handle);
            }
            OpCode::AssignObjOp(op) => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let prop_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let prop_name = match &self.arena.get(prop_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };

                let payload_handle = if let Val::Object(h) = self.arena.get(obj_handle).value {
                    h
                } else {
                    return Err(VmError::RuntimeError(
                        "Attempt to assign property on non-object".into(),
                    ));
                };

                // 1. Get current value (with __get support)
                let current_val = {
                    let (class_name, prop_handle_opt, has_prop) = {
                        let payload_zval = self.arena.get(payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload_zval.value {
                            (
                                obj_data.class,
                                obj_data.properties.get(&prop_name).copied(),
                                obj_data.properties.contains_key(&prop_name),
                            )
                        } else {
                            return Err(VmError::RuntimeError("Invalid object payload".into()));
                        }
                    };

                    // Check if we should use __get
                    let current_scope = self.get_current_class();
                    let visibility_ok = has_prop
                        && self
                            .check_prop_visibility(class_name, prop_name, current_scope)
                            .is_ok();

                    if let Some(val_handle) = prop_handle_opt {
                        if visibility_ok {
                            self.arena.get(val_handle).value.clone()
                        } else {
                            // Try __get for inaccessible property
                            let magic_get = self.context.interner.intern(b"__get");
                            if let Some((method, _, _, defined_class)) =
                                self.find_method(class_name, magic_get)
                            {
                                let prop_name_bytes = self
                                    .context
                                    .interner
                                    .lookup(prop_name)
                                    .unwrap_or(b"")
                                    .to_vec();
                                let name_handle =
                                    self.arena.alloc(Val::String(prop_name_bytes.into()));

                                let mut frame = CallFrame::new(method.chunk.clone());
                                frame.func = Some(method.clone());
                                frame.this = Some(obj_handle);
                                frame.class_scope = Some(defined_class);
                                frame.called_scope = Some(class_name);

                                if let Some(param) = method.params.get(0) {
                                    frame.locals.insert(param.name, name_handle);
                                }

                                self.push_frame(frame);

                                if let Some(ret_val) = self.last_return_value {
                                    self.arena.get(ret_val).value.clone()
                                } else {
                                    Val::Null
                                }
                            } else {
                                Val::Null
                            }
                        }
                    } else {
                        // Property doesn't exist, try __get
                        let magic_get = self.context.interner.intern(b"__get");
                        if let Some((method, _, _, defined_class)) =
                            self.find_method(class_name, magic_get)
                        {
                            let prop_name_bytes = self
                                .context
                                .interner
                                .lookup(prop_name)
                                .unwrap_or(b"")
                                .to_vec();
                            let name_handle = self.arena.alloc(Val::String(prop_name_bytes.into()));

                            let mut frame = CallFrame::new(method.chunk.clone());
                            frame.func = Some(method.clone());
                            frame.this = Some(obj_handle);
                            frame.class_scope = Some(defined_class);
                            frame.called_scope = Some(class_name);

                            if let Some(param) = method.params.get(0) {
                                frame.locals.insert(param.name, name_handle);
                            }

                            self.push_frame(frame);

                            if let Some(ret_val) = self.last_return_value {
                                self.arena.get(ret_val).value.clone()
                            } else {
                                Val::Null
                            }
                        } else {
                            Val::Null
                        }
                    }
                };

                // 2. Perform Op
                let val = self.arena.get(val_handle).value.clone();

                use crate::vm::assign_op::AssignOpType;
                let op_type = AssignOpType::from_u8(op)
                    .ok_or_else(|| VmError::RuntimeError(format!("Invalid assign op: {}", op)))?;

                let res = op_type.apply(current_val, val)?;

                // 3. Set new value
                let res_handle = self.arena.alloc(res.clone());

                let payload_zval = self.arena.get_mut(payload_handle);
                if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                    obj_data.properties.insert(prop_name, res_handle);
                }

                self.operand_stack.push(res_handle);
            }
            OpCode::PreIncObj => {
                let prop_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let prop_name = match &self.arena.get(prop_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };

                let payload_handle = if let Val::Object(h) = self.arena.get(obj_handle).value {
                    h
                } else {
                    return Err(VmError::RuntimeError(
                        "Attempt to increment property on non-object".into(),
                    ));
                };

                let class_name = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        obj_data.class
                    } else {
                        return Err(VmError::RuntimeError("Invalid object payload".into()));
                    }
                };

                // 1. Read current value (with __get support)
                let current_scope = self.get_current_class();
                let (has_prop, visibility_ok, prop_handle_opt) = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        let has = obj_data.properties.contains_key(&prop_name);
                        let vis_ok = has
                            && self
                                .check_prop_visibility(class_name, prop_name, current_scope)
                                .is_ok();
                        (has, vis_ok, obj_data.properties.get(&prop_name).copied())
                    } else {
                        (false, false, None)
                    }
                };

                let current_val = if has_prop && visibility_ok {
                    if let Some(h) = prop_handle_opt {
                        self.arena.get(h).value.clone()
                    } else {
                        Val::Null
                    }
                } else {
                    // Try __get
                    let magic_get = self.context.interner.intern(b"__get");
                    let prop_name_bytes = self
                        .context
                        .interner
                        .lookup(prop_name)
                        .unwrap_or(b"")
                        .to_vec();
                    let name_handle = self.arena.alloc(Val::String(prop_name_bytes.into()));

                    if let Some(ret_handle) = self.call_magic_method_sync(
                        obj_handle,
                        class_name,
                        magic_get,
                        vec![name_handle],
                    )? {
                        self.arena.get(ret_handle).value.clone()
                    } else {
                        Val::Null
                    }
                };

                // 2. Increment value
                use crate::vm::inc_dec::increment_value;
                let new_val = increment_value(current_val, &mut *self.error_handler)?;
                let res_handle = self.arena.alloc(new_val.clone());

                // 3. Write back (with __set support)
                if has_prop && visibility_ok {
                    let payload_zval = self.arena.get_mut(payload_handle);
                    if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                        obj_data.properties.insert(prop_name, res_handle);
                    }
                } else {
                    // Try __set
                    let magic_set = self.context.interner.intern(b"__set");
                    let prop_name_bytes = self
                        .context
                        .interner
                        .lookup(prop_name)
                        .unwrap_or(b"")
                        .to_vec();
                    let name_handle = self.arena.alloc(Val::String(prop_name_bytes.into()));

                    if self
                        .call_magic_method_sync(
                            obj_handle,
                            class_name,
                            magic_set,
                            vec![name_handle, res_handle],
                        )?
                        .is_none()
                    {
                        // No __set, do direct assignment
                        let payload_zval = self.arena.get_mut(payload_handle);
                        if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                            obj_data.properties.insert(prop_name, res_handle);
                        }
                    }
                }

                self.operand_stack.push(res_handle);
            }
            OpCode::PreDecObj => {
                let prop_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let prop_name = match &self.arena.get(prop_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };

                let payload_handle = if let Val::Object(h) = self.arena.get(obj_handle).value {
                    h
                } else {
                    return Err(VmError::RuntimeError(
                        "Attempt to decrement property on non-object".into(),
                    ));
                };

                let class_name = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        obj_data.class
                    } else {
                        return Err(VmError::RuntimeError("Invalid object payload".into()));
                    }
                };

                // 1. Read current value (with __get support)
                let current_scope = self.get_current_class();
                let (has_prop, visibility_ok, prop_handle_opt) = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        let has = obj_data.properties.contains_key(&prop_name);
                        let vis_ok = has
                            && self
                                .check_prop_visibility(class_name, prop_name, current_scope)
                                .is_ok();
                        (has, vis_ok, obj_data.properties.get(&prop_name).copied())
                    } else {
                        (false, false, None)
                    }
                };

                let current_val = if has_prop && visibility_ok {
                    if let Some(h) = prop_handle_opt {
                        self.arena.get(h).value.clone()
                    } else {
                        Val::Null
                    }
                } else {
                    // Try __get
                    let magic_get = self.context.interner.intern(b"__get");
                    let prop_name_bytes = self
                        .context
                        .interner
                        .lookup(prop_name)
                        .unwrap_or(b"")
                        .to_vec();
                    let name_handle = self.arena.alloc(Val::String(prop_name_bytes.into()));

                    if let Some(ret_handle) = self.call_magic_method_sync(
                        obj_handle,
                        class_name,
                        magic_get,
                        vec![name_handle],
                    )? {
                        self.arena.get(ret_handle).value.clone()
                    } else {
                        Val::Null
                    }
                };

                // 2. Decrement value
                use crate::vm::inc_dec::decrement_value;
                let new_val = decrement_value(current_val, &mut *self.error_handler)?;
                let res_handle = self.arena.alloc(new_val.clone());

                // 3. Write back (with __set support)
                if has_prop && visibility_ok {
                    let payload_zval = self.arena.get_mut(payload_handle);
                    if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                        obj_data.properties.insert(prop_name, res_handle);
                    }
                } else {
                    // Try __set
                    let magic_set = self.context.interner.intern(b"__set");
                    let prop_name_bytes = self
                        .context
                        .interner
                        .lookup(prop_name)
                        .unwrap_or(b"")
                        .to_vec();
                    let name_handle = self.arena.alloc(Val::String(prop_name_bytes.into()));

                    if self
                        .call_magic_method_sync(
                            obj_handle,
                            class_name,
                            magic_set,
                            vec![name_handle, res_handle],
                        )?
                        .is_none()
                    {
                        // No __set, do direct assignment
                        let payload_zval = self.arena.get_mut(payload_handle);
                        if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                            obj_data.properties.insert(prop_name, res_handle);
                        }
                    }
                }

                self.operand_stack.push(res_handle);
            }
            OpCode::PostIncObj => {
                let prop_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let prop_name = match &self.arena.get(prop_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };

                let payload_handle = if let Val::Object(h) = self.arena.get(obj_handle).value {
                    h
                } else {
                    return Err(VmError::RuntimeError(
                        "Attempt to increment property on non-object".into(),
                    ));
                };

                let class_name = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        obj_data.class
                    } else {
                        return Err(VmError::RuntimeError("Invalid object payload".into()));
                    }
                };

                // 1. Read current value (with __get support)
                let current_scope = self.get_current_class();
                let (has_prop, visibility_ok, prop_handle_opt) = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        let has = obj_data.properties.contains_key(&prop_name);
                        let vis_ok = has
                            && self
                                .check_prop_visibility(class_name, prop_name, current_scope)
                                .is_ok();
                        (has, vis_ok, obj_data.properties.get(&prop_name).copied())
                    } else {
                        (false, false, None)
                    }
                };

                let current_val = if has_prop && visibility_ok {
                    if let Some(h) = prop_handle_opt {
                        self.arena.get(h).value.clone()
                    } else {
                        Val::Null
                    }
                } else {
                    // Try __get
                    let magic_get = self.context.interner.intern(b"__get");
                    let prop_name_bytes = self
                        .context
                        .interner
                        .lookup(prop_name)
                        .unwrap_or(b"")
                        .to_vec();
                    let name_handle = self.arena.alloc(Val::String(prop_name_bytes.into()));

                    if let Some(ret_handle) = self.call_magic_method_sync(
                        obj_handle,
                        class_name,
                        magic_get,
                        vec![name_handle],
                    )? {
                        self.arena.get(ret_handle).value.clone()
                    } else {
                        Val::Null
                    }
                };

                // 2. Increment value
                use crate::vm::inc_dec::increment_value;
                let new_val = increment_value(current_val.clone(), &mut *self.error_handler)?;

                let res_handle = self.arena.alloc(current_val); // Return old value
                let new_val_handle = self.arena.alloc(new_val);

                // 3. Write back (with __set support)
                if has_prop && visibility_ok {
                    let payload_zval = self.arena.get_mut(payload_handle);
                    if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                        obj_data.properties.insert(prop_name, new_val_handle);
                    }
                } else {
                    // Try __set
                    let magic_set = self.context.interner.intern(b"__set");
                    let prop_name_bytes = self
                        .context
                        .interner
                        .lookup(prop_name)
                        .unwrap_or(b"")
                        .to_vec();
                    let name_handle = self.arena.alloc(Val::String(prop_name_bytes.into()));

                    if self
                        .call_magic_method_sync(
                            obj_handle,
                            class_name,
                            magic_set,
                            vec![name_handle, new_val_handle],
                        )?
                        .is_none()
                    {
                        // No __set, do direct assignment
                        let payload_zval = self.arena.get_mut(payload_handle);
                        if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                            obj_data.properties.insert(prop_name, new_val_handle);
                        }
                    }
                }

                self.operand_stack.push(res_handle);
            }
            OpCode::PostDecObj => {
                let prop_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let prop_name = match &self.arena.get(prop_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };

                let payload_handle = if let Val::Object(h) = self.arena.get(obj_handle).value {
                    h
                } else {
                    return Err(VmError::RuntimeError(
                        "Attempt to decrement property on non-object".into(),
                    ));
                };

                let class_name = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        obj_data.class
                    } else {
                        return Err(VmError::RuntimeError("Invalid object payload".into()));
                    }
                };

                // 1. Read current value (with __get support)
                let current_scope = self.get_current_class();
                let (has_prop, visibility_ok, prop_handle_opt) = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        let has = obj_data.properties.contains_key(&prop_name);
                        let vis_ok = has
                            && self
                                .check_prop_visibility(class_name, prop_name, current_scope)
                                .is_ok();
                        (has, vis_ok, obj_data.properties.get(&prop_name).copied())
                    } else {
                        (false, false, None)
                    }
                };

                let current_val = if has_prop && visibility_ok {
                    if let Some(h) = prop_handle_opt {
                        self.arena.get(h).value.clone()
                    } else {
                        Val::Null
                    }
                } else {
                    // Try __get
                    let magic_get = self.context.interner.intern(b"__get");
                    let prop_name_bytes = self
                        .context
                        .interner
                        .lookup(prop_name)
                        .unwrap_or(b"")
                        .to_vec();
                    let name_handle = self.arena.alloc(Val::String(prop_name_bytes.into()));

                    if let Some(ret_handle) = self.call_magic_method_sync(
                        obj_handle,
                        class_name,
                        magic_get,
                        vec![name_handle],
                    )? {
                        self.arena.get(ret_handle).value.clone()
                    } else {
                        Val::Null
                    }
                };

                // 2. Decrement value
                use crate::vm::inc_dec::decrement_value;
                let new_val = decrement_value(current_val.clone(), &mut *self.error_handler)?;

                let res_handle = self.arena.alloc(current_val); // Return old value
                let new_val_handle = self.arena.alloc(new_val);

                // 3. Write back (with __set support)
                if has_prop && visibility_ok {
                    let payload_zval = self.arena.get_mut(payload_handle);
                    if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                        obj_data.properties.insert(prop_name, new_val_handle);
                    }
                } else {
                    // Try __set
                    let magic_set = self.context.interner.intern(b"__set");
                    let prop_name_bytes = self
                        .context
                        .interner
                        .lookup(prop_name)
                        .unwrap_or(b"")
                        .to_vec();
                    let name_handle = self.arena.alloc(Val::String(prop_name_bytes.into()));

                    if self
                        .call_magic_method_sync(
                            obj_handle,
                            class_name,
                            magic_set,
                            vec![name_handle, new_val_handle],
                        )?
                        .is_none()
                    {
                        // No __set, do direct assignment
                        let payload_zval = self.arena.get_mut(payload_handle);
                        if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                            obj_data.properties.insert(prop_name, new_val_handle);
                        }
                    }
                }

                self.operand_stack.push(res_handle);
            }
            OpCode::RopeInit | OpCode::RopeAdd | OpCode::RopeEnd => {
                // Treat as Concat for now
                let b_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let a_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let b_val = self.arena.get(b_handle).value.clone();
                let a_val = self.arena.get(a_handle).value.clone();

                let res = match (a_val, b_val) {
                    (Val::String(a), Val::String(b)) => {
                        let mut s = String::from_utf8_lossy(&a).to_string();
                        s.push_str(&String::from_utf8_lossy(&b));
                        Val::String(s.into_bytes().into())
                    }
                    (Val::String(a), Val::Int(b)) => {
                        let mut s = String::from_utf8_lossy(&a).to_string();
                        s.push_str(&b.to_string());
                        Val::String(s.into_bytes().into())
                    }
                    (Val::Int(a), Val::String(b)) => {
                        let mut s = a.to_string();
                        s.push_str(&String::from_utf8_lossy(&b));
                        Val::String(s.into_bytes().into())
                    }
                    _ => Val::String(b"".to_vec().into()),
                };

                let res_handle = self.arena.alloc(res);
                self.operand_stack.push(res_handle);
            }
            OpCode::GetClass => {
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val = self.arena.get(obj_handle).value.clone();

                match val {
                    Val::Object(h) => {
                        if let Val::ObjPayload(data) = &self.arena.get(h).value {
                            let name_bytes =
                                self.context.interner.lookup(data.class).unwrap_or(b"");
                            let res_handle =
                                self.arena.alloc(Val::String(name_bytes.to_vec().into()));
                            self.operand_stack.push(res_handle);
                        } else {
                            return Err(VmError::RuntimeError("Invalid object payload".into()));
                        }
                    }
                    Val::String(s) => {
                        let res_handle = self.arena.alloc(Val::String(s));
                        self.operand_stack.push(res_handle);
                    }
                    _ => {
                        return Err(VmError::RuntimeError(
                            "::class lookup on non-object/non-string".into(),
                        ));
                    }
                }
            }
            OpCode::GetCalledClass => {
                let frame = self
                    .frames
                    .last()
                    .ok_or(VmError::RuntimeError("No active frame".into()))?;
                if let Some(scope) = frame.called_scope {
                    let name_bytes = self.context.interner.lookup(scope).unwrap_or(b"");
                    let res_handle = self.arena.alloc(Val::String(name_bytes.to_vec().into()));
                    self.operand_stack.push(res_handle);
                } else {
                    return Err(VmError::RuntimeError(
                        "get_called_class() called from outside a class".into(),
                    ));
                }
            }
            OpCode::GetType => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val = &self.arena.get(handle).value;
                let type_str = match val {
                    Val::Null => "NULL",
                    Val::Bool(_) => "boolean",
                    Val::Int(_) => "integer",
                    Val::Float(_) => "double",
                    Val::String(_) => "string",
                    Val::Array(_) => "array",
                    Val::Object(_) => "object",
                    Val::Resource(_) => "resource",
                    _ => "unknown",
                };
                let res_handle = self
                    .arena
                    .alloc(Val::String(type_str.as_bytes().to_vec().into()));
                self.operand_stack.push(res_handle);
            }
            OpCode::Clone => {
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let mut new_obj_data_opt = None;
                let mut class_name_opt = None;

                {
                    let obj_val = self.arena.get(obj_handle);
                    if let Val::Object(payload_handle) = &obj_val.value {
                        let payload_val = self.arena.get(*payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload_val.value {
                            new_obj_data_opt = Some(obj_data.clone());
                            class_name_opt = Some(obj_data.class);
                        }
                    }
                }

                if let Some(new_obj_data) = new_obj_data_opt {
                    let new_payload_handle = self.arena.alloc(Val::ObjPayload(new_obj_data));
                    let new_obj_handle = self.arena.alloc(Val::Object(new_payload_handle));
                    self.operand_stack.push(new_obj_handle);

                    if let Some(class_name) = class_name_opt {
                        let clone_sym = self.context.interner.intern(b"__clone");
                        if let Some((method, _, _, _)) = self.find_method(class_name, clone_sym) {
                            let mut frame = CallFrame::new(method.chunk.clone());
                            frame.func = Some(method.clone());
                            frame.this = Some(new_obj_handle);
                            frame.class_scope = Some(class_name);
                            frame.discard_return = true;

                            self.push_frame(frame);
                        }
                    }
                } else {
                    return Err(VmError::RuntimeError(
                        "__clone method called on non-object".into(),
                    ));
                }
            }
            OpCode::Copy => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val = self.arena.get(handle).value.clone();
                let new_handle = self.arena.alloc(val);
                self.operand_stack.push(new_handle);
            }
            OpCode::IssetVar(sym) => {
                let frame = self.frames.last().unwrap();
                let is_set = if let Some(&handle) = frame.locals.get(&sym) {
                    !matches!(self.arena.get(handle).value, Val::Null)
                } else {
                    false
                };
                let res_handle = self.arena.alloc(Val::Bool(is_set));
                self.operand_stack.push(res_handle);
            }
            OpCode::IssetVarDynamic => {
                let name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let name_bytes = self.convert_to_string(name_handle)?;
                let sym = self.context.interner.intern(&name_bytes);

                let frame = self.frames.last().unwrap();
                let is_set = if let Some(&handle) = frame.locals.get(&sym) {
                    !matches!(self.arena.get(handle).value, Val::Null)
                } else {
                    false
                };
                let res_handle = self.arena.alloc(Val::Bool(is_set));
                self.operand_stack.push(res_handle);
            }
            OpCode::IssetDim => {
                let key_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let array_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let container_zval = self.arena.get(array_handle);
                let is_set = match &container_zval.value {
                    Val::Array(map) => {
                        let key_val = &self.arena.get(key_handle).value;
                        let key = match key_val {
                            Val::Int(i) => ArrayKey::Int(*i),
                            Val::String(s) => ArrayKey::Str(s.clone()),
                            _ => ArrayKey::Int(0),
                        };

                        if let Some(val_handle) = map.map.get(&key) {
                            !matches!(self.arena.get(*val_handle).value, Val::Null)
                        } else {
                            false
                        }
                    }
                    Val::Object(payload_handle) => {
                        // Check if it's ArrayAccess
                        let payload = self.arena.get(*payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload.value {
                            let class_name = obj_data.class;
                            if self.implements_array_access(class_name) {
                                // Call offsetExists
                                match self.call_array_access_offset_exists(array_handle, key_handle)
                                {
                                    Ok(exists) => {
                                        if !exists {
                                            false
                                        } else {
                                            // offsetExists returned true, now check if value is not null
                                            match self.call_array_access_offset_get(
                                                array_handle,
                                                key_handle,
                                            ) {
                                                Ok(val_handle) => !matches!(
                                                    self.arena.get(val_handle).value,
                                                    Val::Null
                                                ),
                                                Err(_) => false,
                                            }
                                        }
                                    }
                                    Err(_) => false,
                                }
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    Val::String(s) => {
                        // String offset access - check if offset is valid
                        // Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_ISSET_ISEMPTY_DIM_OBJ
                        let offset = self.arena.get(key_handle).value.to_int();
                        let len = s.len() as i64;

                        // Handle negative offsets
                        let actual_offset = if offset < 0 {
                            let adjusted = len + offset;
                            if adjusted < 0 {
                                -1i64 as usize // Out of bounds - use impossible value
                            } else {
                                adjusted as usize
                            }
                        } else {
                            offset as usize
                        };

                        actual_offset < s.len()
                    }
                    _ => false,
                };

                let res_handle = self.arena.alloc(Val::Bool(is_set));
                self.operand_stack.push(res_handle);
            }
            OpCode::IssetProp(prop_name) => {
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                // Extract data to avoid borrow issues
                let (class_name, is_set_result) = {
                    let obj_zval = self.arena.get(obj_handle);
                    if let Val::Object(payload_handle) = obj_zval.value {
                        let payload_zval = self.arena.get(payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload_zval.value {
                            let current_scope = self.get_current_class();
                            if self
                                .check_prop_visibility(obj_data.class, prop_name, current_scope)
                                .is_ok()
                            {
                                if let Some(val_handle) = obj_data.properties.get(&prop_name) {
                                    (
                                        obj_data.class,
                                        Some(!matches!(
                                            self.arena.get(*val_handle).value,
                                            Val::Null
                                        )),
                                    )
                                } else {
                                    (obj_data.class, None) // Not found
                                }
                            } else {
                                (obj_data.class, None) // Not accessible
                            }
                        } else {
                            return Err(VmError::RuntimeError("Invalid object payload".into()));
                        }
                    } else {
                        return Err(VmError::RuntimeError("Isset on non-object".into()));
                    }
                };

                if let Some(result) = is_set_result {
                    let res_handle = self.arena.alloc(Val::Bool(result));
                    self.operand_stack.push(res_handle);
                } else {
                    // Property not found or not accessible. Check for __isset.
                    let isset_magic = self.context.interner.intern(b"__isset");

                    // Create method name string (prop name)
                    let prop_name_str = self
                        .context
                        .interner
                        .lookup(prop_name)
                        .expect("Prop name should be interned")
                        .to_vec();
                    let name_handle = self.arena.alloc(Val::String(prop_name_str.into()));

                    // Save caller's return value to avoid corruption (similar to __toString)
                    let saved_return_value = self.last_return_value.take();

                    // Call __isset synchronously
                    let isset_result = self.call_magic_method_sync(
                        obj_handle,
                        class_name,
                        isset_magic,
                        vec![name_handle],
                    )?;

                    // Restore caller's return value
                    self.last_return_value = saved_return_value;

                    let result = if let Some(result_handle) = isset_result {
                        // __isset returned a value - convert to bool
                        let result_val = &self.arena.get(result_handle).value;
                        result_val.to_bool()
                    } else {
                        // No __isset method, return false
                        false
                    };

                    let res_handle = self.arena.alloc(Val::Bool(result));
                    self.operand_stack.push(res_handle);
                }
            }
            OpCode::IssetStaticProp(prop_name) => {
                let class_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let class_name = match &self.arena.get(class_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                };

                let resolved_class = self.resolve_class_name(class_name)?;

                let is_set = match self.find_static_prop(resolved_class, prop_name) {
                    Ok((val, _, _)) => !matches!(val, Val::Null),
                    Err(_) => false,
                };

                let res_handle = self.arena.alloc(Val::Bool(is_set));
                self.operand_stack.push(res_handle);
            }
            OpCode::CallStaticMethod(class_name, method_name, arg_count) => {
                self.exec_call_static_method(class_name, method_name, arg_count, false)?;
            }
            OpCode::CallStaticMethodDynamic(arg_count) => {
                let method_name_handle = self
                    .operand_stack
                    .peek_at(arg_count as usize)
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let class_name_handle = self
                    .operand_stack
                    .peek_at(arg_count as usize + 1)
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let method_name_bytes = self.convert_to_string(method_name_handle)?;
                let method_name = self.context.interner.intern(&method_name_bytes);

                let class_name_bytes = self.convert_to_string(class_name_handle)?;
                let class_name = self.context.interner.intern(&class_name_bytes);

                self.exec_call_static_method(class_name, method_name, arg_count, true)?;
            }

            OpCode::Concat => {
                let b_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let a_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let b_str = self.convert_to_string(b_handle)?;
                let a_str = self.convert_to_string(a_handle)?;

                let mut res = a_str;
                res.extend(b_str);

                let res_handle = self.arena.alloc(Val::String(res.into()));
                self.operand_stack.push(res_handle);
            }

            OpCode::FastConcat => {
                let b_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let a_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let b_str = self.convert_to_string(b_handle)?;
                let a_str = self.convert_to_string(a_handle)?;

                let mut res = a_str;
                res.extend(b_str);

                let res_handle = self.arena.alloc(Val::String(res.into()));
                self.operand_stack.push(res_handle);
            }

            // Comparison operations - delegated to opcodes::comparison
            OpCode::IsEqual => self.exec_equal()?,
            OpCode::IsNotEqual => self.exec_not_equal()?,
            OpCode::IsIdentical => self.exec_identical()?,
            OpCode::IsNotIdentical => self.exec_not_identical()?,
            OpCode::IsGreater => self.exec_greater_than()?,
            OpCode::IsLess => self.exec_less_than()?,
            OpCode::IsGreaterOrEqual => self.exec_greater_than_or_equal()?,
            OpCode::IsLessOrEqual => self.exec_less_than_or_equal()?,
            OpCode::Spaceship => self.exec_spaceship()?,
            OpCode::BoolXor => {
                let b_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let a_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let b_val = &self.arena.get(b_handle).value;
                let a_val = &self.arena.get(a_handle).value;

                let to_bool = |v: &Val| match v {
                    Val::Bool(b) => *b,
                    Val::Int(i) => *i != 0,
                    Val::Null => false,
                    _ => true,
                };

                let res = to_bool(a_val) ^ to_bool(b_val);
                let res_handle = self.arena.alloc(Val::Bool(res));
                self.operand_stack.push(res_handle);
            }
            OpCode::CheckVar(sym) => {
                let frame = self.frames.last().unwrap();
                if !frame.locals.contains_key(&sym) {
                    // Variable is undefined.
                    // In Zend, this might trigger a warning depending on flags.
                    // For now, we do nothing, but we could check error_reporting.
                    // If we wanted to support "undefined variable" notice, we'd do it here.
                }
            }
            OpCode::AssignObj => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let prop_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let prop_name = match &self.arena.get(prop_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };

                let payload_handle = if let Val::Object(h) = self.arena.get(obj_handle).value {
                    h
                } else {
                    return Err(VmError::RuntimeError(
                        "Attempt to assign property on non-object".into(),
                    ));
                };

                // Extract data
                let (class_name, prop_exists) = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        (obj_data.class, obj_data.properties.contains_key(&prop_name))
                    } else {
                        return Err(VmError::RuntimeError("Invalid object payload".into()));
                    }
                };

                let current_scope = self.get_current_class();
                let visibility_check =
                    self.check_prop_visibility(class_name, prop_name, current_scope);

                let mut use_magic = false;

                if prop_exists {
                    if visibility_check.is_err() {
                        use_magic = true;
                    }
                } else {
                    use_magic = true;
                }

                if use_magic {
                    let magic_set = self.context.interner.intern(b"__set");
                    if let Some((method, _, _, defined_class)) =
                        self.find_method(class_name, magic_set)
                    {
                        let prop_name_bytes = self
                            .context
                            .interner
                            .lookup(prop_name)
                            .unwrap_or(b"")
                            .to_vec();
                        let name_handle = self.arena.alloc(Val::String(prop_name_bytes.into()));

                        let mut frame = CallFrame::new(method.chunk.clone());
                        frame.func = Some(method.clone());
                        frame.this = Some(obj_handle);
                        frame.class_scope = Some(defined_class);
                        frame.called_scope = Some(class_name);
                        frame.discard_return = true;

                        if let Some(param) = method.params.get(0) {
                            frame.locals.insert(param.name, name_handle);
                        }
                        if let Some(param) = method.params.get(1) {
                            frame.locals.insert(param.name, val_handle);
                        }

                        self.operand_stack.push(val_handle);
                        self.push_frame(frame);
                    } else {
                        if let Err(e) = visibility_check {
                            return Err(e);
                        }

                        let payload_zval = self.arena.get_mut(payload_handle);
                        if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                            obj_data.properties.insert(prop_name, val_handle);
                        }
                        self.operand_stack.push(val_handle);
                    }
                } else {
                    let payload_zval = self.arena.get_mut(payload_handle);
                    if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                        obj_data.properties.insert(prop_name, val_handle);
                    } else {
                        return Err(VmError::RuntimeError("Invalid object payload".into()));
                    }
                    self.operand_stack.push(val_handle);
                }
            }
            OpCode::AssignObjRef => {
                let ref_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let prop_name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let obj_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                // Ensure value is a reference
                self.arena.get_mut(ref_handle).is_ref = true;

                let prop_name = match &self.arena.get(prop_name_handle).value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Property name must be string".into())),
                };

                let payload_handle = if let Val::Object(h) = self.arena.get(obj_handle).value {
                    h
                } else {
                    return Err(VmError::RuntimeError(
                        "Attempt to assign property on non-object".into(),
                    ));
                };

                let payload_zval = self.arena.get_mut(payload_handle);
                if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                    obj_data.properties.insert(prop_name, ref_handle);
                } else {
                    return Err(VmError::RuntimeError("Invalid object payload".into()));
                }
                self.operand_stack.push(ref_handle);
            }
            OpCode::FetchClass => {
                let name_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let name_val = self.arena.get(name_handle);
                let name_sym = match &name_val.value {
                    Val::String(s) => self.context.interner.intern(s),
                    _ => return Err(VmError::RuntimeError("Class name must be string".into())),
                };

                let resolved_sym = self.resolve_class_name(name_sym)?;
                if !self.context.classes.contains_key(&resolved_sym) {
                    let name_str = String::from_utf8_lossy(
                        self.context.interner.lookup(resolved_sym).unwrap_or(b"???"),
                    );
                    return Err(VmError::RuntimeError(format!(
                        "Class '{}' not found",
                        name_str
                    )));
                }

                let resolved_name_bytes =
                    self.context.interner.lookup(resolved_sym).unwrap().to_vec();
                let res_handle = self.arena.alloc(Val::String(resolved_name_bytes.into()));
                self.operand_stack.push(res_handle);
            }

            // Zend-semantic opcodes that require specific implementation.
            // These are currently not emitted by the compiler, but if they appear,
            // we should fail loudly rather than silently no-op.
            OpCode::OpData => {
                return Err(VmError::RuntimeError(
                    "OpData opcode not implemented (compiler should not emit this)".into(),
                ));
            }
            OpCode::Separate => {
                return Err(VmError::RuntimeError(
                    "Separate opcode not implemented - requires proper COW/reference separation semantics".into(),
                ));
            }
            OpCode::BindLexical => {
                return Err(VmError::RuntimeError(
                    "BindLexical opcode not implemented - requires closure capture semantics"
                        .into(),
                ));
            }
            OpCode::CheckUndefArgs => {
                return Err(VmError::RuntimeError(
                    "CheckUndefArgs opcode not implemented - requires variadic argument handling"
                        .into(),
                ));
            }
            OpCode::JmpNull => {
                return Err(VmError::RuntimeError(
                    "JmpNull opcode not implemented - requires nullsafe operator support".into(),
                ));
            }
            OpCode::GeneratorCreate | OpCode::GeneratorReturn => {
                return Err(VmError::RuntimeError(format!(
                    "{:?} opcode not implemented - requires generator unwinding semantics",
                    op
                )));
            }

            // Class/function declaration opcodes that may need implementation
            OpCode::DeclareLambdaFunction
            | OpCode::DeclareClassDelayed
            | OpCode::DeclareAnonClass
            | OpCode::DeclareAttributedConst => {
                return Err(VmError::RuntimeError(format!(
                    "{:?} opcode not implemented - declaration semantics need modeling",
                    op
                )));
            }

            // VM-internal opcodes that shouldn't appear in user code
            OpCode::UnsetCv
            | OpCode::IssetIsemptyCv
            | OpCode::FetchClassName
            | OpCode::CopyTmp
            | OpCode::IssetIsemptyThis
            | OpCode::BindInitStaticOrJmp
            | OpCode::InitParentPropertyHookCall
            | OpCode::UserOpcode => {
                return Err(VmError::RuntimeError(format!(
                    "{:?} is a Zend-internal opcode that should not be emitted by this compiler",
                    op
                )));
            }

            OpCode::CallTrampoline
            | OpCode::DiscardException
            | OpCode::FastCall
            | OpCode::FastRet
            | OpCode::FramelessIcall0
            | OpCode::FramelessIcall1
            | OpCode::FramelessIcall2
            | OpCode::FramelessIcall3
            | OpCode::JmpFrameless => {
                // Treat frameless/fast-call opcodes like normal calls by consuming the pending call.
                let call = self.pending_calls.pop().ok_or(VmError::RuntimeError(
                    "No pending call for frameless invocation".into(),
                ))?;
                self.execute_pending_call(call)?;
            }

            OpCode::Free => {
                self.operand_stack.pop();
            }
            OpCode::Bool => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val = self.arena.get(handle);
                let b = match val.value {
                    Val::Bool(v) => v,
                    Val::Int(v) => v != 0,
                    Val::Null => false,
                    _ => true,
                };
                let res_handle = self.arena.alloc(Val::Bool(b));
                self.operand_stack.push(res_handle);
            }
        }
        Ok(())
    }

    fn clone_value_for_assignment(&mut self, target_sym: Symbol, val_handle: Handle) -> Val {
        let mut cloned = self.arena.get(val_handle).value.clone();

        if !self.is_globals_symbol(target_sym) {
            let globals_sym = self.context.interner.intern(b"GLOBALS");
            if let Some(&globals_handle) = self.context.globals.get(&globals_sym) {
                if globals_handle == val_handle {
                    if let Val::Array(array_rc) = &mut cloned {
                        let duplicated = (**array_rc).clone();
                        *array_rc = Rc::new(duplicated);
                    }
                }
            }
        }

        cloned
    }
}

impl VM {
    pub(crate) fn binary_cmp<F>(&mut self, op: F) -> Result<(), VmError>
    where
        F: Fn(&Val, &Val) -> bool,
    {
        let b_handle = self
            .operand_stack
            .pop()
            .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
        let a_handle = self
            .operand_stack
            .pop()
            .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

        let b_val = &self.arena.get(b_handle).value;
        let a_val = &self.arena.get(a_handle).value;

        let res = op(a_val, b_val);
        let res_handle = self.arena.alloc(Val::Bool(res));
        self.operand_stack.push(res_handle);
        Ok(())
    }

    pub(crate) fn assign_dim_value(
        &mut self,
        array_handle: Handle,
        key_handle: Handle,
        val_handle: Handle,
    ) -> Result<(), VmError> {
        // Check if we have a reference at the key
        let key_val = &self.arena.get(key_handle).value;
        let key = self.array_key_from_value(key_val)?;

        let array_zval = self.arena.get(array_handle);
        if let Val::Array(map) = &array_zval.value {
            if let Some(existing_handle) = map.map.get(&key) {
                if self.arena.get(*existing_handle).is_ref {
                    // Update the value pointed to by the reference
                    let new_val = self.arena.get(val_handle).value.clone();
                    self.arena.get_mut(*existing_handle).value = new_val;

                    self.operand_stack.push(array_handle);
                    return Ok(());
                }
            }
        }

        self.assign_dim(array_handle, key_handle, val_handle)
    }

    pub(crate) fn assign_dim(
        &mut self,
        array_handle: Handle,
        key_handle: Handle,
        val_handle: Handle,
    ) -> Result<(), VmError> {
        // Check if this is an ArrayAccess object
        // Reference: $PHP_SRC_PATH/Zend/zend_execute.c - ZEND_ASSIGN_DIM_SPEC
        let array_val = &self.arena.get(array_handle).value;

        if let Val::Object(payload_handle) = array_val {
            let payload = self.arena.get(*payload_handle);
            if let Val::ObjPayload(obj_data) = &payload.value {
                let class_name = obj_data.class;
                if self.implements_array_access(class_name) {
                    // Call ArrayAccess::offsetSet($offset, $value)
                    self.call_array_access_offset_set(array_handle, key_handle, val_handle)?;
                    self.operand_stack.push(array_handle);
                    return Ok(());
                }
            }
        }

        // Standard array assignment logic
        let key_val = &self.arena.get(key_handle).value;
        let key = self.array_key_from_value(key_val)?;

        // Check if this is a write to $GLOBALS and sync it
        let globals_sym = self.context.interner.intern(b"GLOBALS");
        let globals_handle = self.context.globals.get(&globals_sym).copied();
        let is_globals_write = if let Some(globals_handle) = globals_handle {
            if globals_handle == array_handle {
                true
            } else {
                match (
                    &self.arena.get(globals_handle).value,
                    &self.arena.get(array_handle).value,
                ) {
                    (Val::Array(globals_map), Val::Array(current_map)) => {
                        Rc::ptr_eq(globals_map, current_map)
                    }
                    _ => false,
                }
            }
        } else {
            false
        };

        let is_ref = self.arena.get(array_handle).is_ref;

        if is_ref {
            let array_zval_mut = self.arena.get_mut(array_handle);

            if let Val::Null | Val::Bool(false) = array_zval_mut.value {
                array_zval_mut.value = Val::Array(ArrayData::new().into());
            }

            if let Val::Array(map) = &mut array_zval_mut.value {
                Rc::make_mut(map).insert(key.clone(), val_handle);

                // Sync to global symbol table if this is $GLOBALS
                if is_globals_write {
                    self.sync_globals_key(&key, val_handle);
                }
            } else {
                return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
            }
            self.operand_stack.push(array_handle);
        } else {
            let array_zval = self.arena.get(array_handle);
            let mut new_val = array_zval.value.clone();

            if let Val::Null | Val::Bool(false) = new_val {
                new_val = Val::Array(ArrayData::new().into());
            }

            if let Val::Array(ref mut map) = new_val {
                Rc::make_mut(map).insert(key.clone(), val_handle);

                // Sync to global symbol table if this is $GLOBALS
                if is_globals_write {
                    self.sync_globals_key(&key, val_handle);
                }
            } else {
                return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
            }

            let new_handle = self.arena.alloc(new_val);
            self.operand_stack.push(new_handle);
        }
        Ok(())
    }

    /// Note: Array append now uses O(1) ArrayData::push() instead of O(n) index computation
    /// Reference: $PHP_SRC_PATH/Zend/zend_hash.c - zend_hash_next_free_element

    pub(crate) fn append_array(
        &mut self,
        array_handle: Handle,
        val_handle: Handle,
    ) -> Result<(), VmError> {
        let is_ref = self.arena.get(array_handle).is_ref;
        // Check if this handle is a global variable (accessed via $GLOBALS)
        // In that case, modify in-place to ensure $arr and $GLOBALS['arr'] stay in sync
        let is_global_handle = self.is_global_variable_handle(array_handle);

        if is_ref || is_global_handle {
            let array_zval_mut = self.arena.get_mut(array_handle);

            if let Val::Null | Val::Bool(false) = array_zval_mut.value {
                array_zval_mut.value = Val::Array(ArrayData::new().into());
            }

            if let Val::Array(map) = &mut array_zval_mut.value {
                // Use O(1) push method instead of O(n) index computation
                Rc::make_mut(map).push(val_handle);
            } else {
                return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
            }
            self.operand_stack.push(array_handle);
        } else {
            let array_zval = self.arena.get(array_handle);
            let mut new_val = array_zval.value.clone();

            if let Val::Null | Val::Bool(false) = new_val {
                new_val = Val::Array(ArrayData::new().into());
            }

            if let Val::Array(ref mut map) = new_val {
                // Use O(1) push method instead of O(n) index computation
                Rc::make_mut(map).push(val_handle);
            } else {
                return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
            }

            let new_handle = self.arena.alloc(new_val);
            self.operand_stack.push(new_handle);
        }
        Ok(())
    }

    pub(crate) fn assign_nested_dim(
        &mut self,
        array_handle: Handle,
        keys: &[Handle],
        val_handle: Handle,
    ) -> Result<(), VmError> {
        // We need to traverse down, creating copies if necessary (COW),
        // then update the bottom, then reconstruct the path up.

        let new_handle = self.assign_nested_recursive(array_handle, keys, val_handle)?;
        self.operand_stack.push(new_handle);
        Ok(())
    }

    pub(crate) fn unset_nested_dim(
        &mut self,
        array_handle: Handle,
        keys: &[Handle],
    ) -> Result<Handle, VmError> {
        // Similar to assign_nested_dim, but removes the element instead of setting it
        // We need to traverse down, creating copies if necessary (COW),
        // then unset the bottom element, then reconstruct the path up.

        self.unset_nested_recursive(array_handle, keys)
    }

    pub(crate) fn fetch_nested_dim(
        &mut self,
        array_handle: Handle,
        keys: &[Handle],
    ) -> Result<Handle, VmError> {
        let mut current_handle = array_handle;

        for key_handle in keys {
            let current_val = &self.arena.get(current_handle).value;

            match current_val {
                Val::Array(map) => {
                    let key_val = &self.arena.get(*key_handle).value;
                    let key = self.array_key_from_value(key_val)?;

                    if let Some(val) = map.map.get(&key) {
                        current_handle = *val;
                    } else {
                        // Undefined index: emit notice and return NULL
                        let key_str = match &key {
                            ArrayKey::Int(i) => i.to_string(),
                            ArrayKey::Str(s) => String::from_utf8_lossy(s).to_string(),
                        };
                        self.report_error(
                            ErrorLevel::Notice,
                            &format!("Undefined array key \"{}\"", key_str),
                        );
                        return Ok(self.arena.alloc(Val::Null));
                    }
                }
                Val::Object(payload_handle) => {
                    // Check if it's ArrayAccess
                    let payload = self.arena.get(*payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload.value {
                        let class_name = obj_data.class;
                        if self.implements_array_access(class_name) {
                            // Call offsetGet
                            current_handle =
                                self.call_array_access_offset_get(current_handle, *key_handle)?;
                        } else {
                            // Object doesn't implement ArrayAccess
                            self.report_error(
                                ErrorLevel::Warning,
                                "Trying to access array offset on value of type object",
                            );
                            return Ok(self.arena.alloc(Val::Null));
                        }
                    } else {
                        self.report_error(
                            ErrorLevel::Warning,
                            "Trying to access array offset on value of type object",
                        );
                        return Ok(self.arena.alloc(Val::Null));
                    }
                }
                Val::String(s) => {
                    // String offset access
                    // Reference: $PHP_SRC_PATH/Zend/zend_operators.c - string offset handlers
                    let key_val = &self.arena.get(*key_handle).value;
                    let offset = key_val.to_int();

                    let len = s.len() as i64;

                    // Handle negative offsets (count from end, PHP 7.1+)
                    let actual_offset = if offset < 0 { len + offset } else { offset };

                    if actual_offset < 0 || actual_offset >= len {
                        // Out of bounds
                        self.report_error(
                            ErrorLevel::Warning,
                            &format!("Uninitialized string offset {}", offset),
                        );
                        return Ok(self.arena.alloc(Val::String(Rc::new(vec![]))));
                    }

                    // Return single-byte string
                    let byte = s[actual_offset as usize];
                    let result = self.arena.alloc(Val::String(Rc::new(vec![byte])));
                    return Ok(result);
                }
                _ => {
                    // Trying to access dim on scalar (non-array, non-string)
                    let type_str = match current_val {
                        Val::Null => "null",
                        Val::Bool(_) => "bool",
                        Val::Int(_) => "int",
                        Val::Float(_) => "float",
                        _ => "value",
                    };
                    self.report_error(
                        ErrorLevel::Warning,
                        &format!(
                            "Trying to access array offset on value of type {}",
                            type_str
                        ),
                    );
                    return Ok(self.arena.alloc(Val::Null));
                }
            }
        }

        Ok(current_handle)
    }

    fn assign_nested_recursive(
        &mut self,
        current_handle: Handle,
        keys: &[Handle],
        val_handle: Handle,
    ) -> Result<Handle, VmError> {
        if keys.is_empty() {
            return Ok(val_handle);
        }

        // Check if current handle is an ArrayAccess object
        let current_val = &self.arena.get(current_handle).value;
        if let Val::Object(payload_handle) = current_val {
            let payload = self.arena.get(*payload_handle);
            if let Val::ObjPayload(obj_data) = &payload.value {
                let class_name = obj_data.class;
                if self.implements_array_access(class_name) {
                    // If there's only one key, call offsetSet directly
                    return if keys.len() == 1 {
                        self.call_array_access_offset_set(current_handle, keys[0], val_handle)?;
                        Ok(current_handle)
                    } else {
                        // Multiple keys: fetch the intermediate value and recurse
                        let first_key = keys[0];
                        let remaining_keys = &keys[1..];

                        // Call offsetGet to get the intermediate value
                        let intermediate =
                            self.call_array_access_offset_get(current_handle, first_key)?;

                        // Recurse on the intermediate value
                        let new_intermediate =
                            self.assign_nested_recursive(intermediate, remaining_keys, val_handle)?;

                        // If the intermediate value changed, call offsetSet to update it
                        if new_intermediate != intermediate {
                            self.call_array_access_offset_set(
                                current_handle,
                                first_key,
                                new_intermediate,
                            )?;
                        }

                        Ok(current_handle)
                    };
                }
            }
        }

        let key_handle = keys[0];
        let remaining_keys = &keys[1..];

        // Check if current handle is a reference OR a global variable
        // Global variables should be modified in-place even if not marked as ref
        let is_ref = self.arena.get(current_handle).is_ref;
        let is_global = self.is_global_variable_handle(current_handle);

        if is_ref || is_global {
            // For refs, we need to mutate in place
            // First, get the key and auto-vivify if needed
            let (needs_autovivify, key) = {
                let current_zval = self.arena.get(current_handle);
                let needs_autovivify = matches!(current_zval.value, Val::Null | Val::Bool(false));

                // Resolve key
                let key_val = &self.arena.get(key_handle).value;
                let key = if let Val::AppendPlaceholder = key_val {
                    // We'll compute this after autovivify
                    None
                } else {
                    Some(self.array_key_from_value(key_val)?)
                };

                (needs_autovivify, key)
            };

            // Auto-vivify if needed
            if needs_autovivify {
                self.arena.get_mut(current_handle).value = Val::Array(ArrayData::new().into());
            }

            // Now compute the actual key if it was AppendPlaceholder
            let key = if let Some(k) = key {
                k
            } else {
                // Compute next auto-index using O(1) next_index()
                let current_zval = self.arena.get(current_handle);
                if let Val::Array(map) = &current_zval.value {
                    ArrayKey::Int(map.next_index())
                } else {
                    return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
                }
            };

            if remaining_keys.is_empty() {
                // Check if this is a write to $GLOBALS and sync it
                let globals_sym = self.context.interner.intern(b"GLOBALS");
                let globals_handle = self.context.globals.get(&globals_sym).copied();
                let is_globals_write = if let Some(globals_handle) = globals_handle {
                    if globals_handle == current_handle {
                        true
                    } else {
                        match (
                            &self.arena.get(globals_handle).value,
                            &self.arena.get(current_handle).value,
                        ) {
                            (Val::Array(globals_map), Val::Array(current_map)) => {
                                Rc::ptr_eq(globals_map, current_map)
                            }
                            _ => false,
                        }
                    }
                } else {
                    false
                };

                // We are at the last key - check for existing ref
                let existing_ref: Option<Handle> = {
                    let current_zval = self.arena.get(current_handle);
                    if let Val::Array(map) = &current_zval.value {
                        map.map.get(&key).and_then(|&h| {
                            if self.arena.get(h).is_ref {
                                Some(h)
                            } else {
                                None
                            }
                        })
                    } else {
                        return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
                    }
                };

                if let Some(existing_handle) = existing_ref {
                    // Update the ref value
                    let new_val = self.arena.get(val_handle).value.clone();
                    self.arena.get_mut(existing_handle).value = new_val;
                } else {
                    // Insert new value
                    let current_zval = self.arena.get_mut(current_handle);
                    if let Val::Array(ref mut map) = current_zval.value {
                        Rc::make_mut(map).insert(key.clone(), val_handle);
                    }
                }

                // Sync to global symbol table if this is $GLOBALS
                if is_globals_write {
                    self.sync_globals_key(&key, val_handle);
                }
            } else {
                // Go deeper - get or create next level
                let next_handle_opt: Option<Handle> = {
                    let current_zval = self.arena.get(current_handle);
                    if let Val::Array(map) = &current_zval.value {
                        map.map.get(&key).copied()
                    } else {
                        return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
                    }
                };

                let next_handle = if let Some(h) = next_handle_opt {
                    h
                } else {
                    // Create empty array and insert it
                    let empty_handle = self.arena.alloc(Val::Array(ArrayData::new().into()));
                    let current_zval_mut = self.arena.get_mut(current_handle);
                    if let Val::Array(ref mut map) = current_zval_mut.value {
                        Rc::make_mut(map).insert(key.clone(), empty_handle);
                    }
                    empty_handle
                };

                let new_next_handle =
                    self.assign_nested_recursive(next_handle, remaining_keys, val_handle)?;

                // Only update if changed (if next_handle is a ref, it's mutated in place)
                if new_next_handle != next_handle {
                    let current_zval = self.arena.get_mut(current_handle);
                    if let Val::Array(ref mut map) = current_zval.value {
                        Rc::make_mut(map).insert(key, new_next_handle);
                    }
                }
            }

            return Ok(current_handle);
        }

        // Not a reference - COW: Clone current array
        let current_zval = self.arena.get(current_handle);
        let mut new_val = current_zval.value.clone();

        if let Val::Null | Val::Bool(false) = new_val {
            new_val = Val::Array(ArrayData::new().into());
        }

        if let Val::Array(ref mut map) = new_val {
            let map_mut = Rc::make_mut(map);
            // Resolve key
            let key_val = &self.arena.get(key_handle).value;
            let key = if let Val::AppendPlaceholder = key_val {
                // Use O(1) next_index() instead of O(n) computation
                ArrayKey::Int(map_mut.next_index())
            } else {
                self.array_key_from_value(key_val)?
            };

            if remaining_keys.is_empty() {
                // We are at the last key.
                let mut updated_ref = false;
                if let Some(existing_handle) = map_mut.map.get(&key) {
                    if self.arena.get(*existing_handle).is_ref {
                        // Update Ref value
                        let new_val = self.arena.get(val_handle).value.clone();
                        self.arena.get_mut(*existing_handle).value = new_val;
                        updated_ref = true;
                    }
                }

                if !updated_ref {
                    map_mut.insert(key, val_handle);
                }
            } else {
                // We need to go deeper.
                let next_handle = if let Some(h) = map_mut.map.get(&key) {
                    *h
                } else {
                    // Create empty array
                    self.arena.alloc(Val::Array(ArrayData::new().into()))
                };

                let new_next_handle =
                    self.assign_nested_recursive(next_handle, remaining_keys, val_handle)?;
                map_mut.insert(key, new_next_handle);
            }
        } else {
            return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
        }

        let new_handle = self.arena.alloc(new_val);
        Ok(new_handle)
    }

    fn unset_nested_recursive(
        &mut self,
        current_handle: Handle,
        keys: &[Handle],
    ) -> Result<Handle, VmError> {
        if keys.is_empty() {
            // No keys - nothing to unset
            return Ok(current_handle);
        }

        let key_handle = keys[0];
        let remaining_keys = &keys[1..];

        // Check if current handle is a reference OR a global variable
        let is_ref = self.arena.get(current_handle).is_ref;
        let is_global = self.is_global_variable_handle(current_handle);

        if is_ref || is_global {
            // For refs, we need to mutate in place
            let key = {
                let key_val = &self.arena.get(key_handle).value;
                self.array_key_from_value(key_val)?
            };

            if remaining_keys.is_empty() {
                // We are at the last key - remove it
                let current_zval = self.arena.get_mut(current_handle);
                if let Val::Array(ref mut map) = current_zval.value {
                    Rc::make_mut(map).map.shift_remove(&key);

                    // Check if this is a write to $GLOBALS and sync it
                    let globals_sym = self.context.interner.intern(b"GLOBALS");
                    if self.context.globals.get(&globals_sym).copied() == Some(current_handle) {
                        // Sync the deletion back to the global symbol table
                        if let ArrayKey::Str(key_bytes) = &key {
                            let sym = self.context.interner.intern(key_bytes);
                            if key_bytes.as_ref() != b"GLOBALS" {
                                self.context.globals.remove(&sym);
                            }
                        }
                    }
                }
            } else {
                // Go deeper - get the next level
                let next_handle_opt: Option<Handle> = {
                    let current_zval = self.arena.get(current_handle);
                    if let Val::Array(map) = &current_zval.value {
                        map.map.get(&key).copied()
                    } else {
                        None
                    }
                };

                if let Some(next_handle) = next_handle_opt {
                    let new_next_handle =
                        self.unset_nested_recursive(next_handle, remaining_keys)?;

                    // Only update if changed (if next_handle is a ref, it's mutated in place)
                    if new_next_handle != next_handle {
                        let current_zval = self.arena.get_mut(current_handle);
                        if let Val::Array(ref mut map) = current_zval.value {
                            Rc::make_mut(map).insert(key, new_next_handle);
                        }
                    }
                }
                // If the key doesn't exist, there's nothing to unset - silently succeed
            }

            return Ok(current_handle);
        }

        // Not a reference - COW: Clone current array
        let current_zval = self.arena.get(current_handle);
        let mut new_val = current_zval.value.clone();

        if let Val::Array(ref mut map) = new_val {
            let map_mut = Rc::make_mut(map);
            let key_val = &self.arena.get(key_handle).value;
            let key = self.array_key_from_value(key_val)?;

            if remaining_keys.is_empty() {
                // We are at the last key - remove it
                map_mut.map.shift_remove(&key);
            } else {
                // We need to go deeper
                if let Some(next_handle) = map_mut.map.get(&key) {
                    let new_next_handle =
                        self.unset_nested_recursive(*next_handle, remaining_keys)?;
                    map_mut.insert(key, new_next_handle);
                }
                // If the key doesn't exist, there's nothing to unset - silently succeed
            }
        }
        // If not an array, there's nothing to unset - silently succeed

        let new_handle = self.arena.alloc(new_val);
        Ok(new_handle)
    }

    #[inline]
    fn array_key_from_value(&self, value: &Val) -> Result<ArrayKey, VmError> {
        match value {
            Val::Int(i) => Ok(ArrayKey::Int(*i)),
            Val::Bool(b) => Ok(ArrayKey::Int(if *b { 1 } else { 0 })),
            Val::Float(f) => Ok(ArrayKey::Int(*f as i64)),
            Val::String(s) => {
                if let Ok(text) = std::str::from_utf8(s) {
                    if let Ok(int_val) = text.parse::<i64>() {
                        return Ok(ArrayKey::Int(int_val));
                    }
                }
                Ok(ArrayKey::Str(s.clone()))
            }
            Val::Null => Ok(ArrayKey::Str(Rc::new(Vec::new()))),
            Val::Object(payload_handle) => Err(VmError::RuntimeError(format!(
                "TypeError: Cannot access offset of type {} on array",
                self.describe_object_class(*payload_handle)
            ))),
            _ => Err(VmError::RuntimeError(format!(
                "Illegal offset type {}",
                value.type_name()
            ))),
        }
    }

    /// Check if a value matches the expected return type
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_verify_internal_return_type, zend_check_type
    fn check_return_type(
        &mut self,
        val_handle: Handle,
        ret_type: &ReturnType,
    ) -> Result<bool, VmError> {
        let val = &self.arena.get(val_handle).value;

        match ret_type {
            ReturnType::Void => {
                // void must return null
                Ok(matches!(val, Val::Null))
            }
            ReturnType::Never => {
                // never-returning function must not return at all (should have exited or thrown)
                Ok(false)
            }
            ReturnType::Mixed => {
                // mixed accepts any type
                Ok(true)
            }
            ReturnType::Int => {
                // In strict mode, only exact type matches; in weak mode, coercion is attempted
                match val {
                    Val::Int(_) => Ok(true),
                    _ => Ok(false),
                }
            }
            ReturnType::Float => {
                // Float accepts int or float in strict mode (int->float is allowed)
                match val {
                    Val::Float(_) => Ok(true),
                    Val::Int(_) => Ok(true), // SSTH exception: int may be accepted as float
                    _ => Ok(false),
                }
            }
            ReturnType::String => Ok(matches!(val, Val::String(_))),
            ReturnType::Bool => Ok(matches!(val, Val::Bool(_))),
            ReturnType::Array => Ok(matches!(val, Val::Array(_))),
            ReturnType::Object => Ok(matches!(val, Val::Object(_))),
            ReturnType::Null => Ok(matches!(val, Val::Null)),
            ReturnType::True => Ok(matches!(val, Val::Bool(true))),
            ReturnType::False => Ok(matches!(val, Val::Bool(false))),
            ReturnType::Callable => {
                // Check if value is callable (string function name, closure, or array [obj, method])
                // Reference: $PHP_SRC_PATH/Zend/zend_API.c - zend_is_callable
                Ok(self.is_callable(val_handle))
            }
            ReturnType::Iterable => {
                // iterable accepts arrays and Traversable objects
                match val {
                    Val::Array(_) => Ok(true),
                    Val::Object(_) => {
                        // Check if object implements Traversable
                        let traversable_sym = self.context.interner.intern(b"Traversable");
                        Ok(self.is_instance_of(val_handle, traversable_sym))
                    }
                    _ => Ok(false),
                }
            }
            ReturnType::Named(class_sym) => {
                // Check if value is instance of the named class
                match val {
                    Val::Object(_) => Ok(self.is_instance_of(val_handle, *class_sym)),
                    _ => Ok(false),
                }
            }
            ReturnType::Union(types) => {
                // Check if value matches any of the union types
                for ty in types {
                    if self.check_return_type(val_handle, ty)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            ReturnType::Intersection(types) => {
                // Check if value matches all intersection types
                for ty in types {
                    if !self.check_return_type(val_handle, ty)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            ReturnType::Nullable(inner) => {
                // Nullable accepts null or the inner type
                if matches!(val, Val::Null) {
                    Ok(true)
                } else {
                    self.check_return_type(val_handle, inner)
                }
            }
            ReturnType::Static => {
                // static return type means it must return an instance of the called class
                match val {
                    Val::Object(_) => {
                        // Get the called scope from the current frame
                        let frame = self.current_frame()?;
                        if let Some(called_scope) = frame.called_scope {
                            Ok(self.is_instance_of(val_handle, called_scope))
                        } else {
                            Ok(false)
                        }
                    }
                    _ => Ok(false),
                }
            }
        }
    }

    /// Check if a value is callable
    /// Reference: $PHP_SRC_PATH/Zend/zend_API.c - zend_is_callable
    fn is_callable(&mut self, val_handle: Handle) -> bool {
        let val = &self.arena.get(val_handle).value;

        match val {
            // String: function name
            Val::String(s) => {
                if let Ok(_func_name) = std::str::from_utf8(s) {
                    let func_sym = self.context.interner.intern(s);
                    // Check if it's a registered function
                    self.context.user_functions.contains_key(&func_sym)
                        || self.context.engine.registry.get_function(s).is_some()
                } else {
                    false
                }
            }
            // Object: check for Closure or __invoke
            Val::Object(payload_handle) => {
                if let Val::ObjPayload(obj_data) = &self.arena.get(*payload_handle).value {
                    // Check if it's a Closure
                    let closure_sym = self.context.interner.intern(b"Closure");
                    if self.is_instance_of_class(obj_data.class, closure_sym) {
                        return true;
                    }

                    // Check if it has __invoke method
                    let invoke_sym = self.context.interner.intern(b"__invoke");
                    if let Some(_) = self.find_method(obj_data.class, invoke_sym) {
                        return true;
                    }
                }
                false
            }
            // Array: [object/class, method] or [class, static_method]
            Val::Array(arr_data) => {
                if arr_data.map.len() != 2 {
                    return false;
                }

                // Check if we have indices 0 and 1
                let key0 = ArrayKey::Int(0);
                let key1 = ArrayKey::Int(1);

                if let (Some(&class_or_obj_handle), Some(&method_handle)) =
                    (arr_data.map.get(&key0), arr_data.map.get(&key1))
                {
                    // Method name must be a string
                    let method_val = &self.arena.get(method_handle).value;
                    if let Val::String(method_name) = method_val {
                        let method_sym = self.context.interner.intern(method_name);

                        let class_or_obj_val = &self.arena.get(class_or_obj_handle).value;
                        match class_or_obj_val {
                            // [object, method]
                            Val::Object(payload_handle) => {
                                if let Val::ObjPayload(obj_data) =
                                    &self.arena.get(*payload_handle).value
                                {
                                    // Check if method exists
                                    self.find_method(obj_data.class, method_sym).is_some()
                                } else {
                                    false
                                }
                            }
                            // ["ClassName", "method"]
                            Val::String(class_name) => {
                                let class_sym = self.context.interner.intern(class_name);
                                if let Ok(resolved_class) = self.resolve_class_name(class_sym) {
                                    // Check if static method exists
                                    self.find_method(resolved_class, method_sym).is_some()
                                } else {
                                    false
                                }
                            }
                            _ => false,
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Check and coerce parameter type based on strictness
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_verify_arg_type
    fn check_parameter_type(
        &mut self,
        arg_handle: Handle,
        param_type: &ReturnType,
        strict: bool,
        param_name: Symbol,
        func_name: &str,
    ) -> Result<Handle, VmError> {
        // Check if type matches
        let matches = self.check_return_type(arg_handle, param_type)?;

        if matches {
            return Ok(arg_handle);
        }

        // Type doesn't match - decide whether to coerce or error
        if strict {
            // Strict mode: no coercion, throw TypeError
            let param_name_str = String::from_utf8_lossy(
                self.context.interner.lookup(param_name).unwrap_or(b"?"),
            );
            let expected = self.return_type_name(param_type);
            let val_type = self.arena.get(arg_handle).value.type_name();
            return Err(VmError::RuntimeError(format!(
                "{}(): Argument #{} (${}
) must be of type {}, {} given",
                func_name, param_name_str, param_name_str, expected, val_type
            )));
        }

        // Weak mode: attempt coercion for scalar types
        let coerced = self.coerce_parameter_value(arg_handle, param_type)?;
        if let Some(coerced_handle) = coerced {
            Ok(coerced_handle)
        } else {
            // Coercion failed - emit warning and use original value
            let param_name_str = String::from_utf8_lossy(
                self.context.interner.lookup(param_name).unwrap_or(b"?"),
            );
            let expected = self.return_type_name(param_type);
            let val_type = self.arena.get(arg_handle).value.type_name();
            let message = format!(
                "{}(): Argument #{} (${}
) must be of type {}, {} given",
                func_name, param_name_str, param_name_str, expected, val_type
            );
            self.trigger_error(ErrorLevel::Warning, &message);
            Ok(arg_handle)
        }
    }

    /// Attempt to coerce a parameter value to the expected type (weak mode only)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_verify_scalar_type_hint
    fn coerce_parameter_value(
        &mut self,
        arg_handle: Handle,
        target_type: &ReturnType,
    ) -> Result<Option<Handle>, VmError> {
        let val = &self.arena.get(arg_handle).value;

        match target_type {
            ReturnType::Int => {
                // Attempt to convert to int
                match val {
                    Val::Int(_) => Ok(Some(arg_handle)),
                    Val::Float(f) => Ok(Some(self.arena.alloc(Val::Int(*f as i64)))),
                    Val::Bool(b) => Ok(Some(self.arena.alloc(Val::Int(if *b { 1 } else { 0 })))),
                    Val::String(s) => {
                        // Try to parse string as int
                        if let Ok(text) = std::str::from_utf8(s) {
                            let trimmed = text.trim();
                            if let Ok(int_val) = trimmed.parse::<i64>() {
                                return Ok(Some(self.arena.alloc(Val::Int(int_val))));
                            }
                        }
                        Ok(None) // Cannot coerce
                    }
                    Val::Null => Ok(Some(self.arena.alloc(Val::Int(0)))),
                    _ => Ok(None),
                }
            }
            ReturnType::Float => {
                // Attempt to convert to float
                match val {
                    Val::Float(_) => Ok(Some(arg_handle)),
                    Val::Int(i) => Ok(Some(self.arena.alloc(Val::Float(*i as f64)))),
                    Val::Bool(b) => Ok(Some(self.arena.alloc(Val::Float(if *b { 1.0 } else { 0.0 })))),
                    Val::String(s) => {
                        if let Ok(text) = std::str::from_utf8(s) {
                            let trimmed = text.trim();
                            if let Ok(float_val) = trimmed.parse::<f64>() {
                                return Ok(Some(self.arena.alloc(Val::Float(float_val))));
                            }
                        }
                        Ok(None)
                    }
                    Val::Null => Ok(Some(self.arena.alloc(Val::Float(0.0)))),
                    _ => Ok(None),
                }
            }
            ReturnType::String => {
                // Attempt to convert to string
                match val {
                    Val::String(_) => Ok(Some(arg_handle)),
                    Val::Int(i) => Ok(Some(self.arena.alloc(Val::String(Rc::new(i.to_string().into_bytes()))))),
                    Val::Float(f) => Ok(Some(self.arena.alloc(Val::String(Rc::new(f.to_string().into_bytes()))))),
                    Val::Bool(b) => Ok(Some(self.arena.alloc(Val::String(Rc::new(if *b { b"1".to_vec() } else { vec![] }))))),
                    Val::Null => Ok(Some(self.arena.alloc(Val::String(Rc::new(vec![]))))),
                    _ => Ok(None),
                }
            }
            ReturnType::Bool => {
                // Convert to bool
                let bool_val = self.value_to_bool(arg_handle);
                Ok(Some(self.arena.alloc(Val::Bool(bool_val))))
            }
            ReturnType::Nullable(inner) => {
                // Nullable accepts null or the inner type
                if matches!(val, Val::Null) {
                    Ok(Some(arg_handle))
                } else {
                    self.coerce_parameter_value(arg_handle, inner)
                }
            }
            // Union types: try each type in order
            ReturnType::Union(types) => {
                for ty in types {
                    if let Ok(Some(coerced)) = self.coerce_parameter_value(arg_handle, ty) {
                        return Ok(Some(coerced));
                    }
                }
                Ok(None)
            }
            // Non-scalar types cannot be coerced
            _ => Ok(None),
        }
    }

    /// Get a human-readable type name from ReturnType
    fn return_type_name(&self, ty: &ReturnType) -> String {
        match ty {
            ReturnType::Int => "int".to_string(),
            ReturnType::Float => "float".to_string(),
            ReturnType::String => "string".to_string(),
            ReturnType::Bool => "bool".to_string(),
            ReturnType::Array => "array".to_string(),
            ReturnType::Object => "object".to_string(),
            ReturnType::Void => "void".to_string(),
            ReturnType::Never => "never".to_string(),
            ReturnType::Mixed => "mixed".to_string(),
            ReturnType::Null => "null".to_string(),
            ReturnType::True => "true".to_string(),
            ReturnType::False => "false".to_string(),
            ReturnType::Callable => "callable".to_string(),
            ReturnType::Iterable => "iterable".to_string(),
            ReturnType::Static => "static".to_string(),
            ReturnType::Named(sym) => {
                String::from_utf8_lossy(self.context.interner.lookup(*sym).unwrap_or(b"?")).to_string()
            }
            ReturnType::Union(types) => {
                let names: Vec<String> = types.iter().map(|t| self.return_type_name(t)).collect();
                names.join("|")
            }
            ReturnType::Intersection(types) => {
                let names: Vec<String> = types.iter().map(|t| self.return_type_name(t)).collect();
                names.join("&")
            }
            ReturnType::Nullable(inner) => format!("?{}", self.return_type_name(inner)),
        }
    }

    /// Get a human-readable type name for a value
    /// Check if a class is a subclass of another (or the same class)

    // ========================================
    // Built-in Function Type Validation Helpers
    // Reference: $PHP_SRC_PATH/Zend/zend_API.c - zend_parse_arg_*
    // ========================================

    /// Validate and coerce a parameter to string for built-in functions
    /// Respects the caller's strict_types mode
    /// Note: Arrays and objects should be handled separately by the caller
    /// (PHP emits Warning and returns null, not TypeError)
    pub(crate) fn check_builtin_param_string(
        &mut self,
        arg: Handle,
        param_num: usize,
        func_name: &str,
    ) -> Result<Vec<u8>, String> {
        let val = &self.arena.get(arg).value;
        match val {
            Val::String(s) => Ok(s.to_vec()),
            Val::Int(_) | Val::Float(_) | Val::Bool(_) | Val::Null => {
                if self.builtin_call_strict {
                    // Strict mode: reject coercion
                    return Err(format!(
                        "{}(): Argument #{} must be of type string, {} given",
                        func_name, param_num, val.type_name()
                    ));
                }
                // Weak mode: coerce to string
                Ok(val.to_php_string_bytes())
            }
            Val::Array(_) | Val::ConstArray(_) | Val::Object(_) | Val::ObjPayload(_) => {
                // Arrays/Objects should be handled by caller with warnings
                // Don't use TypeError here - PHP uses Warning + null
                Err(format!(
                    "{}(): Argument #{} must be of type string, {} given",
                    func_name, param_num, val.type_name()
                ))
            }
            _ => Ok(vec![]),
        }
    }

    /// Validate and coerce a parameter to int for built-in functions
    /// Respects the caller's strict_types mode
    pub(crate) fn check_builtin_param_int(
        &mut self,
        arg: Handle,
        param_num: usize,
        func_name: &str,
    ) -> Result<i64, String> {
        let val = &self.arena.get(arg).value;
        match val {
            Val::Int(i) => Ok(*i),
            Val::Float(f) => {
                if self.builtin_call_strict {
                    // Strict mode: reject float to int (unlike int->float)
                    return Err(format!(
                        "{}(): Argument #{} must be of type int, float given",
                        func_name, param_num
                    ));
                }
                // Weak mode: truncate float to int
                Ok(*f as i64)
            }
            Val::Bool(b) => {
                if self.builtin_call_strict {
                    return Err(format!(
                        "{}(): Argument #{} must be of type int, bool given",
                        func_name, param_num
                    ));
                }
                Ok(if *b { 1 } else { 0 })
            }
            Val::String(s) => {
                if self.builtin_call_strict {
                    return Err(format!(
                        "{}(): Argument #{} must be of type int, string given",
                        func_name, param_num
                    ));
                }
                // Weak mode: parse string to int
                if let Ok(text) = std::str::from_utf8(s) {
                    if let Ok(int_val) = text.trim().parse::<i64>() {
                        return Ok(int_val);
                    }
                }
                Ok(0) // Non-numeric strings become 0
            }
            Val::Null => {
                if self.builtin_call_strict {
                    return Err(format!(
                        "{}(): Argument #{} must be of type int, null given",
                        func_name, param_num
                    ));
                }
                Ok(0)
            }
            _ => Err(format!(
                "{}(): Argument #{} must be of type int, {} given",
                func_name, param_num, val.type_name()
            )),
        }
    }

    /// Validate and coerce a parameter to bool for built-in functions
    /// Respects the caller's strict_types mode
    pub(crate) fn check_builtin_param_bool(
        &mut self,
        arg: Handle,
        param_num: usize,
        func_name: &str,
    ) -> Result<bool, String> {
        let val = &self.arena.get(arg).value;
        match val {
            Val::Bool(b) => Ok(*b),
            _ => {
                if self.builtin_call_strict {
                    // Strict mode: only bool accepted
                    return Err(format!(
                        "{}(): Argument #{} must be of type bool, {} given",
                        func_name, param_num, val.type_name()
                    ));
                }
                // Weak mode: convert to bool using PHP rules
                Ok(val.to_bool())
            }
        }
    }

    /// Validate and coerce a parameter to array for built-in functions
    /// Arrays cannot be coerced, so this only validates
    pub(crate) fn check_builtin_param_array(
        &self,
        arg: Handle,
        param_num: usize,
        func_name: &str,
    ) -> Result<(), String> {
        let val = &self.arena.get(arg).value;
        if matches!(val, Val::Array(_) | Val::ConstArray(_)) {
            Ok(())
        } else {
            Err(format!(
                "{}(): Argument #{} must be of type array, {} given",
                func_name, param_num, val.type_name()
            ))
        }
    }

    /// Get a human-readable type name for a value
    /// Check if a class is a subclass of another (or the same class)
    fn is_instance_of_class(&self, obj_class: Symbol, target_class: Symbol) -> bool {
        if obj_class == target_class {
            return true;
        }

        // Check parent classes
        if let Some(class_def) = self.context.classes.get(&obj_class) {
            if let Some(parent) = class_def.parent {
                return self.is_instance_of_class(parent, target_class);
            }
        }

        false
    }

    /// Convert a ReturnType to a human-readable string
    fn return_type_to_string(&self, ret_type: &ReturnType) -> String {
        match ret_type {
            ReturnType::Int => "int".to_string(),
            ReturnType::Float => "float".to_string(),
            ReturnType::String => "string".to_string(),
            ReturnType::Bool => "bool".to_string(),
            ReturnType::Array => "array".to_string(),
            ReturnType::Object => "object".to_string(),
            ReturnType::Void => "void".to_string(),
            ReturnType::Never => "never".to_string(),
            ReturnType::Mixed => "mixed".to_string(),
            ReturnType::Null => "null".to_string(),
            ReturnType::True => "true".to_string(),
            ReturnType::False => "false".to_string(),
            ReturnType::Callable => "callable".to_string(),
            ReturnType::Iterable => "iterable".to_string(),
            ReturnType::Named(sym) => self
                .context
                .interner
                .lookup(*sym)
                .map(|bytes| String::from_utf8_lossy(bytes).to_string())
                .unwrap_or_else(|| "object".to_string()),
            ReturnType::Union(types) => types
                .iter()
                .map(|t| self.return_type_to_string(t))
                .collect::<Vec<_>>()
                .join("|"),
            ReturnType::Intersection(types) => types
                .iter()
                .map(|t| self.return_type_to_string(t))
                .collect::<Vec<_>>()
                .join("&"),
            ReturnType::Nullable(inner) => {
                format!("?{}", self.return_type_to_string(inner))
            }
            ReturnType::Static => "static".to_string(),
        }
    }

    fn exec_call_method(
        &mut self,
        method_name: Symbol,
        arg_count: u8,
        is_dynamic: bool,
    ) -> Result<(), VmError> {
        let obj_handle = self
            .operand_stack
            .peek_at(arg_count as usize + if is_dynamic { 1 } else { 0 })
            .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

        let class_name = if let Val::Object(h) = self.arena.get(obj_handle).value {
            if let Val::ObjPayload(data) = &self.arena.get(h).value {
                data.class
            } else {
                return Err(VmError::RuntimeError("Invalid object payload".into()));
            }
        } else {
            return Err(VmError::RuntimeError(
                "Call to member function on non-object".into(),
            ));
        };

        // Check for native method first
        let native_method = self.find_native_method(class_name, method_name);
        if let Some(native_entry) = native_method {
            self.check_method_visibility(
                native_entry.declaring_class,
                native_entry.visibility,
                Some(method_name),
            )?;

            // Collect args
            let args = self.collect_call_args(arg_count)?;

            // Pop method name if dynamic
            if is_dynamic {
                self.operand_stack.pop();
            }

            // Pop object
            let obj_handle = self.operand_stack.pop().unwrap();

            // Set this in current frame temporarily for native method to access
            let saved_this = self.frames.last().and_then(|f| f.this);
            if let Some(frame) = self.frames.last_mut() {
                frame.this = Some(obj_handle);
            }

            // Call native handler
            let result = (native_entry.handler)(self, &args).map_err(VmError::RuntimeError)?;

            // Restore previous this
            if let Some(frame) = self.frames.last_mut() {
                frame.this = saved_this;
            }

            self.operand_stack.push(result);
        } else {
            let mut method_lookup = self.find_method(class_name, method_name);

            if method_lookup.is_none() {
                // Fallback: Check if we are in a scope that has this method as private.
                // This handles calling private methods of parent class from parent scope on child object.
                if let Some(scope) = self.get_current_class() {
                    if let Some((func, vis, is_static, decl_class)) =
                        self.find_method(scope, method_name)
                    {
                        if vis == Visibility::Private && decl_class == scope {
                            method_lookup = Some((func, vis, is_static, decl_class));
                        }
                    }
                }
            }

            if let Some((user_func, visibility, is_static, defined_class)) = method_lookup {
                self.check_method_visibility(defined_class, visibility, Some(method_name))?;

                let args = self.collect_call_args(arg_count)?;

                if is_dynamic {
                    self.operand_stack.pop();
                }
                let obj_handle = self.operand_stack.pop().unwrap();

                let mut frame = CallFrame::new(user_func.chunk.clone());
                frame.func = Some(user_func.clone());
                if !is_static {
                    frame.this = Some(obj_handle);
                }
                frame.class_scope = Some(defined_class);
                frame.called_scope = Some(class_name);
                frame.args = args;

                self.push_frame(frame);
            } else {
                // Method not found. Check for __call.
                let call_magic = self.context.interner.intern(b"__call");
                if let Some((magic_func, _, _, magic_class)) =
                    self.find_method(class_name, call_magic)
                {
                    // Found __call.

                    // Pop args
                    let args = self.collect_call_args(arg_count)?;

                    if is_dynamic {
                        self.operand_stack.pop();
                    }
                    let obj_handle = self.operand_stack.pop().unwrap();

                    // Create array from args
                    let mut array_map = IndexMap::new();
                    for (i, arg) in args.into_iter().enumerate() {
                        array_map.insert(ArrayKey::Int(i as i64), arg);
                    }
                    let args_array_handle = self
                        .arena
                        .alloc(Val::Array(ArrayData::from(array_map).into()));

                    // Create method name string
                    let method_name_str = self
                        .context
                        .interner
                        .lookup(method_name)
                        .expect("Method name should be interned")
                        .to_vec();
                    let name_handle = self.arena.alloc(Val::String(method_name_str.into()));

                    // Prepare frame for __call
                    let mut frame = CallFrame::new(magic_func.chunk.clone());
                    frame.func = Some(magic_func.clone());
                    frame.this = Some(obj_handle);
                    frame.class_scope = Some(magic_class);
                    frame.called_scope = Some(class_name);
                    let mut frame_args = ArgList::new();
                    frame_args.push(name_handle);
                    frame_args.push(args_array_handle);
                    frame.args = frame_args;

                    // Pass args: $name, $arguments
                    // Param 0: name
                    if let Some(param) = magic_func.params.get(0) {
                        frame.locals.insert(param.name, frame.args[0]);
                    }
                    // Param 1: arguments
                    if let Some(param) = magic_func.params.get(1) {
                        frame.locals.insert(param.name, frame.args[1]);
                    }

                    self.push_frame(frame);
                } else {
                    let method_str = String::from_utf8_lossy(
                        self.context
                            .interner
                            .lookup(method_name)
                            .unwrap_or(b"<unknown>"),
                    );
                    return Err(VmError::RuntimeError(format!(
                        "Call to undefined method {}",
                        method_str
                    )));
                }
            }
        }
        Ok(())
    }

    fn exec_call_static_method(
        &mut self,
        class_name: Symbol,
        method_name: Symbol,
        arg_count: u8,
        is_dynamic: bool,
    ) -> Result<(), VmError> {
        let resolved_class = if is_dynamic {
            class_name
        } else {
            self.resolve_class_name(class_name)?
        };

        let mut method_lookup = self.find_method(resolved_class, method_name);

        if method_lookup.is_none() {
            if let Some(scope) = self.get_current_class() {
                if let Some((func, vis, is_static, decl_class)) =
                    self.find_method(scope, method_name)
                {
                    if vis == Visibility::Private && decl_class == scope {
                        method_lookup = Some((func, vis, is_static, decl_class));
                    }
                }
            }
        }

        if let Some((user_func, visibility, is_static, defined_class)) = method_lookup {
            let mut this_handle = None;
            if !is_static {
                if let Some(current_frame) = self.frames.last() {
                    if let Some(th) = current_frame.this {
                        if self.is_instance_of(th, defined_class) {
                            this_handle = Some(th);
                        }
                    }
                }
                if this_handle.is_none() {
                    return Err(VmError::RuntimeError(
                        "Non-static method called statically".into(),
                    ));
                }
            }

            self.check_method_visibility(defined_class, visibility, Some(method_name))?;

            let args = self.collect_call_args(arg_count)?;

            if is_dynamic {
                self.operand_stack.pop(); // method name
                self.operand_stack.pop(); // class name
            }

            let mut frame = CallFrame::new(user_func.chunk.clone());
            frame.func = Some(user_func.clone());
            frame.this = this_handle;
            frame.class_scope = Some(defined_class);
            frame.called_scope = Some(resolved_class);
            frame.args = args;

            self.push_frame(frame);
        } else {
            // Method not found. Check for __callStatic.
            let call_static_magic = self.context.interner.intern(b"__callStatic");
            if let Some((magic_func, _, is_static, magic_class)) =
                self.find_method(resolved_class, call_static_magic)
            {
                if !is_static {
                    return Err(VmError::RuntimeError("__callStatic must be static".into()));
                }

                // Pop args
                let args = self.collect_call_args(arg_count)?;

                if is_dynamic {
                    self.operand_stack.pop(); // method name
                    self.operand_stack.pop(); // class name
                }

                // Create array from args
                let mut array_map = IndexMap::new();
                for (i, arg) in args.into_iter().enumerate() {
                    array_map.insert(ArrayKey::Int(i as i64), arg);
                }
                let args_array_handle = self
                    .arena
                    .alloc(Val::Array(ArrayData::from(array_map).into()));

                // Create method name string
                let method_name_str = self
                    .context
                    .interner
                    .lookup(method_name)
                    .expect("Method name should be interned")
                    .to_vec();
                let name_handle = self.arena.alloc(Val::String(method_name_str.into()));

                // Prepare frame for __callStatic
                let mut frame = CallFrame::new(magic_func.chunk.clone());
                frame.func = Some(magic_func.clone());
                frame.this = None;
                frame.class_scope = Some(magic_class);
                frame.called_scope = Some(resolved_class);
                let mut frame_args = ArgList::new();
                frame_args.push(name_handle);
                frame_args.push(args_array_handle);
                frame.args = frame_args;

                // Pass args: $name, $arguments
                // Param 0: name
                if let Some(param) = magic_func.params.get(0) {
                    frame.locals.insert(param.name, frame.args[0]);
                }
                // Param 1: arguments
                if let Some(param) = magic_func.params.get(1) {
                    frame.locals.insert(param.name, frame.args[1]);
                }

                self.push_frame(frame);
            } else {
                let method_str = String::from_utf8_lossy(
                    self.context
                        .interner
                        .lookup(method_name)
                        .unwrap_or(b"<unknown>"),
                );
                return Err(VmError::RuntimeError(format!(
                    "Call to undefined static method {}::{}",
                    String::from_utf8_lossy(
                        self.context
                            .interner
                            .lookup(resolved_class)
                            .unwrap_or(b"<unknown>")
                    ),
                    method_str
                )));
            }
        }
        Ok(())
    }

    /// Validate that a class properly implements an interface
    fn validate_interface_implementation(
        &self,
        class_name: Symbol,
        interface_name: Symbol,
    ) -> Result<(), VmError> {
        // 1. Check that interface exists and is actually an interface
        let interface_def = self.context.classes.get(&interface_name)
            .ok_or_else(|| {
                let name = self.context.interner.lookup(interface_name)
                    .map(|b| String::from_utf8_lossy(b).to_string())
                    .unwrap_or_else(|| format!("{:?}", interface_name));
                VmError::RuntimeError(format!("Interface {} not found", name))
            })?;
        
        if !interface_def.is_interface {
            let iface_name = self.context.interner.lookup(interface_name)
                .map(|b| String::from_utf8_lossy(b).to_string())
                .unwrap_or_else(|| format!("{:?}", interface_name));
            let class_name_str = self.context.interner.lookup(class_name)
                .map(|b| String::from_utf8_lossy(b).to_string())
                .unwrap_or_else(|| format!("{:?}", class_name));
            return Err(VmError::RuntimeError(format!(
                "{} cannot implement {} - it is not an interface",
                class_name_str, iface_name
            )));
        }

        // 2. Check for enum-specific interfaces
        let interface_name_bytes = self.context.interner.lookup(interface_name).unwrap_or(b"");
        if interface_name_bytes == b"BackedEnum" || interface_name_bytes == b"UnitEnum" {
            let class_def = self.context.classes.get(&class_name)
                .ok_or_else(|| VmError::RuntimeError("Class not found".into()))?;
            
            if !class_def.is_enum {
                let class_name_str = self.context.interner.lookup(class_name)
                    .map(|b| String::from_utf8_lossy(b).to_string())
                    .unwrap_or_else(|| format!("{:?}", class_name));
                let iface_name_str = String::from_utf8_lossy(interface_name_bytes);
                return Err(VmError::RuntimeError(format!(
                    "Non-enum class {} cannot implement interface {}",
                    class_name_str, iface_name_str
                )));
            }

            // For BackedEnum, validate backing type
            if interface_name_bytes == b"BackedEnum" && class_def.enum_backed_type.is_none() {
                let class_name_str = self.context.interner.lookup(class_name)
                    .map(|b| String::from_utf8_lossy(b).to_string())
                    .unwrap_or_else(|| format!("{:?}", class_name));
                return Err(VmError::RuntimeError(format!(
                    "Enum {} must be a backed enum to implement BackedEnum",
                    class_name_str
                )));
            }
        }

        // 3. Collect required interface methods
        let required_methods = self.collect_interface_methods(interface_name);
        
        // 4. Get implementing class
        let class_def = self.context.classes.get(&class_name)
            .ok_or_else(|| VmError::RuntimeError("Class not found".into()))?;
        
        // 5. Validate each required method is implemented
        for (method_name, iface_method) in &required_methods {
            let class_name_str = self.context.interner.lookup(class_name)
                .map(|b| String::from_utf8_lossy(b).to_string())
                .unwrap_or_else(|| format!("{:?}", class_name));
            let iface_name_str = self.context.interner.lookup(interface_name)
                .map(|b| String::from_utf8_lossy(b).to_string())
                .unwrap_or_else(|| format!("{:?}", interface_name));
            let method_name_str = self.context.interner.lookup(*method_name)
                .map(|b| String::from_utf8_lossy(b).to_string())
                .unwrap_or_else(|| format!("{:?}", method_name));
                
            if !class_def.methods.contains_key(method_name) {
                return Err(VmError::RuntimeError(format!(
                    "Class {} contains 1 abstract method and must therefore be declared abstract or implement the remaining method ({}::{})",
                    class_name_str, iface_name_str, method_name_str
                )));
            }
            
            // Validate signature compatibility (parameter and return types)
            if let Some(class_method) = class_def.methods.get(method_name) {
                // Validate parameter types (contravariance)
                for (i, iface_param) in iface_method.signature.parameters.iter().enumerate() {
                    if i >= class_method.signature.parameters.len() {
                        return Err(VmError::RuntimeError(format!(
                            "Declaration of {}::{}() must be compatible with {}::{}()",
                            class_name_str, method_name_str, iface_name_str, method_name_str
                        )));
                    }
                    let class_param = &class_method.signature.parameters[i];
                    
                    // Interface parameter types must match exactly or be contravariant
                    if let Some(iface_type) = &iface_param.type_hint {
                        match &class_param.type_hint {
                            None => {
                                let param_name = self.context.interner.lookup(class_param.name)
                                    .map(|b| String::from_utf8_lossy(b).to_string())
                                    .unwrap_or_else(|| format!("{:?}", class_param.name));
                                return Err(VmError::RuntimeError(format!(
                                    "Type of parameter ${} must be compatible with interface in {}::{}()",
                                    param_name, class_name_str, method_name_str
                                )));
                            }
                            Some(class_type) => {
                                if !self.types_equal(class_type, iface_type) && !self.is_type_contravariant(class_type, iface_type) {
                                    return Err(VmError::RuntimeError(format!(
                                        "Declaration of {}::{}() must be compatible with {}::{}()",
                                        class_name_str, method_name_str, iface_name_str, method_name_str
                                    )));
                                }
                            }
                        }
                    }
                }
                
                // Validate return type (covariance)
                match (&iface_method.signature.return_type, &class_method.signature.return_type) {
                    (Some(iface_ret), None) => {
                        return Err(VmError::RuntimeError(format!(
                            "Declaration of {}::{}() must be compatible with interface return type",
                            class_name_str, method_name_str
                        )));
                    }
                    (Some(iface_ret), Some(class_ret)) => {
                        if !self.types_equal(class_ret, iface_ret) && !self.is_type_covariant(class_ret, iface_ret) {
                            return Err(VmError::RuntimeError(format!(
                                "Declaration of {}::{}() must be compatible with interface return type",
                                class_name_str, method_name_str
                            )));
                        }
                    }
                    _ => {} // Interface has no return type, class can have any
                }
            }
        }
        
        Ok(())
    }

    /// Collect all method signatures required by an interface (including parent interfaces)
    fn collect_interface_methods(&self, interface_sym: Symbol) -> HashMap<Symbol, MethodEntry> {
        let mut methods = HashMap::new();
        
        if let Some(interface_def) = self.context.classes.get(&interface_sym) {
            if !interface_def.is_interface {
                return methods;
            }
            
            // Collect methods from this interface
            for (method_name, method_entry) in &interface_def.methods {
                methods.insert(*method_name, method_entry.clone());
            }
            
            // Recursively collect from parent interfaces
            for &parent_interface in &interface_def.interfaces {
                methods.extend(self.collect_interface_methods(parent_interface));
            }
        }
        
        methods
    }

    /// Validate that all abstract methods inherited by a concrete class are implemented
    /// Reference: $PHP_SRC_PATH/Zend/zend_compile.c - zend_verify_abstract_class
    fn validate_abstract_methods_implemented(&self, class_name: Symbol) -> Result<(), VmError> {
        let class_def = self.context.classes.get(&class_name)
            .ok_or_else(|| VmError::RuntimeError("Class not found".into()))?;
        
        // Don't check abstract classes
        if class_def.is_abstract {
            return Ok(());
        }
        
        // Check if there are any unimplemented abstract methods
        // The abstract_methods set should contain only methods that are still abstract after inheritance
        if !class_def.abstract_methods.is_empty() {
            let class_name_str = self.context.interner.lookup(class_name)
                .map(|b| String::from_utf8_lossy(b).to_string())
                .unwrap_or_else(|| format!("{:?}", class_name));
            
            // Find which class declared each abstract method
            let unimplemented: Vec<String> = class_def.abstract_methods.iter()
                .map(|&method_sym| {
                    // Find the declaring class by walking up the parent chain
                    let mut current = Some(class_name);
                    let mut declaring_class = class_name;
                    
                    while let Some(curr_sym) = current {
                        if let Some(curr_def) = self.context.classes.get(&curr_sym) {
                            if curr_def.methods.contains_key(&method_sym) {
                                if let Some(method_entry) = curr_def.methods.get(&method_sym) {
                                    if method_entry.is_abstract {
                                        declaring_class = method_entry.declaring_class;
                                        break;
                                    }
                                }
                            }
                            current = curr_def.parent;
                        } else {
                            break;
                        }
                    }
                    
                    let method_name_str = self.context.interner.lookup(method_sym)
                        .map(|b| String::from_utf8_lossy(b).to_string())
                        .unwrap_or_else(|| format!("{:?}", method_sym));
                    let declaring_name_str = self.context.interner.lookup(declaring_class)
                        .map(|b| String::from_utf8_lossy(b).to_string())
                        .unwrap_or_else(|| format!("{:?}", declaring_class));
                    
                    format!("{}::{}", declaring_name_str, method_name_str)
                })
                .collect();
            
            return Err(VmError::RuntimeError(format!(
                "Class {} contains {} abstract method{} and must therefore be declared abstract or implement the remaining method{} ({})",
                class_name_str,
                unimplemented.len(),
                if unimplemented.len() == 1 { "" } else { "s" },
                if unimplemented.len() == 1 { "" } else { "s" },
                unimplemented.join(", ")
            )));
        }
        
        Ok(())
    }

    /// Validate method override compatibility
    /// Reference: $PHP_SRC_PATH/Zend/zend_inheritance.c - do_inheritance_check_on_method
    fn validate_method_override(
        &self,
        child_class: Symbol,
        method_name: Symbol,
        child_signature: &MethodSignature,
        child_static: bool,
        child_vis: Visibility,
        parent_func: &UserFunc,
        parent_static: bool,
        parent_vis: Visibility,
    ) -> Result<(), VmError> {
        let child_class_str = self.context.interner.lookup(child_class)
            .map(|b| String::from_utf8_lossy(b).to_string())
            .unwrap_or_else(|| format!("{:?}", child_class));
        let method_name_str = self.context.interner.lookup(method_name)
            .map(|b| String::from_utf8_lossy(b).to_string())
            .unwrap_or_else(|| format!("{:?}", method_name));

        // 1. Static/non-static must match
        if child_static != parent_static {
            return Err(VmError::RuntimeError(format!(
                "Cannot make {}static method {}::{}() {}static in class {}",
                if parent_static { "" } else { "non-" },
                method_name_str,
                method_name_str,
                if child_static { "" } else { "non-" },
                child_class_str,
            )));
        }
        
        // 2. Visibility can only widen (private -> protected -> public)
        let valid_visibility = match parent_vis {
            Visibility::Private => true, // Can override with any visibility
            Visibility::Protected => matches!(
                child_vis, 
                Visibility::Protected | Visibility::Public
            ),
            Visibility::Public => child_vis == Visibility::Public,
        };
        
        if !valid_visibility {
            let parent_vis_str = match parent_vis {
                Visibility::Public => "public",
                Visibility::Protected => "protected",
                Visibility::Private => "private",
            };
            return Err(VmError::RuntimeError(format!(
                "Access level to {}::{}() must be {} (as in parent class) or weaker",
                child_class_str,
                method_name_str,
                parent_vis_str,
            )));
        }
        
        // 3. Build parent signature from UserFunc for comparison
        let parent_signature = MethodSignature {
            parameters: parent_func.params.iter().map(|p| ParameterInfo {
                name: p.name,
                type_hint: p.param_type.as_ref().and_then(|rt| self.return_type_to_type_hint(rt)),
                is_reference: p.by_ref,
                is_variadic: p.is_variadic,
                default_value: p.default_value.clone(),
            }).collect(),
            return_type: parent_func.return_type.as_ref().and_then(|rt| self.return_type_to_type_hint(rt)),
        };
        
        // 4. Parameter count validation (can have more with defaults, but not fewer)
        if child_signature.parameters.len() < parent_signature.parameters.len() {
            return Err(VmError::RuntimeError(format!(
                "Declaration of {}::{}() must be compatible with parent signature",
                child_class_str,
                method_name_str,
            )));
        }
        
        // 5. Validate parameter types (contravariance)
        for (i, parent_param) in parent_signature.parameters.iter().enumerate() {
            let child_param = &child_signature.parameters[i];
            
            // If parent has no type hint, child can have any type hint or none
            if let Some(parent_type) = &parent_param.type_hint {
                match &child_param.type_hint {
                    None => {
                        // Child removed type hint - not allowed in PHP 8.x
                        let param_name = self.context.interner.lookup(child_param.name)
                            .map(|b| String::from_utf8_lossy(b).to_string())
                            .unwrap_or_else(|| format!("{:?}", child_param.name));
                        return Err(VmError::RuntimeError(format!(
                            "Type of parameter ${} must be compatible with parent in {}::{}()",
                            param_name,
                            child_class_str,
                            method_name_str,
                        )));
                    }
                    Some(child_type) => {
                        // Validate contravariance: child type must be same or wider
                        if !self.types_equal(child_type, parent_type) && !self.is_type_contravariant(child_type, parent_type) {
                            return Err(VmError::RuntimeError(format!(
                                "Declaration of {}::{}() must be compatible with parent signature",
                                child_class_str,
                                method_name_str,
                            )));
                        }
                    }
                }
            }
        }
        
        // 6. Validate return type (covariance)
        match (&parent_signature.return_type, &child_signature.return_type) {
            (None, _) => {}, // Parent has no return type - child can have any or none
            (Some(parent_type), None) => {
                // Parent has return type, child has none - not allowed
                return Err(VmError::RuntimeError(format!(
                    "Declaration of {}::{}() must be compatible with parent return type",
                    child_class_str,
                    method_name_str,
                )));
            }
            (Some(parent_type), Some(child_type)) => {
                // Both have return types - validate covariance
                if !self.types_equal(child_type, parent_type) && !self.is_type_covariant(child_type, parent_type) {
                    return Err(VmError::RuntimeError(format!(
                        "Declaration of {}::{}() must be compatible with parent return type",
                        child_class_str,
                        method_name_str,
                    )));
                }
            }
        }
        
        Ok(())
    }
    
    /// Check if two types are equal
    fn types_equal(&self, a: &TypeHint, b: &TypeHint) -> bool {
        match (a, b) {
            (TypeHint::Int, TypeHint::Int) => true,
            (TypeHint::Float, TypeHint::Float) => true,
            (TypeHint::String, TypeHint::String) => true,
            (TypeHint::Bool, TypeHint::Bool) => true,
            (TypeHint::Array, TypeHint::Array) => true,
            (TypeHint::Object, TypeHint::Object) => true,
            (TypeHint::Callable, TypeHint::Callable) => true,
            (TypeHint::Iterable, TypeHint::Iterable) => true,
            (TypeHint::Mixed, TypeHint::Mixed) => true,
            (TypeHint::Void, TypeHint::Void) => true,
            (TypeHint::Never, TypeHint::Never) => true,
            (TypeHint::Null, TypeHint::Null) => true,
            (TypeHint::Class(a_sym), TypeHint::Class(b_sym)) => a_sym == b_sym,
            (TypeHint::Union(a_types), TypeHint::Union(b_types)) => {
                a_types.len() == b_types.len() &&
                a_types.iter().all(|at| b_types.iter().any(|bt| self.types_equal(at, bt)))
            }
            (TypeHint::Intersection(a_types), TypeHint::Intersection(b_types)) => {
                a_types.len() == b_types.len() &&
                a_types.iter().all(|at| b_types.iter().any(|bt| self.types_equal(at, bt)))
            }
            _ => false,
        }
    }
    
    /// Check if child_type is contravariant with parent_type
    /// Contravariance: child can accept wider types
    /// Example: parent accepts Dog, child can accept Animal (wider)
    /// Reference: $PHP_SRC_PATH/Zend/zend_inheritance.c
    fn is_type_contravariant(&self, child_type: &TypeHint, parent_type: &TypeHint) -> bool {
        match (child_type, parent_type) {
            // Mixed accepts anything - widest type
            (TypeHint::Mixed, _) => true,
            (_, TypeHint::Mixed) => false,
            
            // Union types: child union must be superset of parent union
            (TypeHint::Union(child_types), TypeHint::Union(parent_types)) => {
                parent_types.iter().all(|parent_t| {
                    child_types.iter().any(|child_t| {
                        self.types_equal(child_t, parent_t) || self.is_type_contravariant(child_t, parent_t)
                    })
                })
            }
            
            // Child union can accept parent single type if union contains it
            (TypeHint::Union(child_types), parent_single) => {
                child_types.iter().any(|ct| 
                    self.types_equal(ct, parent_single) || self.is_type_contravariant(ct, parent_single)
                )
            }
            
            // Class inheritance: child can accept parent class (wider)
            (TypeHint::Class(child_class), TypeHint::Class(parent_class)) => {
                *child_class == *parent_class || self.is_subclass_of(*parent_class, *child_class)
            }
            
            // Object type compatibility
            (TypeHint::Object, TypeHint::Class(_)) => true, // object is wider than any class
            
            // Iterable compatibility
            (TypeHint::Iterable, TypeHint::Array) => true, // iterable is wider than array
            
            _ => false,
        }
    }
    
    /// Check if child_type is covariant with parent_type
    /// Covariance: child can return narrower types
    /// Example: parent returns Animal, child can return Dog (narrower)
    /// Reference: $PHP_SRC_PATH/Zend/zend_inheritance.c
    fn is_type_covariant(&self, child_type: &TypeHint, parent_type: &TypeHint) -> bool {
        match (child_type, parent_type) {
            // Mixed can be returned when parent expects anything
            (_, TypeHint::Mixed) => true,
            (TypeHint::Mixed, _) => false,
            
            // Never is covariant with everything (never returns)
            (TypeHint::Never, _) => true,
            
            // Void compatibility
            (TypeHint::Void, TypeHint::Void) => true,
            (TypeHint::Void, _) | (_, TypeHint::Void) => false,
            
            // Union types: child union must be subset of parent union
            (TypeHint::Union(child_types), TypeHint::Union(parent_types)) => {
                child_types.iter().all(|child_t| {
                    parent_types.iter().any(|parent_t| {
                        self.types_equal(child_t, parent_t) || self.is_type_covariant(child_t, parent_t)
                    })
                })
            }
            
            // Parent union, child single - child must be subtype of something in union
            (child_single, TypeHint::Union(parent_types)) => {
                parent_types.iter().any(|pt| 
                    self.types_equal(child_single, pt) || self.is_type_covariant(child_single, pt)
                )
            }
            
            // Intersection types: child must implement all interfaces
            (TypeHint::Intersection(child_types), TypeHint::Intersection(parent_types)) => {
                parent_types.iter().all(|parent_t| {
                    child_types.iter().any(|child_t| self.types_equal(child_t, parent_t))
                })
            }
            
            // Class inheritance: child can return subclass (narrower)
            (TypeHint::Class(child_class), TypeHint::Class(parent_class)) => {
                *child_class == *parent_class || self.is_subclass_of(*child_class, *parent_class)
            }
            
            // Class is covariant with Object
            (TypeHint::Class(_), TypeHint::Object) => true,
            
            // Array/Iterable compatibility
            (TypeHint::Array, TypeHint::Iterable) => true,
            
            // Null compatibility
            (TypeHint::Null, TypeHint::Null) => true,
            
            _ => false,
        }
    }
}

#[cfg(test)]

mod tests {
    use super::*;
    use crate::builtins::string::php_strlen;
    use crate::compiler::chunk::{FuncParam, UserFunc};
    use crate::core::value::Symbol;
    use crate::runtime::context::EngineContext;
    use std::sync::Arc;

    fn create_vm() -> VM {
        // Use EngineBuilder to properly register core extensions
        let engine = crate::runtime::context::EngineBuilder::new()
            .with_core_extensions()
            .build()
            .expect("Failed to build engine");

        VM::new(engine)
    }

    fn make_add_user_func() -> Rc<UserFunc> {
        let mut func_chunk = CodeChunk::default();
        let sym_a = Symbol(0);
        let sym_b = Symbol(1);

        func_chunk.code.push(OpCode::Recv(0));
        func_chunk.code.push(OpCode::Recv(1));
        func_chunk.code.push(OpCode::LoadVar(sym_a));
        func_chunk.code.push(OpCode::LoadVar(sym_b));
        func_chunk.code.push(OpCode::Add);
        func_chunk.code.push(OpCode::Return);

        Rc::new(UserFunc {
            params: vec![
                FuncParam {
                    name: sym_a,
                    by_ref: false,
                    param_type: None,
                    is_variadic: false,
                    default_value: None,
                },
                FuncParam {
                    name: sym_b,
                    by_ref: false,
                    param_type: None,
                    is_variadic: false,
                    default_value: None,
                },
            ],
            uses: Vec::new(),
            chunk: Rc::new(func_chunk),
            is_static: false,
            is_generator: false,
            statics: Rc::new(RefCell::new(HashMap::new())),
            return_type: None,
        })
    }

    #[test]
    fn test_store_dim_stack_order() {
        // Test that StoreDim correctly assigns a value to an array element
        // exec_store_dim pops: val, key, array (in that order from top of stack)
        // So we need to push: array, key, val (to make val on top)

        let mut vm = create_vm();
        // Create a reference array so assign_dim modifies it in-place
        let array_zval = vm.arena.alloc(Val::Array(ArrayData::new().into()));
        vm.arena.get_mut(array_zval).is_ref = true;
        let key_handle = vm.arena.alloc(Val::Int(0));
        let val_handle = vm.arena.alloc(Val::Int(99));

        // Push in reverse order so pops get them in the right order
        vm.operand_stack.push(array_zval);
        vm.operand_stack.push(key_handle);
        vm.operand_stack.push(val_handle);

        // Call exec_store_dim directly instead of going through run()
        vm.exec_store_dim().unwrap();

        let result_handle = vm.operand_stack.pop().unwrap();
        let result = vm.arena.get(result_handle);

        if let Val::Array(map) = &result.value {
            let key = ArrayKey::Int(0);
            let val = map.map.get(&key).unwrap();
            let val_val = vm.arena.get(*val);
            if let Val::Int(i) = val_val.value {
                assert_eq!(i, 99);
            } else {
                panic!("Expected Int(99)");
            }
        } else {
            panic!("Expected Array");
        }
    }

    #[test]
    fn test_calculator_1_plus_2_mul_3() {
        // 1 + 2 * 3 = 7
        let mut chunk = CodeChunk::default();
        chunk.constants.push(Val::Int(1)); // 0
        chunk.constants.push(Val::Int(2)); // 1
        chunk.constants.push(Val::Int(3)); // 2

        chunk.code.push(OpCode::Const(0));
        chunk.code.push(OpCode::Const(1));
        chunk.code.push(OpCode::Const(2));
        chunk.code.push(OpCode::Mul);
        chunk.code.push(OpCode::Add);

        let mut vm = create_vm();
        vm.run(Rc::new(chunk)).unwrap();

        let result_handle = vm.operand_stack.pop().unwrap();
        let result = vm.arena.get(result_handle);

        if let Val::Int(val) = result.value {
            assert_eq!(val, 7);
        } else {
            panic!("Expected Int result");
        }
    }

    #[test]
    fn test_control_flow_if_else() {
        // if (false) { $b = 10; } else { $b = 20; }
        // $b should be 20
        let mut chunk = CodeChunk::default();
        chunk.constants.push(Val::Int(0)); // 0: False
        chunk.constants.push(Val::Int(10)); // 1: 10
        chunk.constants.push(Val::Int(20)); // 2: 20

        let var_b = Symbol(1);

        // 0: Const(0) (False)
        chunk.code.push(OpCode::Const(0));
        // 1: JmpIfFalse(5) -> Jump to 5 (Else)
        chunk.code.push(OpCode::JmpIfFalse(5));
        // 2: Const(1) (10)
        chunk.code.push(OpCode::Const(1));
        // 3: StoreVar($b)
        chunk.code.push(OpCode::StoreVar(var_b));
        // 4: Jmp(7) -> Jump to 7 (End)
        chunk.code.push(OpCode::Jmp(7));
        // 5: Const(2) (20)
        chunk.code.push(OpCode::Const(2));
        // 6: StoreVar($b)
        chunk.code.push(OpCode::StoreVar(var_b));
        // 7: LoadVar($b)
        chunk.code.push(OpCode::LoadVar(var_b));

        let mut vm = create_vm();
        vm.run(Rc::new(chunk)).unwrap();

        let result_handle = vm.operand_stack.pop().unwrap();
        let result = vm.arena.get(result_handle);

        if let Val::Int(val) = result.value {
            assert_eq!(val, 20);
        } else {
            panic!("Expected Int result 20, got {:?}", result.value);
        }
    }

    #[test]
    fn test_echo_and_call() {
        // echo str_repeat("hi", 3);
        let mut chunk = CodeChunk::default();
        chunk.constants.push(Val::String(b"hi".to_vec().into())); // 0
        chunk.constants.push(Val::Int(3)); // 1
        chunk
            .constants
            .push(Val::String(b"str_repeat".to_vec().into())); // 2

        // Push "str_repeat" (function name)
        chunk.code.push(OpCode::Const(2));
        // Push "hi"
        chunk.code.push(OpCode::Const(0));
        // Push 3
        chunk.code.push(OpCode::Const(1));

        // Call(2) -> pops 2 args, then pops func
        chunk.code.push(OpCode::Call(2));
        // Echo -> pops result
        chunk.code.push(OpCode::Echo);

        let mut vm = create_vm();
        vm.run(Rc::new(chunk)).unwrap();

        assert!(vm.operand_stack.is_empty());
    }

    #[test]
    fn test_user_function_call() {
        // function add($a, $b) { return $a + $b; }
        // echo add(1, 2);

        let user_func = make_add_user_func();

        // Main chunk
        let mut chunk = CodeChunk::default();
        chunk.constants.push(Val::Int(1)); // 0
        chunk.constants.push(Val::Int(2)); // 1
        chunk.constants.push(Val::String(b"add".to_vec().into())); // 2

        // Push "add"
        chunk.code.push(OpCode::Const(2));
        // Push 1
        chunk.code.push(OpCode::Const(0));
        // Push 2
        chunk.code.push(OpCode::Const(1));

        // Call(2)
        chunk.code.push(OpCode::Call(2));
        // Echo (result 3)
        chunk.code.push(OpCode::Echo);

        let mut vm = create_vm();

        let sym_add = vm.context.interner.intern(b"add");
        vm.context.user_functions.insert(sym_add, user_func);

        vm.run(Rc::new(chunk)).unwrap();

        assert!(vm.operand_stack.is_empty());
    }

    #[test]
    fn test_handle_return_trims_stack_to_frame_base() {
        let mut vm = create_vm();

        // Simulate caller data already on the operand stack.
        let caller_sentinel = vm.arena.alloc(Val::Int(123));
        vm.operand_stack.push(caller_sentinel);

        // Prepare a callee frame with a minimal chunk.
        let mut chunk = CodeChunk::default();
        chunk.code.push(OpCode::Return);
        let frame = CallFrame::new(Rc::new(chunk));
        vm.push_frame(frame);

        // The callee leaves an extra stray value in addition to the return value.
        let stray = vm.arena.alloc(Val::Int(999));
        let return_handle = vm.arena.alloc(Val::String(b"ok".to_vec().into()));
        vm.operand_stack.push(stray);
        vm.operand_stack.push(return_handle);

        vm.handle_return(false, 0).unwrap();

        // Frame stack unwound and operand stack restored to caller state.
        assert_eq!(vm.frames.len(), 0);
        assert_eq!(vm.operand_stack.len(), 1);
        assert_eq!(vm.operand_stack.peek(), Some(caller_sentinel));
        assert_eq!(vm.last_return_value, Some(return_handle));
    }

    #[test]
    fn test_pending_call_dynamic_callable_handle() {
        let mut vm = create_vm();
        let sym_add = vm.context.interner.intern(b"add");
        vm.context
            .user_functions
            .insert(sym_add, make_add_user_func());

        let callable_handle = vm.arena.alloc(Val::String(b"add".to_vec().into()));
        let mut args = ArgList::new();
        args.push(vm.arena.alloc(Val::Int(1)));
        args.push(vm.arena.alloc(Val::Int(2)));

        let call = PendingCall {
            func_name: None,
            func_handle: Some(callable_handle),
            args,
            is_static: false,
            class_name: None,
            this_handle: None,
        };

        vm.execute_pending_call(call).unwrap();
        vm.run_loop(0).unwrap();

        let result_handle = vm.last_return_value.expect("missing return value");
        let result = vm.arena.get(result_handle);
        if let Val::Int(i) = result.value {
            assert_eq!(i, 3);
        } else {
            panic!("Expected int 3, got {:?}", result.value);
        }
    }

    #[test]
    fn test_pop_underflow_errors() {
        let mut vm = create_vm();
        let mut chunk = CodeChunk::default();
        chunk.code.push(OpCode::Pop);

        let result = vm.run(Rc::new(chunk));
        match result {
            Err(VmError::StackUnderflow { operation }) => assert_eq!(operation, "pop"),
            other => panic!("Expected stack underflow error, got {:?}", other),
        }
    }
}
