use crate::core::value::Val;
use crate::vm::engine::VmError;
use std::rc::Rc;

/// Binary assignment operation types
/// These map to Zend opcodes (ZEND_ADD through ZEND_POW) minus 1
/// Ref: Zend/zend_vm_opcodes.h in PHP source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AssignOpType {
    Add = 0,    // ZEND_ADD - 1
    Sub = 1,    // ZEND_SUB - 1
    Mul = 2,    // ZEND_MUL - 1
    Div = 3,    // ZEND_DIV - 1
    Mod = 4,    // ZEND_MOD - 1
    Sl = 5,     // ZEND_SL - 1 (Shift Left)
    Sr = 6,     // ZEND_SR - 1 (Shift Right)
    Concat = 7, // ZEND_CONCAT - 1
    BwOr = 8,   // ZEND_BW_OR - 1
    BwAnd = 9,  // ZEND_BW_AND - 1
    BwXor = 10, // ZEND_BW_XOR - 1
    Pow = 11,   // ZEND_POW - 1
}

impl AssignOpType {
    /// Try to construct from a u8 value
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Add),
            1 => Some(Self::Sub),
            2 => Some(Self::Mul),
            3 => Some(Self::Div),
            4 => Some(Self::Mod),
            5 => Some(Self::Sl),
            6 => Some(Self::Sr),
            7 => Some(Self::Concat),
            8 => Some(Self::BwOr),
            9 => Some(Self::BwAnd),
            10 => Some(Self::BwXor),
            11 => Some(Self::Pow),
            _ => None,
        }
    }

    /// Get the operation name for error messages
    pub fn name(&self) -> &'static str {
        match self {
            Self::Add => "Add",
            Self::Sub => "Sub",
            Self::Mul => "Mul",
            Self::Div => "Div",
            Self::Mod => "Mod",
            Self::Sl => "Shift Left",
            Self::Sr => "Shift Right",
            Self::Concat => "Concat",
            Self::BwOr => "Bitwise OR",
            Self::BwAnd => "Bitwise AND",
            Self::BwXor => "Bitwise XOR",
            Self::Pow => "Pow",
        }
    }

    /// Perform the binary operation with PHP-like type coercion
    /// Ref: Zend/zend_operators.c - zend_binary_op()
    pub fn apply(&self, left: Val, right: Val) -> Result<Val, VmError> {
        match self {
            Self::Add => Self::add(left, right),
            Self::Sub => Self::sub(left, right),
            Self::Mul => Self::mul(left, right),
            Self::Div => Self::div(left, right),
            Self::Mod => Self::mod_op(left, right),
            Self::Sl => Self::shift_left(left, right),
            Self::Sr => Self::shift_right(left, right),
            Self::Concat => Self::concat(left, right),
            Self::BwOr => Self::bitwise_or(left, right),
            Self::BwAnd => Self::bitwise_and(left, right),
            Self::BwXor => Self::bitwise_xor(left, right),
            Self::Pow => Self::pow(left, right),
        }
    }

    fn add(left: Val, right: Val) -> Result<Val, VmError> {
        match (left, right) {
            (Val::Int(a), Val::Int(b)) => Ok(Val::Int(a.wrapping_add(b))),
            (Val::Float(a), Val::Float(b)) => Ok(Val::Float(a + b)),
            (Val::Int(a), Val::Float(b)) => Ok(Val::Float(a as f64 + b)),
            (Val::Float(a), Val::Int(b)) => Ok(Val::Float(a + b as f64)),
            (Val::Array(a), Val::Array(b)) => {
                // Array union
                let mut result = (*a).clone();
                for (k, v) in b.map.iter() {
                    result.map.entry(k.clone()).or_insert(*v);
                }
                Ok(Val::Array(Rc::new(result)))
            }
            _ => Ok(Val::Int(0)), // PHP coerces to numeric
        }
    }

    fn sub(left: Val, right: Val) -> Result<Val, VmError> {
        match (left, right) {
            (Val::Int(a), Val::Int(b)) => Ok(Val::Int(a.wrapping_sub(b))),
            (Val::Float(a), Val::Float(b)) => Ok(Val::Float(a - b)),
            (Val::Int(a), Val::Float(b)) => Ok(Val::Float(a as f64 - b)),
            (Val::Float(a), Val::Int(b)) => Ok(Val::Float(a - b as f64)),
            _ => Ok(Val::Int(0)),
        }
    }

    fn mul(left: Val, right: Val) -> Result<Val, VmError> {
        match (left, right) {
            (Val::Int(a), Val::Int(b)) => Ok(Val::Int(a.wrapping_mul(b))),
            (Val::Float(a), Val::Float(b)) => Ok(Val::Float(a * b)),
            (Val::Int(a), Val::Float(b)) => Ok(Val::Float(a as f64 * b)),
            (Val::Float(a), Val::Int(b)) => Ok(Val::Float(a * b as f64)),
            _ => Ok(Val::Int(0)),
        }
    }

    fn div(left: Val, right: Val) -> Result<Val, VmError> {
        match (left, right) {
            (Val::Int(a), Val::Int(b)) => {
                if b == 0 {
                    eprintln!("Warning: Division by zero");
                    return Ok(Val::Float(f64::INFINITY));
                }
                // Always return float for division to match PHP behavior
                Ok(Val::Float(a as f64 / b as f64))
            }
            (Val::Float(a), Val::Float(b)) => {
                if b == 0.0 {
                    eprintln!("Warning: Division by zero");
                    return Ok(Val::Float(f64::INFINITY));
                }
                Ok(Val::Float(a / b))
            }
            (Val::Int(a), Val::Float(b)) => {
                if b == 0.0 {
                    eprintln!("Warning: Division by zero");
                    return Ok(Val::Float(f64::INFINITY));
                }
                Ok(Val::Float(a as f64 / b))
            }
            (Val::Float(a), Val::Int(b)) => {
                if b == 0 {
                    eprintln!("Warning: Division by zero");
                    return Ok(Val::Float(f64::INFINITY));
                }
                Ok(Val::Float(a / b as f64))
            }
            _ => {
                eprintln!("Warning: Division by zero");
                Ok(Val::Float(f64::INFINITY))
            }
        }
    }

    fn mod_op(left: Val, right: Val) -> Result<Val, VmError> {
        match (left, right) {
            (Val::Int(a), Val::Int(b)) => {
                if b == 0 {
                    eprintln!("Warning: Modulo by zero");
                    return Ok(Val::Bool(false));
                }
                Ok(Val::Int(a % b))
            }
            (Val::Float(a), Val::Float(b)) => {
                if b == 0.0 {
                    eprintln!("Warning: Modulo by zero");
                    return Ok(Val::Bool(false));
                }
                Ok(Val::Int((a as i64) % (b as i64)))
            }
            (Val::Int(a), Val::Float(b)) => {
                if b == 0.0 {
                    eprintln!("Warning: Modulo by zero");
                    return Ok(Val::Bool(false));
                }
                Ok(Val::Int(a % (b as i64)))
            }
            (Val::Float(a), Val::Int(b)) => {
                if b == 0 {
                    eprintln!("Warning: Modulo by zero");
                    return Ok(Val::Bool(false));
                }
                Ok(Val::Int((a as i64) % b))
            }
            _ => {
                eprintln!("Warning: Modulo by zero");
                Ok(Val::Bool(false))
            }
        }
    }

    fn shift_left(left: Val, right: Val) -> Result<Val, VmError> {
        match (left, right) {
            (Val::Int(a), Val::Int(b)) => {
                if b < 0 || b >= 64 {
                    Ok(Val::Int(0))
                } else {
                    Ok(Val::Int(a.wrapping_shl(b as u32)))
                }
            }
            (Val::Float(a), Val::Int(b)) => {
                let a_int = a as i64;
                if b < 0 {
                    Ok(Val::Int(0))
                } else if b >= 64 {
                    Ok(Val::Int(0))
                } else {
                    Ok(Val::Int(a_int << b))
                }
            }
            _ => Ok(Val::Int(0)),
        }
    }

    fn shift_right(left: Val, right: Val) -> Result<Val, VmError> {
        match (left, right) {
            (Val::Int(a), Val::Int(b)) => {
                if b < 0 || b >= 64 {
                    Ok(Val::Int(if a < 0 { -1 } else { 0 }))
                } else {
                    Ok(Val::Int(a.wrapping_shr(b as u32)))
                }
            }
            (Val::Float(a), Val::Int(b)) => {
                let a_int = a as i64;
                if b < 0 {
                    Ok(Val::Int(0))
                } else if b >= 64 {
                    Ok(Val::Int(if a_int < 0 { -1 } else { 0 }))
                } else {
                    Ok(Val::Int(a_int >> b))
                }
            }
            _ => Ok(Val::Int(0)),
        }
    }

    fn concat(left: Val, right: Val) -> Result<Val, VmError> {
        let left_str = match left {
            Val::String(s) => String::from_utf8_lossy(&s).to_string(),
            Val::Int(i) => i.to_string(),
            Val::Float(f) => f.to_string(),
            Val::Bool(b) => if b { "1" } else { "" }.to_string(),
            Val::Null => String::new(),
            _ => String::new(),
        };

        let right_str = match right {
            Val::String(s) => String::from_utf8_lossy(&s).to_string(),
            Val::Int(i) => i.to_string(),
            Val::Float(f) => f.to_string(),
            Val::Bool(b) => if b { "1" } else { "" }.to_string(),
            Val::Null => String::new(),
            _ => String::new(),
        };

        let result = left_str + &right_str;
        Ok(Val::String(result.into_bytes().into()))
    }

    fn bitwise_or(left: Val, right: Val) -> Result<Val, VmError> {
        match (&left, &right) {
            (Val::String(a), Val::String(b)) => {
                // PHP performs bitwise OR on strings character by character
                let mut result = Vec::new();
                let max_len = a.len().max(b.len());
                for i in 0..max_len {
                    let byte_a = if i < a.len() { a[i] } else { 0 };
                    let byte_b = if i < b.len() { b[i] } else { 0 };
                    result.push(byte_a | byte_b);
                }
                Ok(Val::String(result.into()))
            }
            _ => {
                // Convert to int for bitwise operation (handles Bool, Null, etc.)
                let a = left.to_int();
                let b = right.to_int();
                Ok(Val::Int(a | b))
            }
        }
    }

    fn bitwise_and(left: Val, right: Val) -> Result<Val, VmError> {
        match (&left, &right) {
            (Val::String(a), Val::String(b)) => {
                // PHP performs bitwise AND on strings character by character
                let mut result = Vec::new();
                let min_len = a.len().min(b.len());
                for i in 0..min_len {
                    result.push(a[i] & b[i]);
                }
                Ok(Val::String(result.into()))
            }
            _ => {
                // Convert to int for bitwise operation (handles Bool, Null, etc.)
                let a = left.to_int();
                let b = right.to_int();
                Ok(Val::Int(a & b))
            }
        }
    }

    fn bitwise_xor(left: Val, right: Val) -> Result<Val, VmError> {
        match (&left, &right) {
            (Val::String(a), Val::String(b)) => {
                // PHP performs bitwise XOR on strings character by character
                // Uses MIN length (stops at shorter string)
                let mut result = Vec::new();
                let min_len = a.len().min(b.len());
                for i in 0..min_len {
                    result.push(a[i] ^ b[i]);
                }
                Ok(Val::String(result.into()))
            }
            _ => {
                // Convert to int for bitwise operation (handles Bool, Null, etc.)
                let a = left.to_int();
                let b = right.to_int();
                Ok(Val::Int(a ^ b))
            }
        }
    }

    fn pow(left: Val, right: Val) -> Result<Val, VmError> {
        match (left, right) {
            (Val::Int(a), Val::Int(b)) => {
                if b < 0 {
                    // Negative exponent returns float
                    Ok(Val::Float((a as f64).powf(b as f64)))
                } else if b > u32::MAX as i64 {
                    Ok(Val::Float((a as f64).powf(b as f64)))
                } else {
                    // Try to compute as int, fallback to float on overflow
                    match a.checked_pow(b as u32) {
                        Some(result) => Ok(Val::Int(result)),
                        None => Ok(Val::Float((a as f64).powf(b as f64))),
                    }
                }
            }
            (Val::Float(a), Val::Float(b)) => Ok(Val::Float(a.powf(b))),
            (Val::Int(a), Val::Float(b)) => Ok(Val::Float((a as f64).powf(b))),
            (Val::Float(a), Val::Int(b)) => Ok(Val::Float(a.powf(b as f64))),
            _ => Ok(Val::Int(0)),
        }
    }
}
