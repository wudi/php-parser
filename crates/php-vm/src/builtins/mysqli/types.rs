//! MySQLi Type Conversion
//!
//! Bidirectional conversion between PHP values and MySQL values.
//!
//! Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - php_mysqli_fetch_into_hash

use crate::core::value::{Handle, Val};
use crate::vm::engine::VM;
use mysql::Value as MySqlValue;
use std::rc::Rc;

/// Convert MySQL value to PHP Val
///
/// Reference: $PHP_SRC_PATH/ext/mysqli/mysqli_api.c - php_mysqli_fetch_into_hash
pub fn mysql_to_php(vm: &mut VM, value: MySqlValue) -> Handle {
    match value {
        MySqlValue::NULL => vm.arena.alloc(Val::Null),

        MySqlValue::Int(i) => vm.arena.alloc(Val::Int(i)),

        MySqlValue::UInt(u) => {
            // Try to fit in i64, otherwise convert to float
            if u <= i64::MAX as u64 {
                vm.arena.alloc(Val::Int(u as i64))
            } else {
                vm.arena.alloc(Val::Float(u as f64))
            }
        }

        MySqlValue::Float(f) => vm.arena.alloc(Val::Float(f as f64)),

        MySqlValue::Double(d) => vm.arena.alloc(Val::Float(d)),

        MySqlValue::Bytes(b) => {
            // Try to parse as number first (MySQL returns numbers as strings sometimes)
            if let Ok(s) = std::str::from_utf8(&b) {
                // Try parsing as int first
                if let Ok(i) = s.parse::<i64>() {
                    return vm.arena.alloc(Val::Int(i));
                }
                // Try parsing as float
                if let Ok(f) = s.parse::<f64>() {
                    return vm.arena.alloc(Val::Float(f));
                }
            }
            // Return as string if not a number
            vm.arena.alloc(Val::String(Rc::new(b)))
        }

        MySqlValue::Date(year, month, day, hour, minute, second, _micro) => {
            // Format as MySQL datetime string: YYYY-MM-DD HH:MM:SS
            let datetime_str = format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                year, month, day, hour, minute, second
            );
            vm.arena
                .alloc(Val::String(Rc::new(datetime_str.into_bytes())))
        }

        MySqlValue::Time(is_negative, days, hours, minutes, seconds, _micros) => {
            // Format as TIME string: [-][D ]HH:MM:SS
            let total_hours = days * 24 + hours as u32;
            let sign = if is_negative { "-" } else { "" };
            let time_str = format!(
                "{}{:02}:{:02}:{:02}",
                sign, total_hours, minutes, seconds
            );
            vm.arena.alloc(Val::String(Rc::new(time_str.into_bytes())))
        }
    }
}

/// Convert PHP Val to MySQL parameter value
///
/// Used for prepared statement parameter binding.
pub fn php_to_mysql(vm: &VM, handle: Handle) -> Result<MySqlValue, String> {
    match &vm.arena.get(handle).value {
        Val::Null => Ok(MySqlValue::NULL),

        Val::Int(i) => Ok(MySqlValue::Int(*i)),

        Val::Float(f) => Ok(MySqlValue::Double(*f)),

        Val::Bool(b) => Ok(MySqlValue::Int(*b as i64)),

        Val::String(s) => Ok(MySqlValue::Bytes(s.as_ref().clone())),

        _ => Err(format!(
            "Unsupported type for MySQL parameter: {:?}",
            vm.arena.get(handle).value
        )),
    }
}
