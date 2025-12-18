use crate::core::value::{Handle, Val};
use crate::vm::engine::VM;
use std::rc::Rc;

/// Exception::__construct($message = "", $code = 0, Throwable $previous = null)
pub fn exception_construct(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // Get $this from current frame
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or_else(|| "No $this in exception construct".to_string())?;

    // Get message (arg 0, default "")
    let message = if let Some(&msg_handle) = args.get(0) {
        if let Val::String(s) = &vm.arena.get(msg_handle).value {
            s.clone()
        } else {
            Rc::new(Vec::new())
        }
    } else {
        Rc::new(Vec::new())
    };

    // Get code (arg 1, default 0)
    let code = if let Some(&code_handle) = args.get(1) {
        if let Val::Int(c) = &vm.arena.get(code_handle).value {
            *c
        } else {
            0
        }
    } else {
        0
    };

    // Get previous (arg 2, default null)
    let previous = if let Some(&prev_handle) = args.get(2) {
        prev_handle
    } else {
        vm.arena.alloc(Val::Null)
    };

    // Set properties on the exception object
    let message_sym = vm.context.interner.intern(b"message");
    let code_sym = vm.context.interner.intern(b"code");
    let previous_sym = vm.context.interner.intern(b"previous");

    // Allocate property values
    let message_handle = vm.arena.alloc(Val::String(message));
    let code_handle = vm.arena.alloc(Val::Int(code));

    // Update object properties
    if let Val::Object(payload_handle) = &vm.arena.get(this_handle).value {
        let payload = vm.arena.get_mut(*payload_handle);
        if let Val::ObjPayload(ref mut obj_data) = payload.value {
            obj_data.properties.insert(message_sym, message_handle);
            obj_data.properties.insert(code_sym, code_handle);
            obj_data.properties.insert(previous_sym, previous);
        }
    }

    Ok(vm.arena.alloc(Val::Null))
}

/// Exception::getMessage() - Returns the exception message
pub fn exception_get_message(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("getMessage() called outside object context")?;

    let message_sym = vm.context.interner.intern(b"message");

    if let Val::Object(payload_handle) = &vm.arena.get(this_handle).value {
        if let Val::ObjPayload(obj_data) = &vm.arena.get(*payload_handle).value {
            if let Some(&msg_handle) = obj_data.properties.get(&message_sym) {
                return Ok(msg_handle);
            }
        }
    }

    // Return empty string if no message
    Ok(vm.arena.alloc(Val::String(Rc::new(Vec::new()))))
}

/// Exception::getCode() - Returns the exception code
pub fn exception_get_code(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("getCode() called outside object context")?;

    let code_sym = vm.context.interner.intern(b"code");

    if let Val::Object(payload_handle) = &vm.arena.get(this_handle).value {
        if let Val::ObjPayload(obj_data) = &vm.arena.get(*payload_handle).value {
            if let Some(&code_handle) = obj_data.properties.get(&code_sym) {
                return Ok(code_handle);
            }
        }
    }

    // Return 0 if no code
    Ok(vm.arena.alloc(Val::Int(0)))
}

/// Exception::getFile() - Returns the filename where the exception was created
pub fn exception_get_file(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("getFile() called outside object context")?;

    let file_sym = vm.context.interner.intern(b"file");

    if let Val::Object(payload_handle) = &vm.arena.get(this_handle).value {
        if let Val::ObjPayload(obj_data) = &vm.arena.get(*payload_handle).value {
            if let Some(&file_handle) = obj_data.properties.get(&file_sym) {
                return Ok(file_handle);
            }
        }
    }

    // Return "unknown" if no file
    Ok(vm.arena.alloc(Val::String(Rc::new(b"unknown".to_vec()))))
}

/// Exception::getLine() - Returns the line where the exception was created
pub fn exception_get_line(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("getLine() called outside object context")?;

    let line_sym = vm.context.interner.intern(b"line");

    if let Val::Object(payload_handle) = &vm.arena.get(this_handle).value {
        if let Val::ObjPayload(obj_data) = &vm.arena.get(*payload_handle).value {
            if let Some(&line_handle) = obj_data.properties.get(&line_sym) {
                return Ok(line_handle);
            }
        }
    }

    // Return 0 if no line
    Ok(vm.arena.alloc(Val::Int(0)))
}

/// Exception::getTrace() - Returns the stack trace as array
pub fn exception_get_trace(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("getTrace() called outside object context")?;

    let trace_sym = vm.context.interner.intern(b"trace");

    if let Val::Object(payload_handle) = &vm.arena.get(this_handle).value {
        if let Val::ObjPayload(obj_data) = &vm.arena.get(*payload_handle).value {
            if let Some(&trace_handle) = obj_data.properties.get(&trace_sym) {
                return Ok(trace_handle);
            }
        }
    }

    // Return empty array if no trace
    Ok(vm
        .arena
        .alloc(Val::Array(crate::core::value::ArrayData::new().into())))
}

/// Exception::getTraceAsString() - Returns the stack trace as a string
pub fn exception_get_trace_as_string(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("getTraceAsString() called outside object context")?;

    let trace_sym = vm.context.interner.intern(b"trace");

    if let Val::Object(payload_handle) = &vm.arena.get(this_handle).value {
        if let Val::ObjPayload(obj_data) = &vm.arena.get(*payload_handle).value {
            if let Some(&trace_handle) = obj_data.properties.get(&trace_sym) {
                if let Val::Array(arr) = &vm.arena.get(trace_handle).value {
                    // Build trace string from array
                    let mut trace_str = String::new();
                    for (idx, (_key, val_handle)) in arr.map.iter().enumerate() {
                        if let Val::Array(frame_arr) = &vm.arena.get(*val_handle).value {
                            let file_key = Rc::new(b"file".to_vec());
                            let line_key = Rc::new(b"line".to_vec());
                            let function_key = Rc::new(b"function".to_vec());

                            let file = if let Some(fh) = frame_arr
                                .map
                                .get(&crate::core::value::ArrayKey::Str(file_key.clone()))
                            {
                                if let Val::String(s) = &vm.arena.get(*fh).value {
                                    String::from_utf8_lossy(s).to_string()
                                } else {
                                    "[unknown]".to_string()
                                }
                            } else {
                                "[unknown]".to_string()
                            };

                            let line = if let Some(lh) = frame_arr
                                .map
                                .get(&crate::core::value::ArrayKey::Str(line_key.clone()))
                            {
                                if let Val::Int(i) = &vm.arena.get(*lh).value {
                                    i.to_string()
                                } else {
                                    "0".to_string()
                                }
                            } else {
                                "0".to_string()
                            };

                            let function = if let Some(fh) = frame_arr
                                .map
                                .get(&crate::core::value::ArrayKey::Str(function_key.clone()))
                            {
                                if let Val::String(s) = &vm.arena.get(*fh).value {
                                    String::from_utf8_lossy(s).to_string()
                                } else {
                                    "[unknown]".to_string()
                                }
                            } else {
                                "[unknown]".to_string()
                            };

                            trace_str
                                .push_str(&format!("#{} {}({}): {}\n", idx, file, line, function));
                        }
                    }
                    return Ok(vm.arena.alloc(Val::String(Rc::new(trace_str.into_bytes()))));
                }
            }
        }
    }

    // Return empty string if no trace
    Ok(vm.arena.alloc(Val::String(Rc::new(Vec::new()))))
}

/// Exception::getPrevious() - Returns previous exception
pub fn exception_get_previous(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("getPrevious() called outside object context")?;

    let previous_sym = vm.context.interner.intern(b"previous");

    if let Val::Object(payload_handle) = &vm.arena.get(this_handle).value {
        if let Val::ObjPayload(obj_data) = &vm.arena.get(*payload_handle).value {
            if let Some(&prev_handle) = obj_data.properties.get(&previous_sym) {
                return Ok(prev_handle);
            }
        }
    }

    // Return null if no previous
    Ok(vm.arena.alloc(Val::Null))
}

/// Exception::__toString() - String representation of the exception
pub fn exception_to_string(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("__toString() called outside object context")?;

    // Get exception details
    let message_sym = vm.context.interner.intern(b"message");
    let code_sym = vm.context.interner.intern(b"code");
    let file_sym = vm.context.interner.intern(b"file");
    let line_sym = vm.context.interner.intern(b"line");

    let mut class_name = "Exception".to_string();
    let mut message = String::new();
    let mut code = 0i64;
    let mut file = "unknown".to_string();
    let mut line = 0i64;

    if let Val::Object(payload_handle) = &vm.arena.get(this_handle).value {
        if let Val::ObjPayload(obj_data) = &vm.arena.get(*payload_handle).value {
            class_name = String::from_utf8_lossy(
                vm.context
                    .interner
                    .lookup(obj_data.class)
                    .unwrap_or(b"Exception"),
            )
            .to_string();

            if let Some(&msg_handle) = obj_data.properties.get(&message_sym) {
                if let Val::String(s) = &vm.arena.get(msg_handle).value {
                    message = String::from_utf8_lossy(s).to_string();
                }
            }

            if let Some(&code_handle) = obj_data.properties.get(&code_sym) {
                if let Val::Int(c) = &vm.arena.get(code_handle).value {
                    code = *c;
                }
            }

            if let Some(&file_handle) = obj_data.properties.get(&file_sym) {
                if let Val::String(s) = &vm.arena.get(file_handle).value {
                    file = String::from_utf8_lossy(s).to_string();
                }
            }

            if let Some(&line_handle) = obj_data.properties.get(&line_sym) {
                if let Val::Int(l) = &vm.arena.get(line_handle).value {
                    line = *l;
                }
            }
        }
    }

    // Format: exception 'ClassName' with message 'message' in file:line
    let result = if message.is_empty() {
        format!("{} in {}:{}\nStack trace:\n", class_name, file, line)
    } else {
        format!(
            "exception '{}' with message '{}' in {}:{}\nStack trace:\n",
            class_name, message, file, line
        )
    };

    // Get trace string
    let trace_str = exception_get_trace_as_string(vm, _args)?;
    let trace_str_val = &vm.arena.get(trace_str).value;
    let trace_text = if let Val::String(s) = trace_str_val {
        String::from_utf8_lossy(s).to_string()
    } else {
        String::new()
    };

    let final_str = format!("{}{}", result, trace_text);
    Ok(vm.arena.alloc(Val::String(Rc::new(final_str.into_bytes()))))
}
