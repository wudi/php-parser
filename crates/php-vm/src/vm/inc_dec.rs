/// Increment/Decrement operations for PHP values
/// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - increment_function/decrement_function

use crate::core::value::Val;
use crate::vm::engine::VmError;
use std::rc::Rc;

/// Increment a value in-place, following PHP semantics
/// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - increment_function
pub fn increment_value(val: Val) -> Result<Val, VmError> {
    match val {
        // INT: increment by 1, overflow to float
        Val::Int(i) => {
            if i == i64::MAX {
                Ok(Val::Float(i as f64 + 1.0))
            } else {
                Ok(Val::Int(i + 1))
            }
        }

        // FLOAT: increment by 1.0
        Val::Float(f) => Ok(Val::Float(f + 1.0)),

        // NULL: becomes 1
        Val::Null => Ok(Val::Int(1)),

        // STRING: special handling
        Val::String(s) => increment_string(s),

        // BOOL: warning + no effect (PHP 8.3+)
        Val::Bool(_) => {
            // In PHP 8.3+, this generates a warning but doesn't change the value
            // For now, we'll just return the original value
            // TODO: Add warning to error handler
            Ok(val)
        }

        // Other types: no effect
        Val::Array(_) | Val::Object(_) | Val::ObjPayload(_) | Val::Resource(_) => Ok(val),

        Val::AppendPlaceholder => Err(VmError::RuntimeError(
            "Cannot increment append placeholder".into(),
        )),
    }
}

/// Decrement a value in-place, following PHP semantics
/// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - decrement_function
pub fn decrement_value(val: Val) -> Result<Val, VmError> {
    match val {
        // INT: decrement by 1, underflow to float
        Val::Int(i) => {
            if i == i64::MIN {
                Ok(Val::Float(i as f64 - 1.0))
            } else {
                Ok(Val::Int(i - 1))
            }
        }

        // FLOAT: decrement by 1.0
        Val::Float(f) => Ok(Val::Float(f - 1.0)),

        // STRING: only numeric strings are decremented
        Val::String(s) => decrement_string(s),

        // NULL: PHP treats NULL-- as 0 - 1 = -1
        // But actually PHP keeps it as NULL in some versions, check exact behavior
        // Reference shows NULL-- stays NULL but emits deprecated warning in PHP 8.3
        Val::Null => {
            // TODO: Add deprecated warning
            Ok(Val::Null)
        }

        // BOOL: warning + no effect (PHP 8.3+)
        Val::Bool(_) => {
            // In PHP 8.3+, this generates a warning but doesn't change the value
            Ok(val)
        }

        // Other types: no effect
        Val::Array(_) | Val::Object(_) | Val::ObjPayload(_) | Val::Resource(_) => Ok(val),

        Val::AppendPlaceholder => Err(VmError::RuntimeError(
            "Cannot decrement append placeholder".into(),
        )),
    }
}

/// Increment a string value following PHP's Perl-style string increment
/// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - increment_string
fn increment_string(s: Rc<Vec<u8>>) -> Result<Val, VmError> {
    // Empty string becomes "1"
    if s.is_empty() {
        return Ok(Val::String(Rc::new(b"1".to_vec())));
    }

    // Try parsing as numeric
    if let Ok(s_str) = std::str::from_utf8(&s) {
        let trimmed = s_str.trim();
        
        // Try as integer
        if let Ok(i) = trimmed.parse::<i64>() {
            if i == i64::MAX {
                return Ok(Val::Float(i as f64 + 1.0));
            } else {
                return Ok(Val::Int(i + 1));
            }
        }
        
        // Try as float
        if let Ok(f) = trimmed.parse::<f64>() {
            return Ok(Val::Float(f + 1.0));
        }
    }

    // Non-numeric string: Perl-style alphanumeric increment
    // Reference: $PHP_SRC_PATH/Zend/zend_operators.c - increment_string
    let mut result = (*s).clone();
    
    // Find the last alphanumeric character
    let mut pos = result.len();
    while pos > 0 {
        pos -= 1;
        let ch = result[pos];
        
        // Check if alphanumeric
        if (ch >= b'0' && ch <= b'9') || (ch >= b'a' && ch <= b'z') || (ch >= b'A' && ch <= b'Z') {
            // Increment this character
            if ch == b'9' {
                result[pos] = b'0';
                // Carry to next position
            } else if ch == b'z' {
                result[pos] = b'a';
                // Carry to next position
            } else if ch == b'Z' {
                result[pos] = b'A';
                // Carry to next position
            } else if ch >= b'0' && ch < b'9' {
                result[pos] = ch + 1;
                return Ok(Val::String(Rc::new(result)));
            } else if ch >= b'a' && ch < b'z' {
                result[pos] = ch + 1;
                return Ok(Val::String(Rc::new(result)));
            } else if ch >= b'A' && ch < b'Z' {
                result[pos] = ch + 1;
                return Ok(Val::String(Rc::new(result)));
            }
            
            // If we got here, we need to carry
            if pos == 0 {
                // Need to prepend
                if ch == b'9' || (ch >= b'0' && ch <= b'9') {
                    result.insert(0, b'1');
                } else if ch >= b'a' && ch <= b'z' {
                    result.insert(0, b'a');
                } else if ch >= b'A' && ch <= b'Z' {
                    result.insert(0, b'A');
                }
                return Ok(Val::String(Rc::new(result)));
            }
        } else {
            // Non-alphanumeric, break and append
            break;
        }
    }
    
    // If we reach here and pos was decremented to 0, we've carried all the way
    // This should have been handled above, but as a fallback:
    Ok(Val::String(Rc::new(result)))
}

/// Decrement a string value - only numeric strings are affected
/// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - decrement_function
fn decrement_string(s: Rc<Vec<u8>>) -> Result<Val, VmError> {
    // Empty string: deprecated warning, becomes -1
    if s.is_empty() {
        // TODO: Add deprecated warning "Decrement on empty string is deprecated"
        return Ok(Val::Int(-1));
    }

    // Try parsing as numeric
    if let Ok(s_str) = std::str::from_utf8(&s) {
        let trimmed = s_str.trim();
        
        // Try as integer
        if let Ok(i) = trimmed.parse::<i64>() {
            if i == i64::MIN {
                return Ok(Val::Float(i as f64 - 1.0));
            } else {
                return Ok(Val::Int(i - 1));
            }
        }
        
        // Try as float
        if let Ok(f) = trimmed.parse::<f64>() {
            return Ok(Val::Float(f - 1.0));
        }
    }

    // Non-numeric string: NO CHANGE (unlike increment)
    // This is a key difference from increment - decrement doesn't do alphanumeric
    Ok(Val::String(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_increment_int() {
        assert_eq!(increment_value(Val::Int(5)).unwrap(), Val::Int(6));
        assert_eq!(increment_value(Val::Int(0)).unwrap(), Val::Int(1));
        assert_eq!(increment_value(Val::Int(-1)).unwrap(), Val::Int(0));
    }

    #[test]
    fn test_increment_int_overflow() {
        let result = increment_value(Val::Int(i64::MAX)).unwrap();
        match result {
            Val::Float(f) => assert!((f - 9223372036854775808.0).abs() < 1.0),
            _ => panic!("Expected float"),
        }
    }

    #[test]
    fn test_increment_float() {
        assert_eq!(increment_value(Val::Float(5.5)).unwrap(), Val::Float(6.5));
    }

    #[test]
    fn test_increment_null() {
        assert_eq!(increment_value(Val::Null).unwrap(), Val::Int(1));
    }

    #[test]
    fn test_increment_string_numeric() {
        assert_eq!(
            increment_value(Val::String(Rc::new(b"5".to_vec()))).unwrap(),
            Val::Int(6)
        );
        assert_eq!(
            increment_value(Val::String(Rc::new(b"5.5".to_vec()))).unwrap(),
            Val::Float(6.5)
        );
    }

    #[test]
    fn test_increment_string_alphanumeric() {
        // Test basic increment
        let result = increment_value(Val::String(Rc::new(b"a".to_vec()))).unwrap();
        if let Val::String(s) = result {
            assert_eq!(&*s, b"b");
        } else {
            panic!("Expected string");
        }

        // Test carry
        let result = increment_value(Val::String(Rc::new(b"z".to_vec()))).unwrap();
        if let Val::String(s) = result {
            assert_eq!(&*s, b"aa");
        } else {
            panic!("Expected string");
        }
    }

    #[test]
    fn test_decrement_int() {
        assert_eq!(decrement_value(Val::Int(5)).unwrap(), Val::Int(4));
        assert_eq!(decrement_value(Val::Int(0)).unwrap(), Val::Int(-1));
    }

    #[test]
    fn test_decrement_string_numeric() {
        assert_eq!(
            decrement_value(Val::String(Rc::new(b"5".to_vec()))).unwrap(),
            Val::Int(4)
        );
    }

    #[test]
    fn test_decrement_string_non_numeric() {
        // Non-numeric strings don't change
        let s = Rc::new(b"abc".to_vec());
        let result = decrement_value(Val::String(s.clone())).unwrap();
        if let Val::String(result_s) = result {
            assert_eq!(&*result_s, &*s);
        } else {
            panic!("Expected string");
        }
    }
}
