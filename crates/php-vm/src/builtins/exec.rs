use crate::core::value::{ArrayData, ArrayKey, Handle, Val};
use crate::vm::engine::VM;
use std::cell::RefCell;
use std::io::Read;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::rc::Rc;

// ============================================================================
// Resource Types
// ============================================================================

#[derive(Debug)]
pub struct ProcessResource {
    pub child: RefCell<Child>,
    pub command: String,
}

#[derive(Debug)]
pub enum PipeKind {
    Stdin(ChildStdin),
    Stdout(ChildStdout),
    Stderr(ChildStderr),
}

#[derive(Debug)]
pub struct PipeResource {
    pub pipe: RefCell<PipeKind>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Create a platform-appropriate shell command
fn create_shell_command(cmd_str: &str) -> Command {
    if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(&["/C", cmd_str]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(cmd_str);
        cmd
    }
}

/// Extract command string from VM handle
fn get_command_string(vm: &VM, handle: Handle) -> Result<String, String> {
    let val = vm.arena.get(handle);
    match &val.value {
        Val::String(s) => Ok(String::from_utf8_lossy(s).to_string()),
        _ => Err("Command must be a string".into()),
    }
}

/// Set exit code in output parameter if provided
fn set_exit_code(vm: &mut VM, args: &[Handle], arg_index: usize, status: &ExitStatus) {
    if args.len() > arg_index {
        let code = status.code().unwrap_or(-1) as i64;
        vm.arena.get_mut(args[arg_index]).value = Val::Int(code);
    }
}

// ============================================================================
// Shell Escaping Functions
// ============================================================================

/// escapeshellarg(arg) - Escape a string to be used as a shell argument
///
/// Note: Windows escaping is currently a no-op. Full Windows shell escaping
/// is complex and would require handling cmd.exe vs PowerShell differences.
pub fn php_escapeshellarg(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("escapeshellarg() expects exactly 1 parameter".into());
    }

    let val = vm.arena.get(args[0]);
    let arg = match &val.value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        Val::Int(i) => i.to_string(),
        Val::Float(f) => f.to_string(),
        _ => return Err("escapeshellarg() expects parameter 1 to be string".into()),
    };

    #[cfg(unix)]
    let escaped = {
        // POSIX shell: wrap in single quotes and escape embedded single quotes
        format!("'{}'", arg.replace('\'', "'\\''"))
    };

    #[cfg(not(unix))]
    let escaped = {
        // TODO: Implement proper Windows cmd.exe escaping
        // For now, return as-is (unsafe but matches current behavior)
        arg
    };

    Ok(vm.arena.alloc(Val::String(Rc::new(escaped.into_bytes()))))
}

/// escapeshellcmd(command) - Escape shell metacharacters
///
/// Note: This is a simplified implementation. PHP's version handles quote
/// pairing differently (only escapes unpaired quotes).
pub fn php_escapeshellcmd(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("escapeshellcmd() expects exactly 1 parameter".into());
    }

    let val = vm.arena.get(args[0]);
    let cmd = match &val.value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("escapeshellcmd() expects parameter 1 to be string".into()),
    };

    let mut escaped = String::with_capacity(cmd.len());
    for c in cmd.chars() {
        match c {
            '&' | '#' | ';' | '`' | '|' | '*' | '?' | '~' | '<' | '>' | '^' | '(' | ')' | '['
            | ']' | '{' | '}' | '$' | '\\' => {
                escaped.push('\\');
                escaped.push(c);
            }
            _ => escaped.push(c),
        }
    }

    Ok(vm.arena.alloc(Val::String(Rc::new(escaped.into_bytes()))))
}

// ============================================================================
// Command Execution Functions
// ============================================================================

/// exec(command, &output = null, &result_code = null) - Execute an external program
///
/// Reference: $PHP_SRC_PATH/ext/standard/exec.c - PHP_FUNCTION(exec)
pub fn php_exec(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("exec() expects at least 1 parameter".into());
    }

    let cmd_str = get_command_string(vm, args[0])?;
    let output = create_shell_command(&cmd_str)
        .output()
        .map_err(|e| format!("exec(): {}", e))?;

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout_str.lines().collect();

    // Populate output array (2nd parameter)
    if args.len() > 1 {
        let mut output_arr = ArrayData::new();
        for (i, line) in lines.iter().enumerate() {
            let line_handle = vm
                .arena
                .alloc(Val::String(Rc::new(line.as_bytes().to_vec())));
            output_arr.insert(ArrayKey::Int(i as i64), line_handle);
        }
        vm.arena.get_mut(args[1]).value = Val::Array(Rc::new(output_arr));
    }

    // Set exit code (3rd parameter)
    set_exit_code(vm, args, 2, &output.status);

    // Return last line of output
    let last_line = lines.last().unwrap_or(&"").as_bytes().to_vec();
    Ok(vm.arena.alloc(Val::String(Rc::new(last_line))))
}

/// passthru(command, &result_code = null) - Execute an external program and display raw output
///
/// Reference: $PHP_SRC_PATH/ext/standard/exec.c - PHP_FUNCTION(passthru)
pub fn php_passthru(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("passthru() expects at least 1 parameter".into());
    }

    let cmd_str = get_command_string(vm, args[0])?;

    // Note: passthru() should inherit stdout/stderr, but we use .status()
    // which doesn't capture output. For true passthru behavior, we'd need
    // to use .spawn() with inherited stdio.
    let status = create_shell_command(&cmd_str)
        .status()
        .map_err(|e| format!("passthru(): {}", e))?;

    set_exit_code(vm, args, 1, &status);
    Ok(vm.arena.alloc(Val::Null))
}

/// shell_exec(command) - Execute command via shell and return the complete output as a string
///
/// Reference: $PHP_SRC_PATH/ext/standard/exec.c - PHP_FUNCTION(shell_exec)
pub fn php_shell_exec(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("shell_exec() expects exactly 1 parameter".into());
    }

    let cmd_str = get_command_string(vm, args[0])?;

    match create_shell_command(&cmd_str).output() {
        Ok(output) => Ok(vm.arena.alloc(Val::String(Rc::new(output.stdout)))),
        Err(_) => Ok(vm.arena.alloc(Val::Null)),
    }
}

/// system(command, &result_code = null) - Execute an external program and display the output
///
/// Reference: $PHP_SRC_PATH/ext/standard/exec.c - PHP_FUNCTION(system)
pub fn php_system(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("system() expects at least 1 parameter".into());
    }

    let cmd_str = get_command_string(vm, args[0])?;

    let mut cmd = create_shell_command(&cmd_str);
    cmd.stdout(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("system(): {}", e))?;
    let mut stdout = child.stdout.take().unwrap();

    // Stream output to VM while capturing it
    let mut output_bytes = Vec::new();
    let mut buf = [0u8; 4096];

    loop {
        match stdout.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let chunk = &buf[0..n];
                output_bytes.extend_from_slice(chunk);
                vm.print_bytes(chunk)
                    .map_err(|e| format!("system(): {}", e))?;
            }
            Err(e) => return Err(format!("system(): {}", e)),
        }
    }

    let status = child.wait().map_err(|e| format!("system(): {}", e))?;
    set_exit_code(vm, args, 1, &status);

    // Return last line of output
    let output_str = String::from_utf8_lossy(&output_bytes);
    let last_line = output_str.lines().last().unwrap_or("").as_bytes().to_vec();

    Ok(vm.arena.alloc(Val::String(Rc::new(last_line))))
}

// ============================================================================
// Process Control Functions
// ============================================================================

/// Parse descriptor specification for proc_open
/// Returns (fd, should_pipe)
fn parse_descriptor_spec(vm: &VM, spec_handle: Handle) -> Option<(i64, bool)> {
    let spec_val = vm.arena.get(spec_handle);

    if let Val::Array(spec) = &spec_val.value {
        // Get descriptor type (first element)
        if let Some(type_handle) = spec.map.get(&ArrayKey::Int(0)) {
            let type_val = vm.arena.get(*type_handle);
            if let Val::String(s) = &type_val.value {
                if s.as_slice() == b"pipe" {
                    // For now, we only support pipe descriptors
                    return Some((0, true));
                }
            }
        }
    }

    None
}

/// proc_open(command, descriptors, &pipes, cwd = null, env = null, other_options = null)
///
/// Reference: $PHP_SRC_PATH/ext/standard/proc_open.c - PHP_FUNCTION(proc_open)
///
/// Supported descriptor formats:
/// - ["pipe", "r"] or ["pipe", "w"] - Create a pipe
///
/// Not yet supported:
/// - ["file", "/path/to/file", "mode"] - File descriptor
/// - ["pty"] - Pseudo-terminal
pub fn php_proc_open(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 {
        return Err("proc_open() expects at least 3 parameters".into());
    }

    let cmd_str = get_command_string(vm, args[0])?;

    // Parse descriptors array to determine which pipes to create
    let requested_pipes = {
        let descriptors_val = vm.arena.get(args[1]);
        let descriptors = match &descriptors_val.value {
            Val::Array(arr) => arr,
            _ => return Err("proc_open() expects parameter 2 to be an array".into()),
        };

        let mut pipes = Vec::new();
        for (key, val_handle) in descriptors.map.iter() {
            if let ArrayKey::Int(fd) = key {
                if let Some((_, should_pipe)) = parse_descriptor_spec(vm, *val_handle) {
                    if should_pipe && *fd >= 0 && *fd <= 2 {
                        pipes.push(*fd);
                    }
                }
            }
        }
        pipes
    };

    // Build command with appropriate stdio configuration
    let mut command = create_shell_command(&cmd_str);

    for &fd in &requested_pipes {
        match fd {
            0 => {
                command.stdin(Stdio::piped());
            }
            1 => {
                command.stdout(Stdio::piped());
            }
            2 => {
                command.stderr(Stdio::piped());
            }
            _ => {}
        }
    }

    let mut child = command.spawn().map_err(|e| format!("proc_open(): {}", e))?;

    // Create pipe resources and populate pipes array
    let mut pipes_arr = ArrayData::new();

    for fd in requested_pipes {
        let resource = match fd {
            0 => child.stdin.take().map(|stdin| PipeResource {
                pipe: RefCell::new(PipeKind::Stdin(stdin)),
            }),
            1 => child.stdout.take().map(|stdout| PipeResource {
                pipe: RefCell::new(PipeKind::Stdout(stdout)),
            }),
            2 => child.stderr.take().map(|stderr| PipeResource {
                pipe: RefCell::new(PipeKind::Stderr(stderr)),
            }),
            _ => None,
        };

        if let Some(res) = resource {
            let handle = vm.arena.alloc(Val::Resource(Rc::new(res)));
            pipes_arr.insert(ArrayKey::Int(fd), handle);
        }
    }

    // Update pipes argument (by reference)
    vm.arena.get_mut(args[2]).value = Val::Array(Rc::new(pipes_arr));

    // Create and return process resource
    let proc_res = ProcessResource {
        child: RefCell::new(child),
        command: cmd_str,
    };

    Ok(vm.arena.alloc(Val::Resource(Rc::new(proc_res))))
}

/// proc_close(process) - Close a process opened by proc_open
///
/// Reference: $PHP_SRC_PATH/ext/standard/proc_open.c - PHP_FUNCTION(proc_close)
pub fn php_proc_close(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("proc_close() expects exactly 1 parameter".into());
    }

    let resource_rc = {
        let val = vm.arena.get(args[0]);
        match &val.value {
            Val::Resource(rc) => rc.clone(),
            _ => {
                return Err(
                    "proc_close(): supplied argument is not a valid process resource".into(),
                )
            }
        }
    };

    if let Some(proc) = resource_rc.downcast_ref::<ProcessResource>() {
        let mut child = proc.child.borrow_mut();
        let status = child.wait().map_err(|e| format!("proc_close(): {}", e))?;
        Ok(vm.arena.alloc(Val::Int(status.code().unwrap_or(-1) as i64)))
    } else {
        Err("proc_close(): supplied argument is not a valid process resource".into())
    }
}

/// proc_get_status(process) - Get information about a process opened by proc_open
///
/// Reference: $PHP_SRC_PATH/ext/standard/proc_open.c - PHP_FUNCTION(proc_get_status)
///
/// Returns an array with:
/// - "command" (string): The command string that was passed to proc_open
/// - "pid" (int): Process ID (Unix only, -1 on Windows)
/// - "running" (bool): Whether the process is still running
/// - "exitcode" (int): Exit code if process has terminated, -1 otherwise
pub fn php_proc_get_status(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("proc_get_status() expects exactly 1 parameter".into());
    }

    let (command, pid, is_running, exit_code) = {
        let val = vm.arena.get(args[0]);
        let resource_rc = match &val.value {
            Val::Resource(rc) => rc.clone(),
            _ => {
                return Err(
                    "proc_get_status(): supplied argument is not a valid process resource".into(),
                )
            }
        };

        if let Some(proc) = resource_rc.downcast_ref::<ProcessResource>() {
            let mut child = proc.child.borrow_mut();

            // Get PID (Unix only)
            #[cfg(unix)]
            let pid = child.id() as i64;

            #[cfg(not(unix))]
            let pid = -1i64;

            // Check if process is still running
            match child
                .try_wait()
                .map_err(|e| format!("proc_get_status(): {}", e))?
            {
                Some(status) => (
                    proc.command.clone(),
                    pid,
                    false,
                    status.code().unwrap_or(-1) as i64,
                ),
                None => (proc.command.clone(), pid, true, -1),
            }
        } else {
            return Err(
                "proc_get_status(): supplied argument is not a valid process resource".into(),
            );
        }
    };

    // Build result array
    let mut arr = ArrayData::new();

    arr.insert(
        ArrayKey::Str(Rc::new(b"command".to_vec())),
        vm.arena.alloc(Val::String(Rc::new(command.into_bytes()))),
    );

    arr.insert(
        ArrayKey::Str(Rc::new(b"pid".to_vec())),
        vm.arena.alloc(Val::Int(pid)),
    );

    arr.insert(
        ArrayKey::Str(Rc::new(b"running".to_vec())),
        vm.arena.alloc(Val::Bool(is_running)),
    );

    arr.insert(
        ArrayKey::Str(Rc::new(b"exitcode".to_vec())),
        vm.arena.alloc(Val::Int(exit_code)),
    );

    Ok(vm.arena.alloc(Val::Array(Rc::new(arr))))
}

/// proc_nice(priority) - Change the priority of the current process
///
/// Reference: $PHP_SRC_PATH/ext/standard/proc_open.c - PHP_FUNCTION(proc_nice)
///
/// Note: Not implemented. Requires platform-specific code (setpriority on Unix,
/// SetPriorityClass on Windows).
pub fn php_proc_nice(_vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("proc_nice() is not supported in this build".into())
}

/// proc_terminate(process, signal = SIGTERM) - Kill a process opened by proc_open
///
/// Reference: $PHP_SRC_PATH/ext/standard/proc_open.c - PHP_FUNCTION(proc_terminate)
pub fn php_proc_terminate(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("proc_terminate() expects at least 1 parameter".into());
    }

    let resource_rc = {
        let val = vm.arena.get(args[0]);
        match &val.value {
            Val::Resource(rc) => rc.clone(),
            _ => {
                return Err(
                    "proc_terminate(): supplied argument is not a valid process resource".into(),
                )
            }
        }
    };

    if let Some(proc) = resource_rc.downcast_ref::<ProcessResource>() {
        let mut child = proc.child.borrow_mut();
        child
            .kill()
            .map_err(|e| format!("proc_terminate(): {}", e))?;
        Ok(vm.arena.alloc(Val::Bool(true)))
    } else {
        Err("proc_terminate(): supplied argument is not a valid process resource".into())
    }
}

// ============================================================================
// Execution Time Limit
// ============================================================================

/// set_time_limit(seconds) - Limits the maximum execution time
///
/// Sets the maximum time in seconds a script is allowed to run.
/// When the limit is reached, the script will be terminated with a fatal error.
/// 
/// A value of 0 means unlimited execution time.
/// Negative values are treated as valid and set the limit (PHP allows this).
///
/// Returns: bool - Always returns true on success
///
/// Note: The execution timer is reset when set_time_limit is called.
/// In native PHP, this also resets the timeout counter.
pub fn php_set_time_limit(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("set_time_limit() expects exactly 1 argument, 0 given".into());
    }

    let seconds = match &vm.arena.get(args[0]).value {
        Val::Int(i) => *i,
        Val::Float(f) => *f as i64,
        Val::Bool(b) => if *b { 1 } else { 0 },
        Val::String(s) => {
            let s_str = String::from_utf8_lossy(s);
            let trimmed = s_str.trim();
            // Try parsing as int first, then as float
            if let Ok(i) = trimmed.parse::<i64>() {
                i
            } else if let Ok(f) = trimmed.parse::<f64>() {
                f as i64
            } else {
                0
            }
        }
        _ => {
            return Err(format!(
                "set_time_limit(): Argument #1 ($seconds) must be of type int, {} given",
                match &vm.arena.get(args[0]).value {
                    Val::Array(_) => "array",
                    Val::Object(_) => "object",
                    Val::Null => "null",
                    _ => "unknown",
                }
            ));
        }
    };

    // Set the new execution time limit
    vm.context.max_execution_time = seconds;
    
    // Reset the execution start time (resets the timeout counter)
    vm.execution_start_time = std::time::SystemTime::now();

    // Always returns true in PHP
    Ok(vm.arena.alloc(Val::Bool(true)))
}
