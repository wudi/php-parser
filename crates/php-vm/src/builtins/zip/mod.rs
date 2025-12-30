use crate::core::value::{ArrayData, ArrayKey, Handle, Val, Visibility};
use crate::runtime::registry::{ExtensionRegistry, NativeClassDef, NativeMethodEntry};
use crate::vm::engine::VM;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::Path;
use std::rc::Rc;
use zip::ZipArchive;

#[derive(Debug)]
pub struct ZipArchiveWrapper {
    pub path: String,
    pub last_error: i64,
    pub status: i64,
    #[allow(dead_code)]
    pub reader: Option<ZipArchive<File>>,
    pub password: Option<String>,
    pub additions: IndexMap<String, Vec<u8>>,
    pub deletions: HashSet<String>,
    pub current_entry_index: usize,
}

impl ZipArchiveWrapper {
    pub fn new() -> Self {
        Self {
            path: String::new(),
            last_error: 0,
            status: 0,
            reader: None,
            password: None,
            additions: IndexMap::new(),
            deletions: HashSet::new(),
            current_entry_index: 0,
        }
    }
}

fn get_zip_wrapper<'a>(
    vm: &'a mut VM,
    this_handle: Handle,
) -> Result<Rc<RefCell<ZipArchiveWrapper>>, String> {
    let obj_handle = match &vm.arena.get(this_handle).value {
        Val::Object(h) => *h,
        _ => return Err("Invalid 'this' object".into()),
    };

    let id_sym = vm.context.interner.intern(b"__id");
    let archive_id = if let Val::ObjPayload(obj) = &vm.arena.get(obj_handle).value {
        match obj.properties.get(&id_sym) {
            Some(h) => match &vm.arena.get(*h).value {
                Val::Int(id) => *id as u64,
                _ => return Err("ZipArchive not initialized".into()),
            },
            None => return Err("ZipArchive not initialized".into()),
        }
    } else {
        return Err("Invalid object payload".into());
    };

    vm.context
        .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).archives
        .get(&archive_id)
        .cloned()
        .ok_or_else(|| "ZipArchive not found in context".to_string())
}

pub fn register_zip_extension_to_registry(registry: &mut ExtensionRegistry) {
    let mut zip_methods = HashMap::new();

    zip_methods.insert(
        b"open".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_open,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"close".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_close,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"addEmptyDir".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_add_empty_dir,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"addFile".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_add_file,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"addFromString".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_add_from_string,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"count".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_count,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"deleteIndex".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_delete_index,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"deleteName".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_delete_name,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"extractTo".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_extract_to,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"renameIndex".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_rename_index,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"renameName".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_rename_name,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"getFromIndex".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_get_from_index,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"getFromName".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_get_from_name,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"getNameIndex".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_get_name_index,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"getStatusString".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_get_status_string,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"locateName".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_locate_name,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"statIndex".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_stat_index,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"statName".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_stat_name,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"unchangeAll".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_unchange_all,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"setPassword".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_set_password,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"getNameIndex".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_get_name_index,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"locateName".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_locate_name,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    zip_methods.insert(
        b"extractTo".to_vec(),
        NativeMethodEntry {
            handler: php_zip_archive_extract_to,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    let mut zip_constants = HashMap::new();

    // Archive open modes
    zip_constants.insert(b"CREATE".to_vec(), (Val::Int(1), Visibility::Public));
    zip_constants.insert(b"OVERWRITE".to_vec(), (Val::Int(8), Visibility::Public));
    zip_constants.insert(b"EXCL".to_vec(), (Val::Int(2), Visibility::Public));
    zip_constants.insert(b"RDONLY".to_vec(), (Val::Int(16), Visibility::Public));
    zip_constants.insert(b"CHECKCONS".to_vec(), (Val::Int(4), Visibility::Public));

    // Errors
    zip_constants.insert(b"ER_OK".to_vec(), (Val::Int(0), Visibility::Public));
    zip_constants.insert(b"ER_MULTIDISK".to_vec(), (Val::Int(1), Visibility::Public));
    zip_constants.insert(b"ER_RENAME".to_vec(), (Val::Int(2), Visibility::Public));
    zip_constants.insert(b"ER_CLOSE".to_vec(), (Val::Int(3), Visibility::Public));
    zip_constants.insert(b"ER_SEEK".to_vec(), (Val::Int(4), Visibility::Public));
    zip_constants.insert(b"ER_READ".to_vec(), (Val::Int(5), Visibility::Public));
    zip_constants.insert(b"ER_WRITE".to_vec(), (Val::Int(6), Visibility::Public));
    zip_constants.insert(b"ER_CRC".to_vec(), (Val::Int(7), Visibility::Public));
    zip_constants.insert(b"ER_ZIPCLOSED".to_vec(), (Val::Int(8), Visibility::Public));
    zip_constants.insert(b"ER_NOENT".to_vec(), (Val::Int(9), Visibility::Public));
    zip_constants.insert(b"ER_EXISTS".to_vec(), (Val::Int(10), Visibility::Public));
    zip_constants.insert(b"ER_OPEN".to_vec(), (Val::Int(11), Visibility::Public));
    zip_constants.insert(b"ER_TMPOPEN".to_vec(), (Val::Int(12), Visibility::Public));
    zip_constants.insert(b"ER_ZLIB".to_vec(), (Val::Int(13), Visibility::Public));
    zip_constants.insert(b"ER_MEMORY".to_vec(), (Val::Int(14), Visibility::Public));
    zip_constants.insert(b"ER_CHANGED".to_vec(), (Val::Int(15), Visibility::Public));
    zip_constants.insert(
        b"ER_COMPNOTSUPP".to_vec(),
        (Val::Int(16), Visibility::Public),
    );
    zip_constants.insert(b"ER_EOF".to_vec(), (Val::Int(17), Visibility::Public));
    zip_constants.insert(b"ER_INVAL".to_vec(), (Val::Int(18), Visibility::Public));
    zip_constants.insert(b"ER_NOZIP".to_vec(), (Val::Int(19), Visibility::Public));
    zip_constants.insert(b"ER_INTERNAL".to_vec(), (Val::Int(20), Visibility::Public));
    zip_constants.insert(b"ER_INCONS".to_vec(), (Val::Int(21), Visibility::Public));
    zip_constants.insert(b"ER_REMOVE".to_vec(), (Val::Int(22), Visibility::Public));
    zip_constants.insert(b"ER_DELETED".to_vec(), (Val::Int(23), Visibility::Public));
    zip_constants.insert(
        b"ER_ENCRNOTSUPP".to_vec(),
        (Val::Int(24), Visibility::Public),
    );
    zip_constants.insert(b"ER_RDONLY".to_vec(), (Val::Int(25), Visibility::Public));
    zip_constants.insert(b"ER_NOPASSWD".to_vec(), (Val::Int(26), Visibility::Public));
    zip_constants.insert(
        b"ER_WRONGPASSWD".to_vec(),
        (Val::Int(27), Visibility::Public),
    );
    zip_constants.insert(b"ER_OPNOTSUPP".to_vec(), (Val::Int(28), Visibility::Public));
    zip_constants.insert(b"ER_INUSE".to_vec(), (Val::Int(29), Visibility::Public));
    zip_constants.insert(b"ER_TELL".to_vec(), (Val::Int(30), Visibility::Public));
    zip_constants.insert(
        b"ER_COMPRESSED_DATA".to_vec(),
        (Val::Int(31), Visibility::Public),
    );
    zip_constants.insert(b"ER_CANCELLED".to_vec(), (Val::Int(32), Visibility::Public));

    // Flags
    zip_constants.insert(b"FL_NOCASE".to_vec(), (Val::Int(1), Visibility::Public));
    zip_constants.insert(b"FL_NODIR".to_vec(), (Val::Int(2), Visibility::Public));
    zip_constants.insert(b"FL_COMPRESSED".to_vec(), (Val::Int(4), Visibility::Public));
    zip_constants.insert(b"FL_UNCHANGED".to_vec(), (Val::Int(8), Visibility::Public));
    zip_constants.insert(
        b"FL_RECOMPRESS".to_vec(),
        (Val::Int(16), Visibility::Public),
    );
    zip_constants.insert(b"FL_ENCRYPTED".to_vec(), (Val::Int(32), Visibility::Public));
    zip_constants.insert(b"FL_OVERWRITE".to_vec(), (Val::Int(64), Visibility::Public));
    zip_constants.insert(b"FL_LOCAL".to_vec(), (Val::Int(128), Visibility::Public));
    zip_constants.insert(b"FL_CENTRAL".to_vec(), (Val::Int(256), Visibility::Public));

    // Compression modes
    zip_constants.insert(b"CM_DEFAULT".to_vec(), (Val::Int(-1), Visibility::Public));
    zip_constants.insert(b"CM_STORE".to_vec(), (Val::Int(0), Visibility::Public));
    zip_constants.insert(b"CM_SHRINK".to_vec(), (Val::Int(1), Visibility::Public));
    zip_constants.insert(b"CM_REDUCE_1".to_vec(), (Val::Int(2), Visibility::Public));
    zip_constants.insert(b"CM_REDUCE_2".to_vec(), (Val::Int(3), Visibility::Public));
    zip_constants.insert(b"CM_REDUCE_3".to_vec(), (Val::Int(4), Visibility::Public));
    zip_constants.insert(b"CM_REDUCE_4".to_vec(), (Val::Int(5), Visibility::Public));
    zip_constants.insert(b"CM_IMPLODE".to_vec(), (Val::Int(6), Visibility::Public));
    zip_constants.insert(b"CM_DEFLATE".to_vec(), (Val::Int(8), Visibility::Public));
    zip_constants.insert(b"CM_DEFLATE64".to_vec(), (Val::Int(9), Visibility::Public));
    zip_constants.insert(
        b"CM_PKWARE_IMPLODE".to_vec(),
        (Val::Int(10), Visibility::Public),
    );
    zip_constants.insert(b"CM_BZIP2".to_vec(), (Val::Int(12), Visibility::Public));
    zip_constants.insert(b"CM_LZMA".to_vec(), (Val::Int(14), Visibility::Public));
    zip_constants.insert(b"CM_LZMA2".to_vec(), (Val::Int(33), Visibility::Public));
    zip_constants.insert(b"CM_ZSTD".to_vec(), (Val::Int(93), Visibility::Public));
    zip_constants.insert(b"CM_XZ".to_vec(), (Val::Int(95), Visibility::Public));

    // Encryption modes
    zip_constants.insert(b"EM_NONE".to_vec(), (Val::Int(0), Visibility::Public));
    zip_constants.insert(
        b"EM_TRAD_PKWARE".to_vec(),
        (Val::Int(1), Visibility::Public),
    );
    zip_constants.insert(b"EM_AES_128".to_vec(), (Val::Int(257), Visibility::Public));
    zip_constants.insert(b"EM_AES_192".to_vec(), (Val::Int(258), Visibility::Public));
    zip_constants.insert(b"EM_AES_256".to_vec(), (Val::Int(259), Visibility::Public));
    zip_constants.insert(
        b"EM_UNKNOWN".to_vec(),
        (Val::Int(65535), Visibility::Public),
    );

    registry.register_class(NativeClassDef {
        name: b"ZipArchive".to_vec(),
        parent: None,
            is_interface: false,
            is_trait: false,
        interfaces: Vec::new(),
        methods: zip_methods,
        constants: zip_constants,
        constructor: None,
    });

    // Procedural functions
    registry.register_function(b"zip_open", php_zip_open);
    registry.register_function(b"zip_close", php_zip_close);
    registry.register_function(b"zip_read", php_zip_read);
    registry.register_function(b"zip_entry_open", php_zip_entry_open);
    registry.register_function(b"zip_entry_close", php_zip_entry_close);
    registry.register_function(b"zip_entry_read", php_zip_entry_read);
    registry.register_function(b"zip_entry_name", php_zip_entry_name);
    registry.register_function(b"zip_entry_filesize", php_zip_entry_filesize);
    registry.register_function(
        b"zip_entry_compressionmethod",
        php_zip_entry_compressionmethod,
    );
    registry.register_function(b"zip_entry_compressedsize", php_zip_entry_compressedsize);
}

fn update_zip_properties(
    vm: &mut VM,
    this_handle: Handle,
    wrapper: &ZipArchiveWrapper,
) -> Result<(), String> {
    let base_count = wrapper.reader.as_ref().map(|r| r.len()).unwrap_or(0);
    let num_files = (base_count + wrapper.additions.len() - wrapper.deletions.len()) as i64;
    let filename = wrapper.path.clone();
    let comment = wrapper
        .reader
        .as_ref()
        .map(|r| r.comment().to_vec())
        .unwrap_or_default();

    let num_files_sym = vm.context.interner.intern(b"numFiles");
    let filename_sym = vm.context.interner.intern(b"filename");
    let comment_sym = vm.context.interner.intern(b"comment");
    let status_sym = vm.context.interner.intern(b"status");

    let num_files_handle = vm.arena.alloc(Val::Int(num_files));
    let filename_handle = vm.arena.alloc(Val::String(Rc::new(filename.into_bytes())));
    let comment_handle = vm.arena.alloc(Val::String(Rc::new(comment)));
    let status_handle = vm.arena.alloc(Val::Int(0)); // Success for now

    let this_val = vm.arena.get(this_handle);
    if let Val::Object(obj_handle) = &this_val.value {
        let obj_handle = *obj_handle;
        let obj_val = vm.arena.get_mut(obj_handle);
        if let Val::ObjPayload(obj_data) = &mut obj_val.value {
            obj_data.properties.insert(num_files_sym, num_files_handle);
            obj_data.properties.insert(filename_sym, filename_handle);
            obj_data.properties.insert(comment_sym, comment_handle);
            obj_data.properties.insert(status_sym, status_handle);
        }
    }

    Ok(())
}

pub fn php_zip_archive_open(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("ZipArchive::open() expects at least 1 parameter".into());
    }

    let filename = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("ZipArchive::open(): Argument #1 (filename) must be string".into()),
    };

    let flags = if args.len() > 1 {
        match &vm.arena.get(args[1]).value {
            Val::Int(i) => *i,
            _ => 0,
        }
    } else {
        0
    };

    let path = Path::new(&filename);
    let exists = path.exists();

    if (flags & 2 != 0) && exists {
        // ZipArchive::EXCL
        return Ok(vm.arena.alloc(Val::Int(10))); // ER_EXISTS
    }

    if !exists && (flags & 1 == 0) {
        // Not ZipArchive::CREATE
        return Ok(vm.arena.alloc(Val::Int(9))); // ER_NOENT
    }

    let mut wrapper = ZipArchiveWrapper::new();
    wrapper.path = filename.clone();

    if exists && (flags & 8 == 0) {
        // Not ZipArchive::OVERWRITE, try to open existing
        match File::open(path) {
            Ok(file) => match ZipArchive::new(file) {
                Ok(archive) => {
                    wrapper.reader = Some(archive);
                }
                Err(_) => {
                    return Ok(vm.arena.alloc(Val::Int(19))); // ER_NOZIP
                }
            },
            Err(_) => {
                return Ok(vm.arena.alloc(Val::Int(11))); // ER_OPEN
            }
        }
    }

    let archive_id = vm.context.next_resource_id;
    vm.context.next_resource_id += 1;
    let wrapper_rc = Rc::new(RefCell::new(wrapper));
    vm.context
        .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).archives
        .insert(archive_id, wrapper_rc.clone());

    // Store ID in object
    if let Some(this_handle) = vm.frames.last().and_then(|f| f.this) {
        let obj_handle = match &vm.arena.get(this_handle).value {
            Val::Object(h) => *h,
            _ => return Err("No 'this' in ZipArchive::open".into()),
        };

        let id_sym = vm.context.interner.intern(b"__id");
        let id_val = vm.arena.alloc(Val::Int(archive_id as i64));

        if let Val::ObjPayload(obj) = &mut vm.arena.get_mut(obj_handle).value {
            obj.properties.insert(id_sym, id_val);
        }

        // Update properties
        update_zip_properties(vm, this_handle, &wrapper_rc.borrow())?;
    }

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_zip_archive_close(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::close")?;
    let wrapper_rc = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper_rc.borrow_mut();

    if wrapper.additions.is_empty() && wrapper.deletions.is_empty() {
        wrapper.reader = None;
        return Ok(vm.arena.alloc(Val::Bool(true)));
    }

    // We have changes, need to write
    let path = wrapper.path.clone();
    let temp_path = format!("{}.tmp", path);

    {
        let file = File::create(&temp_path).map_err(|e| e.to_string())?;
        let mut writer = zip::ZipWriter::new(file);

        let deletions = wrapper.deletions.clone();

        // Copy old entries (if not deleted)
        if let Some(reader) = &mut wrapper.reader {
            for i in 0..reader.len() {
                let mut entry = reader.by_index(i).map_err(|e| e.to_string())?;
                let name = entry.name().to_string();

                if deletions.contains(&name) {
                    continue;
                }

                let options = zip::write::SimpleFileOptions::default()
                    .compression_method(entry.compression())
                    .last_modified_time(entry.last_modified().unwrap_or_default());

                writer
                    .start_file(name, options)
                    .map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut writer).map_err(|e| e.to_string())?;
            }
        }

        // Add new entries
        for (name, content) in &wrapper.additions {
            let options = zip::write::SimpleFileOptions::default();
            writer
                .start_file(name, options)
                .map_err(|e| e.to_string())?;
            use std::io::Write;
            writer.write_all(content).map_err(|e| e.to_string())?;
        }

        writer.finish().map_err(|e| e.to_string())?;
    }

    // Replace old file with new one
    std::fs::rename(temp_path, path).map_err(|e| e.to_string())?;

    wrapper.reader = None;
    wrapper.additions.clear();
    wrapper.deletions.clear();

    // Update properties
    update_zip_properties(vm, this_handle, &wrapper)?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_zip_archive_add_file(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("ZipArchive::addFile() expects at least 1 parameter".into());
    }

    let filename = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("ZipArchive::addFile(): Argument #1 (filename) must be string".into()),
    };

    let localname = if args.len() >= 2 {
        match &vm.arena.get(args[1]).value {
            Val::String(s) => String::from_utf8_lossy(s).to_string(),
            _ => filename.clone(),
        }
    } else {
        filename.clone()
    };

    // Read file content
    let content = match std::fs::read(&filename) {
        Ok(c) => c,
        Err(_) => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::addFile")?;
    let wrapper = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper.borrow_mut();

    wrapper.additions.insert(localname, content);

    // Update properties
    update_zip_properties(vm, this_handle, &wrapper)?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_zip_archive_add_empty_dir(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("ZipArchive::addEmptyDir() expects 1 parameter".into());
    }

    let dirname = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("ZipArchive::addEmptyDir(): Argument #1 (dirname) must be string".into()),
    };

    // Ensure it ends with /
    let mut dirname = dirname;
    if !dirname.ends_with('/') {
        dirname.push('/');
    }

    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::addEmptyDir")?;
    let wrapper = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper.borrow_mut();

    wrapper.additions.insert(dirname, Vec::new());

    // Update properties
    update_zip_properties(vm, this_handle, &wrapper)?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_zip_archive_add_from_string(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("ZipArchive::addFromString() expects at least 2 parameters".into());
    }

    let name = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("ZipArchive::addFromString(): Argument #1 (name) must be string".into()),
    };

    let content = match &vm.arena.get(args[1]).value {
        Val::String(s) => s.to_vec(),
        _ => {
            return Err("ZipArchive::addFromString(): Argument #2 (content) must be string".into())
        }
    };

    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::addFromString")?;
    let wrapper = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper.borrow_mut();

    wrapper.additions.insert(name, content);

    // Update properties
    update_zip_properties(vm, this_handle, &wrapper)?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_zip_archive_count(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::count")?;
    let wrapper = get_zip_wrapper(vm, this_handle)?;
    let wrapper = wrapper.borrow();

    let count = if let Some(reader) = &wrapper.reader {
        reader.len() as i64
    } else {
        0
    };

    Ok(vm.arena.alloc(Val::Int(count)))
}

pub fn php_zip_archive_delete_index(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("ZipArchive::deleteIndex() expects 1 parameter".into());
    }

    let index = match &vm.arena.get(args[0]).value {
        Val::Int(i) => *i as usize,
        _ => return Err("ZipArchive::deleteIndex(): Argument #1 (index) must be integer".into()),
    };

    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::deleteIndex")?;
    let wrapper = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper.borrow_mut();

    let reader_len = wrapper.reader.as_ref().map(|r| r.len()).unwrap_or(0);
    let mut name_to_delete = None;

    if index < reader_len {
        if let Some(reader) = &mut wrapper.reader {
            if let Ok(entry) = reader.by_index(index) {
                name_to_delete = Some(entry.name().to_string());
            }
        }
    } else {
        let addition_index = index - reader_len;
        if addition_index < wrapper.additions.len() {
            if let Some((name, _)) = wrapper.additions.get_index(addition_index) {
                name_to_delete = Some(name.clone());
            }
        }
    }

    if let Some(name) = name_to_delete {
        if wrapper.additions.contains_key(&name) {
            wrapper.additions.shift_remove(&name);
        } else {
            wrapper.deletions.insert(name);
        }

        // Update properties
        update_zip_properties(vm, this_handle, &wrapper)?;

        return Ok(vm.arena.alloc(Val::Bool(true)));
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_zip_archive_delete_name(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("ZipArchive::deleteName() expects 1 parameter".into());
    }

    let name = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("ZipArchive::deleteName(): Argument #1 (name) must be string".into()),
    };

    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::deleteName")?;
    let wrapper = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper.borrow_mut();

    wrapper.deletions.insert(name);

    // Update properties
    update_zip_properties(vm, this_handle, &wrapper)?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_zip_archive_extract_to(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("ZipArchive::extractTo() expects at least 1 parameter".into());
    }

    let destination = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => {
            return Err("ZipArchive::extractTo(): Argument #1 (destination) must be string".into())
        }
    };

    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::extractTo")?;
    let wrapper_rc = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper_rc.borrow_mut();

    if let Some(reader) = &mut wrapper.reader {
        for i in 0..reader.len() {
            let mut file = reader.by_index(i).map_err(|e| e.to_string())?;
            let outpath = match file.enclosed_name() {
                Some(path) => Path::new(&destination).join(path),
                None => continue,
            };

            if file.is_dir() {
                std::fs::create_dir_all(&outpath).map_err(|e| e.to_string())?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
                    }
                }
                let mut outfile = std::fs::File::create(&outpath).map_err(|e| e.to_string())?;
                std::io::copy(&mut file, &mut outfile).map_err(|e| e.to_string())?;
            }
        }
        return Ok(vm.arena.alloc(Val::Bool(true)));
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_zip_archive_rename_index(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("ZipArchive::renameIndex() expects 2 parameters".into());
    }

    let index = match &vm.arena.get(args[0]).value {
        Val::Int(i) => *i as usize,
        _ => return Err("ZipArchive::renameIndex(): Argument #1 (index) must be integer".into()),
    };

    let new_name = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("ZipArchive::renameIndex(): Argument #2 (new_name) must be string".into()),
    };

    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::renameIndex")?;
    let wrapper = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper.borrow_mut();

    let old_data = if let Some(reader) = &mut wrapper.reader {
        if let Ok(mut entry) = reader.by_index(index) {
            let old_name = entry.name().to_string();
            let mut content = Vec::new();
            use std::io::Read;
            if entry.read_to_end(&mut content).is_ok() {
                Some((old_name, content))
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if let Some((old_name, content)) = old_data {
        wrapper.additions.insert(new_name, content);
        wrapper.deletions.insert(old_name);
        return Ok(vm.arena.alloc(Val::Bool(true)));
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_zip_archive_rename_name(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("ZipArchive::renameName() expects 2 parameters".into());
    }

    let name = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("ZipArchive::renameName(): Argument #1 (name) must be string".into()),
    };

    let new_name = match &vm.arena.get(args[1]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("ZipArchive::renameName(): Argument #2 (new_name) must be string".into()),
    };

    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::renameName")?;
    let wrapper = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper.borrow_mut();

    if let Some(content) = wrapper.additions.shift_remove(&name) {
        wrapper.additions.insert(new_name, content);
        return Ok(vm.arena.alloc(Val::Bool(true)));
    }

    let old_data = if let Some(reader) = &mut wrapper.reader {
        if let Ok(mut entry) = reader.by_name(&name) {
            let mut content = Vec::new();
            use std::io::Read;
            if entry.read_to_end(&mut content).is_ok() {
                Some(content)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if let Some(content) = old_data {
        wrapper.additions.insert(new_name, content);
        wrapper.deletions.insert(name);
        return Ok(vm.arena.alloc(Val::Bool(true)));
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_zip_archive_get_from_index(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_zip_archive_get_from_name(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("ZipArchive::getFromName() expects at least 1 parameter".into());
    }

    let name = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("ZipArchive::getFromName(): Argument #1 (name) must be string".into()),
    };

    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::getFromName")?;
    let wrapper = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper.borrow_mut();

    if let Some(reader) = &mut wrapper.reader {
        match reader.by_name(&name) {
            Ok(mut file) => {
                let mut content = Vec::new();
                use std::io::Read;
                if file.read_to_end(&mut content).is_ok() {
                    return Ok(vm.arena.alloc(Val::String(Rc::new(content))));
                }
            }
            Err(_) => {}
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_zip_archive_get_name_index(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("ZipArchive::getNameIndex() expects 1 parameter".into());
    }

    let index = match &vm.arena.get(args[0]).value {
        Val::Int(i) => *i as usize,
        _ => return Err("ZipArchive::getNameIndex(): Argument #1 (index) must be integer".into()),
    };

    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::getNameIndex")?;
    let wrapper_rc = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper_rc.borrow_mut();

    let reader_len = wrapper.reader.as_ref().map(|r| r.len()).unwrap_or(0);
    if index < reader_len {
        if let Some(reader) = &mut wrapper.reader {
            if let Ok(file) = reader.by_index(index) {
                return Ok(vm
                    .arena
                    .alloc(Val::String(Rc::new(file.name().as_bytes().to_vec()))));
            }
        }
    } else {
        let addition_index = index - reader_len;
        if let Some((name, _)) = wrapper.additions.get_index(addition_index) {
            return Ok(vm
                .arena
                .alloc(Val::String(Rc::new(name.as_bytes().to_vec()))));
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_zip_archive_get_status_string(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::String(b"No error".to_vec().into())))
}

pub fn php_zip_archive_locate_name(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("ZipArchive::locateName() expects at least 1 parameter".into());
    }

    let name = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("ZipArchive::locateName(): Argument #1 (name) must be string".into()),
    };

    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::locateName")?;
    let wrapper_rc = get_zip_wrapper(vm, this_handle)?;
    let wrapper = wrapper_rc.borrow();

    if let Some(reader) = &wrapper.reader {
        if let Some(index) = reader.index_for_name(&name) {
            return Ok(vm.arena.alloc(Val::Int(index as i64)));
        }
    }

    let reader_len = wrapper.reader.as_ref().map(|r| r.len()).unwrap_or(0);
    if let Some(index) = wrapper.additions.get_index_of(&name) {
        return Ok(vm.arena.alloc(Val::Int((reader_len + index) as i64)));
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_zip_archive_stat_index(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("ZipArchive::statIndex() expects 1 parameter".into());
    }

    let index = match &vm.arena.get(args[0]).value {
        Val::Int(i) => *i as usize,
        _ => return Err("ZipArchive::statIndex(): Argument #1 (index) must be integer".into()),
    };

    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::statIndex")?;
    let wrapper = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper.borrow_mut();

    if let Some(reader) = &mut wrapper.reader {
        if let Ok(file) = reader.by_index(index) {
            let mut map = IndexMap::new();
            map.insert(
                ArrayKey::Str(Rc::new(b"name".to_vec())),
                vm.arena
                    .alloc(Val::String(Rc::new(file.name().as_bytes().to_vec()))),
            );
            map.insert(
                ArrayKey::Str(Rc::new(b"index".to_vec())),
                vm.arena.alloc(Val::Int(index as i64)),
            );
            map.insert(
                ArrayKey::Str(Rc::new(b"crc".to_vec())),
                vm.arena.alloc(Val::Int(file.crc32() as i64)),
            );
            map.insert(
                ArrayKey::Str(Rc::new(b"size".to_vec())),
                vm.arena.alloc(Val::Int(file.size() as i64)),
            );
            map.insert(
                ArrayKey::Str(Rc::new(b"comp_size".to_vec())),
                vm.arena.alloc(Val::Int(file.compressed_size() as i64)),
            );
            map.insert(
                ArrayKey::Str(Rc::new(b"mtime".to_vec())),
                vm.arena.alloc(Val::Int(0)),
            );
            map.insert(
                ArrayKey::Str(Rc::new(b"comp_method".to_vec())),
                vm.arena.alloc(Val::Int(0)),
            );

            return Ok(vm
                .arena
                .alloc(Val::Array(Rc::new(ArrayData { map, next_free: 0 }))));
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_zip_archive_stat_name(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("ZipArchive::statName() expects 1 parameter".into());
    }

    let name = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("ZipArchive::statName(): Argument #1 (name) must be string".into()),
    };

    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::statName")?;
    let wrapper = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper.borrow_mut();

    if let Some(reader) = &mut wrapper.reader {
        if let Some(index) = reader.index_for_name(&name) {
            if let Ok(file) = reader.by_index(index) {
                let mut map = IndexMap::new();
                map.insert(
                    ArrayKey::Str(Rc::new(b"name".to_vec())),
                    vm.arena
                        .alloc(Val::String(Rc::new(file.name().as_bytes().to_vec()))),
                );
                map.insert(
                    ArrayKey::Str(Rc::new(b"index".to_vec())),
                    vm.arena.alloc(Val::Int(index as i64)),
                );
                map.insert(
                    ArrayKey::Str(Rc::new(b"crc".to_vec())),
                    vm.arena.alloc(Val::Int(file.crc32() as i64)),
                );
                map.insert(
                    ArrayKey::Str(Rc::new(b"size".to_vec())),
                    vm.arena.alloc(Val::Int(file.size() as i64)),
                );
                map.insert(
                    ArrayKey::Str(Rc::new(b"comp_size".to_vec())),
                    vm.arena.alloc(Val::Int(file.compressed_size() as i64)),
                );
                map.insert(
                    ArrayKey::Str(Rc::new(b"mtime".to_vec())),
                    vm.arena.alloc(Val::Int(0)),
                );
                map.insert(
                    ArrayKey::Str(Rc::new(b"comp_method".to_vec())),
                    vm.arena.alloc(Val::Int(0)),
                );

                return Ok(vm
                    .arena
                    .alloc(Val::Array(Rc::new(ArrayData { map, next_free: 0 }))));
            }
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_zip_archive_unchange_all(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::unchangeAll")?;
    let wrapper = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper.borrow_mut();

    wrapper.additions.clear();
    wrapper.deletions.clear();

    // Update properties
    update_zip_properties(vm, this_handle, &wrapper)?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_zip_archive_set_password(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("ZipArchive::setPassword() expects 1 parameter".into());
    }

    let password = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("ZipArchive::setPassword(): Argument #1 (password) must be string".into()),
    };

    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in ZipArchive::setPassword")?;
    let wrapper = get_zip_wrapper(vm, this_handle)?;
    let mut wrapper = wrapper.borrow_mut();

    wrapper.password = Some(password);

    Ok(vm.arena.alloc(Val::Bool(true)))
}

// Procedural functions
pub fn php_zip_open(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("zip_open() expects 1 parameter".into());
    }

    let filename = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("zip_open(): Argument #1 (filename) must be string".into()),
    };

    let path = Path::new(&filename);
    if !path.exists() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    match File::open(path) {
        Ok(file) => match ZipArchive::new(file) {
            Ok(archive) => {
                let mut wrapper = ZipArchiveWrapper::new();
                wrapper.path = filename;
                wrapper.reader = Some(archive);

                let resource_id = vm.context.next_resource_id;
                vm.context.next_resource_id += 1;
                vm.context
                    .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).resources
                    .insert(resource_id, Rc::new(RefCell::new(wrapper)));

                Ok(vm.arena.alloc(Val::Resource(Rc::new(resource_id))))
            }
            Err(_) => Ok(vm.arena.alloc(Val::Bool(false))),
        },
        Err(_) => Ok(vm.arena.alloc(Val::Bool(false))),
    }
}

pub fn php_zip_close(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("zip_close() expects 1 parameter".into());
    }

    let resource_id = match &vm.arena.get(args[0]).value {
        Val::Resource(id) => *id.downcast_ref::<u64>().ok_or("Invalid resource type")?,
        _ => return Err("zip_close(): Argument #1 must be a zip resource".into()),
    };

    vm.context.get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).resources.remove(&resource_id);

    Ok(vm.arena.alloc(Val::Null))
}

pub fn php_zip_read(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("zip_read() expects 1 parameter".into());
    }

    let resource_id = match &vm.arena.get(args[0]).value {
        Val::Resource(id) => *id.downcast_ref::<u64>().ok_or("Invalid resource type")?,
        _ => return Err("zip_read(): Argument #1 must be a zip resource".into()),
    };

    let wrapper_rc = vm
        .context
        .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).resources
        .get(&resource_id)
        .cloned()
        .ok_or("Invalid zip resource")?;
    let mut wrapper = wrapper_rc.borrow_mut();

    if let Some(reader) = &wrapper.reader {
        if wrapper.current_entry_index < reader.len() {
            let entry_index = wrapper.current_entry_index;
            wrapper.current_entry_index += 1;

            let entry_id = vm.context.next_resource_id;
            vm.context.next_resource_id += 1;
            vm.context
                .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).entries
                .insert(entry_id, (resource_id, entry_index));

            return Ok(vm.arena.alloc(Val::Resource(Rc::new(entry_id))));
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_zip_entry_open(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_zip_entry_close(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_zip_entry_read(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("zip_entry_read() expects at least 1 parameter".into());
    }

    let entry_id = match &vm.arena.get(args[0]).value {
        Val::Resource(id) => *id.downcast_ref::<u64>().ok_or("Invalid resource type")?,
        _ => return Err("zip_entry_read(): Argument #1 must be a zip entry resource".into()),
    };

    let (resource_id, entry_index) = vm
        .context
        .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).entries
        .get(&entry_id)
        .cloned()
        .ok_or("Invalid zip entry resource")?;
    let wrapper_rc = vm
        .context
        .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).resources
        .get(&resource_id)
        .cloned()
        .ok_or("Zip resource not found")?;
    let mut wrapper = wrapper_rc.borrow_mut();

    if let Some(reader) = &mut wrapper.reader {
        if let Ok(mut entry) = reader.by_index(entry_index) {
            let mut content = Vec::new();
            use std::io::Read;
            if entry.read_to_end(&mut content).is_ok() {
                return Ok(vm.arena.alloc(Val::String(Rc::new(content))));
            }
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_zip_entry_name(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("zip_entry_name() expects 1 parameter".into());
    }

    let entry_id = match &vm.arena.get(args[0]).value {
        Val::Resource(id) => *id.downcast_ref::<u64>().ok_or("Invalid resource type")?,
        _ => return Err("zip_entry_name(): Argument #1 must be a zip entry resource".into()),
    };

    let (resource_id, entry_index) = vm
        .context
        .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).entries
        .get(&entry_id)
        .cloned()
        .ok_or("Invalid zip entry resource")?;
    let wrapper_rc = vm
        .context
        .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).resources
        .get(&resource_id)
        .cloned()
        .ok_or("Zip resource not found")?;
    let mut wrapper = wrapper_rc.borrow_mut();

    if let Some(reader) = &mut wrapper.reader {
        if let Ok(entry) = reader.by_index(entry_index) {
            return Ok(vm
                .arena
                .alloc(Val::String(Rc::new(entry.name().as_bytes().to_vec()))));
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_zip_entry_filesize(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("zip_entry_filesize() expects 1 parameter".into());
    }

    let entry_id = match &vm.arena.get(args[0]).value {
        Val::Resource(id) => *id.downcast_ref::<u64>().ok_or("Invalid resource type")?,
        _ => return Err("zip_entry_filesize(): Argument #1 must be a zip entry resource".into()),
    };

    let (resource_id, entry_index) = vm
        .context
        .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).entries
        .get(&entry_id)
        .cloned()
        .ok_or("Invalid zip entry resource")?;
    let wrapper_rc = vm
        .context
        .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).resources
        .get(&resource_id)
        .cloned()
        .ok_or("Zip resource not found")?;
    let mut wrapper = wrapper_rc.borrow_mut();

    if let Some(reader) = &mut wrapper.reader {
        if let Ok(entry) = reader.by_index(entry_index) {
            return Ok(vm.arena.alloc(Val::Int(entry.size() as i64)));
        }
    }

    Ok(vm.arena.alloc(Val::Int(0)))
}

pub fn php_zip_entry_compressionmethod(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("zip_entry_compressionmethod() expects 1 parameter".into());
    }

    let entry_id = match &vm.arena.get(args[0]).value {
        Val::Resource(id) => *id.downcast_ref::<u64>().ok_or("Invalid resource type")?,
        _ => {
            return Err(
                "zip_entry_compressionmethod(): Argument #1 must be a zip entry resource".into(),
            )
        }
    };

    let (resource_id, entry_index) = vm
        .context
        .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).entries
        .get(&entry_id)
        .cloned()
        .ok_or("Invalid zip entry resource")?;
    let wrapper_rc = vm
        .context
        .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).resources
        .get(&resource_id)
        .cloned()
        .ok_or("Zip resource not found")?;
    let mut wrapper = wrapper_rc.borrow_mut();

    if let Some(reader) = &mut wrapper.reader {
        if let Ok(entry) = reader.by_index(entry_index) {
            return Ok(vm.arena.alloc(Val::String(
                format!("{:?}", entry.compression()).into_bytes().into(),
            )));
        }
    }

    Ok(vm.arena.alloc(Val::String(b"stored".to_vec().into())))
}

pub fn php_zip_entry_compressedsize(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("zip_entry_compressedsize() expects 1 parameter".into());
    }

    let entry_id = match &vm.arena.get(args[0]).value {
        Val::Resource(id) => *id.downcast_ref::<u64>().ok_or("Invalid resource type")?,
        _ => {
            return Err(
                "zip_entry_compressedsize(): Argument #1 must be a zip entry resource".into(),
            )
        }
    };

    let (resource_id, entry_index) = vm
        .context
        .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).entries
        .get(&entry_id)
        .cloned()
        .ok_or("Invalid zip entry resource")?;
    let wrapper_rc = vm
        .context
        .get_or_init_extension_data(|| crate::runtime::zip_extension::ZipExtensionData::default()).resources
        .get(&resource_id)
        .cloned()
        .ok_or("Zip resource not found")?;
    let mut wrapper = wrapper_rc.borrow_mut();

    if let Some(reader) = &mut wrapper.reader {
        if let Ok(entry) = reader.by_index(entry_index) {
            return Ok(vm.arena.alloc(Val::Int(entry.compressed_size() as i64)));
        }
    }

    Ok(vm.arena.alloc(Val::Int(0)))
}
