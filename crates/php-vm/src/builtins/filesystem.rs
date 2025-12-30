use crate::builtins::exec::{PipeKind, PipeResource};
use crate::core::value::{ArrayData, ArrayKey, Handle, Val};
use crate::vm::engine::VM;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::rc::Rc;

/// File handle resource for fopen/fread/fwrite/fclose
/// Uses RefCell for interior mutability to allow read/write operations
#[derive(Debug)]
#[allow(dead_code)]
pub struct FileHandle {
    pub file: RefCell<File>,
    pub path: PathBuf,
    pub mode: String,
    pub eof: RefCell<bool>,
}

/// Convert VM handle to string bytes for path operations
fn handle_to_path(vm: &VM, handle: Handle) -> Result<Vec<u8>, String> {
    let val = vm.arena.get(handle);
    match &val.value {
        Val::String(s) => Ok(s.to_vec()),
        Val::Int(i) => Ok(i.to_string().into_bytes()),
        Val::Float(f) => Ok(f.to_string().into_bytes()),
        _ => Err("Expected string path".into()),
    }
}

/// Convert bytes to PathBuf, handling encoding
fn bytes_to_path(bytes: &[u8]) -> Result<PathBuf, String> {
    #[cfg(unix)]
    {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        Ok(PathBuf::from(OsStr::from_bytes(bytes)))
    }

    #[cfg(not(unix))]
    {
        String::from_utf8(bytes.to_vec())
            .map(PathBuf::from)
            .map_err(|_| "Invalid UTF-8 in path".to_string())
    }
}

/// Parse file mode string (e.g., "r", "w", "a", "r+", "rb", "w+b")
/// Reference: $PHP_SRC_PATH/main/streams/plain_wrapper.c - php_stream_fopen_from_file_rel
fn parse_mode(mode: &[u8]) -> Result<(bool, bool, bool, bool), String> {
    let mode_str = std::str::from_utf8(mode).map_err(|_| "Invalid mode string".to_string())?;

    let mut read = false;
    let mut write = false;
    let mut append = false;
    let mut create = false;
    let mut truncate = false;

    let chars: Vec<char> = mode_str.chars().collect();
    if chars.is_empty() {
        return Err("Empty mode string".into());
    }

    match chars[0] {
        'r' => {
            read = true;
            // Check for + (read/write)
            if chars.len() > 1 && chars[1] == '+' {
                write = true;
            }
        }
        'w' => {
            write = true;
            create = true;
            truncate = true;
            if chars.len() > 1 && chars[1] == '+' {
                read = true;
            }
        }
        'a' => {
            write = true;
            create = true;
            append = true;
            if chars.len() > 1 && chars[1] == '+' {
                read = true;
            }
        }
        'x' => {
            write = true;
            create = true;
            // Exclusive - fail if exists (handled separately)
            if chars.len() > 1 && chars[1] == '+' {
                read = true;
            }
        }
        'c' => {
            write = true;
            create = true;
            if chars.len() > 1 && chars[1] == '+' {
                read = true;
            }
        }
        _ => return Err(format!("Invalid mode: {}", mode_str)),
    }

    Ok((read, write, append || create && !truncate, truncate))
}

/// fopen(filename, mode) - Open file and return resource
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(fopen)
pub fn php_fopen(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("fopen() expects at least 2 parameters".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let mode_val = vm.arena.get(args[1]);
    let mode_bytes = match &mode_val.value {
        Val::String(s) => s.to_vec(),
        _ => return Err("fopen(): Mode must be string".into()),
    };

    let path = bytes_to_path(&path_bytes)?;
    let mode_str =
        std::str::from_utf8(&mode_bytes).map_err(|_| "Invalid mode encoding".to_string())?;

    // Parse mode
    let (read, write, append, truncate) = parse_mode(&mode_bytes)?;
    let exclusive = mode_str.starts_with('x');

    // Build OpenOptions
    let mut options = OpenOptions::new();
    options.read(read);
    options.write(write);
    options.append(append);
    options.truncate(truncate);

    if mode_str.starts_with('w') || mode_str.starts_with('a') || mode_str.starts_with('c') {
        options.create(true);
    }

    if exclusive {
        options.create_new(true);
    }

    // Open file
    let file = options.open(&path).map_err(|e| {
        format!(
            "fopen({}): failed to open stream: {}",
            String::from_utf8_lossy(&path_bytes),
            e
        )
    })?;

    let resource = FileHandle {
        file: RefCell::new(file),
        path: path.clone(),
        mode: mode_str.to_string(),
        eof: RefCell::new(false),
    };

    Ok(vm.arena.alloc(Val::Resource(Rc::new(resource))))
}

/// fclose(resource) - Close file handle
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(fclose)
pub fn php_fclose(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("fclose() expects exactly 1 parameter".into());
    }

    let is_resource = {
        let val = vm.arena.get(args[0]);
        match &val.value {
            Val::Resource(rc) => rc.is::<FileHandle>() || rc.is::<PipeResource>(),
            _ => false,
        }
    };

    if is_resource {
        // Resource will be dropped when last reference goes away
        Ok(vm.arena.alloc(Val::Bool(true)))
    } else {
        Err("fclose(): supplied argument is not a valid stream resource".into())
    }
}

/// fread(resource, length) - Read from file
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(fread)
pub fn php_fread(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("fread() expects exactly 2 parameters".into());
    }

    let length = {
        let val = vm.arena.get(args[1]);
        match &val.value {
            Val::Int(i) => {
                if *i < 0 {
                    return Err("fread(): Length must be greater than or equal to zero".into());
                }
                *i as usize
            }
            _ => return Err("fread(): Length must be integer".into()),
        }
    };

    let resource_rc = {
        let val = vm.arena.get(args[0]);
        if let Val::Resource(rc) = &val.value {
            rc.clone()
        } else {
            return Err("fread(): supplied argument is not a valid stream resource".into());
        }
    };

    if let Some(fh) = resource_rc.downcast_ref::<FileHandle>() {
        let mut buffer = vec![0u8; length];
        let bytes_read = fh
            .file
            .borrow_mut()
            .read(&mut buffer)
            .map_err(|e| format!("fread(): {}", e))?;

        if bytes_read == 0 {
            *fh.eof.borrow_mut() = true;
        }

        buffer.truncate(bytes_read);
        return Ok(vm.arena.alloc(Val::String(Rc::new(buffer))));
    }

    if let Some(pr) = resource_rc.downcast_ref::<PipeResource>() {
        let mut pipe = pr.pipe.borrow_mut();
        let result = match &mut *pipe {
            PipeKind::Stdout(stdout) => {
                let mut buffer = vec![0u8; length];
                let bytes_read = stdout
                    .read(&mut buffer)
                    .map_err(|e| format!("fread(): {}", e))?;
                buffer.truncate(bytes_read);
                Ok(buffer)
            }
            PipeKind::Stderr(stderr) => {
                let mut buffer = vec![0u8; length];
                let bytes_read = stderr
                    .read(&mut buffer)
                    .map_err(|e| format!("fread(): {}", e))?;
                buffer.truncate(bytes_read);
                Ok(buffer)
            }
            _ => Err("fread(): cannot read from this pipe".into()),
        };

        match result {
            Ok(buffer) => return Ok(vm.arena.alloc(Val::String(Rc::new(buffer)))),
            Err(e) => return Err(e),
        }
    }

    Err("fread(): supplied argument is not a valid stream resource".into())
}

/// fwrite(resource, data) - Write to file
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(fwrite)
pub fn php_fwrite(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("fwrite() expects at least 2 parameters".into());
    }

    // Capture arguments first
    let data = {
        let val = vm.arena.get(args[1]);
        match &val.value {
            Val::String(s) => s.to_vec(),
            Val::Int(i) => i.to_string().into_bytes(),
            Val::Float(f) => f.to_string().into_bytes(),
            _ => return Err("fwrite(): Data must be string or scalar".into()),
        }
    };

    let max_len = if args.len() > 2 {
        let val = vm.arena.get(args[2]);
        match &val.value {
            Val::Int(i) if *i >= 0 => Some(*i as usize),
            _ => return Err("fwrite(): Length must be non-negative integer".into()),
        }
    } else {
        None
    };

    let resource_rc = {
        let val = vm.arena.get(args[0]);
        if let Val::Resource(rc) = &val.value {
            rc.clone()
        } else {
            return Err("fwrite(): supplied argument is not a valid stream resource".into());
        }
    };

    if let Some(fh) = resource_rc.downcast_ref::<FileHandle>() {
        let write_data = if let Some(max) = max_len {
            &data[..data.len().min(max)]
        } else {
            &data
        };

        let bytes_written = fh
            .file
            .borrow_mut()
            .write(write_data)
            .map_err(|e| format!("fwrite(): {}", e))?;

        return Ok(vm.arena.alloc(Val::Int(bytes_written as i64)));
    }

    if let Some(pr) = resource_rc.downcast_ref::<PipeResource>() {
        let mut pipe = pr.pipe.borrow_mut();
        if let PipeKind::Stdin(stdin) = &mut *pipe {
            let write_data = if let Some(max) = max_len {
                &data[..data.len().min(max)]
            } else {
                &data
            };
            let bytes_written = stdin
                .write(write_data)
                .map_err(|e| format!("fwrite(): {}", e))?;
            return Ok(vm.arena.alloc(Val::Int(bytes_written as i64)));
        } else {
            return Err("fwrite(): cannot write to this pipe".into());
        }
    }

    Err("fwrite(): supplied argument is not a valid stream resource".into())
}

/// file_get_contents(filename) - Read entire file into string
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(file_get_contents)
pub fn php_file_get_contents(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("file_get_contents() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let contents = fs::read(&path).map_err(|e| {
        format!(
            "file_get_contents({}): failed to open stream: {}",
            String::from_utf8_lossy(&path_bytes),
            e
        )
    })?;

    Ok(vm.arena.alloc(Val::String(Rc::new(contents))))
}

/// file_put_contents(filename, data) - Write data to file
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(file_put_contents)
pub fn php_file_put_contents(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("file_put_contents() expects at least 2 parameters".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let data_val = vm.arena.get(args[1]);
    let data = match &data_val.value {
        Val::String(s) => s.to_vec(),
        Val::Int(i) => i.to_string().into_bytes(),
        Val::Float(f) => f.to_string().into_bytes(),
        Val::Array(arr) => {
            // PHP concatenates array elements
            let mut result = Vec::new();
            for (_, elem_handle) in arr.map.iter() {
                let elem = vm.arena.get(*elem_handle);
                match &elem.value {
                    Val::String(s) => result.extend_from_slice(s),
                    Val::Int(i) => result.extend_from_slice(i.to_string().as_bytes()),
                    Val::Float(f) => result.extend_from_slice(f.to_string().as_bytes()),
                    _ => {}
                }
            }
            result
        }
        _ => return Err("file_put_contents(): Data must be string, array, or scalar".into()),
    };

    // Check for FILE_APPEND flag (3rd argument)
    let append = if args.len() > 2 {
        let flags_val = vm.arena.get(args[2]);
        if let Val::Int(flags) = flags_val.value {
            (flags & 8) != 0 // FILE_APPEND = 8
        } else {
            false
        }
    } else {
        false
    };

    let written = if append {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| {
                format!(
                    "file_put_contents({}): {}",
                    String::from_utf8_lossy(&path_bytes),
                    e
                )
            })?;
        file.write(&data).map_err(|e| {
            format!(
                "file_put_contents({}): write failed: {}",
                String::from_utf8_lossy(&path_bytes),
                e
            )
        })?
    } else {
        fs::write(&path, &data).map_err(|e| {
            format!(
                "file_put_contents({}): {}",
                String::from_utf8_lossy(&path_bytes),
                e
            )
        })?;
        data.len()
    };

    Ok(vm.arena.alloc(Val::Int(written as i64)))
}

/// file_exists(filename) - Check if file or directory exists
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(file_exists)
pub fn php_file_exists(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("file_exists() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let exists = path.exists();
    Ok(vm.arena.alloc(Val::Bool(exists)))
}

/// is_file(filename) - Check if path is a regular file
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(is_file)
pub fn php_is_file(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("is_file() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let is_file = path.is_file();
    Ok(vm.arena.alloc(Val::Bool(is_file)))
}

/// is_dir(filename) - Check if path is a directory
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(is_dir)
pub fn php_is_dir(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("is_dir() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let is_dir = path.is_dir();
    Ok(vm.arena.alloc(Val::Bool(is_dir)))
}

/// filesize(filename) - Get file size in bytes
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(filesize)
pub fn php_filesize(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("filesize() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let metadata = fs::metadata(&path).map_err(|e| {
        format!(
            "filesize(): stat failed for {}: {}",
            String::from_utf8_lossy(&path_bytes),
            e
        )
    })?;

    Ok(vm.arena.alloc(Val::Int(metadata.len() as i64)))
}

/// is_readable(filename) - Check if file is readable
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(is_readable)
pub fn php_is_readable(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("is_readable() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    // Try to open for reading
    let readable = File::open(&path).is_ok();
    Ok(vm.arena.alloc(Val::Bool(readable)))
}

/// is_writable(filename) - Check if file is writable
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(is_writable)
pub fn php_is_writable(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("is_writable() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    // Check if we can open for writing
    let writable = if path.exists() {
        OpenOptions::new().write(true).open(&path).is_ok()
    } else {
        // Check parent directory
        if let Some(parent) = path.parent() {
            parent.exists() && parent.is_dir()
        } else {
            false
        }
    };

    Ok(vm.arena.alloc(Val::Bool(writable)))
}

/// unlink(filename) - Delete a file
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(unlink)
pub fn php_unlink(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("unlink() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    fs::remove_file(&path)
        .map_err(|e| format!("unlink({}): {}", String::from_utf8_lossy(&path_bytes), e))?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// rename(oldname, newname) - Rename a file or directory
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(rename)
pub fn php_rename(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("rename() expects at least 2 parameters".into());
    }

    let old_bytes = handle_to_path(vm, args[0])?;
    let new_bytes = handle_to_path(vm, args[1])?;

    let old_path = bytes_to_path(&old_bytes)?;
    let new_path = bytes_to_path(&new_bytes)?;

    fs::rename(&old_path, &new_path).map_err(|e| {
        format!(
            "rename({}, {}): {}",
            String::from_utf8_lossy(&old_bytes),
            String::from_utf8_lossy(&new_bytes),
            e
        )
    })?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// mkdir(pathname, mode = 0777, recursive = false) - Create directory
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(mkdir)
pub fn php_mkdir(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("mkdir() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    // Check for recursive flag (3rd argument)
    let recursive = if args.len() > 2 {
        let flag_val = vm.arena.get(args[2]);
        flag_val.value.to_bool()
    } else {
        false
    };

    let result = if recursive {
        fs::create_dir_all(&path)
    } else {
        fs::create_dir(&path)
    };

    result.map_err(|e| format!("mkdir({}): {}", String::from_utf8_lossy(&path_bytes), e))?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// rmdir(dirname) - Remove directory
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(rmdir)
pub fn php_rmdir(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("rmdir() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    fs::remove_dir(&path)
        .map_err(|e| format!("rmdir({}): {}", String::from_utf8_lossy(&path_bytes), e))?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// scandir(directory) - List files in directory
/// Reference: $PHP_SRC_PATH/ext/standard/dir.c - PHP_FUNCTION(scandir)
pub fn php_scandir(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("scandir() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let entries = fs::read_dir(&path)
        .map_err(|e| format!("scandir({}): {}", String::from_utf8_lossy(&path_bytes), e))?;

    let mut files = Vec::new();
    for entry_result in entries {
        let entry = entry_result.map_err(|e| {
            format!(
                "scandir({}): error reading entry: {}",
                String::from_utf8_lossy(&path_bytes),
                e
            )
        })?;

        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            files.push(entry.file_name().as_bytes().to_vec());
        }

        #[cfg(not(unix))]
        {
            if let Some(name) = entry.file_name().to_str() {
                files.push(name.as_bytes().to_vec());
            }
        }
    }

    // Sort alphabetically (PHP behavior)
    files.sort();

    // Build array
    let mut map = IndexMap::new();
    for (idx, name) in files.iter().enumerate() {
        let name_handle = vm.arena.alloc(Val::String(Rc::new(name.clone())));
        map.insert(ArrayKey::Int(idx as i64), name_handle);
    }

    Ok(vm.arena.alloc(Val::Array(ArrayData::from(map).into())))
}

/// sys_get_temp_dir() - Get directory path used for temporary files
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(sys_get_temp_dir)
pub fn php_sys_get_temp_dir(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let temp_dir = std::env::temp_dir();

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(vm.arena.alloc(Val::String(Rc::new(
            temp_dir.as_os_str().as_bytes().to_vec(),
        ))))
    }

    #[cfg(not(unix))]
    {
        let path_str = temp_dir.to_string_lossy().into_owned();
        Ok(vm.arena.alloc(Val::String(Rc::new(path_str.into_bytes()))))
    }
}

/// tmpfile() - Creates a temporary file
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(tmpfile)
pub fn php_tmpfile(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let file = tempfile::tempfile().map_err(|e| format!("tmpfile(): {}", e))?;

    let resource = FileHandle {
        file: RefCell::new(file),
        path: PathBuf::new(), // Anonymous file
        mode: "w+b".to_string(),
        eof: RefCell::new(false),
    };

    Ok(vm.arena.alloc(Val::Resource(Rc::new(resource))))
}

/// getcwd() - Get current working directory
/// Reference: $PHP_SRC_PATH/ext/standard/dir.c - PHP_FUNCTION(getcwd)
pub fn php_getcwd(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if !args.is_empty() {
        return Err("getcwd() expects no parameters".into());
    }

    let cwd = std::env::current_dir().map_err(|e| format!("getcwd(): {}", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(vm
            .arena
            .alloc(Val::String(Rc::new(cwd.as_os_str().as_bytes().to_vec()))))
    }

    #[cfg(not(unix))]
    {
        let path_str = cwd.to_string_lossy().into_owned();
        Ok(vm.arena.alloc(Val::String(Rc::new(path_str.into_bytes()))))
    }
}

/// chdir(directory) - Change working directory
/// Reference: $PHP_SRC_PATH/ext/standard/dir.c - PHP_FUNCTION(chdir)
pub fn php_chdir(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("chdir() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    std::env::set_current_dir(&path)
        .map_err(|e| format!("chdir({}): {}", String::from_utf8_lossy(&path_bytes), e))?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// realpath(path) - Get absolute canonical path
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(realpath)
pub fn php_realpath(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("realpath() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let canonical = path.canonicalize().map_err(|_| {
        // PHP returns false on error, but we use errors for now
        format!(
            "realpath({}): No such file or directory",
            String::from_utf8_lossy(&path_bytes)
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(vm.arena.alloc(Val::String(Rc::new(
            canonical.as_os_str().as_bytes().to_vec(),
        ))))
    }

    #[cfg(not(unix))]
    {
        let path_str = canonical.to_string_lossy().into_owned();
        Ok(vm.arena.alloc(Val::String(Rc::new(path_str.into_bytes()))))
    }
}

/// basename(path, suffix = "") - Get filename component
/// Reference: $PHP_SRC_PATH/ext/standard/string.c - PHP_FUNCTION(basename)
pub fn php_basename(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("basename() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let basename = path
        .file_name()
        .map(|os_str| {
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStrExt;
                os_str.as_bytes().to_vec()
            }
            #[cfg(not(unix))]
            {
                os_str.to_string_lossy().into_owned().into_bytes()
            }
        })
        .unwrap_or_default();

    // Handle suffix removal
    let result = if args.len() > 1 {
        let suffix_val = vm.arena.get(args[1]);
        if let Val::String(suffix) = &suffix_val.value {
            if basename.ends_with(suffix.as_slice()) {
                basename[..basename.len() - suffix.len()].to_vec()
            } else {
                basename
            }
        } else {
            basename
        }
    } else {
        basename
    };

    Ok(vm.arena.alloc(Val::String(Rc::new(result))))
}

/// dirname(path, levels = 1) - Get directory component
/// Reference: $PHP_SRC_PATH/ext/standard/string.c - PHP_FUNCTION(dirname)
pub fn php_dirname(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("dirname() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let mut path = bytes_to_path(&path_bytes)?;

    let levels = if args.len() > 1 {
        let level_val = vm.arena.get(args[1]);
        level_val.value.to_int().max(1) as usize
    } else {
        1
    };

    for _ in 0..levels {
        if let Some(parent) = path.parent() {
            path = parent.to_path_buf();
        } else {
            break;
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let result = if path.as_os_str().is_empty() {
            b".".to_vec()
        } else {
            path.as_os_str().as_bytes().to_vec()
        };
        Ok(vm.arena.alloc(Val::String(Rc::new(result))))
    }

    #[cfg(not(unix))]
    {
        let result = if path.as_os_str().is_empty() {
            b".".to_vec()
        } else {
            path.to_string_lossy().into_owned().into_bytes()
        };
        Ok(vm.arena.alloc(Val::String(Rc::new(result))))
    }
}

/// copy(source, dest) - Copy file
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(copy)
pub fn php_copy(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("copy() expects at least 2 parameters".into());
    }

    let src_bytes = handle_to_path(vm, args[0])?;
    let dst_bytes = handle_to_path(vm, args[1])?;

    let src_path = bytes_to_path(&src_bytes)?;
    let dst_path = bytes_to_path(&dst_bytes)?;

    fs::copy(&src_path, &dst_path).map_err(|e| {
        format!(
            "copy({}, {}): {}",
            String::from_utf8_lossy(&src_bytes),
            String::from_utf8_lossy(&dst_bytes),
            e
        )
    })?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// file(filename, flags = 0) - Read entire file into array
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(file)
pub fn php_file(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("file() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let contents = fs::read(&path).map_err(|e| {
        format!(
            "file({}): failed to open stream: {}",
            String::from_utf8_lossy(&path_bytes),
            e
        )
    })?;

    // Split by newlines
    let mut lines = Vec::new();
    let mut current_line = Vec::new();

    for &byte in &contents {
        current_line.push(byte);
        if byte == b'\n' {
            lines.push(current_line.clone());
            current_line.clear();
        }
    }

    // Add last line if not empty
    if !current_line.is_empty() {
        lines.push(current_line);
    }

    // Build array
    let mut map = IndexMap::new();
    for (idx, line) in lines.iter().enumerate() {
        let line_handle = vm.arena.alloc(Val::String(Rc::new(line.clone())));
        map.insert(ArrayKey::Int(idx as i64), line_handle);
    }

    Ok(vm.arena.alloc(Val::Array(ArrayData::from(map).into())))
}

/// is_executable(filename) - Check if file is executable
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(is_executable)
pub fn php_is_executable(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("is_executable() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let executable = if let Ok(metadata) = fs::metadata(&path) {
            let mode = metadata.permissions().mode();
            (mode & 0o111) != 0
        } else {
            false
        };
        Ok(vm.arena.alloc(Val::Bool(executable)))
    }

    #[cfg(not(unix))]
    {
        // On Windows, check file extension or try to execute
        let executable = path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| matches!(ext.to_lowercase().as_str(), "exe" | "bat" | "cmd" | "com"))
            .unwrap_or(false);
        Ok(vm.arena.alloc(Val::Bool(executable)))
    }
}

/// touch(filename, time = null, atime = null) - Set file access/modification time
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(touch)
pub fn php_touch(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("touch() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    // Create file if it doesn't exist
    if !path.exists() {
        File::create(&path)
            .map_err(|e| format!("touch({}): {}", String::from_utf8_lossy(&path_bytes), e))?;
    }

    // Note: Setting specific mtime/atime requires platform-specific code
    // For now, just creating/touching the file is sufficient

    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// fseek(resource, offset, whence = SEEK_SET) - Seek to position in file
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(fseek)
pub fn php_fseek(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("fseek() expects at least 2 parameters".into());
    }

    let resource_val = vm.arena.get(args[0]);
    let offset_val = vm.arena.get(args[1]);

    let offset = match &offset_val.value {
        Val::Int(i) => *i,
        _ => return Err("fseek(): Offset must be integer".into()),
    };

    let whence = if args.len() > 2 {
        let whence_val = vm.arena.get(args[2]);
        match &whence_val.value {
            Val::Int(w) => *w,
            _ => 0, // SEEK_SET
        }
    } else {
        0 // SEEK_SET
    };

    let seek_from = match whence {
        0 => SeekFrom::Start(offset as u64), // SEEK_SET
        1 => SeekFrom::Current(offset),      // SEEK_CUR
        2 => SeekFrom::End(offset),          // SEEK_END
        _ => return Err("fseek(): Invalid whence value".into()),
    };

    if let Val::Resource(rc) = &resource_val.value {
        if let Some(fh) = rc.downcast_ref::<FileHandle>() {
            fh.file
                .borrow_mut()
                .seek(seek_from)
                .map_err(|e| format!("fseek(): {}", e))?;
            *fh.eof.borrow_mut() = false;
            return Ok(vm.arena.alloc(Val::Int(0)));
        }
    }

    Err("fseek(): supplied argument is not a valid stream resource".into())
}

/// ftell(resource) - Get current position in file
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(ftell)
pub fn php_ftell(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("ftell() expects exactly 1 parameter".into());
    }

    let resource_val = vm.arena.get(args[0]);

    if let Val::Resource(rc) = &resource_val.value {
        if let Some(fh) = rc.downcast_ref::<FileHandle>() {
            let pos = fh
                .file
                .borrow_mut()
                .stream_position()
                .map_err(|e| format!("ftell(): {}", e))?;
            return Ok(vm.arena.alloc(Val::Int(pos as i64)));
        }
    }

    Err("ftell(): supplied argument is not a valid stream resource".into())
}

/// rewind(resource) - Rewind file position to beginning
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(rewind)
pub fn php_rewind(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("rewind() expects exactly 1 parameter".into());
    }

    let resource_val = vm.arena.get(args[0]);

    if let Val::Resource(rc) = &resource_val.value {
        if let Some(fh) = rc.downcast_ref::<FileHandle>() {
            fh.file
                .borrow_mut()
                .seek(SeekFrom::Start(0))
                .map_err(|e| format!("rewind(): {}", e))?;
            *fh.eof.borrow_mut() = false;
            return Ok(vm.arena.alloc(Val::Bool(true)));
        }
    }

    Err("rewind(): supplied argument is not a valid stream resource".into())
}

/// feof(resource) - Test for end-of-file
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(feof)
pub fn php_feof(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("feof() expects exactly 1 parameter".into());
    }

    let resource_val = vm.arena.get(args[0]);

    if let Val::Resource(rc) = &resource_val.value {
        if let Some(fh) = rc.downcast_ref::<FileHandle>() {
            let eof = *fh.eof.borrow();
            return Ok(vm.arena.alloc(Val::Bool(eof)));
        }
    }

    Err("feof(): supplied argument is not a valid stream resource".into())
}

/// fgets(resource, length = null) - Read line from file
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(fgets)
pub fn php_fgets(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("fgets() expects at least 1 parameter".into());
    }

    let resource_val = vm.arena.get(args[0]);

    let max_len = if args.len() > 1 {
        let len_val = vm.arena.get(args[1]);
        match &len_val.value {
            Val::Int(i) if *i > 0 => Some(*i as usize),
            _ => return Err("fgets(): Length must be positive integer".into()),
        }
    } else {
        None
    };

    if let Val::Resource(rc) = &resource_val.value {
        if let Some(fh) = rc.downcast_ref::<FileHandle>() {
            let mut line = Vec::new();
            let mut buf = [0u8; 1];
            let mut bytes_read = 0;

            loop {
                let n = fh
                    .file
                    .borrow_mut()
                    .read(&mut buf)
                    .map_err(|e| format!("fgets(): {}", e))?;

                if n == 0 {
                    break;
                }

                line.push(buf[0]);
                bytes_read += 1;

                // Stop at newline or max length
                if buf[0] == b'\n' {
                    break;
                }

                if let Some(max) = max_len {
                    if bytes_read >= max - 1 {
                        break;
                    }
                }
            }

            if bytes_read == 0 {
                *fh.eof.borrow_mut() = true;
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }

            return Ok(vm.arena.alloc(Val::String(Rc::new(line))));
        }
    }

    Err("fgets(): supplied argument is not a valid stream resource".into())
}

/// fgetc(resource) - Read single character from file
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(fgetc)
pub fn php_fgetc(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("fgetc() expects exactly 1 parameter".into());
    }

    let resource_val = vm.arena.get(args[0]);

    if let Val::Resource(rc) = &resource_val.value {
        if let Some(fh) = rc.downcast_ref::<FileHandle>() {
            let mut buf = [0u8; 1];
            let bytes_read = fh
                .file
                .borrow_mut()
                .read(&mut buf)
                .map_err(|e| format!("fgetc(): {}", e))?;

            if bytes_read == 0 {
                *fh.eof.borrow_mut() = true;
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }

            return Ok(vm.arena.alloc(Val::String(Rc::new(vec![buf[0]]))));
        }
    }

    Err("fgetc(): supplied argument is not a valid stream resource".into())
}

/// fputs(resource, string) - Alias for fwrite
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(fputs)
pub fn php_fputs(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    php_fwrite(vm, args)
}

/// fflush(resource) - Flush output to file
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(fflush)
pub fn php_fflush(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("fflush() expects exactly 1 parameter".into());
    }

    let resource_val = vm.arena.get(args[0]);

    if let Val::Resource(rc) = &resource_val.value {
        if let Some(fh) = rc.downcast_ref::<FileHandle>() {
            fh.file
                .borrow_mut()
                .flush()
                .map_err(|e| format!("fflush(): {}", e))?;
            return Ok(vm.arena.alloc(Val::Bool(true)));
        }
    }

    Err("fflush(): supplied argument is not a valid stream resource".into())
}

/// filemtime(filename) - Get file modification time
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(filemtime)
pub fn php_filemtime(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("filemtime() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let metadata = fs::metadata(&path)
        .map_err(|e| format!("filemtime({}): {}", String::from_utf8_lossy(&path_bytes), e))?;

    let mtime = metadata
        .modified()
        .map_err(|e| format!("filemtime(): {}", e))?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("filemtime(): {}", e))?
        .as_secs();

    Ok(vm.arena.alloc(Val::Int(mtime as i64)))
}

/// fileatime(filename) - Get file access time
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(fileatime)
pub fn php_fileatime(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("fileatime() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let metadata = fs::metadata(&path)
        .map_err(|e| format!("fileatime({}): {}", String::from_utf8_lossy(&path_bytes), e))?;

    let atime = metadata
        .accessed()
        .map_err(|e| format!("fileatime(): {}", e))?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("fileatime(): {}", e))?
        .as_secs();

    Ok(vm.arena.alloc(Val::Int(atime as i64)))
}

/// filectime(filename) - Get file inode change time
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(filectime)
pub fn php_filectime(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("filectime() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let metadata = fs::metadata(&path)
        .map_err(|e| format!("filectime({}): {}", String::from_utf8_lossy(&path_bytes), e))?;

    // On Unix, this is ctime (change time). On Windows, use creation time.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let ctime = metadata.ctime();
        Ok(vm.arena.alloc(Val::Int(ctime)))
    }

    #[cfg(not(unix))]
    {
        let ctime = metadata
            .created()
            .map_err(|e| format!("filectime(): {}", e))?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("filectime(): {}", e))?
            .as_secs();
        Ok(vm.arena.alloc(Val::Int(ctime as i64)))
    }
}

/// fileperms(filename) - Get file permissions
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(fileperms)
pub fn php_fileperms(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("fileperms() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let metadata = fs::metadata(&path)
        .map_err(|e| format!("fileperms({}): {}", String::from_utf8_lossy(&path_bytes), e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        Ok(vm.arena.alloc(Val::Int(mode as i64)))
    }

    #[cfg(not(unix))]
    {
        // On Windows, approximate permissions
        let readonly = metadata.permissions().readonly();
        let perms = if readonly { 0o444 } else { 0o666 };
        Ok(vm.arena.alloc(Val::Int(perms)))
    }
}

/// fileowner(filename) - Get file owner
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(fileowner)
pub fn php_fileowner(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("fileowner() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(&path)
            .map_err(|e| format!("fileowner({}): {}", String::from_utf8_lossy(&path_bytes), e))?;

        let uid = metadata.uid();
        Ok(vm.arena.alloc(Val::Int(uid as i64)))
    }

    #[cfg(not(unix))]
    {
        // Not supported on Windows
        Err("fileowner(): Not supported on this platform".into())
    }
}

/// filegroup(filename) - Get file group
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(filegroup)
pub fn php_filegroup(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("filegroup() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = fs::metadata(&path)
            .map_err(|e| format!("filegroup({}): {}", String::from_utf8_lossy(&path_bytes), e))?;

        let gid = metadata.gid();
        Ok(vm.arena.alloc(Val::Int(gid as i64)))
    }

    #[cfg(not(unix))]
    {
        // Not supported on Windows
        Err("filegroup(): Not supported on this platform".into())
    }
}

/// chmod(filename, mode) - Change file permissions
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(chmod)
pub fn php_chmod(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("chmod() expects at least 2 parameters".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let mode_val = vm.arena.get(args[1]);
    let mode = match &mode_val.value {
        Val::Int(m) => *m as u32,
        _ => return Err("chmod(): Mode must be integer".into()),
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(mode);
        fs::set_permissions(&path, perms)
            .map_err(|e| format!("chmod({}): {}", String::from_utf8_lossy(&path_bytes), e))?;
        Ok(vm.arena.alloc(Val::Bool(true)))
    }

    #[cfg(not(unix))]
    {
        // On Windows, only read-only bit can be set
        let readonly = (mode & 0o200) == 0;
        let mut perms = fs::metadata(&path)
            .map_err(|e| format!("chmod(): {}", e))?
            .permissions();
        perms.set_readonly(readonly);
        fs::set_permissions(&path, perms)
            .map_err(|e| format!("chmod({}): {}", String::from_utf8_lossy(&path_bytes), e))?;
        Ok(vm.arena.alloc(Val::Bool(true)))
    }
}

/// stat(filename) - Get file statistics
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(stat)
pub fn php_stat(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("stat() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let metadata = fs::metadata(&path)
        .map_err(|e| format!("stat({}): {}", String::from_utf8_lossy(&path_bytes), e))?;

    build_stat_array(vm, &metadata)
}

/// lstat(filename) - Get file statistics (don't follow symlinks)
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(lstat)
pub fn php_lstat(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("lstat() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let metadata = fs::symlink_metadata(&path)
        .map_err(|e| format!("lstat({}): {}", String::from_utf8_lossy(&path_bytes), e))?;

    build_stat_array(vm, &metadata)
}

/// Helper to build stat array from metadata
fn build_stat_array(vm: &mut VM, metadata: &Metadata) -> Result<Handle, String> {
    let mut map = IndexMap::new();

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        // Numeric indices
        map.insert(
            ArrayKey::Int(0),
            vm.arena.alloc(Val::Int(metadata.dev() as i64)),
        );
        map.insert(
            ArrayKey::Int(1),
            vm.arena.alloc(Val::Int(metadata.ino() as i64)),
        );
        map.insert(
            ArrayKey::Int(2),
            vm.arena.alloc(Val::Int(metadata.mode() as i64)),
        );
        map.insert(
            ArrayKey::Int(3),
            vm.arena.alloc(Val::Int(metadata.nlink() as i64)),
        );
        map.insert(
            ArrayKey::Int(4),
            vm.arena.alloc(Val::Int(metadata.uid() as i64)),
        );
        map.insert(
            ArrayKey::Int(5),
            vm.arena.alloc(Val::Int(metadata.gid() as i64)),
        );
        map.insert(
            ArrayKey::Int(6),
            vm.arena.alloc(Val::Int(metadata.rdev() as i64)),
        );
        map.insert(
            ArrayKey::Int(7),
            vm.arena.alloc(Val::Int(metadata.size() as i64)),
        );
        map.insert(ArrayKey::Int(8), vm.arena.alloc(Val::Int(metadata.atime())));
        map.insert(ArrayKey::Int(9), vm.arena.alloc(Val::Int(metadata.mtime())));
        map.insert(
            ArrayKey::Int(10),
            vm.arena.alloc(Val::Int(metadata.ctime())),
        );
        map.insert(
            ArrayKey::Int(11),
            vm.arena.alloc(Val::Int(metadata.blksize() as i64)),
        );
        map.insert(
            ArrayKey::Int(12),
            vm.arena.alloc(Val::Int(metadata.blocks() as i64)),
        );

        // String indices
        map.insert(
            ArrayKey::Str(Rc::new(b"dev".to_vec())),
            vm.arena.alloc(Val::Int(metadata.dev() as i64)),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"ino".to_vec())),
            vm.arena.alloc(Val::Int(metadata.ino() as i64)),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"mode".to_vec())),
            vm.arena.alloc(Val::Int(metadata.mode() as i64)),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"nlink".to_vec())),
            vm.arena.alloc(Val::Int(metadata.nlink() as i64)),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"uid".to_vec())),
            vm.arena.alloc(Val::Int(metadata.uid() as i64)),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"gid".to_vec())),
            vm.arena.alloc(Val::Int(metadata.gid() as i64)),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"rdev".to_vec())),
            vm.arena.alloc(Val::Int(metadata.rdev() as i64)),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"size".to_vec())),
            vm.arena.alloc(Val::Int(metadata.size() as i64)),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"atime".to_vec())),
            vm.arena.alloc(Val::Int(metadata.atime())),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"mtime".to_vec())),
            vm.arena.alloc(Val::Int(metadata.mtime())),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"ctime".to_vec())),
            vm.arena.alloc(Val::Int(metadata.ctime())),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"blksize".to_vec())),
            vm.arena.alloc(Val::Int(metadata.blksize() as i64)),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"blocks".to_vec())),
            vm.arena.alloc(Val::Int(metadata.blocks() as i64)),
        );
    }

    #[cfg(not(unix))]
    {
        // Windows - provide subset of stat data
        let size = metadata.len() as i64;
        let mtime = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let atime = metadata
            .accessed()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let ctime = metadata
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        map.insert(ArrayKey::Int(7), vm.arena.alloc(Val::Int(size)));
        map.insert(ArrayKey::Int(8), vm.arena.alloc(Val::Int(atime)));
        map.insert(ArrayKey::Int(9), vm.arena.alloc(Val::Int(mtime)));
        map.insert(ArrayKey::Int(10), vm.arena.alloc(Val::Int(ctime)));

        map.insert(
            ArrayKey::Str(Rc::new(b"size".to_vec())),
            vm.arena.alloc(Val::Int(size)),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"atime".to_vec())),
            vm.arena.alloc(Val::Int(atime)),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"mtime".to_vec())),
            vm.arena.alloc(Val::Int(mtime)),
        );
        map.insert(
            ArrayKey::Str(Rc::new(b"ctime".to_vec())),
            vm.arena.alloc(Val::Int(ctime)),
        );
    }

    Ok(vm.arena.alloc(Val::Array(ArrayData::from(map).into())))
}

/// tempnam(dir, prefix) - Create temporary file with unique name
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(tempnam)
pub fn php_tempnam(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("tempnam() expects at least 2 parameters".into());
    }

    let dir_bytes = handle_to_path(vm, args[0])?;
    let prefix_bytes = handle_to_path(vm, args[1])?;

    let dir = bytes_to_path(&dir_bytes)?;
    let prefix = String::from_utf8_lossy(&prefix_bytes).to_string();

    let named_temp_file = tempfile::Builder::new()
        .prefix(&prefix)
        .tempfile_in(&dir)
        .map_err(|e| format!("tempnam(): {}", e))?;

    // Persist the file so it's not deleted when NamedTempFile drops
    let (_file, path) = named_temp_file
        .keep()
        .map_err(|e| format!("tempnam(): {}", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(vm
            .arena
            .alloc(Val::String(Rc::new(path.as_os_str().as_bytes().to_vec()))))
    }

    #[cfg(not(unix))]
    {
        let path_str = path.to_string_lossy().into_owned();
        Ok(vm.arena.alloc(Val::String(Rc::new(path_str.into_bytes()))))
    }
}

/// is_link(filename) - Check if file is a symbolic link
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(is_link)
pub fn php_is_link(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("is_link() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let is_link = if let Ok(metadata) = fs::symlink_metadata(&path) {
        metadata.is_symlink()
    } else {
        false
    };

    Ok(vm.arena.alloc(Val::Bool(is_link)))
}

/// readlink(filename) - Read symbolic link target
/// Reference: $PHP_SRC_PATH/ext/standard/file.c - PHP_FUNCTION(readlink)
pub fn php_readlink(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("readlink() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let path = bytes_to_path(&path_bytes)?;

    let target = fs::read_link(&path)
        .map_err(|e| format!("readlink({}): {}", String::from_utf8_lossy(&path_bytes), e))?;

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(vm
            .arena
            .alloc(Val::String(Rc::new(target.as_os_str().as_bytes().to_vec()))))
    }

    #[cfg(not(unix))]
    {
        let target_str = target.to_string_lossy().into_owned();
        Ok(vm
            .arena
            .alloc(Val::String(Rc::new(target_str.into_bytes()))))
    }
}

/// disk_free_space(directory) - Get available disk space
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(disk_free_space)
pub fn php_disk_free_space(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("disk_free_space() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let _path = bytes_to_path(&path_bytes)?;

    // This requires platform-specific syscalls (statvfs on Unix, GetDiskFreeSpaceEx on Windows)
    // For now, return a placeholder
    Err("disk_free_space(): Not yet implemented".into())
}

/// disk_total_space(directory) - Get total disk space
/// Reference: $PHP_SRC_PATH/ext/standard/filestat.c - PHP_FUNCTION(disk_total_space)
pub fn php_disk_total_space(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("disk_total_space() expects at least 1 parameter".into());
    }

    let path_bytes = handle_to_path(vm, args[0])?;
    let _path = bytes_to_path(&path_bytes)?;

    // This requires platform-specific syscalls
    Err("disk_total_space(): Not yet implemented".into())
}
