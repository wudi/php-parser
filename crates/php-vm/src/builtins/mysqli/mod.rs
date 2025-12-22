//! MySQLi Extension - MySQL Improved Extension
//!
//! This module implements PHP's mysqli extension with the following features:
//! - Connection management (mysqli_connect, mysqli_close)
//! - Query execution (mysqli_query)
//! - Result fetching (mysqli_fetch_assoc, mysqli_fetch_row, mysqli_fetch_array)
//! - Error handling (mysqli_error, mysqli_errno)
//! - Prepared statements (mysqli_prepare, mysqli_stmt_execute)
//! - Transactions (mysqli_begin_transaction, mysqli_commit, mysqli_rollback)
//!
//! # Architecture
//!
//! - **Connection Management**: RAII-based with automatic cleanup
//! - **Type Conversion**: Bidirectional PHP ↔ MySQL value conversion
//! - **Error Handling**: No panics - all errors return Result
//! - **Zero-Heap AST**: All allocations via Arena
//!
//! # References
//!
//! - PHP Source: $PHP_SRC_PATH/ext/mysqli/mysqli.c
//! - PHP API: $PHP_SRC_PATH/ext/mysqli/php_mysqli_structs.h
//! - MySQL API: https://dev.mysql.com/doc/c-api/8.0/en/

pub mod connection;
pub mod error;
pub mod result;
pub mod types;

use crate::core::value::{ArrayData, ArrayKey, Handle, Val};
use crate::vm::engine::VM;
use std::rc::Rc;

pub use connection::MysqliConnection;
pub use error::MysqliError;
pub use result::MysqliResult;

/// mysqli_connect(string $host, string $username, string $password, string $database, int $port = 3306): resource|false
///
/// Opens a connection to a MySQL server.
///
/// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_connect
pub fn php_mysqli_connect(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // Validate arguments (4-5 parameters)
    if args.is_empty() || args.len() > 5 {
        return Err("mysqli_connect() expects 4 or 5 parameters".into());
    }

    // Extract host
    let host = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("mysqli_connect(): Argument #1 (host) must be string".into()),
    };

    // Extract username
    let username = if args.len() > 1 {
        match &vm.arena.get(args[1]).value {
            Val::String(s) => String::from_utf8_lossy(s).to_string(),
            _ => return Err("mysqli_connect(): Argument #2 (username) must be string".into()),
        }
    } else {
        "root".to_string()
    };

    // Extract password
    let password = if args.len() > 2 {
        match &vm.arena.get(args[2]).value {
            Val::String(s) => String::from_utf8_lossy(s).to_string(),
            _ => return Err("mysqli_connect(): Argument #3 (password) must be string".into()),
        }
    } else {
        String::new()
    };

    // Extract database
    let database = if args.len() > 3 {
        match &vm.arena.get(args[3]).value {
            Val::String(s) => String::from_utf8_lossy(s).to_string(),
            _ => return Err("mysqli_connect(): Argument #4 (database) must be string".into()),
        }
    } else {
        String::new()
    };

    // Extract port
    let port = if args.len() > 4 {
        match &vm.arena.get(args[4]).value {
            Val::Int(i) => *i as u16,
            _ => 3306,
        }
    } else {
        3306
    };

    // Create connection
    match MysqliConnection::new(&host, &username, &password, &database, port) {
        Ok(conn) => {
            // Store connection in context
            let conn_id = vm.context.next_resource_id;
            vm.context.next_resource_id += 1;

            vm.context
                .mysqli_connections
                .insert(conn_id, Rc::new(std::cell::RefCell::new(conn)));

            // Return resource handle
            let resource_val = Val::Resource(Rc::new(conn_id));
            Ok(vm.arena.alloc(resource_val))
        }
        Err(e) => {
            // Return false on connection error (PHP behavior)
            Ok(vm.arena.alloc(Val::Bool(false)))
        }
    }
}

/// mysqli_close(resource $link): bool
///
/// Closes a previously opened database connection.
///
/// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_close
pub fn php_mysqli_close(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("mysqli_close() expects exactly 1 parameter".into());
    }

    // Extract connection resource ID
    let conn_id = match &vm.arena.get(args[0]).value {
        Val::Resource(rc) => *rc
            .downcast_ref::<u64>()
            .ok_or("mysqli_close(): Invalid resource type")?,
        _ => return Err("mysqli_close(): Argument #1 must be mysqli link".into()),
    };

    // Remove connection from context (triggers Drop/cleanup)
    let removed = vm.context.mysqli_connections.remove(&conn_id).is_some();

    Ok(vm.arena.alloc(Val::Bool(removed)))
}

/// mysqli_query(resource $link, string $query): resource|bool
///
/// Performs a query on the database.
///
/// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_query
pub fn php_mysqli_query(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("mysqli_query() expects exactly 2 parameters".into());
    }

    // Extract connection
    let conn_id = match &vm.arena.get(args[0]).value {
        Val::Resource(rc) => *rc
            .downcast_ref::<u64>()
            .ok_or("mysqli_query(): Invalid resource type")?,
        _ => return Err("mysqli_query(): Argument #1 must be mysqli link".into()),
    };

    // Extract query string
    let query = match &vm.arena.get(args[1]).value {
        Val::String(s) => s.clone(),
        _ => return Err("mysqli_query(): Argument #2 must be string".into()),
    };

    // Get connection
    let conn_ref = vm
        .context
        .mysqli_connections
        .get(&conn_id)
        .ok_or_else(|| "mysqli_query(): Invalid mysqli link".to_string())?;

    let query_str = String::from_utf8_lossy(&query);

    // Execute query
    match conn_ref.borrow_mut().query(&query_str) {
        Ok(result) => {
            // Store result
            let result_id = vm.context.next_resource_id;
            vm.context.next_resource_id += 1;

            vm.context
                .mysqli_results
                .insert(result_id, Rc::new(std::cell::RefCell::new(result)));

            // Return result resource
            Ok(vm.arena.alloc(Val::Resource(Rc::new(result_id))))
        }
        Err(e) => {
            // Query failed - return false (PHP behavior)
            Ok(vm.arena.alloc(Val::Bool(false)))
        }
    }
}

/// mysqli_fetch_assoc(resource $result): array|null|false
///
/// Fetch result row as an associative array.
///
/// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_fetch_assoc
pub fn php_mysqli_fetch_assoc(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("mysqli_fetch_assoc() expects exactly 1 parameter".into());
    }

    // Extract result resource ID
    let result_id = match &vm.arena.get(args[0]).value {
        Val::Resource(rc) => *rc
            .downcast_ref::<u64>()
            .ok_or("mysqli_fetch_assoc(): Invalid resource type")?,
        _ => return Err("mysqli_fetch_assoc(): Argument #1 must be mysqli_result".into()),
    };

    // Get result
    let result_ref = vm
        .context
        .mysqli_results
        .get(&result_id)
        .ok_or_else(|| "mysqli_fetch_assoc(): Invalid mysqli_result".to_string())?;

    // Fetch next row (release borrow immediately)
    let row_opt = result_ref.borrow_mut().fetch_assoc();

    match row_opt {
        Some(row) => {
            // Convert to PHP array
            let mut arr = ArrayData::new();

            for (key, value) in row {
                let key_bytes = key.into_bytes();
                let array_key = ArrayKey::Str(Rc::new(key_bytes));
                let val_handle = types::mysql_to_php(vm, value);
                arr.insert(array_key, val_handle);
            }

            Ok(vm.arena.alloc(Val::Array(Rc::new(arr))))
        }
        None => {
            // No more rows - return false
            Ok(vm.arena.alloc(Val::Bool(false)))
        }
    }
}

/// mysqli_fetch_row(resource $result): array|null|false
///
/// Fetch result row as a numeric array.
///
/// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_fetch_row
pub fn php_mysqli_fetch_row(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("mysqli_fetch_row() expects exactly 1 parameter".into());
    }

    // Extract result resource ID
    let result_id = match &vm.arena.get(args[0]).value {
        Val::Resource(rc) => *rc
            .downcast_ref::<u64>()
            .ok_or("mysqli_fetch_row(): Invalid resource type")?,
        _ => return Err("mysqli_fetch_row(): Argument #1 must be mysqli_result".into()),
    };

    // Get result
    let result_ref = vm
        .context
        .mysqli_results
        .get(&result_id)
        .ok_or_else(|| "mysqli_fetch_row(): Invalid mysqli_result".to_string())?;

    // Fetch next row (release borrow immediately)
    let row_opt = result_ref.borrow_mut().fetch_row();

    match row_opt {
        Some(row) => {
            // Convert to PHP numeric array
            let mut arr = ArrayData::new();

            for (idx, value) in row.iter().enumerate() {
                let array_key = ArrayKey::Int(idx as i64);
                let val_handle = types::mysql_to_php(vm, value.clone());
                arr.insert(array_key, val_handle);
            }

            Ok(vm.arena.alloc(Val::Array(Rc::new(arr))))
        }
        None => {
            // No more rows - return false
            Ok(vm.arena.alloc(Val::Bool(false)))
        }
    }
}

/// mysqli_num_rows(resource $result): int
///
/// Returns the number of rows in the result set.
///
/// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_num_rows
pub fn php_mysqli_num_rows(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("mysqli_num_rows() expects exactly 1 parameter".into());
    }

    // Extract result resource ID
    let result_id = match &vm.arena.get(args[0]).value {
        Val::Resource(rc) => *rc
            .downcast_ref::<u64>()
            .ok_or("mysqli_num_rows(): Invalid resource type")?,
        _ => return Err("mysqli_num_rows(): Argument #1 must be mysqli_result".into()),
    };

    // Get result
    let result_ref = vm
        .context
        .mysqli_results
        .get(&result_id)
        .ok_or_else(|| "mysqli_num_rows(): Invalid mysqli_result".to_string())?;

    let num_rows = result_ref.borrow().num_rows() as i64;

    Ok(vm.arena.alloc(Val::Int(num_rows)))
}

/// mysqli_affected_rows(resource $link): int
///
/// Returns the number of affected rows in the previous MySQL operation.
///
/// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_affected_rows
pub fn php_mysqli_affected_rows(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("mysqli_affected_rows() expects exactly 1 parameter".into());
    }

    // Extract connection resource ID
    let conn_id = match &vm.arena.get(args[0]).value {
        Val::Resource(rc) => *rc
            .downcast_ref::<u64>()
            .ok_or("mysqli_affected_rows(): Invalid resource type")?,
        _ => return Err("mysqli_affected_rows(): Argument #1 must be mysqli link".into()),
    };

    // Get connection
    let conn_ref = vm
        .context
        .mysqli_connections
        .get(&conn_id)
        .ok_or_else(|| "mysqli_affected_rows(): Invalid mysqli link".to_string())?;

    let affected = conn_ref.borrow().affected_rows() as i64;

    Ok(vm.arena.alloc(Val::Int(affected)))
}

/// mysqli_error(resource $link): string
///
/// Returns the error message for the most recent MySQLi function call.
///
/// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_error
pub fn php_mysqli_error(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("mysqli_error() expects exactly 1 parameter".into());
    }

    // Extract connection resource ID
    let conn_id = match &vm.arena.get(args[0]).value {
        Val::Resource(rc) => *rc
            .downcast_ref::<u64>()
            .ok_or("mysqli_error(): Invalid resource type")?,
        _ => return Err("mysqli_error(): Argument #1 must be mysqli link".into()),
    };

    // Get connection
    let conn_ref = vm
        .context
        .mysqli_connections
        .get(&conn_id)
        .ok_or_else(|| "mysqli_error(): Invalid mysqli link".to_string())?;

    let error_msg = conn_ref.borrow().last_error_message();

    Ok(vm.arena.alloc(Val::String(Rc::new(error_msg.into_bytes()))))
}

/// mysqli_errno(resource $link): int
///
/// Returns the error code for the most recent MySQLi function call.
///
/// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - mysqli_errno
pub fn php_mysqli_errno(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("mysqli_errno() expects exactly 1 parameter".into());
    }

    // Extract connection resource ID
    let conn_id = match &vm.arena.get(args[0]).value {
        Val::Resource(rc) => *rc
            .downcast_ref::<u64>()
            .ok_or("mysqli_errno(): Invalid resource type")?,
        _ => return Err("mysqli_errno(): Argument #1 must be mysqli link".into()),
    };

    // Get connection
    let conn_ref = vm
        .context
        .mysqli_connections
        .get(&conn_id)
        .ok_or_else(|| "mysqli_errno(): Invalid mysqli link".to_string())?;

    let errno = conn_ref.borrow().last_error_code() as i64;

    Ok(vm.arena.alloc(Val::Int(errno)))
}
