/// Bind a parameter to a prepared statement
pub fn php_pdo_stmt_bind_param(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // args[0] = $this (PDOStatement object), args[1] = param (int or string), args[2] = value, args[3] = param_type (optional)
    let this = args.get(0).ok_or("PDOStatement::bindParam: missing $this")?;
    let param = args.get(1).ok_or("PDOStatement::bindParam: missing param")?;
    let value = *args.get(2).ok_or("PDOStatement::bindParam: missing value")?;
    let param_type = args.get(3)
        .and_then(|h| vm.read_long(*h))
        .and_then(crate::builtins::pdo::types::ParamType::from_i64)
        .unwrap_or(crate::builtins::pdo::types::ParamType::Str);
    let param_id = if let Some(i) = vm.read_long(*param) {
        crate::builtins::pdo::types::ParamIdentifier::Positional(i as usize)
    } else if let Some(s) = vm.read_zstr(*param) {
        crate::builtins::pdo::types::ParamIdentifier::Named(s)
    } else {
        return Err("PDOStatement::bindParam: invalid param identifier".to_string());
    };
    let obj = match &vm.arena.get(*this).value {
        Val::ObjPayload(obj) => obj,
        _ => return Err("PDOStatement::bindParam: $this is not an object".to_string()),
    };
    let stmt = obj.internal.as_ref().and_then(|rc| rc.downcast_ref::<Box<dyn crate::builtins::pdo::driver::PdoStatement>>()).ok_or("PDOStatement::bindParam: missing statement")?;
    stmt.bind_param(param_id, value, param_type).map_err(|e| format!("PDOStatement::bindParam error: {e:?}"))?;
    Ok(vm.arena.alloc(Val::Bool(true)))
}
//! PDO and PDOStatement class stubs for VM registration

use crate::core::value::{Handle, Val, ObjectData, Symbol};
use crate::vm::engine::VM;
use crate::builtins::pdo::drivers::DriverRegistry;
use crate::builtins::pdo::types::PdoError;
use std::sync::Arc;
use std::rc::Rc;
use std::any::Any;

// PDO class stub
/// PDO::__construct(dsn, username = null, password = null, options = [])
pub fn php_pdo_construct(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // Parse arguments (dsn, username, password, options)
    let dsn = args.get(0).and_then(|h| vm.read_zstr(*h)).ok_or_else(|| "PDO::__construct: missing DSN".to_string())?;
    let username = args.get(1).and_then(|h| vm.read_zstr(*h));
    let password = args.get(2).and_then(|h| vm.read_zstr(*h));
    // TODO: Parse options (args.get(3))

    // Get driver registry from engine context
    let registry = vm.context.engine.pdo_driver_registry.as_ref().ok_or_else(|| "PDO: driver registry not initialized".to_string())?;
    // Parse DSN
    let (driver, conn_str) = DriverRegistry::parse_dsn(&dsn).map_err(|e| format!("PDO: {e:?}"))?;
    let driver = registry.get(driver).ok_or_else(|| format!("PDO: unknown driver '{driver}'"))?;

    // Connect
    let conn = driver.connect(&conn_str, username.as_deref(), password.as_deref(), &[])
        .map_err(|e| format!("PDO connect error: {e:?}"))?;

    // Store connection in ObjectData.internal
    let class_sym = Symbol(0); // TODO: Intern "PDO" class symbol
    let obj = ObjectData {
        class: class_sym,
        properties: Default::default(),
        internal: Some(Rc::new(conn)),
        dynamic_properties: Default::default(),
    };
    Ok(vm.arena.alloc(Val::ObjPayload(obj)))
}

pub fn php_pdo_prepare(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // args[0] = $this (PDO object), args[1] = SQL
    let this = args.get(0).ok_or_else(|| "PDO::prepare: missing $this".to_string())?;
    let sql = args.get(1).and_then(|h| vm.read_zstr(*h)).ok_or_else(|| "PDO::prepare: missing SQL".to_string())?;
    let obj = match &vm.arena.get(*this).value {
        Val::ObjPayload(obj) => obj,
        _ => return Err("PDO::prepare: $this is not an object".to_string()),
    };
    let conn = obj.internal.as_ref().and_then(|rc| rc.downcast_ref::<Box<dyn crate::builtins::pdo::driver::PdoConnection>>()).ok_or_else(|| "PDO::prepare: missing connection".to_string())?;
    let stmt = conn.prepare(&sql).map_err(|e| format!("PDO::prepare error: {e:?}"))?;
    // Store statement in PDOStatement object
    let class_sym = Symbol(1); // TODO: Intern "PDOStatement" class symbol
    let stmt_obj = ObjectData {
        class: class_sym,
        properties: Default::default(),
        internal: Some(Rc::new(stmt)),
        dynamic_properties: Default::default(),
    };
    Ok(vm.arena.alloc(Val::ObjPayload(stmt_obj)))
}

pub fn php_pdo_exec(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // args[0] = $this (PDO object), args[1] = SQL
    let this = args.get(0).ok_or_else(|| "PDO::exec: missing $this".to_string())?;
    let sql = args.get(1).and_then(|h| vm.read_zstr(*h)).ok_or_else(|| "PDO::exec: missing SQL".to_string())?;
    let obj = match &vm.arena.get(*this).value {
        Val::ObjPayload(obj) => obj,
        _ => return Err("PDO::exec: $this is not an object".to_string()),
    };
    let conn = obj.internal.as_ref().and_then(|rc| rc.downcast_ref::<Box<dyn crate::builtins::pdo::driver::PdoConnection>>()).ok_or_else(|| "PDO::exec: missing connection".to_string())?;
    let affected = conn.exec(&sql).map_err(|e| format!("PDO::exec error: {e:?}"))?;
    Ok(vm.arena.alloc(Val::Int(affected)))
}

// PDOStatement class stub
pub fn php_pdo_stmt_execute(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // args[0] = $this (PDOStatement object)
    let this = args.get(0).ok_or_else(|| "PDOStatement::execute: missing $this".to_string())?;
    let obj = match &vm.arena.get(*this).value {
        Val::ObjPayload(obj) => obj,
        _ => return Err("PDOStatement::execute: $this is not an object".to_string()),
    };
    let stmt = obj.internal.as_ref().and_then(|rc| rc.downcast_ref::<Box<dyn crate::builtins::pdo::driver::PdoStatement>>()).ok_or_else(|| "PDOStatement::execute: missing statement".to_string())?;
    stmt.execute(None).map_err(|e| format!("PDOStatement::execute error: {e:?}"))?;
    Ok(vm.arena.alloc(Val::Null))
}

pub fn php_pdo_stmt_fetch(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // args[0] = $this (PDOStatement object)
    let this = args.get(0).ok_or_else(|| "PDOStatement::fetch: missing $this".to_string())?;
    let obj = match &vm.arena.get(*this).value {
        Val::ObjPayload(obj) => obj,
        _ => return Err("PDOStatement::fetch: $this is not an object".to_string()),
    };
    let stmt = obj.internal.as_ref().and_then(|rc| rc.downcast_ref::<Box<dyn crate::builtins::pdo::driver::PdoStatement>>()).ok_or_else(|| "PDOStatement::fetch: missing statement".to_string())?;
    let row = stmt.fetch(crate::builtins::pdo::types::FetchMode::Assoc).map_err(|e| format!("PDOStatement::fetch error: {e:?}"))?;
    match row {
        Some(crate::builtins::pdo::types::FetchedRow::Assoc(map)) => {
            use crate::core::value::{ArrayData, ArrayKey, Val};
            let mut arr = ArrayData::new();
            for (k, v) in map {
                arr.insert(ArrayKey::Str(Rc::new(k.into_bytes())), v);
            }
            Ok(vm.arena.alloc(Val::Array(Rc::new(arr))))
        }
        Some(crate::builtins::pdo::types::FetchedRow::Num(vec)) => {
            use crate::core::value::{ArrayData, ArrayKey, Val};
            let mut arr = ArrayData::new();
            for (i, v) in vec.into_iter().enumerate() {
                arr.insert(ArrayKey::Int(i as i64), v);
            }
            Ok(vm.arena.alloc(Val::Array(Rc::new(arr))))
        }
        Some(crate::builtins::pdo::types::FetchedRow::Both(map, vec)) => {
            // For now, return Assoc only (can be improved)
            use crate::core::value::{ArrayData, ArrayKey, Val};
            let mut arr = ArrayData::new();
            for (k, v) in map {
                arr.insert(ArrayKey::Str(Rc::new(k.into_bytes())), v);
            }
            Ok(vm.arena.alloc(Val::Array(Rc::new(arr))))
        }
        _ => Ok(vm.arena.alloc(Val::Null)),
    }
}
