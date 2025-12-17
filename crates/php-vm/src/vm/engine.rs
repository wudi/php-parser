use crate::compiler::chunk::{ClosureData, CodeChunk, ReturnType, UserFunc};
use crate::core::heap::Arena;
use crate::core::value::{ArrayData, ArrayKey, Handle, ObjectData, Symbol, Val, Visibility};
use crate::runtime::context::{ClassDef, EngineContext, MethodEntry, RequestContext};
use crate::vm::frame::{
    ArgList, CallFrame, GeneratorData, GeneratorState, SubGenState, SubIterator,
};
use crate::vm::opcode::OpCode;
use crate::vm::stack::Stack;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub enum VmError {
    RuntimeError(String),
    Exception(Handle),
}

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
            ErrorLevel::Error => 1,       // E_ERROR
            ErrorLevel::Warning => 2,     // E_WARNING
            ErrorLevel::ParseError => 4,  // E_PARSE
            ErrorLevel::Notice => 8,      // E_NOTICE
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
    pub execution_start_time: SystemTime,
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
            execution_start_time: SystemTime::now(),
        };
        vm.initialize_superglobals();
        vm
    }

    /// Convert bytes to lowercase for case-insensitive lookups
    #[inline]
    fn to_lowercase_bytes(bytes: &[u8]) -> Vec<u8> {
        bytes.iter().map(|b| b.to_ascii_lowercase()).collect()
    }

    fn method_lookup_key(&self, name: Symbol) -> Option<Symbol> {
        let name_bytes = self.context.interner.lookup(name)?;
        let lower = Self::to_lowercase_bytes(name_bytes);
        self.context.interner.find(&lower)
    }

    fn intern_lowercase_symbol(&mut self, name: Symbol) -> Result<Symbol, VmError> {
        let name_bytes = self
            .context
            .interner
            .lookup(name)
            .ok_or_else(|| VmError::RuntimeError("Invalid method symbol".into()))?;
        let lower = Self::to_lowercase_bytes(name_bytes);
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
            _ => self.arena.alloc(Val::Array(Rc::new(ArrayData::new()))),
        }
    }

    fn create_server_superglobal(&mut self) -> Handle {
        let mut data = ArrayData::new();
        Self::insert_array_value(
            &mut data,
            b"SERVER_PROTOCOL",
            self.alloc_string_handle(b"HTTP/1.1"),
        );
        Self::insert_array_value(
            &mut data,
            b"REQUEST_METHOD",
            self.alloc_string_handle(b"GET"),
        );
        Self::insert_array_value(
            &mut data,
            b"HTTP_HOST",
            self.alloc_string_handle(b"localhost"),
        );
        Self::insert_array_value(
            &mut data,
            b"SERVER_NAME",
            self.alloc_string_handle(b"localhost"),
        );
        Self::insert_array_value(
            &mut data,
            b"SERVER_SOFTWARE",
            self.alloc_string_handle(b"php-vm"),
        );
        Self::insert_array_value(
            &mut data,
            b"SERVER_ADDR",
            self.alloc_string_handle(b"127.0.0.1"),
        );
        Self::insert_array_value(
            &mut data,
            b"REMOTE_ADDR",
            self.alloc_string_handle(b"127.0.0.1"),
        );
        Self::insert_array_value(&mut data, b"REMOTE_PORT", self.arena.alloc(Val::Int(0)));
        Self::insert_array_value(&mut data, b"SERVER_PORT", self.arena.alloc(Val::Int(80)));
        Self::insert_array_value(
            &mut data,
            b"REQUEST_SCHEME",
            self.alloc_string_handle(b"http"),
        );
        Self::insert_array_value(&mut data, b"HTTPS", self.alloc_string_handle(b"off"));
        Self::insert_array_value(&mut data, b"QUERY_STRING", self.alloc_string_handle(b""));
        Self::insert_array_value(&mut data, b"REQUEST_URI", self.alloc_string_handle(b"/"));
        Self::insert_array_value(&mut data, b"PATH_INFO", self.alloc_string_handle(b""));
        Self::insert_array_value(&mut data, b"ORIG_PATH_INFO", self.alloc_string_handle(b""));

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

        Self::insert_array_value(
            &mut data,
            b"DOCUMENT_ROOT",
            self.alloc_string_handle(document_root.as_bytes()),
        );
        Self::insert_array_value(
            &mut data,
            b"SCRIPT_NAME",
            self.alloc_string_handle(script_name.as_bytes()),
        );
        Self::insert_array_value(
            &mut data,
            b"PHP_SELF",
            self.alloc_string_handle(script_name.as_bytes()),
        );
        Self::insert_array_value(
            &mut data,
            b"SCRIPT_FILENAME",
            self.alloc_string_handle(script_filename.as_bytes()),
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

    fn ensure_superglobal_handle(&mut self, sym: Symbol) -> Option<Handle> {
        let kind = self.superglobal_map.get(&sym).copied()?;
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

    fn is_superglobal(&self, sym: Symbol) -> bool {
        self.superglobal_map.contains_key(&sym)
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
            execution_start_time: SystemTime::now(),
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

        let elapsed = self.execution_start_time
            .elapsed()
            .map_err(|e| VmError::RuntimeError(format!("Time error: {}", e)))?;
        
        let elapsed_secs = elapsed.as_secs() as i64;
        
        if elapsed_secs >= self.context.max_execution_time {
            return Err(VmError::RuntimeError(format!(
                "Maximum execution time of {} second{} exceeded",
                self.context.max_execution_time,
                if self.context.max_execution_time == 1 { "" } else { "s" }
            )));
        }

        Ok(())
    }

    /// Report an error respecting the error_reporting level
    /// Also stores the error in context.last_error for error_get_last()
    fn report_error(&mut self, level: ErrorLevel, message: &str) {
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
    pub fn call_user_function(&mut self, callable: Handle, args: &[Handle]) -> Result<Handle, String> {
        // This is a simplified version - the actual implementation would need to handle
        // different callable types (closures, function names, arrays with [object, method], etc.)
        match &self.arena.get(callable).value {
            Val::String(name) => {
                // Function name as string
                let name_bytes = name.as_ref();
                if let Some(func) = self.context.engine.functions.get(name_bytes) {
                    func(self, args)
                } else {
                    Err(format!("Call to undefined function {}", String::from_utf8_lossy(name_bytes)))
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
        match &self.arena.get(handle).value {
            Val::String(s) => Ok(s.as_ref().clone()),
            Val::Int(i) => Ok(i.to_string().into_bytes()),
            Val::Float(f) => Ok(f.to_string().into_bytes()),
            Val::Bool(true) => Ok(b"1".to_vec()),
            Val::Bool(false) => Ok(Vec::new()),
            Val::Null => Ok(Vec::new()),
            Val::Array(_) => Ok(b"Array".to_vec()),
            Val::Object(_) => Ok(b"Object".to_vec()),
            _ => Ok(Vec::new()),
        }
    }

    pub fn print_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.write_output(bytes).map_err(|err| match err {
            VmError::RuntimeError(msg) => msg,
            VmError::Exception(_) => "Output aborted by exception".into(),
        })
    }

    // Safe frame access helpers (no-panic guarantee)
    #[inline]
    fn current_frame(&self) -> Result<&CallFrame, VmError> {
        self.frames
            .last()
            .ok_or_else(|| VmError::RuntimeError("No active frame".into()))
    }

    #[inline]
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
    fn pop_operand(&mut self) -> Result<Handle, VmError> {
        self.operand_stack
            .pop()
            .ok_or_else(|| VmError::RuntimeError("Operand stack empty".into()))
    }

    fn push_frame(&mut self, mut frame: CallFrame) {
        if frame.stack_base.is_none() {
            frame.stack_base = Some(self.operand_stack.len());
        }
        self.frames.push(frame);
    }

    fn collect_call_args<T>(&mut self, arg_count: T) -> Result<ArgList, VmError>
    where
        T: Into<usize>,
    {
        let count = arg_count.into();
        let mut args = ArgList::with_capacity(count);
        for _ in 0..count {
            args.push(self.pop_operand()?);
        }
        args.reverse();
        Ok(args)
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

    fn canonical_path_string(path: &Path) -> String {
        std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .to_string_lossy()
            .into_owned()
    }

    fn trigger_autoload(&mut self, class_name: Symbol) -> Result<(), VmError> {
        // Get class name bytes
        let class_name_bytes = self
            .context
            .interner
            .lookup(class_name)
            .ok_or_else(|| VmError::RuntimeError("Invalid class name".into()))?;
        
        // Create a string handle for the class name
        let class_name_handle = self.arena.alloc(Val::String(Rc::new(class_name_bytes.to_vec())));
        
        // Call each autoloader
        let autoloaders = self.context.autoloaders.clone();
        for autoloader_handle in autoloaders {
            let args = smallvec::smallvec![class_name_handle];
            // Try to invoke the autoloader
            if let Ok(()) = self.invoke_callable_value(autoloader_handle, args) {
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

    pub fn find_method(
        &self,
        class_name: Symbol,
        method_name: Symbol,
    ) -> Option<(Rc<UserFunc>, Visibility, bool, Symbol)> {
        // Walk the inheritance chain (class -> parent -> parent -> ...)
        // Reference: $PHP_SRC_PATH/Zend/zend_API.c - zend_std_get_method
        let mut current_class = Some(class_name);

        while let Some(cls) = current_class {
            if let Some(def) = self.context.classes.get(&cls) {
                // Try direct lookup with case-insensitive key
                if let Some(key) = self.method_lookup_key(method_name) {
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
                if let Some(search_name) = self.context.interner.lookup(method_name) {
                    let search_lower = Self::to_lowercase_bytes(search_name);
                    for entry in def.methods.values() {
                        if let Some(stored_bytes) = self.context.interner.lookup(entry.name) {
                            if Self::to_lowercase_bytes(stored_bytes) == search_lower {
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

                // Move up the inheritance chain
                current_class = def.parent;
            } else {
                break;
            }
        }

        None
    }

    pub fn find_native_method(
        &self,
        class_name: Symbol,
        method_name: Symbol,
    ) -> Option<crate::runtime::context::NativeMethodEntry> {
        // Walk the inheritance chain to find native methods
        let mut current_class = Some(class_name);

        while let Some(cls) = current_class {
            // Check native_methods map
            if let Some(entry) = self.context.native_methods.get(&(cls, method_name)) {
                return Some(entry.clone());
            }

            // Move up the inheritance chain
            if let Some(def) = self.context.classes.get(&cls) {
                current_class = def.parent;
            } else {
                break;
            }
        }

        None
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
                            Self::to_lowercase_bytes(name_bytes)
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
        let mut current_class = Some(class_name);
        while let Some(name) = current_class {
            if let Some(def) = self.context.classes.get(&name) {
                if def.properties.contains_key(&prop_name) {
                    return true;
                }
                current_class = def.parent;
            } else {
                break;
            }
        }
        false
    }

    pub fn collect_properties(
        &mut self,
        class_name: Symbol,
        mode: PropertyCollectionMode,
    ) -> IndexMap<Symbol, Handle> {
        let mut properties = IndexMap::new();
        let mut chain = Vec::new();
        let mut current_class = Some(class_name);

        while let Some(name) = current_class {
            if let Some(def) = self.context.classes.get(&name) {
                chain.push(def);
                current_class = def.parent;
            } else {
                break;
            }
        }

        for def in chain.iter().rev() {
            for (name, (default_val, _visibility)) in &def.properties {
                if let PropertyCollectionMode::VisibleTo(scope) = mode {
                    if self
                        .check_prop_visibility(class_name, *name, scope)
                        .is_err()
                    {
                        continue;
                    }
                }

                let handle = self.arena.alloc(default_val.clone());
                properties.insert(*name, handle);
            }
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

    /// Check if an object implements the ArrayAccess interface
    /// Reference: $PHP_SRC_PATH/Zend/zend_interfaces.c - instanceof_function_ex
    fn implements_array_access(&mut self, class_name: Symbol) -> bool {
        let array_access_sym = self.context.interner.intern(b"ArrayAccess");
        self.is_subclass_of(class_name, array_access_sym)
    }

    /// Call ArrayAccess::offsetExists($offset)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_call_method
    fn call_array_access_offset_exists(
        &mut self,
        obj_handle: Handle,
        offset_handle: Handle,
    ) -> Result<bool, VmError> {
        let method_name = self.context.interner.intern(b"offsetExists");
        
        let class_name = if let Val::Object(payload_handle) = self.arena.get(obj_handle).value {
            let payload = self.arena.get(payload_handle);
            if let Val::ObjPayload(obj_data) = &payload.value {
                obj_data.class
            } else {
                return Err(VmError::RuntimeError("Invalid object payload".into()));
            }
        } else {
            return Err(VmError::RuntimeError("Not an object".into()));
        };

        // Try to find and call the method
        if let Some((user_func, _, _, defined_class)) = self.find_method(class_name, method_name) {
            let args = smallvec::SmallVec::from_vec(vec![offset_handle]);
            let mut frame = CallFrame::new(user_func.chunk.clone());
            frame.func = Some(user_func.clone());
            frame.this = Some(obj_handle);
            frame.class_scope = Some(defined_class);
            frame.called_scope = Some(class_name);
            frame.args = args;
            
            self.push_frame(frame);
            
            // Execute method by running its opcode loop
            let target_depth = self.frames.len() - 1;
            loop {
                if self.frames.len() <= target_depth {
                    break;
                }
                let frame = self.frames.last_mut().unwrap();
                if frame.ip >= frame.chunk.code.len() {
                    let _ = self.pop_frame();
                    break;
                }
                let op = frame.chunk.code[frame.ip].clone();
                frame.ip += 1;
                self.execute_opcode(op, target_depth)?;
            }
            
            // Get result
            let result_handle = self.last_return_value.take()
                .unwrap_or_else(|| self.arena.alloc(Val::Null));
            let result_val = &self.arena.get(result_handle).value;
            Ok(result_val.to_bool())
        } else {
            // Method not found - this should not happen for proper ArrayAccess implementation
            Err(VmError::RuntimeError(format!(
                "ArrayAccess::offsetExists not found in class"
            )))
        }
    }

    /// Call ArrayAccess::offsetGet($offset)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c
    fn call_array_access_offset_get(
        &mut self,
        obj_handle: Handle,
        offset_handle: Handle,
    ) -> Result<Handle, VmError> {
        let method_name = self.context.interner.intern(b"offsetGet");
        
        let class_name = if let Val::Object(payload_handle) = self.arena.get(obj_handle).value {
            let payload = self.arena.get(payload_handle);
            if let Val::ObjPayload(obj_data) = &payload.value {
                obj_data.class
            } else {
                return Err(VmError::RuntimeError("Invalid object payload".into()));
            }
        } else {
            return Err(VmError::RuntimeError("Not an object".into()));
        };

        if let Some((user_func, _, _, defined_class)) = self.find_method(class_name, method_name) {
            let args = smallvec::SmallVec::from_vec(vec![offset_handle]);
            let mut frame = CallFrame::new(user_func.chunk.clone());
            frame.func = Some(user_func.clone());
            frame.this = Some(obj_handle);
            frame.class_scope = Some(defined_class);
            frame.called_scope = Some(class_name);
            frame.args = args;
            
            self.push_frame(frame);
            
            let target_depth = self.frames.len() - 1;
            loop {
                if self.frames.len() <= target_depth {
                    break;
                }
                let frame = self.frames.last_mut().unwrap();
                if frame.ip >= frame.chunk.code.len() {
                    let _ = self.pop_frame();
                    break;
                }
                let op = frame.chunk.code[frame.ip].clone();
                frame.ip += 1;
                self.execute_opcode(op, target_depth)?;
            }
            
            let result_handle = self.last_return_value.take()
                .unwrap_or_else(|| self.arena.alloc(Val::Null));
            Ok(result_handle)
        } else {
            Err(VmError::RuntimeError(format!(
                "ArrayAccess::offsetGet not found in class"
            )))
        }
    }

    /// Call ArrayAccess::offsetSet($offset, $value)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c
    fn call_array_access_offset_set(
        &mut self,
        obj_handle: Handle,
        offset_handle: Handle,
        value_handle: Handle,
    ) -> Result<(), VmError> {
        let method_name = self.context.interner.intern(b"offsetSet");
        
        let class_name = if let Val::Object(payload_handle) = self.arena.get(obj_handle).value {
            let payload = self.arena.get(payload_handle);
            if let Val::ObjPayload(obj_data) = &payload.value {
                obj_data.class
            } else {
                return Err(VmError::RuntimeError("Invalid object payload".into()));
            }
        } else {
            return Err(VmError::RuntimeError("Not an object".into()));
        };

        if let Some((user_func, _, _, defined_class)) = self.find_method(class_name, method_name) {
            let args = smallvec::SmallVec::from_vec(vec![offset_handle, value_handle]);
            let mut frame = CallFrame::new(user_func.chunk.clone());
            frame.func = Some(user_func.clone());
            frame.this = Some(obj_handle);
            frame.class_scope = Some(defined_class);
            frame.called_scope = Some(class_name);
            frame.args = args;
            
            self.push_frame(frame);
            
            let target_depth = self.frames.len() - 1;
            loop {
                if self.frames.len() <= target_depth {
                    break;
                }
                let frame = self.frames.last_mut().unwrap();
                if frame.ip >= frame.chunk.code.len() {
                    let _ = self.pop_frame();
                    break;
                }
                let op = frame.chunk.code[frame.ip].clone();
                frame.ip += 1;
                self.execute_opcode(op, target_depth)?;
            }
            
            // offsetSet returns void, discard result
            self.last_return_value = None;
            Ok(())
        } else {
            Err(VmError::RuntimeError(format!(
                "ArrayAccess::offsetSet not found in class"
            )))
        }
    }

    /// Call ArrayAccess::offsetUnset($offset)
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c
    fn call_array_access_offset_unset(
        &mut self,
        obj_handle: Handle,
        offset_handle: Handle,
    ) -> Result<(), VmError> {
        let method_name = self.context.interner.intern(b"offsetUnset");
        
        let class_name = if let Val::Object(payload_handle) = self.arena.get(obj_handle).value {
            let payload = self.arena.get(payload_handle);
            if let Val::ObjPayload(obj_data) = &payload.value {
                obj_data.class
            } else {
                return Err(VmError::RuntimeError("Invalid object payload".into()));
            }
        } else {
            return Err(VmError::RuntimeError("Not an object".into()));
        };

        if let Some((user_func, _, _, defined_class)) = self.find_method(class_name, method_name) {
            let args = smallvec::SmallVec::from_vec(vec![offset_handle]);
            let mut frame = CallFrame::new(user_func.chunk.clone());
            frame.func = Some(user_func.clone());
            frame.this = Some(obj_handle);
            frame.class_scope = Some(defined_class);
            frame.called_scope = Some(class_name);
            frame.args = args;
            
            self.push_frame(frame);
            
            let target_depth = self.frames.len() - 1;
            loop {
                if self.frames.len() <= target_depth {
                    break;
                }
                let frame = self.frames.last_mut().unwrap();
                if frame.ip >= frame.chunk.code.len() {
                    let _ = self.pop_frame();
                    break;
                }
                let op = frame.chunk.code[frame.ip].clone();
                frame.ip += 1;
                self.execute_opcode(op, target_depth)?;
            }
            
            // offsetUnset returns void, discard result
            self.last_return_value = None;
            Ok(())
        } else {
            Err(VmError::RuntimeError(format!(
                "ArrayAccess::offsetUnset not found in class"
            )))
        }
    }

    fn resolve_class_name(&self, class_name: Symbol) -> Result<Symbol, VmError> {
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

    fn find_class_constant(
        &self,
        start_class: Symbol,
        const_name: Symbol,
    ) -> Result<(Val, Visibility, Symbol), VmError> {
        // Reference: $PHP_SRC_PATH/Zend/zend_compile.c - constant access
        // First pass: find the constant anywhere in hierarchy (ignoring visibility)
        let mut current_class = start_class;
        let mut found: Option<(Val, Visibility, Symbol)> = None;

        loop {
            if let Some(class_def) = self.context.classes.get(&current_class) {
                if let Some((val, vis)) = class_def.constants.get(&const_name) {
                    found = Some((val.clone(), *vis, current_class));
                    break;
                }
                if let Some(parent) = class_def.parent {
                    current_class = parent;
                } else {
                    break;
                }
            } else {
                let class_str = String::from_utf8_lossy(
                    self.context.interner.lookup(start_class).unwrap_or(b"???"),
                );
                return Err(VmError::RuntimeError(format!(
                    "Class {} not found",
                    class_str
                )));
            }
        }

        // Second pass: check visibility if found
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

    fn find_static_prop(
        &self,
        start_class: Symbol,
        prop_name: Symbol,
    ) -> Result<(Val, Visibility, Symbol), VmError> {
        // Reference: $PHP_SRC_PATH/Zend/zend_compile.c - static property access
        // First pass: find the property anywhere in hierarchy (ignoring visibility)
        let mut current_class = start_class;
        let mut found: Option<(Val, Visibility, Symbol)> = None;

        loop {
            if let Some(class_def) = self.context.classes.get(&current_class) {
                if let Some((val, vis)) = class_def.static_properties.get(&prop_name) {
                    found = Some((val.clone(), *vis, current_class));
                    break;
                }
                if let Some(parent) = class_def.parent {
                    current_class = parent;
                } else {
                    break;
                }
            } else {
                let class_str = String::from_utf8_lossy(
                    self.context.interner.lookup(start_class).unwrap_or(b"???"),
                );
                return Err(VmError::RuntimeError(format!(
                    "Class {} not found",
                    class_str
                )));
            }
        }

        // Second pass: check visibility if found
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

    fn property_visible_to(
        &self,
        defining_class: Symbol,
        visibility: Visibility,
        caller_scope: Option<Symbol>,
    ) -> bool {
        match visibility {
            Visibility::Public => true,
            Visibility::Private => caller_scope == Some(defining_class),
            Visibility::Protected => {
                if let Some(scope) = caller_scope {
                    scope == defining_class || self.is_subclass_of(scope, defining_class)
                } else {
                    false
                }
            }
        }
    }

    fn check_const_visibility(
        &self,
        defining_class: Symbol,
        visibility: Visibility,
    ) -> Result<(), VmError> {
        match visibility {
            Visibility::Public => Ok(()),
            Visibility::Private => {
                let frame = self
                    .frames
                    .last()
                    .ok_or(VmError::RuntimeError("No active frame".into()))?;
                let scope = frame.class_scope.ok_or_else(|| {
                    let class_str = String::from_utf8_lossy(
                        self.context
                            .interner
                            .lookup(defining_class)
                            .unwrap_or(b"???"),
                    );
                    VmError::RuntimeError(format!(
                        "Cannot access private constant from {}::",
                        class_str
                    ))
                })?;
                if scope == defining_class {
                    Ok(())
                } else {
                    let class_str = String::from_utf8_lossy(
                        self.context
                            .interner
                            .lookup(defining_class)
                            .unwrap_or(b"???"),
                    );
                    Err(VmError::RuntimeError(format!(
                        "Cannot access private constant from {}::",
                        class_str
                    )))
                }
            }
            Visibility::Protected => {
                let frame = self
                    .frames
                    .last()
                    .ok_or(VmError::RuntimeError("No active frame".into()))?;
                let scope = frame.class_scope.ok_or_else(|| {
                    let class_str = String::from_utf8_lossy(
                        self.context
                            .interner
                            .lookup(defining_class)
                            .unwrap_or(b"???"),
                    );
                    VmError::RuntimeError(format!(
                        "Cannot access protected constant from {}::",
                        class_str
                    ))
                })?;
                // Protected members accessible only from defining class or subclasses (one-directional)
                if scope == defining_class || self.is_subclass_of(scope, defining_class) {
                    Ok(())
                } else {
                    let class_str = String::from_utf8_lossy(
                        self.context
                            .interner
                            .lookup(defining_class)
                            .unwrap_or(b"???"),
                    );
                    Err(VmError::RuntimeError(format!(
                        "Cannot access protected constant from {}::",
                        class_str
                    )))
                }
            }
        }
    }

    fn check_method_visibility(
        &self,
        defining_class: Symbol,
        visibility: Visibility,
        method_name: Option<Symbol>,
    ) -> Result<(), VmError> {
        let caller_scope = self.get_current_class();
        if self.method_visible_to(defining_class, visibility, caller_scope) {
            return Ok(());
        }

        // Build descriptive error message
        let class_str = self
            .context
            .interner
            .lookup(defining_class)
            .map(|b| String::from_utf8_lossy(b).to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let method_str = method_name
            .and_then(|s| self.context.interner.lookup(s))
            .map(|b| String::from_utf8_lossy(b).to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let vis_str = match visibility {
            Visibility::Public => unreachable!("public accesses should always succeed"),
            Visibility::Private => "private",
            Visibility::Protected => "protected",
        };

        Err(VmError::RuntimeError(format!(
            "Cannot access {} method {}::{}",
            vis_str, class_str, method_str
        )))
    }

    fn method_visible_to(
        &self,
        defining_class: Symbol,
        visibility: Visibility,
        caller_scope: Option<Symbol>,
    ) -> bool {
        match visibility {
            Visibility::Public => true,
            Visibility::Private => caller_scope == Some(defining_class),
            Visibility::Protected => {
                if let Some(scope) = caller_scope {
                    scope == defining_class || self.is_subclass_of(scope, defining_class)
                } else {
                    false
                }
            }
        }
    }

    pub(crate) fn get_current_class(&self) -> Option<Symbol> {
        self.frames.last().and_then(|f| f.class_scope)
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

    pub(crate) fn check_prop_visibility(
        &self,
        class_name: Symbol,
        prop_name: Symbol,
        current_scope: Option<Symbol>,
    ) -> Result<(), VmError> {
        let mut current = Some(class_name);
        let mut defined_vis = None;
        let mut defined_class = None;

        while let Some(name) = current {
            if let Some(def) = self.context.classes.get(&name) {
                if let Some((_, vis)) = def.properties.get(&prop_name) {
                    defined_vis = Some(*vis);
                    defined_class = Some(name);
                    break;
                }
                current = def.parent;
            } else {
                break;
            }
        }

        if let Some(vis) = defined_vis {
            let defined = defined_class
                .ok_or_else(|| VmError::RuntimeError("Missing defined class".into()))?;
            match vis {
                Visibility::Public => Ok(()),
                Visibility::Private => {
                    if current_scope == Some(defined) {
                        Ok(())
                    } else {
                        let class_str = String::from_utf8_lossy(
                            self.context.interner.lookup(defined).unwrap_or(b"???"),
                        );
                        let prop_str = String::from_utf8_lossy(
                            self.context.interner.lookup(prop_name).unwrap_or(b"???"),
                        );
                        Err(VmError::RuntimeError(format!(
                            "Cannot access private property {}::${}",
                            class_str, prop_str
                        )))
                    }
                }
                Visibility::Protected => {
                    if let Some(scope) = current_scope {
                        // Protected members accessible only from defining class or subclasses (one-directional)
                        if scope == defined || self.is_subclass_of(scope, defined) {
                            Ok(())
                        } else {
                            let class_str = String::from_utf8_lossy(
                                self.context.interner.lookup(defined).unwrap_or(b"???"),
                            );
                            let prop_str = String::from_utf8_lossy(
                                self.context.interner.lookup(prop_name).unwrap_or(b"???"),
                            );
                            Err(VmError::RuntimeError(format!(
                                "Cannot access protected property {}::${}",
                                class_str, prop_str
                            )))
                        }
                    } else {
                        let class_str = String::from_utf8_lossy(
                            self.context.interner.lookup(defined).unwrap_or(b"???"),
                        );
                        let prop_str = String::from_utf8_lossy(
                            self.context.interner.lookup(prop_name).unwrap_or(b"???"),
                        );
                        Err(VmError::RuntimeError(format!(
                            "Cannot access protected property {}::${}",
                            class_str, prop_str
                        )))
                    }
                }
            }
        } else {
            // Dynamic property - check if allowed in PHP 8.2+
            // Reference: PHP 8.2 deprecated dynamic properties by default
            Ok(())
        }
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
            let mut finally_target = None;

            for entry in &chunk.catch_table {
                if ip >= entry.start && ip < entry.end {
                    // Check for finally block first
                    if entry.catch_type.is_none() && entry.finally_target.is_none() {
                        // This is a finally-only entry
                        finally_target = Some(entry.target);
                        continue;
                    }

                    // Check for matching catch block
                    if let Some(type_sym) = entry.catch_type {
                        if self.is_instance_of(ex_handle, type_sym) {
                            // Found matching catch block
                            self.frames.truncate(frame_idx + 1);
                            let frame = &mut self.frames[frame_idx];
                            frame.ip = entry.target as usize;
                            self.operand_stack.push(ex_handle);
                            
                            // If this catch has a finally, we'll execute it after the catch
                            if let Some(_finally_tgt) = entry.finally_target {
                                // Mark that we need to execute finally after catch completes
                                // Store it for later execution
                            }
                            
                            found_catch = true;
                            break;
                        }
                    }

                    // Track finally target if present
                    if entry.finally_target.is_some() {
                        finally_target = entry.finally_target;
                    }
                }
            }

            if found_catch {
                return true;
            }

            // If we found a finally block, collect it for execution during unwinding
            if let Some(target) = finally_target {
                finally_blocks.push((frame_idx, target));
            }
        }

        // No catch found, but execute finally blocks during unwinding
        // In PHP, finally blocks execute even when exception is not caught
        // For now, we'll just track them but not execute (simplified implementation)
        // Full implementation would require executing finally blocks and re-throwing
        
        self.frames.clear();
        false
    }

    fn execute_pending_call(&mut self, call: PendingCall) -> Result<(), VmError> {
        let PendingCall {
            func_name,
            func_handle,
            args,
            is_static: call_is_static,
            class_name,
            this_handle: call_this,
        } = call;
        if let Some(name) = func_name {
            if let Some(class_name) = class_name {
                // Method call
                let method_lookup = self.find_method(class_name, name);
                if let Some((method, visibility, is_static, defining_class)) = method_lookup {
                    if is_static != call_is_static {
                        if is_static {
                            // PHP allows calling static non-statically with notices; we allow.
                        } else {
                            if call_this.is_none() {
                                return Err(VmError::RuntimeError(
                                    "Non-static method called statically".into(),
                                ));
                            }
                        }
                    }

                    self.check_method_visibility(defining_class, visibility, Some(name))?;

                    let mut frame = CallFrame::new(method.chunk.clone());
                    frame.func = Some(method.clone());
                    frame.this = call_this;
                    frame.class_scope = Some(defining_class);
                    frame.called_scope = Some(class_name);
                    frame.args = args;

                    for (i, param) in method.params.iter().enumerate() {
                        if i < frame.args.len() {
                            let arg_handle = frame.args[i];
                            if param.by_ref {
                                if !self.arena.get(arg_handle).is_ref {
                                    self.arena.get_mut(arg_handle).is_ref = true;
                                }
                                frame.locals.insert(param.name, arg_handle);
                            } else {
                                let val = self.arena.get(arg_handle).value.clone();
                                let final_handle = self.arena.alloc(val);
                                frame.locals.insert(param.name, final_handle);
                            }
                        }
                    }

                    self.push_frame(frame);
                } else {
                    let name_str =
                        String::from_utf8_lossy(self.context.interner.lookup(name).unwrap_or(b""));
                    let class_str = String::from_utf8_lossy(
                        self.context.interner.lookup(class_name).unwrap_or(b""),
                    );
                    return Err(VmError::RuntimeError(format!(
                        "Call to undefined method {}::{}",
                        class_str, name_str
                    )));
                }
            } else {
                self.invoke_function_symbol(name, args)?;
            }
        } else if let Some(callable_handle) = func_handle {
            self.invoke_callable_value(callable_handle, args)?;
        } else {
            return Err(VmError::RuntimeError(
                "Dynamic function call not supported yet".into(),
            ));
        }
        Ok(())
    }

    fn invoke_function_symbol(&mut self, name: Symbol, args: ArgList) -> Result<(), VmError> {
        let name_bytes = self.context.interner.lookup(name).unwrap_or(b"");
        let lower_name = Self::to_lowercase_bytes(name_bytes);

        // Check extension registry first (new way)
        if let Some(handler) = self.context.engine.registry.get_function(&lower_name) {
            let res = handler(self, &args).map_err(VmError::RuntimeError)?;
            self.operand_stack.push(res);
            return Ok(());
        }

        // Fall back to legacy functions HashMap (backward compatibility)
        if let Some(handler) = self.context.engine.functions.get(&lower_name) {
            let res = handler(self, &args).map_err(VmError::RuntimeError)?;
            self.operand_stack.push(res);
            return Ok(());
        }

        if let Some(func) = self.context.user_functions.get(&name) {
            let mut frame = CallFrame::new(func.chunk.clone());
            frame.func = Some(func.clone());
            frame.args = args;

            if func.is_generator {
                let gen_data = GeneratorData {
                    state: GeneratorState::Created(frame),
                    current_val: None,
                    current_key: None,
                    auto_key: 0,
                    sub_iter: None,
                    sent_val: None,
                };
                let obj_data = ObjectData {
                    class: self.context.interner.intern(b"Generator"),
                    properties: IndexMap::new(),
                    internal: Some(Rc::new(RefCell::new(gen_data))),
                    dynamic_properties: std::collections::HashSet::new(),
                };
                let payload_handle = self.arena.alloc(Val::ObjPayload(obj_data));
                let obj_handle = self.arena.alloc(Val::Object(payload_handle));
                self.operand_stack.push(obj_handle);
                return Ok(());
            }

            for (i, param) in func.params.iter().enumerate() {
                if i < frame.args.len() {
                    let arg_handle = frame.args[i];
                    if param.by_ref {
                        if !self.arena.get(arg_handle).is_ref {
                            self.arena.get_mut(arg_handle).is_ref = true;
                        }
                        frame.locals.insert(param.name, arg_handle);
                    } else {
                        let val = self.arena.get(arg_handle).value.clone();
                        let final_handle = self.arena.alloc(val);
                        frame.locals.insert(param.name, final_handle);
                    }
                }
            }

            self.push_frame(frame);
            Ok(())
        } else {
            Err(VmError::RuntimeError(format!(
                "Call to undefined function: {}",
                String::from_utf8_lossy(name_bytes)
            )))
        }
    }

    fn invoke_callable_value(
        &mut self,
        callable_handle: Handle,
        args: ArgList,
    ) -> Result<(), VmError> {
        let callable_zval = self.arena.get(callable_handle);
        match &callable_zval.value {
            Val::String(s) => {
                let sym = self.context.interner.intern(s);
                self.invoke_function_symbol(sym, args)
            }
            Val::Object(payload_handle) => {
                let payload_val = self.arena.get(*payload_handle);
                if let Val::ObjPayload(obj_data) = &payload_val.value {
                    if let Some(internal) = &obj_data.internal {
                        if let Ok(closure) = internal.clone().downcast::<ClosureData>() {
                            let mut frame = CallFrame::new(closure.func.chunk.clone());
                            frame.func = Some(closure.func.clone());
                            frame.args = args;

                            for (sym, handle) in &closure.captures {
                                frame.locals.insert(*sym, *handle);
                            }

                            frame.this = closure.this;
                            self.push_frame(frame);
                            return Ok(());
                        }
                    }

                    let invoke_sym = self.context.interner.intern(b"__invoke");
                    if let Some((method, visibility, _, defining_class)) =
                        self.find_method(obj_data.class, invoke_sym)
                    {
                        self.check_method_visibility(defining_class, visibility, Some(invoke_sym))?;

                        let mut frame = CallFrame::new(method.chunk.clone());
                        frame.func = Some(method.clone());
                        frame.this = Some(callable_handle);
                        frame.class_scope = Some(defining_class);
                        frame.called_scope = Some(obj_data.class);
                        frame.args = args;

                        self.push_frame(frame);
                        Ok(())
                    } else {
                        Err(VmError::RuntimeError(
                            "Object is not a closure and does not implement __invoke".into(),
                        ))
                    }
                } else {
                    Err(VmError::RuntimeError("Invalid object payload".into()))
                }
            }
            Val::Array(map) => {
                if map.map.len() != 2 {
                    return Err(VmError::RuntimeError(
                        "Callable array must have exactly 2 elements".into(),
                    ));
                }

                let class_or_obj = map
                    .map
                    .get_index(0)
                    .map(|(_, v)| *v)
                    .ok_or(VmError::RuntimeError("Invalid callable array".into()))?;
                let method_handle = map
                    .map
                    .get_index(1)
                    .map(|(_, v)| *v)
                    .ok_or(VmError::RuntimeError("Invalid callable array".into()))?;

                let method_name_bytes = self.convert_to_string(method_handle)?;
                let method_sym = self.context.interner.intern(&method_name_bytes);

                match &self.arena.get(class_or_obj).value {
                    Val::String(class_name_bytes) => {
                        let class_sym = self.context.interner.intern(class_name_bytes);
                        let class_sym = self.resolve_class_name(class_sym)?;

                        if let Some((method, visibility, is_static, defining_class)) =
                            self.find_method(class_sym, method_sym)
                        {
                            self.check_method_visibility(
                                defining_class,
                                visibility,
                                Some(method_sym),
                            )?;

                            let mut frame = CallFrame::new(method.chunk.clone());
                            frame.func = Some(method.clone());
                            frame.class_scope = Some(defining_class);
                            frame.called_scope = Some(class_sym);
                            frame.args = args;

                            if !is_static {
                                // Allow but do not provide $this; PHP would emit a notice.
                            }

                            self.push_frame(frame);
                            Ok(())
                        } else {
                            let class_str = String::from_utf8_lossy(class_name_bytes);
                            let method_str = String::from_utf8_lossy(&method_name_bytes);
                            Err(VmError::RuntimeError(format!(
                                "Call to undefined method {}::{}",
                                class_str, method_str
                            )))
                        }
                    }
                    Val::Object(payload_handle) => {
                        let payload_val = self.arena.get(*payload_handle);
                        if let Val::ObjPayload(obj_data) = &payload_val.value {
                            if let Some((method, visibility, _, defining_class)) =
                                self.find_method(obj_data.class, method_sym)
                            {
                                self.check_method_visibility(
                                    defining_class,
                                    visibility,
                                    Some(method_sym),
                                )?;

                                let mut frame = CallFrame::new(method.chunk.clone());
                                frame.func = Some(method.clone());
                                frame.this = Some(class_or_obj);
                                frame.class_scope = Some(defining_class);
                                frame.called_scope = Some(obj_data.class);
                                frame.args = args;

                                self.push_frame(frame);
                                Ok(())
                            } else {
                                let class_str = String::from_utf8_lossy(
                                    self.context.interner.lookup(obj_data.class).unwrap_or(b"?"),
                                );
                                let method_str = String::from_utf8_lossy(&method_name_bytes);
                                Err(VmError::RuntimeError(format!(
                                    "Call to undefined method {}::{}",
                                    class_str, method_str
                                )))
                            }
                        } else {
                            Err(VmError::RuntimeError(
                                "Invalid object in callable array".into(),
                            ))
                        }
                    }
                    _ => Err(VmError::RuntimeError(
                        "First element of callable array must be object or class name".into(),
                    )),
                }
            }
            _ => Err(VmError::RuntimeError(format!(
                "Call expects function name or closure (got {})",
                self.describe_handle(callable_handle)
            ))),
        }
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
    pub fn call_callable(&mut self, callable_handle: Handle, args: ArgList) -> Result<Handle, VmError> {
        self.invoke_callable_value(callable_handle, args)?;
        let depth = self.frames.len();
        if depth > 0 {
            self.run_loop(depth - 1)?;
        }
        Ok(self.last_return_value.unwrap_or_else(|| self.arena.alloc(Val::Null)))
    }

    fn convert_to_string(&mut self, handle: Handle) -> Result<Vec<u8>, VmError> {
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
                Err(VmError::RuntimeError(format!(
                    "Cannot convert value to string"
                )))
            }
        }
    }

    fn describe_handle(&self, handle: Handle) -> String {
        let val = self.arena.get(handle);
        match &val.value {
            Val::Null => "null".into(),
            Val::Bool(b) => format!("bool({})", b),
            Val::Int(i) => format!("int({})", i),
            Val::Float(f) => format!("float({})", f),
            Val::String(s) => {
                let preview = String::from_utf8_lossy(&s[..s.len().min(32)])
                    .replace('\n', "\\n")
                    .replace('\r', "\\r");
                format!(
                    "string(len={}, \"{}{}\")",
                    s.len(),
                    preview,
                    if s.len() > 32 { "…" } else { "" }
                )
            }
            Val::Array(_) => "array".into(),
            Val::Object(_) => "object".into(),
            Val::ObjPayload(_) => "object(payload)".into(),
            Val::Resource(_) => "resource".into(),
            Val::AppendPlaceholder => "append-placeholder".into(),
        }
    }

    fn describe_object_class(&self, payload_handle: Handle) -> String {
        if let Val::ObjPayload(obj_data) = &self.arena.get(payload_handle).value {
            String::from_utf8_lossy(
                self.context
                    .interner
                    .lookup(obj_data.class)
                    .unwrap_or(b"<unknown>"),
            )
            .into_owned()
        } else {
            "<unknown>".into()
        }
    }

    fn handle_return(&mut self, force_by_ref: bool, target_depth: usize) -> Result<(), VmError> {
        let frame_base = {
            let frame = self.current_frame()?;
            frame.stack_base.unwrap_or(0)
        };

        let ret_val = if self.operand_stack.len() > frame_base {
            self.pop_operand()?
        } else {
            self.arena.alloc(Val::Null)
        };

        // Verify return type BEFORE popping the frame
        // Extract type info first to avoid borrow checker issues
        let return_type_check = {
            let frame = self.current_frame()?;
            frame.func.as_ref().and_then(|f| {
                f.return_type.as_ref().map(|rt| {
                    let func_name = self.context
                        .interner
                        .lookup(f.chunk.name)
                        .map(|b| String::from_utf8_lossy(b).to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    (rt.clone(), func_name)
                })
            })
        };

        if let Some((ret_type, func_name)) = return_type_check {
            if !self.check_return_type(ret_val, &ret_type)? {
                let val_type = self.get_type_name(ret_val);
                let expected_type = self.return_type_to_string(&ret_type);

                return Err(VmError::RuntimeError(format!(
                    "{}(): Return value must be of type {}, {} returned",
                    func_name, expected_type, val_type
                )));
            }
        }

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

    fn run_loop(&mut self, target_depth: usize) -> Result<(), VmError> {
        let mut instruction_count = 0u64;
        const TIMEOUT_CHECK_INTERVAL: u64 = 1000; // Check every 1000 instructions
        
        while self.frames.len() > target_depth {
            // Periodically check execution timeout
            instruction_count += 1;
            if instruction_count % TIMEOUT_CHECK_INTERVAL == 0 {
                self.check_execution_timeout()?;
            }

            let op = {
                let frame = self.current_frame_mut()?;
                if frame.ip >= frame.chunk.code.len() {
                    self.frames.pop();
                    continue;
                }
                let op = frame.chunk.code[frame.ip].clone();
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
                if self.operand_stack.pop().is_none() {
                    return Err(VmError::RuntimeError("Stack underflow".into()));
                }
            }
            OpCode::Dup => {
                let handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                self.operand_stack.push(handle);
            }
            OpCode::Nop => {}
            _ => unreachable!("Not a stack op"),
        }
        Ok(())
    }

    fn exec_math_op(&mut self, op: OpCode) -> Result<(), VmError> {
        match op {
            OpCode::Add => self.arithmetic_add()?,
            OpCode::Sub => self.arithmetic_sub()?,
            OpCode::Mul => self.arithmetic_mul()?,
            OpCode::Div => self.arithmetic_div()?,
            OpCode::Mod => self.arithmetic_mod()?,
            OpCode::Pow => self.arithmetic_pow()?,
            OpCode::BitwiseAnd => self.bitwise_and()?,
            OpCode::BitwiseOr => self.bitwise_or()?,
            OpCode::BitwiseXor => self.bitwise_xor()?,
            OpCode::ShiftLeft => self.bitwise_shl()?,
            OpCode::ShiftRight => self.bitwise_shr()?,
            OpCode::BitwiseNot => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val = self.arena.get(handle).value.clone();
                let res = match val {
                    Val::Int(i) => Val::Int(!i),
                    Val::String(s) => {
                        // Bitwise NOT on strings flips each byte
                        let inverted: Vec<u8> = s.iter().map(|&b| !b).collect();
                        Val::String(Rc::new(inverted))
                    }
                    _ => {
                        let i = val.to_int();
                        Val::Int(!i)
                    }
                };
                let res_handle = self.arena.alloc(res);
                self.operand_stack.push(res_handle);
            }
            OpCode::BoolNot => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val = &self.arena.get(handle).value;
                let b = val.to_bool();
                let res_handle = self.arena.alloc(Val::Bool(!b));
                self.operand_stack.push(res_handle);
            }
            _ => unreachable!("Not a math op"),
        }
        Ok(())
    }

    fn exec_control_flow(&mut self, op: OpCode) -> Result<(), VmError> {
        match op {
            OpCode::Jmp(target) => {
                let frame = self.current_frame_mut()?;
                frame.ip = target as usize;
            }
            OpCode::JmpIfFalse(target) => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val = &self.arena.get(handle).value;
                let b = val.to_bool();
                if !b {
                    let frame = self.current_frame_mut()?;
                    frame.ip = target as usize;
                }
            }
            OpCode::JmpIfTrue(target) => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val = &self.arena.get(handle).value;
                let b = val.to_bool();
                if b {
                    let frame = self.current_frame_mut()?;
                    frame.ip = target as usize;
                }
            }
            OpCode::JmpZEx(target) => {
                let handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val = &self.arena.get(handle).value;
                let b = val.to_bool();
                if !b {
                    let frame = self.current_frame_mut()?;
                    frame.ip = target as usize;
                } else {
                    self.operand_stack.pop();
                }
            }
            OpCode::JmpNzEx(target) => {
                let handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val = &self.arena.get(handle).value;
                let b = val.to_bool();
                if b {
                    let frame = self.current_frame_mut()?;
                    frame.ip = target as usize;
                } else {
                    self.operand_stack.pop();
                }
            }
            OpCode::Coalesce(target) => {
                let handle = self
                    .operand_stack
                    .peek()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val = &self.arena.get(handle).value;
                let is_null = matches!(val, Val::Null);

                if !is_null {
                    let frame = self.current_frame_mut()?;
                    frame.ip = target as usize;
                } else {
                    self.operand_stack.pop();
                }
            }
            _ => unreachable!("Not a control flow op"),
        }
        Ok(())
    }

    fn execute_opcode(&mut self, op: OpCode, target_depth: usize) -> Result<(), VmError> {
        match op {
            OpCode::Throw => {
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
                    return Err(VmError::RuntimeError(
                        "Can only throw objects".into(),
                    ));
                }

                let payload_handle = payload_handle_opt.unwrap();

                // Validate that the object implements Throwable interface
                let throwable_sym = self.context.interner.intern(b"Throwable");
                if !self.is_instance_of(ex_handle, throwable_sym) {
                    // Get the class name for error message
                    let class_name = if let Val::ObjPayload(obj_data) = &self.arena.get(payload_handle).value {
                        String::from_utf8_lossy(
                            self.context.interner.lookup(obj_data.class).unwrap_or(b"Object")
                        ).to_string()
                    } else {
                        "Object".to_string()
                    };
                    
                    return Err(VmError::RuntimeError(
                        format!("Cannot throw objects that do not implement Throwable ({})", class_name)
                    ));
                }

                // Set exception properties (file, line, trace) at throw time
                // This mimics PHP's behavior of capturing context when exception is thrown
                let file_sym = self.context.interner.intern(b"file");
                let line_sym = self.context.interner.intern(b"line");
                
                // Get current file and line from frame
                let (file_path, line_no) = if let Some(frame) = self.frames.last() {
                    let file = frame.chunk.file_path.clone().unwrap_or_else(|| "unknown".to_string());
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

                return Err(VmError::Exception(ex_handle));
            }
            OpCode::Catch => {
                // Exception object is already on the operand stack (pushed by handler); nothing else to do.
            }
            OpCode::Const(_) | OpCode::Pop | OpCode::Dup | OpCode::Nop => self.exec_stack_op(op)?,
            OpCode::Add
            | OpCode::Sub
            | OpCode::Mul
            | OpCode::Div
            | OpCode::Mod
            | OpCode::Pow
            | OpCode::BitwiseAnd
            | OpCode::BitwiseOr
            | OpCode::BitwiseXor
            | OpCode::ShiftLeft
            | OpCode::ShiftRight
            | OpCode::BitwiseNot
            | OpCode::BoolNot => self.exec_math_op(op)?,

            OpCode::LoadVar(sym) => {
                let handle = {
                    let frame = self.current_frame()?;
                    frame.locals.get(&sym).copied()
                };

                if let Some(handle) = handle {
                    self.operand_stack.push(handle);
                } else {
                    let name = self.context.interner.lookup(sym);
                    if name == Some(b"this") {
                        let frame = self.current_frame()?;
                        if let Some(this_val) = frame.this {
                            self.operand_stack.push(this_val);
                        } else {
                            return Err(VmError::RuntimeError(
                                "Using $this when not in object context".into(),
                            ));
                        }
                    } else if self.is_superglobal(sym) {
                        if let Some(handle) = self.ensure_superglobal_handle(sym) {
                            let frame = self.current_frame_mut()?;
                            frame.locals.entry(sym).or_insert(handle);
                            self.operand_stack.push(handle);
                        } else {
                            let null = self.arena.alloc(Val::Null);
                            self.operand_stack.push(null);
                        }
                    } else {
                        let var_name = String::from_utf8_lossy(name.unwrap_or(b"unknown"));
                        let msg = format!("Undefined variable: ${}", var_name);
                        self.report_error(ErrorLevel::Notice, &msg);
                        let null = self.arena.alloc(Val::Null);
                        self.operand_stack.push(null);
                    }
                }
            }
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
                    self.operand_stack.push(handle);
                } else if self.is_superglobal(sym) {
                    if let Some(handle) = self.ensure_superglobal_handle(sym) {
                        if let Some(frame) = self.frames.last_mut() {
                            frame.locals.entry(sym).or_insert(handle);
                        }
                        self.operand_stack.push(handle);
                    } else {
                        let null = self.arena.alloc(Val::Null);
                        self.operand_stack.push(null);
                    }
                } else {
                    let var_name = String::from_utf8_lossy(&name_bytes);
                    let msg = format!("Undefined variable: ${}", var_name);
                    self.report_error(ErrorLevel::Notice, &msg);
                    let null = self.arena.alloc(Val::Null);
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
                let to_bind = if self.is_superglobal(sym) {
                    self.ensure_superglobal_handle(sym)
                } else {
                    None
                };
                let frame = self.frames.last_mut().unwrap();

                if let Some(handle) = to_bind {
                    frame.locals.entry(sym).or_insert(handle);
                }

                // Check if the target variable is a reference
                let mut is_target_ref = false;
                if let Some(&old_handle) = frame.locals.get(&sym) {
                    if self.arena.get(old_handle).is_ref {
                        is_target_ref = true;
                        // Assigning to a reference: update the value in place
                        let new_val = self.arena.get(val_handle).value.clone();
                        self.arena.get_mut(old_handle).value = new_val;
                    }
                }

                if !is_target_ref {
                    // Not assigning to a reference.
                    // We MUST clone the value to ensure value semantics (no implicit sharing).
                    // Unless we implement COW with refcounts.
                    let val = self.arena.get(val_handle).value.clone();
                    let final_handle = self.arena.alloc(val);

                    frame.locals.insert(sym, final_handle);
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

                let frame = self.frames.last_mut().unwrap();

                if let Some(handle) = to_bind {
                    frame.locals.entry(sym).or_insert(handle);
                }

                // Check if the target variable is a reference
                let result_handle = if let Some(&old_handle) = frame.locals.get(&sym) {
                    if self.arena.get(old_handle).is_ref {
                        let new_val = self.arena.get(val_handle).value.clone();
                        self.arena.get_mut(old_handle).value = new_val;
                        old_handle
                    } else {
                        let val = self.arena.get(val_handle).value.clone();
                        let final_handle = self.arena.alloc(val);
                        frame.locals.insert(sym, final_handle);
                        final_handle
                    }
                } else {
                    let val = self.arena.get(val_handle).value.clone();
                    let final_handle = self.arena.alloc(val);
                    frame.locals.insert(sym, final_handle);
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
                        5 => match (current_val, val) {
                            // ShiftLeft
                            (Val::Int(a), Val::Int(b)) => Val::Int(a << b),
                            _ => Val::Null,
                        },
                        6 => match (current_val, val) {
                            // ShiftRight
                            (Val::Int(a), Val::Int(b)) => Val::Int(a >> b),
                            _ => Val::Null,
                        },
                        7 => match (current_val, val) {
                            // Concat
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
                            _ => Val::Null,
                        },
                        8 => match (current_val, val) {
                            // BitwiseOr
                            (Val::Int(a), Val::Int(b)) => Val::Int(a | b),
                            _ => Val::Null,
                        },
                        9 => match (current_val, val) {
                            // BitwiseAnd
                            (Val::Int(a), Val::Int(b)) => Val::Int(a & b),
                            _ => Val::Null,
                        },
                        10 => match (current_val, val) {
                            // BitwiseXor
                            (Val::Int(a), Val::Int(b)) => Val::Int(a ^ b),
                            _ => Val::Null,
                        },
                        11 => match (current_val, val) {
                            // Pow
                            (Val::Int(a), Val::Int(b)) => {
                                if b < 0 {
                                    return Err(VmError::RuntimeError(
                                        "Negative exponent not supported for int pow".into(),
                                    ));
                                }
                                Val::Int(a.pow(b as u32))
                            }
                            _ => Val::Null,
                        },
                        _ => Val::Null,
                    };

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

                if self.arena.get(handle).is_ref {
                    self.operand_stack.push(handle);
                } else {
                    // Convert to ref. Clone to ensure uniqueness/safety.
                    let val = self.arena.get(handle).value.clone();
                    let new_handle = self.arena.alloc(val);
                    self.arena.get_mut(new_handle).is_ref = true;
                    self.operand_stack.push(new_handle);
                }
            }

            OpCode::Jmp(_)
            | OpCode::JmpIfFalse(_)
            | OpCode::JmpIfTrue(_)
            | OpCode::JmpZEx(_)
            | OpCode::JmpNzEx(_)
            | OpCode::Coalesce(_) => self.exec_control_flow(op)?,

            OpCode::Echo => {
                let handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let s = self.convert_to_string(handle)?;
                self.write_output(&s)?;
            }
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
                        Val::Null => Val::Array(crate::core::value::ArrayData::new().into()),
                        _ => {
                            let mut map = IndexMap::new();
                            map.insert(ArrayKey::Int(0), self.arena.alloc(val));
                            Val::Array(crate::core::value::ArrayData::from(map).into())
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
                    interfaces: Vec::new(),
                    traits: Vec::new(),
                    methods,
                    properties: IndexMap::new(),
                    constants: HashMap::new(),
                    static_properties: HashMap::new(),
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
                let args = self.collect_call_args(arg_count)?;

                let func_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                self.invoke_callable_value(func_handle, args)?;
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
                let frame = self.frames.last_mut().unwrap();
                if let Some(func) = &frame.func {
                    if (arg_idx as usize) < func.params.len() {
                        let param = &func.params[arg_idx as usize];
                        if (arg_idx as usize) < frame.args.len() {
                            let arg_handle = frame.args[arg_idx as usize];
                            if param.by_ref {
                                if !self.arena.get(arg_handle).is_ref {
                                    self.arena.get_mut(arg_handle).is_ref = true;
                                }
                                frame.locals.insert(param.name, arg_handle);
                            } else {
                                let val = self.arena.get(arg_handle).value.clone();
                                let final_handle = self.arena.alloc(val);
                                frame.locals.insert(param.name, final_handle);
                            }
                        }
                    }
                }
            }
            OpCode::RecvInit(arg_idx, default_val_idx) => {
                let frame = self.frames.last_mut().unwrap();
                if let Some(func) = &frame.func {
                    if (arg_idx as usize) < func.params.len() {
                        let param = &func.params[arg_idx as usize];
                        if (arg_idx as usize) < frame.args.len() {
                            let arg_handle = frame.args[arg_idx as usize];
                            if param.by_ref {
                                if !self.arena.get(arg_handle).is_ref {
                                    self.arena.get_mut(arg_handle).is_ref = true;
                                }
                                frame.locals.insert(param.name, arg_handle);
                            } else {
                                let val = self.arena.get(arg_handle).value.clone();
                                let final_handle = self.arena.alloc(val);
                                frame.locals.insert(param.name, final_handle);
                            }
                        } else {
                            let default_val =
                                frame.chunk.constants[default_val_idx as usize].clone();
                            let default_handle = self.arena.alloc(default_val);
                            frame.locals.insert(param.name, default_handle);
                        }
                    }
                }
            }
            OpCode::RecvVariadic(arg_idx) => {
                let frame = self.frames.last_mut().unwrap();
                if let Some(func) = &frame.func {
                    if (arg_idx as usize) < func.params.len() {
                        let param = &func.params[arg_idx as usize];
                        let mut arr = IndexMap::new();
                        let args_len = frame.args.len();
                        if args_len > arg_idx as usize {
                            for (i, handle) in frame.args[arg_idx as usize..].iter().enumerate() {
                                if param.by_ref {
                                    if !self.arena.get(*handle).is_ref {
                                        self.arena.get_mut(*handle).is_ref = true;
                                    }
                                    arr.insert(ArrayKey::Int(i as i64), *handle);
                                } else {
                                    let val = self.arena.get(*handle).value.clone();
                                    let h = self.arena.alloc(val);
                                    arr.insert(ArrayKey::Int(i as i64), h);
                                }
                            }
                        }
                        let arr_handle = self
                            .arena
                            .alloc(Val::Array(crate::core::value::ArrayData::from(arr).into()));
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
                        let op = frame.chunk.code[frame.ip].clone();
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

            OpCode::InitArray(_size) => {
                let handle = self
                    .arena
                    .alloc(Val::Array(crate::core::value::ArrayData::new().into()));
                self.operand_stack.push(handle);
            }

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
                                let result = self.call_array_access_offset_get(array_handle, key_handle)?;
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
                            let result = self.call_array_access_offset_get(array_handle, key_handle)?;
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

            OpCode::AssignDim => {
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
                self.assign_dim_value(array_handle, key_handle, val_handle)?;
            }

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
                                    let result = self.call_array_access_offset_get(array_handle, key_handle)?;
                                    self.arena.get(result).value.clone()
                                } else {
                                    return Err(VmError::RuntimeError(
                                        "Trying to access offset on non-array".into(),
                                    ))
                                }
                            } else {
                                return Err(VmError::RuntimeError(
                                    "Trying to access offset on non-array".into(),
                                ))
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
                    Rc::make_mut(map).map.insert(key, val_handle);
                } else {
                    return Err(VmError::RuntimeError(
                        "AddArrayElement expects array".into(),
                    ));
                }
            }
            OpCode::StoreDim => {
                let array_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let key_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                self.assign_dim(array_handle, key_handle, val_handle)?;
            }

            OpCode::AppendArray => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let array_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                self.append_array(array_handle, val_handle)?;
            }
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
                        dest_zval.value = Val::Array(crate::core::value::ArrayData::new().into());
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

                let mut next_key = dest_map
                    .map
                    .keys()
                    .filter_map(|k| {
                        if let ArrayKey::Int(i) = k {
                            Some(i)
                        } else {
                            None
                        }
                    })
                    .max()
                    .map(|i| i + 1)
                    .unwrap_or(0);

                for (key, val_handle) in src_map.map.iter() {
                    match key {
                        ArrayKey::Int(_) => {
                            Rc::make_mut(dest_map)
                                .map
                                .insert(ArrayKey::Int(next_key), *val_handle);
                            next_key += 1;
                        }
                        ArrayKey::Str(s) => {
                            Rc::make_mut(dest_map)
                                .map
                                .insert(ArrayKey::Str(s.clone()), *val_handle);
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

            OpCode::StoreNestedDim(depth) => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let mut keys = Vec::with_capacity(depth as usize);
                for _ in 0..depth {
                    let k = self.operand_stack
                            .pop()
                            .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    keys.push(k);
                }
                keys.reverse();
                let array_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                self.assign_nested_dim(array_handle, &keys, val_handle)?;
            }

            OpCode::FetchNestedDim(depth) => {
                // Stack: [array, key_n, ..., key_1] (top is key_1)
                // We need to peek at them without popping.

                // Array is at depth + 1 from top (0-indexed)
                // key_1 is at 0
                // key_n is at depth - 1

                let array_handle = self
                    .operand_stack
                    .peek_at(depth as usize)
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;

                let mut keys = Vec::with_capacity(depth as usize);
                for i in 0..depth {
                    // key_n is at depth - 1 - i
                    // key_1 is at 0
                    // We want keys in order [key_n, ..., key_1]
                    // Wait, StoreNestedDim pops key_1 first (top), then key_2...
                    // So stack top is key_1 (last dimension).
                    // keys vector should be [key_n, ..., key_1].

                    // Stack:
                    // Top: key_1
                    // ...
                    // Bottom: key_n
                    // Bottom-1: array

                    // So key_1 is at index 0.
                    // key_n is at index depth-1.

                    // We want keys to be [key_n, ..., key_1].
                    // So we iterate from depth-1 down to 0.

                    let key_handle = self
                        .operand_stack
                        .peek_at((depth - 1 - i) as usize)
                        .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                    keys.push(key_handle);
                }

                let val_handle = self.fetch_nested_dim(array_handle, &keys)?;
                self.operand_stack.push(val_handle);
            }

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
                                } else {
                                    return Err(VmError::RuntimeError(
                                        "Object not iterable".into(),
                                    ));
                                }
                            } else {
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
                                } else {
                                    return Err(VmError::RuntimeError(
                                        "Object not iterable".into(),
                                    ));
                                }
                            } else {
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
                                } else {
                                    return Err(VmError::RuntimeError(
                                        "Object not iterable".into(),
                                    ));
                                }
                            } else {
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
                if let Val::Array(map) = array_val {
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
                        return Err(VmError::RuntimeError("Iterator index out of bounds".into()));
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

                if let Some(parent_sym) = parent {
                    if let Some(parent_def) = self.context.classes.get(&parent_sym) {
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
                    interfaces: Vec::new(),
                    traits: Vec::new(),
                    methods,
                    properties: IndexMap::new(),
                    constants: HashMap::new(),
                    static_properties: HashMap::new(),
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
                    interfaces: Vec::new(),
                    traits: Vec::new(),
                    methods: HashMap::new(),
                    properties: IndexMap::new(),
                    constants: HashMap::new(),
                    static_properties: HashMap::new(),
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
                    interfaces: Vec::new(),
                    traits: Vec::new(),
                    methods: HashMap::new(),
                    properties: IndexMap::new(),
                    constants: HashMap::new(),
                    static_properties: HashMap::new(),
                    allows_dynamic_properties: false,
                };
                self.context.classes.insert(name, class_def);
            }
            OpCode::AddInterface(class_name, interface_name) => {
                if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                    class_def.interfaces.push(interface_name);
                }
            }
            OpCode::AllowDynamicProperties(class_name) => {
                if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                    class_def.allows_dynamic_properties = true;
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

                if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                    class_def.traits.push(trait_name);
                    for (key, mut entry) in trait_methods {
                        // When using a trait, the methods become part of the class.
                        // The declaring class becomes the class using the trait (effectively).
                        entry.declaring_class = class_name;
                        class_def.methods.entry(key).or_insert(entry);
                    }
                }
            }
            OpCode::DefMethod(class_name, method_name, func_idx, visibility, is_static) => {
                let val = {
                    let frame = self.frames.last().unwrap();
                    frame.chunk.constants[func_idx as usize].clone()
                };
                if let Val::Resource(rc) = val {
                    if let Ok(func) = rc.downcast::<UserFunc>() {
                        let lower_key = self.intern_lowercase_symbol(method_name)?;
                        if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                            let entry = MethodEntry {
                                name: method_name,
                                func,
                                visibility,
                                is_static,
                                declaring_class: class_name,
                            };
                            class_def.methods.insert(lower_key, entry);
                        }
                    }
                }
            }
            OpCode::DefProp(class_name, prop_name, default_idx, visibility) => {
                let val = {
                    let frame = self.frames.last().unwrap();
                    frame.chunk.constants[default_idx as usize].clone()
                };
                if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                    class_def.properties.insert(prop_name, (val, visibility));
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
                } else if let Some(val) = self.context.engine.constants.get(&name) {
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
            OpCode::DefStaticProp(class_name, prop_name, default_idx, visibility) => {
                let val = {
                    let frame = self.frames.last().unwrap();
                    frame.chunk.constants[default_idx as usize].clone()
                };
                if let Some(class_def) = self.context.classes.get_mut(&class_name) {
                    class_def
                        .static_properties
                        .insert(prop_name, (val, visibility));
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
                let handle = self.arena.alloc(val);
                self.operand_stack.push(handle);
            }
            OpCode::AssignStaticProp(class_name, prop_name) => {
                let val_handle = self
                    .operand_stack
                    .pop()
                    .ok_or(VmError::RuntimeError("Stack underflow".into()))?;
                let val = self.arena.get(val_handle).value.clone();

                let resolved_class = self.resolve_class_name(class_name)?;
                let (_, visibility, defining_class) =
                    self.find_static_prop(resolved_class, prop_name)?;
                self.check_const_visibility(defining_class, visibility)?;

                if let Some(class_def) = self.context.classes.get_mut(&defining_class) {
                    if let Some(entry) = class_def.static_properties.get_mut(&prop_name) {
                        entry.0 = val.clone();
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
                        entry.0 = val.clone();
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
                // Try autoloading if class doesn't exist
                if !self.context.classes.contains_key(&class_name) {
                    self.trigger_autoload(class_name)?;
                }
                
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
                        let native_constructor = self.find_native_method(class_name, constructor_name);
                        if let Some(native_entry) = native_constructor {
                            // Call native constructor
                            let args = self.collect_call_args(arg_count)?;
                            
                            // Set this in current frame temporarily
                            let saved_this = self.frames.last().and_then(|f| f.this);
                            if let Some(frame) = self.frames.last_mut() {
                                frame.this = Some(obj_handle);
                            }

                            // Call native handler
                            let _result = (native_entry.handler)(self, &args).map_err(VmError::RuntimeError)?;

                            // Restore previous this
                            if let Some(frame) = self.frames.last_mut() {
                                frame.this = saved_this;
                            }

                            self.operand_stack.push(obj_handle);
                        } else {
                        // No constructor found
                        // For built-in exception/error classes, accept args silently (they have implicit constructors)
                        let is_builtin_exception = {
                            let class_name_bytes = self
                                .context
                                .interner
                                .lookup(class_name)
                                .unwrap_or(b"");
                            matches!(
                                class_name_bytes,
                                b"Exception" | b"Error" | b"Throwable" | b"RuntimeException" |
                                b"LogicException" | b"TypeError" | b"ArithmeticError" |
                                b"DivisionByZeroError" | b"ParseError" | b"ArgumentCountError"
                            )
                        };

                        if arg_count > 0 && !is_builtin_exception {
                            let class_name_bytes = self
                                .context
                                .interner
                                .lookup(class_name)
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

                        let payload_zval = self.arena.get_mut(payload_handle);
                        if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                            obj_data.properties.insert(prop_name, val_handle);
                        }
                        self.operand_stack.push(val_handle);
                    }
                } else {
                    // Check for dynamic property deprecation (PHP 8.2+)
                    if !prop_exists {
                        self.check_dynamic_property_write(obj_handle, prop_name);
                    }

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
                let obj_handle = self
                    .operand_stack
                    .peek_at(arg_count as usize)
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
                    self.check_method_visibility(native_entry.declaring_class, native_entry.visibility, Some(method_name))?;

                    // Collect args and pop object
                    let args = self.collect_call_args(arg_count)?;
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

                        let obj_handle = self.operand_stack.pop().unwrap();

                        // Create array from args
                        let mut array_map = IndexMap::new();
                        for (i, arg) in args.into_iter().enumerate() {
                            array_map.insert(ArrayKey::Int(i as i64), arg);
                        }
                        let args_array_handle = self.arena.alloc(Val::Array(
                            crate::core::value::ArrayData::from(array_map).into(),
                        ));

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
                let arr_handle = self
                    .arena
                    .alloc(Val::Array(crate::core::value::ArrayData::from(map).into()));
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
                    let source = path_str.as_bytes();
                    let arena = bumpalo::Bump::new();
                    let lexer = php_parser::lexer::Lexer::new(source);
                    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
                    let program = parser.parse_program();

                    if !program.errors.is_empty() {
                        // Eval error: in PHP 7+ throws ParseError
                        return Err(VmError::RuntimeError(format!(
                            "Eval parse errors: {:?}",
                            program.errors
                        )));
                    }

                    let emitter =
                        crate::compiler::emitter::Emitter::new(source, &mut self.context.interner);
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

                    // Execute eval'd code (inline run_loop to capture locals before pop)
                    let mut eval_error = None;
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
                            let op = frame.chunk.code[frame.ip].clone();
                            frame.ip += 1;
                            op
                        };

                        if let Err(e) = self.execute_opcode(op, depth) {
                            eval_error = Some(e);
                            break;
                        }
                    }

                    // Capture eval frame's final locals before popping
                    let final_locals = if self.frames.len() >= depth {
                        Some(self.frames[depth - 1].locals.clone())
                    } else {
                        None
                    };

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

                    // Eval returns its explicit return value or null
                    let return_val = self
                        .last_return_value
                        .unwrap_or_else(|| self.arena.alloc(Val::Null));
                    self.last_return_value = None;
                    self.operand_stack.push(return_val);
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
                                        let op = frame.chunk.code[frame.ip].clone();
                                        frame.ip += 1;
                                        op
                                    };

                                    if let Err(e) = self.execute_opcode(op, depth) {
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
                            _ => ArrayKey::Str(std::rc::Rc::new(Vec::<u8>::new())),
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
                            _ => ArrayKey::Str(std::rc::Rc::new(Vec::<u8>::new())),
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
                            &Val::ObjPayload(ref obj_data) => {
                                Some(obj_data.class)
                            }
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
                    _ => ArrayKey::Str(std::rc::Rc::new(Vec::<u8>::new())),
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
                        *container = Val::Array(crate::core::value::ArrayData::new().into());
                    }

                    if let Val::Array(map) = container {
                        Rc::make_mut(map).map.insert(key, val_handle);
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
                let handle = self
                    .arena
                    .alloc(Val::Array(crate::core::value::ArrayData::from(map).into()));
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
                    // Isset
                    // isset returns true if var exists and is not null
                    if let Some(h) = val_handle {
                        !matches!(self.arena.get(h).value, Val::Null)
                    } else {
                        false
                    }
                } else {
                    // Empty
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
                                    match self.call_array_access_offset_get(container_handle, dim_handle) {
                                        Ok(h) => Some(h),
                                        Err(_) => None,
                                    }
                                } else {
                                    // empty: need to check the actual value via offsetGet
                                    match self.call_array_access_offset_get(container_handle, dim_handle) {
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
                            self.context.interner.lookup(class_name).unwrap_or(b"Unknown")
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
                                _ => ArrayKey::Str(std::rc::Rc::new(Vec::<u8>::new())),
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
                        let sym = self.context.interner.intern(&prop_name);
                        
                        let class_name = {
                            let payload = self.arena.get(*obj_handle);
                            if let Val::ObjPayload(data) = &payload.value {
                                data.class
                            } else {
                                self.context.interner.intern(b"")
                            }
                        };

                        let magic_isset = self.context.interner.intern(b"__isset");
                        if let Some((method, _, _, defined_class)) =
                            self.find_method(class_name, magic_isset)
                        {
                            let name_handle = self.arena.alloc(Val::String(prop_name));

                            let mut frame = CallFrame::new(method.chunk.clone());
                            frame.func = Some(method.clone());
                            frame.this = Some(container_handle);
                            frame.class_scope = Some(defined_class);
                            frame.called_scope = Some(class_name);

                            if let Some(param) = method.params.get(0) {
                                frame.locals.insert(param.name, name_handle);
                            }

                            self.push_frame(frame);
                            

                            // __isset returns a boolean value
                            self.last_return_value
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

                let res = match op {
                    0 => match (current_val.clone(), val) {
                        // Add
                        (Val::Int(a), Val::Int(b)) => Val::Int(a + b),
                        _ => Val::Null,
                    },
                    1 => match (current_val.clone(), val) {
                        // Sub
                        (Val::Int(a), Val::Int(b)) => Val::Int(a - b),
                        _ => Val::Null,
                    },
                    2 => match (current_val.clone(), val) {
                        // Mul
                        (Val::Int(a), Val::Int(b)) => Val::Int(a * b),
                        _ => Val::Null,
                    },
                    3 => match (current_val.clone(), val) {
                        // Div
                        (Val::Int(a), Val::Int(b)) => Val::Int(a / b),
                        _ => Val::Null,
                    },
                    4 => match (current_val.clone(), val) {
                        // Mod
                        (Val::Int(a), Val::Int(b)) => {
                            if b == 0 {
                                return Err(VmError::RuntimeError("Modulo by zero".into()));
                            }
                            Val::Int(a % b)
                        }
                        _ => Val::Null,
                    },
                    7 => match (current_val.clone(), val) {
                        // Concat
                        (Val::String(a), Val::String(b)) => {
                            let mut s = String::from_utf8_lossy(&a).to_string();
                            s.push_str(&String::from_utf8_lossy(&b));
                            Val::String(s.into_bytes().into())
                        }
                        _ => Val::Null,
                    },
                    _ => Val::Null, // TODO: Implement other ops
                };

                if let Some(class_def) = self.context.classes.get_mut(&defining_class) {
                    if let Some(entry) = class_def.static_properties.get_mut(&prop_name) {
                        entry.0 = res.clone();
                    }
                }

                let res_handle = self.arena.alloc(res);
                self.operand_stack.push(res_handle);
            }
            OpCode::PreIncStaticProp => {
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

                let new_val = match current_val {
                    Val::Int(i) => Val::Int(i + 1),
                    _ => Val::Null, // TODO: Support other types
                };

                if let Some(class_def) = self.context.classes.get_mut(&defining_class) {
                    if let Some(entry) = class_def.static_properties.get_mut(&prop_name) {
                        entry.0 = new_val.clone();
                    }
                }

                let res_handle = self.arena.alloc(new_val);
                self.operand_stack.push(res_handle);
            }
            OpCode::PreDecStaticProp => {
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

                let new_val = match current_val {
                    Val::Int(i) => Val::Int(i - 1),
                    _ => Val::Null, // TODO: Support other types
                };

                if let Some(class_def) = self.context.classes.get_mut(&defining_class) {
                    if let Some(entry) = class_def.static_properties.get_mut(&prop_name) {
                        entry.0 = new_val.clone();
                    }
                }

                let res_handle = self.arena.alloc(new_val);
                self.operand_stack.push(res_handle);
            }
            OpCode::PostIncStaticProp => {
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

                let new_val = match current_val {
                    Val::Int(i) => Val::Int(i + 1),
                    _ => Val::Null, // TODO: Support other types
                };

                if let Some(class_def) = self.context.classes.get_mut(&defining_class) {
                    if let Some(entry) = class_def.static_properties.get_mut(&prop_name) {
                        entry.0 = new_val.clone();
                    }
                }

                let res_handle = self.arena.alloc(current_val);
                self.operand_stack.push(res_handle);
            }
            OpCode::PostDecStaticProp => {
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

                let new_val = match current_val {
                    Val::Int(i) => Val::Int(i - 1),
                    _ => Val::Null, // TODO: Support other types
                };

                if let Some(class_def) = self.context.classes.get_mut(&defining_class) {
                    if let Some(entry) = class_def.static_properties.get_mut(&prop_name) {
                        entry.0 = new_val.clone();
                    }
                }

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

                // Get class_name first
                let class_name = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        obj_data.class
                    } else {
                        return Err(VmError::RuntimeError("Invalid object payload".into()));
                    }
                };

                let current_val = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        if let Some(val_handle) = obj_data.properties.get(&prop_name) {
                            self.arena.get(*val_handle).value.clone()
                        } else {
                            Val::Null
                        }
                    } else {
                        return Err(VmError::RuntimeError("Invalid object payload".into()));
                    }
                };

                let new_val = match current_val {
                    Val::Int(i) => Val::Int(i + 1),
                    _ => Val::Null,
                };

                let res_handle = self.arena.alloc(new_val.clone());

                // Check if we should use __set
                let current_scope = self.get_current_class();
                let (has_prop, visibility_ok) = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        let has = obj_data.properties.contains_key(&prop_name);
                        let vis_ok = has
                            && self
                                .check_prop_visibility(class_name, prop_name, current_scope)
                                .is_ok();
                        (has, vis_ok)
                    } else {
                        (false, false)
                    }
                };

                if has_prop && visibility_ok {
                    // Direct assignment
                    let payload_zval = self.arena.get_mut(payload_handle);
                    if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                        obj_data.properties.insert(prop_name, res_handle);
                    }
                } else {
                    // Try __set
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
                            frame.locals.insert(param.name, res_handle);
                        }

                        self.push_frame(frame);
                    } else {
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

                // Get current val with __get support
                let (class_name, current_val) = {
                    let (cn, prop_handle_opt, has_prop) = {
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

                    let current_scope = self.get_current_class();
                    let visibility_ok = has_prop
                        && self
                            .check_prop_visibility(cn, prop_name, current_scope)
                            .is_ok();

                    let val = if let Some(val_handle) = prop_handle_opt {
                        if visibility_ok {
                            self.arena.get(val_handle).value.clone()
                        } else {
                            // Try __get
                            let magic_get = self.context.interner.intern(b"__get");
                            if let Some((method, _, _, defined_class)) =
                                self.find_method(cn, magic_get)
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
                                frame.called_scope = Some(cn);

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
                        // Try __get
                        let magic_get = self.context.interner.intern(b"__get");
                        if let Some((method, _, _, defined_class)) =
                            self.find_method(cn, magic_get)
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
                            frame.called_scope = Some(cn);

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
                    };
                    (cn, val)
                };

                let new_val = match current_val {
                    Val::Int(i) => Val::Int(i - 1),
                    _ => Val::Null,
                };

                let res_handle = self.arena.alloc(new_val.clone());

                // Check if we should use __set
                let current_scope = self.get_current_class();
                let (has_prop, visibility_ok) = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        let has = obj_data.properties.contains_key(&prop_name);
                        let vis_ok = has
                            && self
                                .check_prop_visibility(class_name, prop_name, current_scope)
                                .is_ok();
                        (has, vis_ok)
                    } else {
                        (false, false)
                    }
                };

                if has_prop && visibility_ok {
                    // Direct assignment
                    let payload_zval = self.arena.get_mut(payload_handle);
                    if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                        obj_data.properties.insert(prop_name, res_handle);
                    }
                } else {
                    // Try __set
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
                            frame.locals.insert(param.name, res_handle);
                        }

                        self.push_frame(frame);
                        
                    } else {
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

                // Get current val with __get support
                let (class_name, current_val) = {
                    let (cn, prop_handle_opt, has_prop) = {
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

                    let current_scope = self.get_current_class();
                    let visibility_ok = has_prop
                        && self
                            .check_prop_visibility(cn, prop_name, current_scope)
                            .is_ok();

                    let val = if let Some(val_handle) = prop_handle_opt {
                        if visibility_ok {
                            self.arena.get(val_handle).value.clone()
                        } else {
                            // Try __get
                            let magic_get = self.context.interner.intern(b"__get");
                            if let Some((method, _, _, defined_class)) =
                                self.find_method(cn, magic_get)
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
                                frame.called_scope = Some(cn);

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
                        // Try __get
                        let magic_get = self.context.interner.intern(b"__get");
                        if let Some((method, _, _, defined_class)) =
                            self.find_method(cn, magic_get)
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
                            frame.called_scope = Some(cn);

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
                    };
                    (cn, val)
                };

                let new_val = match current_val.clone() {
                    Val::Int(i) => Val::Int(i + 1),
                    _ => Val::Null,
                };

                let res_handle = self.arena.alloc(current_val); // Return old value
                let new_val_handle = self.arena.alloc(new_val.clone());

                // Check if we should use __set
                let current_scope = self.get_current_class();
                let (has_prop, visibility_ok) = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        let has = obj_data.properties.contains_key(&prop_name);
                        let vis_ok = has
                            && self
                                .check_prop_visibility(class_name, prop_name, current_scope)
                                .is_ok();
                        (has, vis_ok)
                    } else {
                        (false, false)
                    }
                };

                if has_prop && visibility_ok {
                    // Direct assignment
                    let payload_zval = self.arena.get_mut(payload_handle);
                    if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                        obj_data.properties.insert(prop_name, new_val_handle);
                    }
                } else {
                    // Try __set
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
                            frame.locals.insert(param.name, new_val_handle);
                        }

                        self.push_frame(frame);
                        
                    } else {
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

                // Get current val with __get support
                let (class_name, current_val) = {
                    let (cn, prop_handle_opt, has_prop) = {
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

                    let current_scope = self.get_current_class();
                    let visibility_ok = has_prop
                        && self
                            .check_prop_visibility(cn, prop_name, current_scope)
                            .is_ok();

                    let val = if let Some(val_handle) = prop_handle_opt {
                        if visibility_ok {
                            self.arena.get(val_handle).value.clone()
                        } else {
                            // Try __get
                            let magic_get = self.context.interner.intern(b"__get");
                            if let Some((method, _, _, defined_class)) =
                                self.find_method(cn, magic_get)
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
                                frame.called_scope = Some(cn);

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
                        // Try __get
                        let magic_get = self.context.interner.intern(b"__get");
                        if let Some((method, _, _, defined_class)) =
                            self.find_method(cn, magic_get)
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
                            frame.called_scope = Some(cn);

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
                    };
                    (cn, val)
                };

                let new_val = match current_val.clone() {
                    Val::Int(i) => Val::Int(i - 1),
                    _ => Val::Null,
                };

                let res_handle = self.arena.alloc(current_val); // Return old value
                let new_val_handle = self.arena.alloc(new_val.clone());

                // Check if we should use __set
                let current_scope = self.get_current_class();
                let (has_prop, visibility_ok) = {
                    let payload_zval = self.arena.get(payload_handle);
                    if let Val::ObjPayload(obj_data) = &payload_zval.value {
                        let has = obj_data.properties.contains_key(&prop_name);
                        let vis_ok = has
                            && self
                                .check_prop_visibility(class_name, prop_name, current_scope)
                                .is_ok();
                        (has, vis_ok)
                    } else {
                        (false, false)
                    }
                };

                if has_prop && visibility_ok {
                    // Direct assignment
                    let payload_zval = self.arena.get_mut(payload_handle);
                    if let Val::ObjPayload(obj_data) = &mut payload_zval.value {
                        obj_data.properties.insert(prop_name, new_val_handle);
                    }
                } else {
                    // Try __set
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
                            frame.locals.insert(param.name, new_val_handle);
                        }

                        self.push_frame(frame);
                        
                    } else {
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
                                match self.call_array_access_offset_exists(array_handle, key_handle) {
                                    Ok(exists) => {
                                        if !exists {
                                            false
                                        } else {
                                            // offsetExists returned true, now check if value is not null
                                            match self.call_array_access_offset_get(array_handle, key_handle) {
                                                Ok(val_handle) => {
                                                    !matches!(self.arena.get(val_handle).value, Val::Null)
                                                }
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
                                -1i64 as usize  // Out of bounds - use impossible value
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
                    if let Some((magic_func, _, _, magic_class)) =
                        self.find_method(class_name, isset_magic)
                    {
                        // Found __isset

                        // Create method name string (prop name)
                        let prop_name_str = self
                            .context
                            .interner
                            .lookup(prop_name)
                            .expect("Prop name should be interned")
                            .to_vec();
                        let name_handle = self.arena.alloc(Val::String(prop_name_str.into()));

                        // Prepare frame for __isset
                        let mut frame = CallFrame::new(magic_func.chunk.clone());
                        frame.func = Some(magic_func.clone());
                        frame.this = Some(obj_handle);
                        frame.class_scope = Some(magic_class);
                        frame.called_scope = Some(class_name);

                        // Param 0: name
                        if let Some(param) = magic_func.params.get(0) {
                            frame.locals.insert(param.name, name_handle);
                        }

                        self.push_frame(frame);
                    } else {
                        // No __isset, return false
                        let res_handle = self.arena.alloc(Val::Bool(false));
                        self.operand_stack.push(res_handle);
                    }
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
                let resolved_class = self.resolve_class_name(class_name)?;

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
                            return Err(VmError::RuntimeError(
                                "__callStatic must be static".into(),
                            ));
                        }

                        // Pop args
                        let args = self.collect_call_args(arg_count)?;

                        // Create array from args
                        let mut array_map = IndexMap::new();
                        for (i, arg) in args.into_iter().enumerate() {
                            array_map.insert(ArrayKey::Int(i as i64), arg);
                        }
                        let args_array_handle = self.arena.alloc(Val::Array(
                            crate::core::value::ArrayData::from(array_map).into(),
                        ));

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
                            "Call to undefined static method {}",
                            method_str
                        )));
                    }
                }
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

            OpCode::IsEqual => self.binary_cmp(|a, b| a == b)?,
            OpCode::IsNotEqual => self.binary_cmp(|a, b| a != b)?,
            OpCode::IsIdentical => self.binary_cmp(|a, b| a == b)?,
            OpCode::IsNotIdentical => self.binary_cmp(|a, b| a != b)?,
            OpCode::IsGreater => self.binary_cmp(|a, b| match (a, b) {
                (Val::Int(i1), Val::Int(i2)) => i1 > i2,
                _ => false,
            })?,
            OpCode::IsLess => self.binary_cmp(|a, b| match (a, b) {
                (Val::Int(i1), Val::Int(i2)) => i1 < i2,
                _ => false,
            })?,
            OpCode::IsGreaterOrEqual => self.binary_cmp(|a, b| match (a, b) {
                (Val::Int(i1), Val::Int(i2)) => i1 >= i2,
                _ => false,
            })?,
            OpCode::IsLessOrEqual => self.binary_cmp(|a, b| match (a, b) {
                (Val::Int(i1), Val::Int(i2)) => i1 <= i2,
                _ => false,
            })?,
            OpCode::Spaceship => {
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
                let res = match (a_val, b_val) {
                    (Val::Int(a), Val::Int(b)) => {
                        if a < b {
                            -1
                        } else if a > b {
                            1
                        } else {
                            0
                        }
                    }
                    _ => 0, // TODO
                };
                let res_handle = self.arena.alloc(Val::Int(res));
                self.operand_stack.push(res_handle);
            }
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

            OpCode::OpData
            | OpCode::GeneratorCreate
            | OpCode::DeclareLambdaFunction
            | OpCode::DeclareClassDelayed
            | OpCode::DeclareAnonClass
            | OpCode::UserOpcode
            | OpCode::UnsetCv
            | OpCode::IssetIsemptyCv
            | OpCode::Separate
            | OpCode::FetchClassName
            | OpCode::GeneratorReturn
            | OpCode::CopyTmp
            | OpCode::BindLexical
            | OpCode::IssetIsemptyThis
            | OpCode::JmpNull
            | OpCode::CheckUndefArgs
            | OpCode::BindInitStaticOrJmp
            | OpCode::InitParentPropertyHookCall
            | OpCode::DeclareAttributedConst => {
                // Zend-only or not yet modeled opcodes; act as harmless no-ops for now.
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

    // Arithmetic operations following PHP type juggling
    // Reference: $PHP_SRC_PATH/Zend/zend_operators.c

    fn arithmetic_add(&mut self) -> Result<(), VmError> {
        let b_handle = self.pop_operand()?;
        let a_handle = self.pop_operand()?;
        let a_val = &self.arena.get(a_handle).value;
        let b_val = &self.arena.get(b_handle).value;

        // Array + Array = union
        if let (Val::Array(a_arr), Val::Array(b_arr)) = (a_val, b_val) {
            let mut result = (**a_arr).clone();
            for (k, v) in b_arr.map.iter() {
                result.map.entry(k.clone()).or_insert(*v);
            }
            let res_handle = self.arena.alloc(Val::Array(Rc::new(result)));
            self.operand_stack.push(res_handle);
            return Ok(());
        }

        // Numeric addition
        let needs_float = matches!(a_val, Val::Float(_)) || matches!(b_val, Val::Float(_));
        let result = if needs_float {
            Val::Float(a_val.to_float() + b_val.to_float())
        } else {
            Val::Int(a_val.to_int() + b_val.to_int())
        };

        let res_handle = self.arena.alloc(result);
        self.operand_stack.push(res_handle);
        Ok(())
    }

    fn arithmetic_sub(&mut self) -> Result<(), VmError> {
        let b_handle = self.pop_operand()?;
        let a_handle = self.pop_operand()?;
        let a_val = &self.arena.get(a_handle).value;
        let b_val = &self.arena.get(b_handle).value;

        let needs_float = matches!(a_val, Val::Float(_)) || matches!(b_val, Val::Float(_));
        let result = if needs_float {
            Val::Float(a_val.to_float() - b_val.to_float())
        } else {
            Val::Int(a_val.to_int() - b_val.to_int())
        };

        let res_handle = self.arena.alloc(result);
        self.operand_stack.push(res_handle);
        Ok(())
    }

    fn arithmetic_mul(&mut self) -> Result<(), VmError> {
        let b_handle = self.pop_operand()?;
        let a_handle = self.pop_operand()?;
        let a_val = &self.arena.get(a_handle).value;
        let b_val = &self.arena.get(b_handle).value;

        let needs_float = matches!(a_val, Val::Float(_)) || matches!(b_val, Val::Float(_));
        let result = if needs_float {
            Val::Float(a_val.to_float() * b_val.to_float())
        } else {
            Val::Int(a_val.to_int() * b_val.to_int())
        };

        let res_handle = self.arena.alloc(result);
        self.operand_stack.push(res_handle);
        Ok(())
    }

    fn arithmetic_div(&mut self) -> Result<(), VmError> {
        let b_handle = self.pop_operand()?;
        let a_handle = self.pop_operand()?;
        let a_val = &self.arena.get(a_handle).value;
        let b_val = &self.arena.get(b_handle).value;

        let divisor = b_val.to_float();
        if divisor == 0.0 {
            self.error_handler
                .report(ErrorLevel::Warning, "Division by zero");
            let res_handle = self.arena.alloc(Val::Float(f64::INFINITY));
            self.operand_stack.push(res_handle);
            return Ok(());
        }

        // PHP always returns float for division
        let result = Val::Float(a_val.to_float() / divisor);
        let res_handle = self.arena.alloc(result);
        self.operand_stack.push(res_handle);
        Ok(())
    }

    fn arithmetic_mod(&mut self) -> Result<(), VmError> {
        let b_handle = self.pop_operand()?;
        let a_handle = self.pop_operand()?;
        let a_val = &self.arena.get(a_handle).value;
        let b_val = &self.arena.get(b_handle).value;

        let divisor = b_val.to_int();
        if divisor == 0 {
            self.error_handler
                .report(ErrorLevel::Warning, "Modulo by zero");
            let res_handle = self.arena.alloc(Val::Bool(false));
            self.operand_stack.push(res_handle);
            return Ok(());
        }

        let result = Val::Int(a_val.to_int() % divisor);
        let res_handle = self.arena.alloc(result);
        self.operand_stack.push(res_handle);
        Ok(())
    }

    fn arithmetic_pow(&mut self) -> Result<(), VmError> {
        let b_handle = self.pop_operand()?;
        let a_handle = self.pop_operand()?;
        let a_val = &self.arena.get(a_handle).value;
        let b_val = &self.arena.get(b_handle).value;

        let base = a_val.to_float();
        let exp = b_val.to_float();
        let result = Val::Float(base.powf(exp));

        let res_handle = self.arena.alloc(result);
        self.operand_stack.push(res_handle);
        Ok(())
    }

    fn bitwise_and(&mut self) -> Result<(), VmError> {
        let b_handle = self.pop_operand()?;
        let a_handle = self.pop_operand()?;
        let a_val = &self.arena.get(a_handle).value;
        let b_val = &self.arena.get(b_handle).value;

        let result = Val::Int(a_val.to_int() & b_val.to_int());
        let res_handle = self.arena.alloc(result);
        self.operand_stack.push(res_handle);
        Ok(())
    }

    fn bitwise_or(&mut self) -> Result<(), VmError> {
        let b_handle = self.pop_operand()?;
        let a_handle = self.pop_operand()?;
        let a_val = &self.arena.get(a_handle).value;
        let b_val = &self.arena.get(b_handle).value;

        let result = Val::Int(a_val.to_int() | b_val.to_int());
        let res_handle = self.arena.alloc(result);
        self.operand_stack.push(res_handle);
        Ok(())
    }

    fn bitwise_xor(&mut self) -> Result<(), VmError> {
        let b_handle = self.pop_operand()?;
        let a_handle = self.pop_operand()?;
        let a_val = &self.arena.get(a_handle).value;
        let b_val = &self.arena.get(b_handle).value;

        let result = Val::Int(a_val.to_int() ^ b_val.to_int());
        let res_handle = self.arena.alloc(result);
        self.operand_stack.push(res_handle);
        Ok(())
    }

    fn bitwise_shl(&mut self) -> Result<(), VmError> {
        let b_handle = self.pop_operand()?;
        let a_handle = self.pop_operand()?;
        let a_val = &self.arena.get(a_handle).value;
        let b_val = &self.arena.get(b_handle).value;

        let result = Val::Int(a_val.to_int() << b_val.to_int());
        let res_handle = self.arena.alloc(result);
        self.operand_stack.push(res_handle);
        Ok(())
    }

    fn bitwise_shr(&mut self) -> Result<(), VmError> {
        let b_handle = self.pop_operand()?;
        let a_handle = self.pop_operand()?;
        let a_val = &self.arena.get(a_handle).value;
        let b_val = &self.arena.get(b_handle).value;

        let result = Val::Int(a_val.to_int() >> b_val.to_int());
        let res_handle = self.arena.alloc(result);
        self.operand_stack.push(res_handle);
        Ok(())
    }

    fn binary_cmp<F>(&mut self, op: F) -> Result<(), VmError>
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

    fn assign_dim_value(
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

    fn assign_dim(
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

        let is_ref = self.arena.get(array_handle).is_ref;

        if is_ref {
            let array_zval_mut = self.arena.get_mut(array_handle);

            if let Val::Null | Val::Bool(false) = array_zval_mut.value {
                array_zval_mut.value = Val::Array(crate::core::value::ArrayData::new().into());
            }

            if let Val::Array(map) = &mut array_zval_mut.value {
                Rc::make_mut(map).map.insert(key, val_handle);
            } else {
                return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
            }
            self.operand_stack.push(array_handle);
        } else {
            let array_zval = self.arena.get(array_handle);
            let mut new_val = array_zval.value.clone();

            if let Val::Null | Val::Bool(false) = new_val {
                new_val = Val::Array(crate::core::value::ArrayData::new().into());
            }

            if let Val::Array(ref mut map) = new_val {
                Rc::make_mut(map).map.insert(key, val_handle);
            } else {
                return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
            }

            let new_handle = self.arena.alloc(new_val);
            self.operand_stack.push(new_handle);
        }
        Ok(())
    }

    /// Compute the next auto-increment array index
    /// Reference: $PHP_SRC_PATH/Zend/zend_hash.c - zend_hash_next_free_element
    ///
    /// OPTIMIZATION NOTE: This is O(n) on every append. PHP tracks this in the HashTable struct
    /// as `nNextFreeElement`. To match PHP performance, we would need to add metadata to Val::Array,
    /// tracking the next auto-index and updating it on insert/delete. For now, we scan all integer
    /// keys to find the max.
    ///
    /// TODO: Consider adding ArrayMeta { next_free: i64, .. } wrapper around IndexMap
    fn compute_next_array_index(map: &indexmap::IndexMap<ArrayKey, Handle>) -> i64 {
        map.keys()
            .filter_map(|k| match k {
                ArrayKey::Int(i) => Some(*i),
                // PHP also considers numeric string keys when computing next index
                ArrayKey::Str(s) => {
                    if let Ok(s_str) = std::str::from_utf8(s) {
                        s_str.parse::<i64>().ok()
                    } else {
                        None
                    }
                }
            })
            .max()
            .map(|i| i + 1)
            .unwrap_or(0)
    }

    fn append_array(&mut self, array_handle: Handle, val_handle: Handle) -> Result<(), VmError> {
        let is_ref = self.arena.get(array_handle).is_ref;

        if is_ref {
            let array_zval_mut = self.arena.get_mut(array_handle);

            if let Val::Null | Val::Bool(false) = array_zval_mut.value {
                array_zval_mut.value = Val::Array(crate::core::value::ArrayData::new().into());
            }

            if let Val::Array(map) = &mut array_zval_mut.value {
                let map_mut = &mut Rc::make_mut(map).map;
                let next_key = Self::compute_next_array_index(&map_mut);

                map_mut.insert(ArrayKey::Int(next_key), val_handle);
            } else {
                return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
            }
            self.operand_stack.push(array_handle);
        } else {
            let array_zval = self.arena.get(array_handle);
            let mut new_val = array_zval.value.clone();

            if let Val::Null | Val::Bool(false) = new_val {
                new_val = Val::Array(crate::core::value::ArrayData::new().into());
            }

            if let Val::Array(ref mut map) = new_val {
                let map_mut = &mut Rc::make_mut(map).map;
                let next_key = Self::compute_next_array_index(&map_mut);

                map_mut.insert(ArrayKey::Int(next_key), val_handle);
            } else {
                return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
            }

            let new_handle = self.arena.alloc(new_val);
            self.operand_stack.push(new_handle);
        }
        Ok(())
    }

    fn assign_nested_dim(
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

    fn fetch_nested_dim(
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
                            current_handle = self.call_array_access_offset_get(current_handle, *key_handle)?;
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
                    if keys.len() == 1 {
                        self.call_array_access_offset_set(current_handle, keys[0], val_handle)?;
                        return Ok(current_handle);
                    } else {
                        // Multiple keys: fetch the intermediate value and recurse
                        let first_key = keys[0];
                        let remaining_keys = &keys[1..];
                        
                        // Call offsetGet to get the intermediate value
                        let intermediate = self.call_array_access_offset_get(current_handle, first_key)?;
                        
                        // Recurse on the intermediate value
                        let new_intermediate = self.assign_nested_recursive(intermediate, remaining_keys, val_handle)?;
                        
                        // If the intermediate value changed, call offsetSet to update it
                        if new_intermediate != intermediate {
                            self.call_array_access_offset_set(current_handle, first_key, new_intermediate)?;
                        }
                        
                        return Ok(current_handle);
                    }
                }
            }
        }

        let key_handle = keys[0];
        let remaining_keys = &keys[1..];

        // Check if current handle is a reference - if so, mutate in place
        let is_ref = self.arena.get(current_handle).is_ref;

        if is_ref {
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
                self.arena.get_mut(current_handle).value =
                    Val::Array(crate::core::value::ArrayData::new().into());
            }

            // Now compute the actual key if it was AppendPlaceholder
            let key = if let Some(k) = key {
                k
            } else {
                // Compute next auto-index
                let current_zval = self.arena.get(current_handle);
                if let Val::Array(map) = &current_zval.value {
                    let next_key = Self::compute_next_array_index(&map.map);
                    ArrayKey::Int(next_key)
                } else {
                    return Err(VmError::RuntimeError("Cannot use scalar as array".into()));
                }
            };

            if remaining_keys.is_empty() {
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
                        Rc::make_mut(map).map.insert(key, val_handle);
                    }
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
                    let empty_handle = self
                        .arena
                        .alloc(Val::Array(crate::core::value::ArrayData::new().into()));
                    let current_zval_mut = self.arena.get_mut(current_handle);
                    if let Val::Array(ref mut map) = current_zval_mut.value {
                        Rc::make_mut(map).map.insert(key.clone(), empty_handle);
                    }
                    empty_handle
                };

                let new_next_handle =
                    self.assign_nested_recursive(next_handle, remaining_keys, val_handle)?;

                // Only update if changed (if next_handle is a ref, it's mutated in place)
                if new_next_handle != next_handle {
                    let current_zval = self.arena.get_mut(current_handle);
                    if let Val::Array(ref mut map) = current_zval.value {
                        Rc::make_mut(map).map.insert(key, new_next_handle);
                    }
                }
            }

            return Ok(current_handle);
        }

        // Not a reference - COW: Clone current array
        let current_zval = self.arena.get(current_handle);
        let mut new_val = current_zval.value.clone();

        if let Val::Null | Val::Bool(false) = new_val {
            new_val = Val::Array(crate::core::value::ArrayData::new().into());
        }

        if let Val::Array(ref mut map) = new_val {
            let map_mut = &mut Rc::make_mut(map).map;
            // Resolve key
            let key_val = &self.arena.get(key_handle).value;
            let key = if let Val::AppendPlaceholder = key_val {
                let next_key = Self::compute_next_array_index(&map_mut);
                ArrayKey::Int(next_key)
            } else {
                self.array_key_from_value(key_val)?
            };

            if remaining_keys.is_empty() {
                // We are at the last key.
                let mut updated_ref = false;
                if let Some(existing_handle) = map_mut.get(&key) {
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
                let next_handle = if let Some(h) = map_mut.get(&key) {
                    *h
                } else {
                    // Create empty array
                    self.arena
                        .alloc(Val::Array(crate::core::value::ArrayData::new().into()))
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
            Val::Object(payload_handle) => {
                Err(VmError::RuntimeError(format!(
                    "TypeError: Cannot access offset of type {} on array",
                    self.describe_object_class(*payload_handle)
                )))
            }
            _ => Err(VmError::RuntimeError(format!(
                "Illegal offset type {}",
                value.type_name()
            ))),
        }
    }

    /// Check if a value matches the expected return type
    /// Reference: $PHP_SRC_PATH/Zend/zend_execute.c - zend_verify_internal_return_type, zend_check_type
    fn check_return_type(&mut self, val_handle: Handle, ret_type: &ReturnType) -> Result<bool, VmError> {
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
                        || self.context.engine.functions.contains_key(&s.to_vec())
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
                    (arr_data.map.get(&key0), arr_data.map.get(&key1)) {
                    
                    // Method name must be a string
                    let method_val = &self.arena.get(method_handle).value;
                    if let Val::String(method_name) = method_val {
                        let method_sym = self.context.interner.intern(method_name);
                        
                        let class_or_obj_val = &self.arena.get(class_or_obj_handle).value;
                        match class_or_obj_val {
                            // [object, method]
                            Val::Object(payload_handle) => {
                                if let Val::ObjPayload(obj_data) = &self.arena.get(*payload_handle).value {
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

    fn get_type_name(&self, val_handle: Handle) -> String {
        let val = &self.arena.get(val_handle).value;
        match val {
            Val::Null => "null".to_string(),
            Val::Bool(_) => "bool".to_string(),
            Val::Int(_) => "int".to_string(),
            Val::Float(_) => "float".to_string(),
            Val::String(_) => "string".to_string(),
            Val::Array(_) => "array".to_string(),
            Val::Object(payload_handle) => {
                if let Val::ObjPayload(obj_data) = &self.arena.get(*payload_handle).value {
                    self.context.interner.lookup(obj_data.class)
                        .map(|bytes| String::from_utf8_lossy(bytes).to_string())
                        .unwrap_or_else(|| "object".to_string())
                } else {
                    "object".to_string()
                }
            }
            Val::Resource(_) => "resource".to_string(),
            Val::ObjPayload(_) => "object".to_string(),
            Val::AppendPlaceholder => "unknown".to_string(),
        }
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
            ReturnType::Named(sym) => {
                self.context.interner.lookup(*sym)
                    .map(|bytes| String::from_utf8_lossy(bytes).to_string())
                    .unwrap_or_else(|| "object".to_string())
            }
            ReturnType::Union(types) => {
                types.iter()
                    .map(|t| self.return_type_to_string(t))
                    .collect::<Vec<_>>()
                    .join("|")
            }
            ReturnType::Intersection(types) => {
                types.iter()
                    .map(|t| self.return_type_to_string(t))
                    .collect::<Vec<_>>()
                    .join("&")
            }
            ReturnType::Nullable(inner) => {
                format!("?{}", self.return_type_to_string(inner))
            }
            ReturnType::Static => "static".to_string(),
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
        let mut functions = std::collections::HashMap::new();
        functions.insert(
            b"strlen".to_vec(),
            php_strlen as crate::runtime::context::NativeHandler,
        );
        let engine = Arc::new(EngineContext::new());

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
                },
                FuncParam {
                    name: sym_b,
                    by_ref: false,
                    param_type: None,
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
        // Stack: [val, key, array]
        // StoreDim should assign val to array[key].

        let mut chunk = CodeChunk::default();
        chunk.constants.push(Val::Int(1)); // 0: val
        chunk.constants.push(Val::Int(0)); // 1: key
                                           // array will be created dynamically

        // Create array [0]
        chunk.code.push(OpCode::InitArray(0));
        chunk.code.push(OpCode::Const(1)); // key 0
        chunk.code.push(OpCode::Const(1)); // val 0 (dummy)
        chunk.code.push(OpCode::AssignDim); // Stack: [array]

        // Now stack has [array].
        // We want to test StoreDim with [val, key, array].
        // But we have [array].
        // We need to push val, key, then array.
        // But array is already there.

        // Let's manually construct stack in VM.
        let mut vm = create_vm();
        let array_handle = vm
            .arena
            .alloc(Val::Array(crate::core::value::ArrayData::new().into()));
        let key_handle = vm.arena.alloc(Val::Int(0));
        let val_handle = vm.arena.alloc(Val::Int(99));

        vm.operand_stack.push(val_handle);
        vm.operand_stack.push(key_handle);
        vm.operand_stack.push(array_handle);

        // Stack: [val, key, array] (Top is array)

        let mut chunk = CodeChunk::default();
        chunk.code.push(OpCode::StoreDim);

        vm.run(Rc::new(chunk)).unwrap();

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
            Err(VmError::RuntimeError(msg)) => assert_eq!(msg, "Stack underflow"),
            other => panic!("Expected stack underflow error, got {:?}", other),
        }
    }
}
