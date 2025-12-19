//! Superglobal management
//!
//! Encapsulates creation and synchronization of PHP superglobal variables.
//! Reference: $PHP_SRC_PATH/main/php_variables.c - php_hash_environment
//!
//! ## Superglobals
//!
//! - $_SERVER: Server and execution environment information
//! - $_GET: HTTP GET variables
//! - $_POST: HTTP POST variables
//! - $_FILES: HTTP File Upload variables
//! - $_COOKIE: HTTP Cookies
//! - $_REQUEST: HTTP Request variables
//! - $_ENV: Environment variables
//! - $_SESSION: Session variables
//! - $GLOBALS: References to all variables available in global scope

use crate::core::value::{ArrayData, ArrayKey, Handle, Symbol, Val};
use crate::vm::engine::VM;
use indexmap::IndexMap;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum SuperglobalKind {
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

pub(crate) struct SuperglobalManager {
    pub(crate) map: HashMap<Symbol, SuperglobalKind>,
}

impl SuperglobalManager {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    /// Register all superglobal symbols
    pub fn register_symbols(&mut self, vm: &mut VM) {
        for (kind, name) in SUPERGLOBAL_SPECS {
            let sym = vm.context.interner.intern(name);
            self.map.insert(sym, *kind);
        }
    }

    /// Initialize all superglobals
    pub fn initialize_all(&self, vm: &mut VM) {
        let entries: Vec<(Symbol, SuperglobalKind)> =
            self.map.iter().map(|(&sym, &kind)| (sym, kind)).collect();

        for (sym, kind) in entries {
            if !vm.context.globals.contains_key(&sym) {
                let handle = self.create_superglobal(vm, kind);
                vm.arena.get_mut(handle).is_ref = true;
                vm.context.globals.insert(sym, handle);
            }
        }
    }

    /// Create a specific superglobal
    pub fn create_superglobal(&self, vm: &mut VM, kind: SuperglobalKind) -> Handle {
        match kind {
            SuperglobalKind::Server => self.create_server(vm),
            SuperglobalKind::Globals => self.create_globals(vm),
            _ => vm.arena.alloc(Val::Array(Rc::new(ArrayData::new()))),
        }
    }

    /// Create $_SERVER superglobal
    fn create_server(&self, vm: &mut VM) -> Handle {
        let mut data = ArrayData::new();

        // Helper to insert string values
        let insert_str = |data: &mut ArrayData, vm: &mut VM, key: &[u8], val: &[u8]| {
            let handle = vm.arena.alloc(Val::String(Rc::new(val.to_vec())));
            data.insert(ArrayKey::Str(Rc::new(key.to_vec())), handle);
        };

        // HTTP protocol information
        insert_str(&mut data, vm, b"SERVER_PROTOCOL", b"HTTP/1.1");
        insert_str(&mut data, vm, b"REQUEST_METHOD", b"GET");
        insert_str(&mut data, vm, b"HTTP_HOST", b"localhost");
        insert_str(&mut data, vm, b"SERVER_NAME", b"localhost");
        insert_str(&mut data, vm, b"SERVER_SOFTWARE", b"php-vm");
        insert_str(&mut data, vm, b"SERVER_ADDR", b"127.0.0.1");
        insert_str(&mut data, vm, b"REMOTE_ADDR", b"127.0.0.1");

        // Numeric values
        data.insert(
            ArrayKey::Str(Rc::new(b"REMOTE_PORT".to_vec())),
            vm.arena.alloc(Val::Int(0)),
        );
        data.insert(
            ArrayKey::Str(Rc::new(b"SERVER_PORT".to_vec())),
            vm.arena.alloc(Val::Int(80)),
        );

        // Request information
        insert_str(&mut data, vm, b"REQUEST_SCHEME", b"http");
        insert_str(&mut data, vm, b"HTTPS", b"off");
        insert_str(&mut data, vm, b"QUERY_STRING", b"");
        insert_str(&mut data, vm, b"REQUEST_URI", b"/");
        insert_str(&mut data, vm, b"PATH_INFO", b"");
        insert_str(&mut data, vm, b"ORIG_PATH_INFO", b"");

        // Script paths
        let (doc_root, script_name, script_filename) = self.compute_script_paths();
        insert_str(&mut data, vm, b"DOCUMENT_ROOT", doc_root.as_bytes());
        insert_str(&mut data, vm, b"SCRIPT_NAME", script_name.as_bytes());
        insert_str(&mut data, vm, b"PHP_SELF", script_name.as_bytes());
        insert_str(&mut data, vm, b"SCRIPT_FILENAME", script_filename.as_bytes());

        // Timing information
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();

        data.insert(
            ArrayKey::Str(Rc::new(b"REQUEST_TIME".to_vec())),
            vm.arena.alloc(Val::Int(now.as_secs() as i64)),
        );
        data.insert(
            ArrayKey::Str(Rc::new(b"REQUEST_TIME_FLOAT".to_vec())),
            vm.arena.alloc(Val::Float(now.as_secs_f64())),
        );

        vm.arena.alloc(Val::Array(Rc::new(data)))
    }

    /// Create $GLOBALS superglobal
    fn create_globals(&self, vm: &mut VM) -> Handle {
        let mut map = IndexMap::new();

        // Include variables from context.globals (superglobals and 'global' keyword vars)
        for (sym, handle) in &vm.context.globals {
            let key_bytes = vm.context.interner.lookup(*sym).unwrap_or(b"");
            if key_bytes != b"GLOBALS" {
                map.insert(ArrayKey::Str(Rc::new(key_bytes.to_vec())), *handle);
            }
        }

        // Include variables from the top-level frame
        if let Some(frame) = vm.frames.first() {
            for (sym, handle) in &frame.locals {
                let key_bytes = vm.context.interner.lookup(*sym).unwrap_or(b"");
                if key_bytes != b"GLOBALS" {
                    let key = ArrayKey::Str(Rc::new(key_bytes.to_vec()));
                    map.entry(key).or_insert(*handle);
                }
            }
        }

        vm.arena.alloc(Val::Array(ArrayData::from(map).into()))
    }

    /// Compute script paths for $_SERVER
    fn compute_script_paths(&self) -> (String, String, String) {
        let document_root = std::env::current_dir()
            .ok()
            .and_then(|p| p.to_str().map(String::from))
            .unwrap_or_else(|| ".".into());

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

        (document_root, script_name, script_filename)
    }

    /// Check if a symbol is a superglobal
    pub fn is_superglobal(&self, sym: Symbol) -> bool {
        self.map.contains_key(&sym)
    }

    /// Check if a symbol is $GLOBALS
    pub fn is_globals(&self, sym: Symbol) -> bool {
        self.map.get(&sym) == Some(&SuperglobalKind::Globals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::context::EngineContext;
    use std::sync::Arc;

    #[test]
    fn test_superglobal_registration() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);
        let mut mgr = SuperglobalManager::new();

        mgr.register_symbols(&mut vm);

        let server_sym = vm.context.interner.intern(b"_SERVER");
        assert!(mgr.is_superglobal(server_sym));

        let globals_sym = vm.context.interner.intern(b"GLOBALS");
        assert!(mgr.is_globals(globals_sym));
    }

    #[test]
    fn test_server_creation() {
        let engine = Arc::new(EngineContext::new());
        let mut vm = VM::new(engine);
        let mgr = SuperglobalManager::new();

        let handle = mgr.create_server(&mut vm);
        let val = &vm.arena.get(handle).value;

        if let Val::Array(arr) = val {
            let protocol_key = ArrayKey::Str(Rc::new(b"SERVER_PROTOCOL".to_vec()));
            assert!(arr.map.contains_key(&protocol_key));
        } else {
            panic!("Expected array");
        }
    }
}
