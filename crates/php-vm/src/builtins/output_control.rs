use crate::core::value::{ArrayData, ArrayKey, Handle, Val};
use crate::vm::engine::VM;
use indexmap::IndexMap;
use std::rc::Rc;

// Output buffer phase flags (passed to handler as second parameter)
pub const PHP_OUTPUT_HANDLER_START: i64 = 1;      // 0b0000_0001
pub const PHP_OUTPUT_HANDLER_WRITE: i64 = 0;      // 0b0000_0000 (also aliased as CONT)
pub const PHP_OUTPUT_HANDLER_FLUSH: i64 = 4;      // 0b0000_0100
pub const PHP_OUTPUT_HANDLER_CLEAN: i64 = 2;      // 0b0000_0010
pub const PHP_OUTPUT_HANDLER_FINAL: i64 = 8;      // 0b0000_1000 (also aliased as END)
pub const PHP_OUTPUT_HANDLER_CONT: i64 = 0;       // Alias for WRITE
pub const PHP_OUTPUT_HANDLER_END: i64 = 8;        // Alias for FINAL

// Output buffer control flags (passed to ob_start as third parameter)
pub const PHP_OUTPUT_HANDLER_CLEANABLE: i64 = 16;   // 0b0001_0000
pub const PHP_OUTPUT_HANDLER_FLUSHABLE: i64 = 32;   // 0b0010_0000
pub const PHP_OUTPUT_HANDLER_REMOVABLE: i64 = 64;   // 0b0100_0000
pub const PHP_OUTPUT_HANDLER_STDFLAGS: i64 = 112;   // CLEANABLE | FLUSHABLE | REMOVABLE

// Output handler status flags (returned by ob_get_status)
pub const PHP_OUTPUT_HANDLER_STARTED: i64 = 4096;     // 0b0001_0000_0000_0000
pub const PHP_OUTPUT_HANDLER_DISABLED: i64 = 8192;    // 0b0010_0000_0000_0000
pub const PHP_OUTPUT_HANDLER_PROCESSED: i64 = 16384;  // 0b0100_0000_0000_0000 (PHP 8.4+)

/// Output buffer structure representing a single level of output buffering
#[derive(Debug, Clone)]
pub struct OutputBuffer {
    /// The buffered content
    pub content: Vec<u8>,
    /// Optional handler callback
    pub handler: Option<Handle>,
    /// Chunk size (0 = unlimited)
    pub chunk_size: usize,
    /// Control flags (cleanable, flushable, removable)
    pub flags: i64,
    /// Status flags (started, disabled, processed)
    pub status: i64,
    /// Handler name for debugging
    pub name: Vec<u8>,
    /// Whether this buffer has been started (handler called with START)
    pub started: bool,
}

impl OutputBuffer {
    pub fn new(handler: Option<Handle>, chunk_size: usize, flags: i64) -> Self {
        let name = if handler.is_some() {
            b"default callback".to_vec()
        } else {
            b"default output handler".to_vec()
        };

        Self {
            content: Vec::new(),
            handler,
            chunk_size,
            flags,
            status: 0,
            name,
            started: false,
        }
    }

    pub fn is_cleanable(&self) -> bool {
        (self.flags & PHP_OUTPUT_HANDLER_CLEANABLE) != 0
    }

    pub fn is_flushable(&self) -> bool {
        (self.flags & PHP_OUTPUT_HANDLER_FLUSHABLE) != 0
    }

    pub fn is_removable(&self) -> bool {
        (self.flags & PHP_OUTPUT_HANDLER_REMOVABLE) != 0
    }

    pub fn is_disabled(&self) -> bool {
        (self.status & PHP_OUTPUT_HANDLER_DISABLED) != 0
    }
}

/// Turn on output buffering
/// ob_start(callable $callback = null, int $chunk_size = 0, int $flags = PHP_OUTPUT_HANDLER_STDFLAGS): bool
pub fn php_ob_start(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let handler = if !args.is_empty() {
        let val = &vm.arena.get(args[0]).value;
        match val {
            Val::Null => None,
            Val::String(_) | Val::Array(_) | Val::Object(_) => Some(args[0]),
            _ => return Err("ob_start(): Argument #1 must be a valid callback or null".into()),
        }
    } else {
        None
    };

    let chunk_size = if args.len() >= 2 {
        match &vm.arena.get(args[1]).value {
            Val::Int(i) => {
                if *i < 0 {
                    return Err("ob_start(): Argument #2 must be greater than or equal to 0".into());
                }
                *i as usize
            }
            _ => 0,
        }
    } else {
        0
    };

    let flags = if args.len() >= 3 {
        match &vm.arena.get(args[2]).value {
            Val::Int(i) => *i,
            _ => PHP_OUTPUT_HANDLER_STDFLAGS,
        }
    } else {
        PHP_OUTPUT_HANDLER_STDFLAGS
    };

    let buffer = OutputBuffer::new(handler, chunk_size, flags);
    vm.output_buffers.push(buffer);

    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// Clean (erase) the contents of the active output buffer
/// ob_clean(): bool
pub fn php_ob_clean(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    if vm.output_buffers.is_empty() {
        vm.trigger_error(
            crate::vm::engine::ErrorLevel::Notice,
            "ob_clean(): Failed to delete buffer. No buffer to delete",
        );
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let buffer = vm.output_buffers.last_mut().unwrap();
    if !buffer.is_cleanable() {
        vm.trigger_error(
            crate::vm::engine::ErrorLevel::Notice,
            "ob_clean(): Failed to delete buffer of default output handler",
        );
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    if buffer.is_disabled() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    buffer.content.clear();
    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// Flush (send) the return value of the active output handler
/// ob_flush(): bool
pub fn php_ob_flush(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    if vm.output_buffers.is_empty() {
        vm.trigger_error(
            crate::vm::engine::ErrorLevel::Notice,
            "ob_flush(): Failed to flush buffer. No buffer to flush",
        );
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let buffer_idx = vm.output_buffers.len() - 1;
    let buffer = &vm.output_buffers[buffer_idx];

    if !buffer.is_flushable() {
        vm.trigger_error(
            crate::vm::engine::ErrorLevel::Notice,
            "ob_flush(): Failed to flush buffer of default output handler",
        );
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    if buffer.is_disabled() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    // Process the buffer through handler if present
    let output = process_buffer(vm, buffer_idx, PHP_OUTPUT_HANDLER_FLUSH)?;

    // Send output to parent buffer or stdout
    if buffer_idx > 0 {
        vm.output_buffers[buffer_idx - 1].content.extend_from_slice(&output);
    } else {
        vm.write_output(&output).map_err(|e| format!("{:?}", e))?;
    }

    // Clear the buffer after flushing
    vm.output_buffers[buffer_idx].content.clear();

    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// Clean (erase) the contents of the active output buffer and turn it off
/// ob_end_clean(): bool
pub fn php_ob_end_clean(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    if vm.output_buffers.is_empty() {
        vm.trigger_error(
            crate::vm::engine::ErrorLevel::Notice,
            "ob_end_clean(): Failed to delete buffer. No buffer to delete",
        );
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let buffer_idx = vm.output_buffers.len() - 1;
    let buffer = &vm.output_buffers[buffer_idx];

    if !buffer.is_removable() {
        vm.trigger_error(
            crate::vm::engine::ErrorLevel::Notice,
            "ob_end_clean(): Failed to delete buffer of default output handler",
        );
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    if buffer.is_disabled() {
        vm.output_buffers.pop();
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    // Call handler with FINAL | CLEAN if handler exists
    if vm.output_buffers[buffer_idx].handler.is_some() {
        let _ = process_buffer(vm, buffer_idx, PHP_OUTPUT_HANDLER_FINAL | PHP_OUTPUT_HANDLER_CLEAN);
    }

    vm.output_buffers.pop();
    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// Flush (send) the return value of the active output handler and turn the active output buffer off
/// ob_end_flush(): bool
pub fn php_ob_end_flush(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    if vm.output_buffers.is_empty() {
        vm.trigger_error(
            crate::vm::engine::ErrorLevel::Notice,
            "ob_end_flush(): Failed to send buffer of default output handler",
        );
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let buffer_idx = vm.output_buffers.len() - 1;
    let buffer = &vm.output_buffers[buffer_idx];

    if !buffer.is_removable() {
        vm.trigger_error(
            crate::vm::engine::ErrorLevel::Notice,
            "ob_end_flush(): Failed to send buffer of default output handler",
        );
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    if buffer.is_disabled() {
        vm.output_buffers.pop();
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    // Process the buffer through handler if present
    let output = process_buffer(vm, buffer_idx, PHP_OUTPUT_HANDLER_FINAL)?;

    // Send output to parent buffer or stdout
    if buffer_idx > 0 {
        vm.output_buffers[buffer_idx - 1].content.extend_from_slice(&output);
    } else {
        vm.write_output(&output).map_err(|e| format!("{:?}", e))?;
    }

    vm.output_buffers.pop();
    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// Get the contents of the active output buffer and turn it off
/// ob_get_clean(): string|false
pub fn php_ob_get_clean(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    if vm.output_buffers.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let buffer = vm.output_buffers.pop().unwrap();
    let content = buffer.content.clone();

    Ok(vm.arena.alloc(Val::String(Rc::new(content))))
}

/// Return the contents of the output buffer
/// ob_get_contents(): string|false
pub fn php_ob_get_contents(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    if let Some(buffer) = vm.output_buffers.last() {
        Ok(vm.arena.alloc(Val::String(Rc::new(buffer.content.clone()))))
    } else {
        Ok(vm.arena.alloc(Val::Bool(false)))
    }
}

/// Flush (send) the return value of the active output handler,
/// return the contents of the active output buffer and turn it off
/// ob_get_flush(): string|false
pub fn php_ob_get_flush(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    if vm.output_buffers.is_empty() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let buffer_idx = vm.output_buffers.len() - 1;

    // Process the buffer through handler if present
    let output = process_buffer(vm, buffer_idx, PHP_OUTPUT_HANDLER_FINAL)?;

    // Send output to parent buffer or stdout
    if buffer_idx > 0 {
        vm.output_buffers[buffer_idx - 1].content.extend_from_slice(&output);
    } else {
        vm.write_output(&output).map_err(|e| format!("{:?}", e))?;
    }

    let buffer = vm.output_buffers.pop().unwrap();
    Ok(vm.arena.alloc(Val::String(Rc::new(buffer.content))))
}

/// Return the length of the output buffer
/// ob_get_length(): int|false
pub fn php_ob_get_length(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    if let Some(buffer) = vm.output_buffers.last() {
        Ok(vm.arena.alloc(Val::Int(buffer.content.len() as i64)))
    } else {
        Ok(vm.arena.alloc(Val::Bool(false)))
    }
}

/// Return the nesting level of the output buffering mechanism
/// ob_get_level(): int
pub fn php_ob_get_level(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Int(vm.output_buffers.len() as i64)))
}

/// Get status of output buffers
/// ob_get_status(bool $full_status = false): array
pub fn php_ob_get_status(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let full_status = if !args.is_empty() {
        vm.arena.get(args[0]).value.to_bool()
    } else {
        false
    };

    if full_status {
        // Return array of all buffer statuses
        // Clone the buffer data to avoid borrow issues
        let buffers_data: Vec<_> = vm.output_buffers.iter()
            .enumerate()
            .map(|(level, buf)| (level, buf.name.clone(), buf.handler, buf.flags, buf.chunk_size, buf.content.len(), buf.status))
            .collect();
        
        let mut result = Vec::new();
        for (level, name, handler, flags, chunk_size, content_len, status) in buffers_data {
            let status_array = create_buffer_status_data(vm, &name, handler, flags, level, chunk_size, content_len, status)?;
            result.push(status_array);
        }
        let mut arr = ArrayData::new();
        for handle in result.into_iter() {
            arr.push(handle);
        }
        Ok(vm.arena.alloc(Val::Array(Rc::new(arr))))
    } else {
        // Return status of top-most buffer
        if let Some(buffer) = vm.output_buffers.last() {
            let level = vm.output_buffers.len() - 1;
            let name = buffer.name.clone();
            let handler = buffer.handler;
            let flags = buffer.flags;
            let chunk_size = buffer.chunk_size;
            let content_len = buffer.content.len();
            let status = buffer.status;
            create_buffer_status_data(vm, &name, handler, flags, level, chunk_size, content_len, status)
        } else {
            // Return empty array if no buffers
            Ok(vm.arena.alloc(Val::Array(Rc::new(ArrayData::new()))))
        }
    }
}

/// Turn implicit flush on/off
/// ob_implicit_flush(int $enable = 1): void
pub fn php_ob_implicit_flush(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let enable = if !args.is_empty() {
        match &vm.arena.get(args[0]).value {
            Val::Int(i) => *i != 0,
            _ => true,
        }
    } else {
        true
    };

    vm.implicit_flush = enable;
    Ok(vm.arena.alloc(Val::Null))
}

/// List all output handlers in use
/// ob_list_handlers(): array
pub fn php_ob_list_handlers(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let mut handlers = Vec::new();

    for buffer in &vm.output_buffers {
        let name = vm.arena.alloc(Val::String(Rc::new(buffer.name.clone())));
        handlers.push(name);
    }

    let mut arr = ArrayData::new();
    for handle in handlers {
        arr.push(handle);
    }
    Ok(vm.arena.alloc(Val::Array(Rc::new(arr))))
}

/// Flush system output buffer
/// flush(): void
pub fn php_flush(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    // If there are output buffers, flush the top-most one
    if !vm.output_buffers.is_empty() {
        php_ob_flush(vm, &[])?;
    }
    
    // Flush the underlying output writer
    vm.flush_output().map_err(|e| format!("{:?}", e))?;
    
    Ok(vm.arena.alloc(Val::Null))
}

/// Add URL rewriter values
/// output_add_rewrite_var(string $name, string $value): bool
pub fn php_output_add_rewrite_var(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("output_add_rewrite_var() expects exactly 2 parameters".into());
    }

    let name = match &vm.arena.get(args[0]).value {
        Val::String(s) => s.clone(),
        _ => return Err("output_add_rewrite_var(): Argument #1 must be a string".into()),
    };

    let value = match &vm.arena.get(args[1]).value {
        Val::String(s) => s.clone(),
        _ => return Err("output_add_rewrite_var(): Argument #2 must be a string".into()),
    };

    vm.url_rewrite_vars.insert(name, value);
    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// Reset URL rewriter values
/// output_reset_rewrite_vars(): bool
pub fn php_output_reset_rewrite_vars(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    vm.url_rewrite_vars.clear();
    Ok(vm.arena.alloc(Val::Bool(true)))
}

// Helper function to process buffer through handler
fn process_buffer(vm: &mut VM, buffer_idx: usize, phase: i64) -> Result<Vec<u8>, String> {
    let buffer = &mut vm.output_buffers[buffer_idx];
    
    // Mark as started and processed
    if !buffer.started {
        buffer.started = true;
        buffer.status |= PHP_OUTPUT_HANDLER_STARTED;
    }
    buffer.status |= PHP_OUTPUT_HANDLER_PROCESSED;

    let handler = buffer.handler;
    let content = buffer.content.clone();
    
    if let Some(handler_handle) = handler {
        // Prepare arguments for handler: (string $buffer, int $phase)
        let buffer_arg = vm.arena.alloc(Val::String(Rc::new(content.clone())));
        let phase_arg = vm.arena.alloc(Val::Int(phase));
        
        // Call the handler
        match vm.call_user_function(handler_handle, &[buffer_arg, phase_arg]) {
            Ok(result_handle) => {
                match &vm.arena.get(result_handle).value {
                    Val::String(s) => Ok(s.as_ref().clone()),
                    Val::Bool(false) => {
                        // Handler returned false, mark as disabled
                        vm.output_buffers[buffer_idx].status |= PHP_OUTPUT_HANDLER_DISABLED;
                        Ok(content)
                    }
                    _ => {
                        // Convert to string
                        let s = vm.value_to_string(result_handle)?;
                        Ok(s)
                    }
                }
            }
            Err(_) => {
                // Handler failed, mark as disabled and return original content
                vm.output_buffers[buffer_idx].status |= PHP_OUTPUT_HANDLER_DISABLED;
                Ok(content)
            }
        }
    } else {
        // No handler, return content as-is
        Ok(content)
    }
}

// Helper function to create buffer status array from data
fn create_buffer_status_data(
    vm: &mut VM,
    name: &[u8],
    handler: Option<Handle>,
    flags: i64,
    level: usize,
    chunk_size: usize,
    content_len: usize,
    status: i64,
) -> Result<Handle, String> {
    let mut status_map = IndexMap::new();

    // 'name' => handler name
    let name_val = vm.arena.alloc(Val::String(Rc::new(name.to_vec())));
    status_map.insert(ArrayKey::Str(Rc::new(b"name".to_vec())), name_val);

    // 'type' => 0 for user handler, 1 for internal handler
    let type_val = vm.arena.alloc(Val::Int(if handler.is_some() { 1 } else { 0 }));
    status_map.insert(ArrayKey::Str(Rc::new(b"type".to_vec())), type_val);

    // 'flags' => control flags
    let flags_val = vm.arena.alloc(Val::Int(flags));
    status_map.insert(ArrayKey::Str(Rc::new(b"flags".to_vec())), flags_val);

    // 'level' => nesting level
    let level_val = vm.arena.alloc(Val::Int(level as i64));
    status_map.insert(ArrayKey::Str(Rc::new(b"level".to_vec())), level_val);

    // 'chunk_size' => chunk size
    let chunk_val = vm.arena.alloc(Val::Int(chunk_size as i64));
    status_map.insert(ArrayKey::Str(Rc::new(b"chunk_size".to_vec())), chunk_val);

    // 'buffer_size' => current buffer size
    let size_val = vm.arena.alloc(Val::Int(content_len as i64));
    status_map.insert(ArrayKey::Str(Rc::new(b"buffer_size".to_vec())), size_val);

    // 'buffer_used' => same as buffer_size (deprecated but kept for compatibility)
    let used_val = vm.arena.alloc(Val::Int(content_len as i64));
    status_map.insert(ArrayKey::Str(Rc::new(b"buffer_used".to_vec())), used_val);

    // 'status' => status flags (PHP 8.4+)
    let status_val = vm.arena.alloc(Val::Int(status));
    status_map.insert(ArrayKey::Str(Rc::new(b"status".to_vec())), status_val);

    Ok(vm.arena.alloc(Val::Array(Rc::new(ArrayData::from(status_map)))))
}
