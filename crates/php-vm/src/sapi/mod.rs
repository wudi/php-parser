//! SAPI (Server API) adapters for different execution modes.
//!
//! Maps external request sources (CLI args, FastCGI params, etc.) to RequestContext.

pub mod fpm;

use crate::core::value::{ArrayData, ArrayKey, Handle, Val};
use crate::vm::engine::VM;
use std::collections::HashMap;
use std::rc::Rc;

/// SAPI name (for PHP_SAPI constant)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SapiMode {
    Cli,
    FpmFcgi,
}

impl SapiMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::FpmFcgi => "fpm-fcgi",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileUpload {
    pub name: String,
    pub type_: String,
    pub tmp_name: String,
    pub error: i32,
    pub size: u64,
}

/// Initialize/override superglobals ($_SERVER, $_ENV, $_GET, $_POST, etc.) in VM.
/// Call this after VM::new() to populate superglobals with request data.
pub fn init_superglobals(
    vm: &mut VM,
    sapi_mode: SapiMode,
    server_vars: HashMap<Vec<u8>, Vec<u8>>,
    env_vars: HashMap<Vec<u8>, Vec<u8>>,
    get_vars: HashMap<Vec<u8>, Vec<u8>>,
    post_vars: HashMap<Vec<u8>, Vec<u8>>,
    cookie_vars: HashMap<Vec<u8>, Vec<u8>>,
    files_vars: HashMap<Vec<u8>, FileUpload>,
) {
    // Override PHP_SAPI constant
    let sapi_val = Val::String(Rc::new(sapi_mode.as_str().as_bytes().to_vec()));
    vm.context.insert_builtin_constant(b"PHP_SAPI", sapi_val);

    // Helper to build an array from key-value byte map
    let build_array = |vm: &mut VM, vars: HashMap<Vec<u8>, Vec<u8>>| -> Handle {
        let mut data = ArrayData::new();
        for (key, value) in vars {
            let key_arr = ArrayKey::Str(Rc::new(key));
            let val_handle = vm.arena.alloc(Val::String(Rc::new(value)));
            data.insert(key_arr, val_handle);
        }
        let handle = vm.arena.alloc(Val::Array(Rc::new(data)));
        vm.arena.get_mut(handle).is_ref = true; // Superglobals are references
        handle
    };

    // $_SERVER
    let server_handle = build_array(vm, server_vars);
    let server_sym = vm.context.interner.intern(b"_SERVER");
    vm.context.globals.insert(server_sym, server_handle);

    // $_ENV
    let env_handle = build_array(vm, env_vars);
    let env_sym = vm.context.interner.intern(b"_ENV");
    vm.context.globals.insert(env_sym, env_handle);

    // $_GET
    let get_handle = build_array(vm, get_vars);
    let get_sym = vm.context.interner.intern(b"_GET");
    vm.context.globals.insert(get_sym, get_handle);

    // $_POST
    let post_handle = build_array(vm, post_vars);
    let post_sym = vm.context.interner.intern(b"_POST");
    vm.context.globals.insert(post_sym, post_handle);

    // $_FILES
    let mut files_data = ArrayData::new();
    for (key, file) in files_vars {
        let key_arr = ArrayKey::Str(Rc::new(key));
        
        let mut file_arr = ArrayData::new();
        
        file_arr.insert(
            ArrayKey::Str(Rc::new(b"name".to_vec())),
            vm.arena.alloc(Val::String(Rc::new(file.name.into_bytes()))),
        );
        file_arr.insert(
            ArrayKey::Str(Rc::new(b"type".to_vec())),
            vm.arena.alloc(Val::String(Rc::new(file.type_.into_bytes()))),
        );
        file_arr.insert(
            ArrayKey::Str(Rc::new(b"tmp_name".to_vec())),
            vm.arena.alloc(Val::String(Rc::new(file.tmp_name.into_bytes()))),
        );
        file_arr.insert(
            ArrayKey::Str(Rc::new(b"error".to_vec())),
            vm.arena.alloc(Val::Int(file.error as i64)),
        );
        file_arr.insert(
            ArrayKey::Str(Rc::new(b"size".to_vec())),
            vm.arena.alloc(Val::Int(file.size as i64)),
        );

        let file_handle = vm.arena.alloc(Val::Array(Rc::new(file_arr)));
        files_data.insert(key_arr, file_handle);
    }
    let files_handle = vm.arena.alloc(Val::Array(Rc::new(files_data)));
    vm.arena.get_mut(files_handle).is_ref = true;
    let files_sym = vm.context.interner.intern(b"_FILES");
    vm.context.globals.insert(files_sym, files_handle);

    // $_COOKIE
    let cookie_handle = build_array(vm, cookie_vars);
    let cookie_sym = vm.context.interner.intern(b"_COOKIE");
    vm.context.globals.insert(cookie_sym, cookie_handle);

    // $_REQUEST (merge of GET + POST + COOKIE)
    let mut request_data = ArrayData::new();
    
    // Add GET
    if let Val::Array(arr) = &vm.arena.get(get_handle).value {
        for (k, v) in &arr.map {
            request_data.insert(k.clone(), *v);
        }
    }
    
    // Add POST
    if let Val::Array(arr) = &vm.arena.get(post_handle).value {
        for (k, v) in &arr.map {
            request_data.insert(k.clone(), *v);
        }
    }
    
    // Add COOKIE
    if let Val::Array(arr) = &vm.arena.get(cookie_handle).value {
        for (k, v) in &arr.map {
            request_data.insert(k.clone(), *v);
        }
    }
    
    let request_handle = vm.arena.alloc(Val::Array(Rc::new(request_data)));
    vm.arena.get_mut(request_handle).is_ref = true;
    let request_sym = vm.context.interner.intern(b"_REQUEST");
    vm.context.globals.insert(request_sym, request_handle);

    // Update $GLOBALS
    let globals_sym = vm.context.interner.intern(b"GLOBALS");
    let mut globals_data = ArrayData::new();
    for (sym, handle) in &vm.context.globals {
        let key_bytes = vm.context.interner.lookup(*sym).unwrap_or(b"");
        if key_bytes != b"GLOBALS" {
            globals_data.insert(ArrayKey::Str(Rc::new(key_bytes.to_vec())), *handle);
        }
    }
    let globals_handle = vm.arena.alloc(Val::Array(Rc::new(globals_data)));
    vm.arena.get_mut(globals_handle).is_ref = true;
    vm.context.globals.insert(globals_sym, globals_handle);
}
