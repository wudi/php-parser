//! PDO Extension - PHP Data Objects
//!
//! This module implements PHP's PDO extension with the following features:
//! - Unified database abstraction layer
//! - Multiple driver support (SQLite, MySQL, etc.)
//! - Prepared statements with parameter binding
//! - Transaction management
//! - Flexible fetch modes
//!
//! # Architecture
//!
//! - **Trait-Based Abstraction**: PdoDriver, PdoConnection, PdoStatement traits
//! - **Static Driver Registry**: All drivers compiled in (no dynamic loading)
//! - **Type Safety**: Rust traits ensure compile-time correctness
//! - **Zero-Heap AST**: All allocations via Arena
//! - **No Panics**: All errors return Result
//!
//! # References
//!
//! - PHP Source: $PHP_SRC_PATH/ext/pdo/
//! - PDO Driver API: $PHP_SRC_PATH/ext/pdo/php_pdo_driver.h
//! - SQLite Driver: $PHP_SRC_PATH/ext/pdo_sqlite/

pub mod driver;
pub mod drivers;
#[cfg(test)]
mod tests;
pub mod types;

use crate::core::value::{ArrayData, ArrayKey, Handle, ObjectData, Val, Visibility};
use crate::runtime::registry::{ExtensionRegistry, NativeClassDef, NativeMethodEntry};
use crate::vm::engine::{PropertyCollectionMode, VM};
use drivers::DriverRegistry;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use types::{Attribute, ParamIdentifier, ParamType, PdoValue};

/// Register the PDO extension components to the registry
pub fn register_pdo_extension_to_registry(registry: &mut ExtensionRegistry) {
    // 1. Register PDO Class
    let mut pdo_methods = HashMap::new();

    pdo_methods.insert(
        b"__construct".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_construct,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    pdo_methods.insert(
        b"prepare".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_prepare,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    pdo_methods.insert(
        b"exec".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_exec,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    pdo_methods.insert(
        b"beginTransaction".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_begin_transaction,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    pdo_methods.insert(
        b"commit".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_commit,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    pdo_methods.insert(
        b"rollBack".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_rollback,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    pdo_methods.insert(
        b"inTransaction".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_in_transaction,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    pdo_methods.insert(
        b"lastInsertId".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_last_insert_id,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    pdo_methods.insert(
        b"setAttribute".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_set_attribute,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    pdo_methods.insert(
        b"getAttribute".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_get_attribute,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    pdo_methods.insert(
        b"query".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_query,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    pdo_methods.insert(
        b"errorCode".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_error_code,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    pdo_methods.insert(
        b"errorInfo".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_error_info,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    let mut pdo_constants = HashMap::new();
    pdo_constants.insert(b"PARAM_NULL".to_vec(), (Val::Int(0), Visibility::Public));
    pdo_constants.insert(b"PARAM_INT".to_vec(), (Val::Int(1), Visibility::Public));
    pdo_constants.insert(b"PARAM_STR".to_vec(), (Val::Int(2), Visibility::Public));
    pdo_constants.insert(b"PARAM_LOB".to_vec(), (Val::Int(3), Visibility::Public));
    pdo_constants.insert(b"PARAM_STMT".to_vec(), (Val::Int(4), Visibility::Public));
    pdo_constants.insert(b"PARAM_BOOL".to_vec(), (Val::Int(5), Visibility::Public));

    pdo_constants.insert(b"FETCH_ASSOC".to_vec(), (Val::Int(2), Visibility::Public));
    pdo_constants.insert(b"FETCH_NUM".to_vec(), (Val::Int(3), Visibility::Public));
    pdo_constants.insert(b"FETCH_BOTH".to_vec(), (Val::Int(4), Visibility::Public));
    pdo_constants.insert(b"FETCH_OBJ".to_vec(), (Val::Int(5), Visibility::Public));
    pdo_constants.insert(b"FETCH_BOUND".to_vec(), (Val::Int(6), Visibility::Public));
    pdo_constants.insert(b"FETCH_COLUMN".to_vec(), (Val::Int(7), Visibility::Public));
    pdo_constants.insert(b"FETCH_CLASS".to_vec(), (Val::Int(8), Visibility::Public));

    pdo_constants.insert(
        b"ERRMODE_SILENT".to_vec(),
        (Val::Int(0), Visibility::Public),
    );
    pdo_constants.insert(
        b"ERRMODE_WARNING".to_vec(),
        (Val::Int(1), Visibility::Public),
    );
    pdo_constants.insert(
        b"ERRMODE_EXCEPTION".to_vec(),
        (Val::Int(2), Visibility::Public),
    );

    pdo_constants.insert(
        b"ATTR_AUTOCOMMIT".to_vec(),
        (Val::Int(0), Visibility::Public),
    );
    pdo_constants.insert(b"ATTR_ERRMODE".to_vec(), (Val::Int(3), Visibility::Public));
    pdo_constants.insert(
        b"ATTR_CLIENT_VERSION".to_vec(),
        (Val::Int(5), Visibility::Public),
    );
    pdo_constants.insert(
        b"ATTR_SERVER_VERSION".to_vec(),
        (Val::Int(4), Visibility::Public),
    );
    pdo_constants.insert(
        b"ATTR_STATEMENT_CLASS".to_vec(),
        (Val::Int(13), Visibility::Public),
    );
    pdo_constants.insert(
        b"ATTR_DEFAULT_FETCH_MODE".to_vec(),
        (Val::Int(19), Visibility::Public),
    );
    pdo_constants.insert(
        b"ATTR_EMULATE_PREPARES".to_vec(),
        (Val::Int(20), Visibility::Public),
    );

    registry.register_class(NativeClassDef {
        name: b"PDO".to_vec(),
        parent: None,
        interfaces: Vec::new(),
        methods: pdo_methods,
        constants: pdo_constants,
        constructor: None, // Used __construct method instead
    });

    // 2. Register PDOStatement Class
    let mut st_methods = HashMap::new();

    st_methods.insert(
        b"execute".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_stmt_execute,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    st_methods.insert(
        b"fetch".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_stmt_fetch,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    st_methods.insert(
        b"fetchAll".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_stmt_fetch_all,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    st_methods.insert(
        b"rowCount".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_stmt_row_count,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    st_methods.insert(
        b"columnCount".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_stmt_column_count,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    st_methods.insert(
        b"errorCode".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_stmt_error_code,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    st_methods.insert(
        b"errorInfo".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_stmt_error_info,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    st_methods.insert(
        b"rowCount".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_stmt_row_count,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    st_methods.insert(
        b"columnCount".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_stmt_column_count,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    st_methods.insert(
        b"bindParam".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_stmt_bind_param,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    st_methods.insert(
        b"bindValue".to_vec(),
        NativeMethodEntry {
            handler: php_pdo_stmt_bind_value,
            visibility: Visibility::Public,
            is_static: false,
        },
    );

    registry.register_class(NativeClassDef {
        name: b"PDOStatement".to_vec(),
        parent: None,
        interfaces: vec![b"Iterator".to_vec()],
        methods: st_methods,
        constants: HashMap::new(),
        constructor: None,
    });

    // 3. Register PDOException Class
    registry.register_class(NativeClassDef {
        name: b"PDOException".to_vec(),
        parent: Some(b"Exception".to_vec()),
        interfaces: Vec::new(),
        methods: HashMap::new(),
        constants: HashMap::new(),
        constructor: None,
    });

    // 4. Register Constants
    register_pdo_constants(registry);
}

/// Helper to get connection ID from PDO object
fn get_pdo_connection_id(vm: &VM, handle: Handle) -> Result<u64, String> {
    let obj_handle = match &vm.arena.get(handle).value {
        Val::Object(h) => *h,
        _ => return Err("Expected PDO object".into()),
    };

    let obj_payload = vm.arena.get(obj_handle);
    let obj = match &obj_payload.value {
        Val::ObjPayload(o) => o,
        _ => return Err("Expected Object payload".into()),
    };

    let id_sym = vm
        .context
        .interner
        .find(b"__id")
        .ok_or("Property __id not found")?;
    let id_val = obj
        .properties
        .get(&id_sym)
        .ok_or("PDO object not initialized")?;

    match &vm.arena.get(*id_val).value {
        Val::Int(id) => Ok(*id as u64),
        _ => Err("Invalid PDO connection ID".into()),
    }
}

/// Helper to get statement ID from PDOStatement object
fn get_pdo_statement_id(vm: &VM, handle: Handle) -> Result<u64, String> {
    let obj_handle = match &vm.arena.get(handle).value {
        Val::Object(h) => *h,
        _ => return Err("Expected PDOStatement object".into()),
    };

    let obj_payload = vm.arena.get(obj_handle);
    let obj = match &obj_payload.value {
        Val::ObjPayload(o) => o,
        _ => return Err("Expected Object payload".into()),
    };

    let id_sym = vm
        .context
        .interner
        .find(b"__id")
        .ok_or("Property __id not found")?;
    let id_val = obj
        .properties
        .get(&id_sym)
        .ok_or("PDOStatement object not initialized")?;

    match &vm.arena.get(*id_val).value {
        Val::Int(id) => Ok(*id as u64),
        _ => Err("Invalid PDO statement ID".into()),
    }
}

// --- PDO Native Methods ---

/// PDO::__construct(string $dsn, ?string $username = null, ?string $password = null, ?array $options = null)
pub fn php_pdo_construct(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("PDO::__construct() expects at least 1 parameter".into());
    }

    let dsn = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("PDO::__construct(): DSN must be a string".into()),
    };

    let username = if args.len() > 1 {
        match &vm.arena.get(args[1]).value {
            Val::String(s) => Some(String::from_utf8_lossy(s).to_string()),
            Val::Null => None,
            _ => return Err("PDO::__construct(): Username must be a string or null".into()),
        }
    } else {
        None
    };

    let password = if args.len() > 2 {
        match &vm.arena.get(args[2]).value {
            Val::String(s) => Some(String::from_utf8_lossy(s).to_string()),
            Val::Null => None,
            _ => return Err("PDO::__construct(): Password must be a string or null".into()),
        }
    } else {
        None
    };

    let mut options = Vec::new();
    if args.len() > 3 {
        match &vm.arena.get(args[3]).value {
            Val::Array(arr) => {
                for (key, val) in arr.map.iter() {
                    let attr_id = match key {
                        ArrayKey::Int(i) => *i,
                        _ => continue,
                    };
                    if let Some(attr) = Attribute::from_i64(attr_id) {
                        options.push((attr, *val));
                    }
                }
            }
            Val::Null => {}
            _ => return Err("PDO::__construct(): Options must be an array or null".into()),
        }
    }

    // Parse DSN and connect
    let (driver_name, _conn_str) =
        DriverRegistry::parse_dsn(&dsn).map_err(|e| format!("PDO::__construct(): {}", e))?;

    let registry = vm
        .context
        .engine
        .pdo_driver_registry
        .as_ref()
        .ok_or("PDO driver registry not initialized")?;

    let driver = registry
        .get(driver_name)
        .ok_or_else(|| format!("PDO::__construct(): Driver '{}' not found", driver_name))?;

    let conn = driver
        .connect(&dsn, username.as_deref(), password.as_deref(), &options)
        .map_err(|e| format!("PDO::__construct(): Connection failed: {}", e))?;

    // Store connection in context
    let conn_id = vm.context.next_resource_id;
    vm.context.next_resource_id += 1;
    vm.context
        .pdo_connections
        .insert(conn_id, Rc::new(std::cell::RefCell::new(conn)));

    // Store ID in object
    if let Some(this_handle) = vm.frames.last().and_then(|f| f.this) {
        let obj_handle = match &vm.arena.get(this_handle).value {
            Val::Object(h) => *h,
            _ => return Err("No 'this' in PDO::__construct".into()),
        };

        let id_sym = vm.context.interner.intern(b"__id");
        let id_val = vm.arena.alloc(Val::Int(conn_id as i64));

        if let Val::ObjPayload(obj) = &mut vm.arena.get_mut(obj_handle).value {
            obj.properties.insert(id_sym, id_val);
        }
    }

    Ok(vm.arena.alloc(Val::Null))
}

/// PDO::prepare(string $query, array $options = [])
pub fn php_pdo_prepare(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in PDO::prepare")?;
    let conn_id = get_pdo_connection_id(vm, this_handle)?;

    if args.is_empty() {
        return Err("PDO::prepare() expects 1 parameter".into());
    }

    let query = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("PDO::prepare(): Query must be a string".into()),
    };

    let conn_ref = vm
        .context
        .pdo_connections
        .get(&conn_id)
        .cloned()
        .ok_or("PDO::prepare(): Invalid connection")?;

    let stmt = conn_ref
        .borrow_mut()
        .prepare(&query)
        .map_err(|e| format!("PDO::prepare(): {}", e))?;

    // Create PDOStatement object
    let stmt_class_sym = vm.context.interner.intern(b"PDOStatement");
    let properties = vm.collect_properties(stmt_class_sym, PropertyCollectionMode::All);
    let obj_data = ObjectData {
        class: stmt_class_sym,
        properties,
        internal: None,
        dynamic_properties: HashSet::new(),
    };
    let payload_handle = vm.arena.alloc(Val::ObjPayload(obj_data));
    let stmt_obj_handle = vm.arena.alloc(Val::Object(payload_handle));

    // Store statement in context
    let stmt_id = vm.context.next_resource_id;
    vm.context.next_resource_id += 1;
    vm.context
        .pdo_statements
        .insert(stmt_id, Rc::new(std::cell::RefCell::new(stmt)));

    // Store ID and default fetch mode in PDOStatement object
    let id_sym = vm.context.interner.intern(b"__id");
    let id_val = vm.arena.alloc(Val::Int(stmt_id as i64));
    let fetch_mode_sym = vm.context.interner.intern(b"fetchMode");
    let default_fetch_mode = conn_ref.borrow().get_attribute(Attribute::DefaultFetchMode);

    if let Val::ObjPayload(obj) = &mut vm.arena.get_mut(payload_handle).value {
        obj.properties.insert(id_sym, id_val);
        if let Some(mode) = default_fetch_mode {
            obj.properties.insert(fetch_mode_sym, mode);
        }
    }

    Ok(stmt_obj_handle)
}

/// PDO::exec(string $statement)
pub fn php_pdo_exec(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in PDO::exec")?;
    let conn_id = get_pdo_connection_id(vm, this_handle)?;

    if args.is_empty() {
        return Err("PDO::exec() expects 1 parameter".into());
    }

    let sql = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("PDO::exec(): Statement must be a string".into()),
    };

    let conn_ref = vm
        .context
        .pdo_connections
        .get(&conn_id)
        .ok_or("PDO::exec(): Invalid connection")?;

    let affected = conn_ref
        .borrow_mut()
        .exec(&sql)
        .map_err(|e| format!("PDO::exec(): {}", e))?;

    Ok(vm.arena.alloc(Val::Int(affected)))
}

pub fn php_pdo_begin_transaction(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm.frames.last().and_then(|f| f.this).ok_or("No 'this'")?;
    let conn_id = get_pdo_connection_id(vm, this_handle)?;
    let conn_ref = vm
        .context
        .pdo_connections
        .get(&conn_id)
        .ok_or("Invalid connection")?;
    conn_ref
        .borrow_mut()
        .begin_transaction()
        .map_err(|e| e.to_string())?;
    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_pdo_commit(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm.frames.last().and_then(|f| f.this).ok_or("No 'this'")?;
    let conn_id = get_pdo_connection_id(vm, this_handle)?;
    let conn_ref = vm
        .context
        .pdo_connections
        .get(&conn_id)
        .ok_or("Invalid connection")?;
    conn_ref.borrow_mut().commit().map_err(|e| e.to_string())?;
    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_pdo_rollback(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm.frames.last().and_then(|f| f.this).ok_or("No 'this'")?;
    let conn_id = get_pdo_connection_id(vm, this_handle)?;
    let conn_ref = vm
        .context
        .pdo_connections
        .get(&conn_id)
        .ok_or("Invalid connection")?;
    conn_ref
        .borrow_mut()
        .rollback()
        .map_err(|e| e.to_string())?;
    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_pdo_in_transaction(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm.frames.last().and_then(|f| f.this).ok_or("No 'this'")?;
    let conn_id = get_pdo_connection_id(vm, this_handle)?;
    let conn_ref = vm
        .context
        .pdo_connections
        .get(&conn_id)
        .ok_or("Invalid connection")?;
    let in_tx = conn_ref.borrow().in_transaction();
    Ok(vm.arena.alloc(Val::Bool(in_tx)))
}

pub fn php_pdo_last_insert_id(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm.frames.last().and_then(|f| f.this).ok_or("No 'this'")?;
    let conn_id = get_pdo_connection_id(vm, this_handle)?;
    let name = if !args.is_empty() {
        match &vm.arena.get(args[0]).value {
            Val::String(s) => Some(String::from_utf8_lossy(s).to_string()),
            _ => None,
        }
    } else {
        None
    };

    let conn_ref = vm
        .context
        .pdo_connections
        .get(&conn_id)
        .ok_or("Invalid connection")?;
    let id = conn_ref
        .borrow_mut()
        .last_insert_id(name.as_deref())
        .map_err(|e| e.to_string())?;
    Ok(vm.arena.alloc(Val::String(id.into_bytes().into())))
}

pub fn php_pdo_set_attribute(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("PDO::setAttribute() expects 2 parameters".into());
    }

    let attr_id = match &vm.arena.get(args[0]).value {
        Val::Int(i) => *i,
        _ => return Err("Attribute ID must be an integer".into()),
    };

    let attr = match Attribute::from_i64(attr_id) {
        Some(a) => a,
        None => return Err(format!("Unknown PDO attribute: {}", attr_id)),
    };

    let this_handle = vm.frames.last().and_then(|f| f.this).ok_or("No 'this'")?;
    let conn_id = get_pdo_connection_id(vm, this_handle)?;
    let conn_ref = vm
        .context
        .pdo_connections
        .get(&conn_id)
        .ok_or("Invalid connection")?;

    conn_ref
        .borrow_mut()
        .set_attribute(attr, args[1])
        .map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_pdo_get_attribute(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("PDO::getAttribute() expects 1 parameter".into());
    }

    let attr_id = match &vm.arena.get(args[0]).value {
        Val::Int(i) => *i,
        _ => return Err("Attribute ID must be an integer".into()),
    };

    let attr = match Attribute::from_i64(attr_id) {
        Some(a) => a,
        None => return Err(format!("Unknown PDO attribute: {}", attr_id)),
    };

    let this_handle = vm.frames.last().and_then(|f| f.this).ok_or("No 'this'")?;
    let conn_id = get_pdo_connection_id(vm, this_handle)?;
    let conn_ref = vm
        .context
        .pdo_connections
        .get(&conn_id)
        .ok_or("Invalid connection")?;

    match conn_ref.borrow().get_attribute(attr) {
        Some(handle) => Ok(handle),
        None => Ok(vm.arena.alloc(Val::Null)),
    }
}

pub fn php_pdo_query(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("PDO::query() expects at least 1 parameter".into());
    }

    // 1. Prepare
    let stmt = php_pdo_prepare(vm, &[args[0]])?;

    // 2. Execute (we need the statement ID to execute it)
    let stmt_id = get_pdo_statement_id(vm, stmt)?;
    let stmt_ref = vm
        .context
        .pdo_statements
        .get(&stmt_id)
        .ok_or("query(): Statement vanished")?;

    stmt_ref
        .borrow_mut()
        .execute(None)
        .map_err(|e| format!("PDO::query(): {}", e))?;

    Ok(stmt)
}

pub fn php_pdo_error_code(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm.frames.last().and_then(|f| f.this).ok_or("No 'this'")?;
    let conn_id = get_pdo_connection_id(vm, this_handle)?;
    let conn_ref = vm
        .context
        .pdo_connections
        .get(&conn_id)
        .ok_or("Invalid connection")?;
    let (state, _, _) = conn_ref.borrow().error_info();
    Ok(vm.arena.alloc(Val::String(Rc::new(state.into_bytes()))))
}

pub fn php_pdo_error_info(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm.frames.last().and_then(|f| f.this).ok_or("No 'this'")?;
    let conn_id = get_pdo_connection_id(vm, this_handle)?;
    let conn_ref = vm
        .context
        .pdo_connections
        .get(&conn_id)
        .ok_or("Invalid connection")?;
    let (state, code, msg) = conn_ref.borrow().error_info();

    let mut arr = ArrayData::new();
    arr.push(vm.arena.alloc(Val::String(Rc::new(state.into_bytes()))));
    arr.push(vm.arena.alloc(code.map(Val::Int).unwrap_or(Val::Null)));
    arr.push(
        vm.arena.alloc(
            msg.map(|s| Val::String(Rc::new(s.into_bytes())))
                .unwrap_or(Val::Null),
        ),
    );

    Ok(vm.arena.alloc(Val::Array(Rc::new(arr))))
}

// --- PDOStatement Native Methods ---

pub fn php_pdo_stmt_execute(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in PDOStatement::execute")?;
    let stmt_id = get_pdo_statement_id(vm, this_handle)?;

    let params = if !args.is_empty() {
        match &vm.arena.get(args[0]).value {
            Val::Array(arr) => {
                let mut p = Vec::new();
                for (key, val) in arr.map.iter() {
                    let id = match key {
                        ArrayKey::Int(i) => ParamIdentifier::Position((*i + 1) as usize),
                        ArrayKey::Str(s) => {
                            ParamIdentifier::Name(String::from_utf8_lossy(s).to_string())
                        }
                    };
                    p.push((id, handle_to_pdo_val(vm, *val)));
                }
                Some(p)
            }
            Val::Null => None,
            _ => return Err("PDOStatement::execute(): Parameter must be an array or null".into()),
        }
    } else {
        None
    };

    let stmt_ref = vm
        .context
        .pdo_statements
        .get(&stmt_id)
        .ok_or("Invalid statement")?;
    stmt_ref
        .borrow_mut()
        .execute(params.as_deref())
        .map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_pdo_stmt_bind_param(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("PDOStatement::bindParam() expects at least 2 parameters".into());
    }

    let param_id = match &vm.arena.get(args[0]).value {
        Val::Int(i) => ParamIdentifier::Position(*i as usize),
        Val::String(s) => ParamIdentifier::Name(String::from_utf8_lossy(s).to_string()),
        _ => return Err("Parameter identifier must be an integer or string".into()),
    };

    // Note: Proper bindParam should bind by reference.
    // For now we implement it as bindValue for simplicity in the native bridge.
    let pdo_val = handle_to_pdo_val(vm, args[1]);

    let param_type = if args.len() >= 3 {
        ParamType::Str
    } else {
        ParamType::Str
    };

    let this_handle = vm.frames.last().and_then(|f| f.this).ok_or("No 'this'")?;
    let stmt_id = get_pdo_statement_id(vm, this_handle)?;
    let stmt_ref = vm
        .context
        .pdo_statements
        .get(&stmt_id)
        .ok_or("Invalid statement")?;

    stmt_ref
        .borrow_mut()
        .bind_param(param_id, pdo_val, param_type)
        .map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_pdo_stmt_bind_value(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("PDOStatement::bindValue() expects 2 parameters".into());
    }

    let param_id = match &vm.arena.get(args[0]).value {
        Val::Int(i) => ParamIdentifier::Position(*i as usize),
        Val::String(s) => ParamIdentifier::Name(String::from_utf8_lossy(s).to_string()),
        _ => return Err("Parameter identifier must be an integer or string".into()),
    };

    let pdo_val = handle_to_pdo_val(vm, args[1]);

    let param_type = if args.len() >= 3 {
        ParamType::Str
    } else {
        ParamType::Str
    };

    let this_handle = vm.frames.last().and_then(|f| f.this).ok_or("No 'this'")?;
    let stmt_id = get_pdo_statement_id(vm, this_handle)?;
    let stmt_ref = vm
        .context
        .pdo_statements
        .get(&stmt_id)
        .ok_or("Invalid statement")?;

    stmt_ref
        .borrow_mut()
        .bind_param(param_id, pdo_val, param_type)
        .map_err(|e| e.to_string())?;

    Ok(vm.arena.alloc(Val::Bool(true)))
}

pub fn php_pdo_stmt_fetch(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm.frames.last().and_then(|f| f.this).ok_or("No 'this'")?;
    let stmt_id = get_pdo_statement_id(vm, this_handle)?;

    let fetch_mode = if !args.is_empty() {
        match &vm.arena.get(args[0]).value {
            Val::Int(i) => types::FetchMode::from_i64(*i).unwrap_or(types::FetchMode::Both),
            _ => types::FetchMode::Both,
        }
    } else {
        // Look for fetchMode property on the statement object
        let fetch_mode_sym = vm.context.interner.intern(b"fetchMode");
        let mut mode = types::FetchMode::Both;

        if let Val::Object(payload_h) = &vm.arena.get(this_handle).value {
            if let Val::ObjPayload(obj) = &vm.arena.get(*payload_h).value {
                if let Some(val_h) = obj.properties.get(&fetch_mode_sym) {
                    if let Val::Int(m) = &vm.arena.get(*val_h).value {
                        mode = types::FetchMode::from_i64(*m).unwrap_or(types::FetchMode::Both);
                    }
                }
            }
        }
        mode
    };

    let stmt_ref = vm
        .context
        .pdo_statements
        .get(&stmt_id)
        .ok_or("Invalid statement")?;
    let row_opt = stmt_ref
        .borrow_mut()
        .fetch(fetch_mode)
        .map_err(|e| e.to_string())?;

    match row_opt {
        Some(row) => Ok(fetched_row_to_val(vm, row)),
        None => Ok(vm.arena.alloc(Val::Bool(false))),
    }
}

pub fn php_pdo_stmt_fetch_all(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm.frames.last().and_then(|f| f.this).ok_or("No 'this'")?;
    let stmt_id = get_pdo_statement_id(vm, this_handle)?;

    let fetch_mode = if !args.is_empty() {
        match &vm.arena.get(args[0]).value {
            Val::Int(i) => types::FetchMode::from_i64(*i).unwrap_or(types::FetchMode::Both),
            _ => types::FetchMode::Both,
        }
    } else {
        // Look for fetchMode property on the statement object
        let fetch_mode_sym = vm.context.interner.intern(b"fetchMode");
        let mut mode = types::FetchMode::Both;

        if let Val::Object(payload_h) = &vm.arena.get(this_handle).value {
            if let Val::ObjPayload(obj) = &vm.arena.get(*payload_h).value {
                if let Some(val_h) = obj.properties.get(&fetch_mode_sym) {
                    if let Val::Int(m) = &vm.arena.get(*val_h).value {
                        mode = types::FetchMode::from_i64(*m).unwrap_or(types::FetchMode::Both);
                    }
                }
            }
        }
        mode
    };

    let stmt_ref = vm
        .context
        .pdo_statements
        .get(&stmt_id)
        .ok_or("Invalid statement")?;
    let rows = stmt_ref
        .borrow_mut()
        .fetch_all(fetch_mode)
        .map_err(|e| e.to_string())?;

    let mut arr = ArrayData::new();
    for row in rows {
        arr.push(fetched_row_to_val(vm, row));
    }

    Ok(vm.arena.alloc(Val::Array(Rc::new(arr))))
}

pub fn php_pdo_stmt_row_count(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in PDOStatement::rowCount")?;
    let stmt_id = get_pdo_statement_id(vm, this_handle)?;

    let stmt_ref = vm
        .context
        .pdo_statements
        .get(&stmt_id)
        .ok_or("Invalid statement")?;
    let count = stmt_ref.borrow().row_count();

    Ok(vm.arena.alloc(Val::Int(count)))
}

pub fn php_pdo_stmt_column_count(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in PDOStatement::columnCount")?;
    let stmt_id = get_pdo_statement_id(vm, this_handle)?;

    let stmt_ref = vm
        .context
        .pdo_statements
        .get(&stmt_id)
        .ok_or("Invalid statement")?;
    let count = stmt_ref.borrow().column_count();

    Ok(vm.arena.alloc(Val::Int(count as i64)))
}

pub fn php_pdo_stmt_error_code(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm.frames.last().and_then(|f| f.this).ok_or("No 'this'")?;
    let stmt_id = get_pdo_statement_id(vm, this_handle)?;
    let stmt_ref = vm
        .context
        .pdo_statements
        .get(&stmt_id)
        .ok_or("Invalid statement")?;
    let (state, _, _) = stmt_ref.borrow().error_info();
    Ok(vm.arena.alloc(Val::String(state.into_bytes().into())))
}

pub fn php_pdo_stmt_error_info(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("No 'this' in PDOStatement::errorInfo")?;
    let stmt_id = get_pdo_statement_id(vm, this_handle)?;

    let stmt_ref = vm
        .context
        .pdo_statements
        .get(&stmt_id)
        .ok_or("Invalid statement")?;
    let (state, code, msg) = stmt_ref.borrow().error_info();

    let mut arr = ArrayData::new();
    arr.push(vm.arena.alloc(Val::String(Rc::new(state.into_bytes()))));
    arr.push(vm.arena.alloc(code.map(Val::Int).unwrap_or(Val::Null)));
    arr.push(
        vm.arena.alloc(
            msg.map(|s| Val::String(Rc::new(s.into_bytes())))
                .unwrap_or(Val::Null),
        ),
    );

    Ok(vm.arena.alloc(Val::Array(Rc::new(arr))))
}

fn handle_to_pdo_val(vm: &VM, handle: Handle) -> PdoValue {
    match &vm.arena.get(handle).value {
        Val::Null => PdoValue::Null,
        Val::Bool(b) => PdoValue::Bool(*b),
        Val::Int(i) => PdoValue::Int(*i),
        Val::Float(f) => PdoValue::Float(*f),
        Val::String(s) => PdoValue::String(s.to_vec()),
        _ => PdoValue::String(b"Object/Array".to_vec()),
    }
}

fn pdo_val_to_handle(vm: &mut VM, val: PdoValue) -> Handle {
    match val {
        PdoValue::Null => vm.arena.alloc(Val::Null),
        PdoValue::Bool(b) => vm.arena.alloc(Val::Bool(b)),
        PdoValue::Int(i) => vm.arena.alloc(Val::Int(i)),
        PdoValue::Float(f) => vm.arena.alloc(Val::Float(f)),
        PdoValue::String(s) => vm.arena.alloc(Val::String(s.into())),
    }
}

fn fetched_row_to_val(vm: &mut VM, row: types::FetchedRow) -> Handle {
    match row {
        types::FetchedRow::Assoc(map) => {
            let mut arr = ArrayData::new();
            for (key, val) in map {
                arr.insert(
                    ArrayKey::Str(Rc::new(key.into_bytes())),
                    pdo_val_to_handle(vm, val),
                );
            }
            vm.arena.alloc(Val::Array(Rc::new(arr)))
        }
        types::FetchedRow::Num(vec) => {
            let mut arr = ArrayData::new();
            for (idx, val) in vec.into_iter().enumerate() {
                arr.insert(ArrayKey::Int(idx as i64), pdo_val_to_handle(vm, val));
            }
            vm.arena.alloc(Val::Array(Rc::new(arr)))
        }
        types::FetchedRow::Both(assoc, num) => {
            let mut arr = ArrayData::new();
            for (key, val) in assoc {
                arr.insert(
                    ArrayKey::Str(Rc::new(key.into_bytes())),
                    pdo_val_to_handle(vm, val),
                );
            }
            for (idx, val) in num.into_iter().enumerate() {
                arr.insert(ArrayKey::Int(idx as i64), pdo_val_to_handle(vm, val));
            }
            vm.arena.alloc(Val::Array(Rc::new(arr)))
        }
        types::FetchedRow::Obj(map) => {
            // Create stdClass
            let std_class_sym = vm.context.interner.intern(b"stdClass");
            let mut properties = vm.collect_properties(std_class_sym, PropertyCollectionMode::All);
            for (key, val) in map {
                let key_sym = vm.context.interner.intern(key.as_bytes());
                properties.insert(key_sym, pdo_val_to_handle(vm, val));
            }
            let obj_data = ObjectData {
                class: std_class_sym,
                properties,
                internal: None,
                dynamic_properties: HashSet::new(),
            };
            let payload_handle = vm.arena.alloc(Val::ObjPayload(obj_data));
            vm.arena.alloc(Val::Object(payload_handle))
        }
    }
}

/// Register PDO constants
/// Reference: $PHP_SRC_PATH/ext/pdo/pdo.c
fn register_pdo_constants(_registry: &mut ExtensionRegistry) {
    // These are now registered as class constants in the PDO class.
}
