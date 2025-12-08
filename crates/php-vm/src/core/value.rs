use indexmap::IndexMap;
use std::rc::Rc;
use std::any::Any;
use std::fmt::Debug;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Handle(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Symbol(pub u32); // Interned String

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Protected,
    Private,
}

#[derive(Debug, Clone)]
pub enum Val {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Rc<Vec<u8>>), // PHP strings are byte arrays (COW)
    Array(Rc<IndexMap<ArrayKey, Handle>>), // Recursive handles (COW)
    Object(Handle),
    ObjPayload(ObjectData),
    Resource(Rc<dyn Any>), // Changed to Rc to support Clone
    AppendPlaceholder, // Internal use for $a[]
}

impl PartialEq for Val {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Val::Null, Val::Null) => true,
            (Val::Bool(a), Val::Bool(b)) => a == b,
            (Val::Int(a), Val::Int(b)) => a == b,
            (Val::Float(a), Val::Float(b)) => a == b,
            (Val::String(a), Val::String(b)) => a == b,
            (Val::Array(a), Val::Array(b)) => a == b,
            (Val::Object(a), Val::Object(b)) => a == b,
            (Val::ObjPayload(a), Val::ObjPayload(b)) => a == b,
            (Val::Resource(a), Val::Resource(b)) => Rc::ptr_eq(a, b),
            (Val::AppendPlaceholder, Val::AppendPlaceholder) => true,
            _ => false,
        }
    }
}

impl Val {
    /// Convert to boolean following PHP's zend_is_true semantics
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - zend_is_true
    pub fn to_bool(&self) -> bool {
        match self {
            Val::Null => false,
            Val::Bool(b) => *b,
            Val::Int(i) => *i != 0,
            Val::Float(f) => *f != 0.0 && !f.is_nan(),
            Val::String(s) => {
                // Empty string or "0" is false
                if s.is_empty() {
                    false
                } else if s.len() == 1 && s[0] == b'0' {
                    false
                } else {
                    true
                }
            }
            Val::Array(arr) => !arr.is_empty(),
            Val::Object(_) | Val::ObjPayload(_) | Val::Resource(_) => true,
            Val::AppendPlaceholder => false,
        }
    }

    /// Convert to integer following PHP's convert_to_long semantics
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - convert_to_long
    pub fn to_int(&self) -> i64 {
        match self {
            Val::Null => 0,
            Val::Bool(b) => if *b { 1 } else { 0 },
            Val::Int(i) => *i,
            Val::Float(f) => *f as i64,
            Val::String(s) => {
                // Parse numeric string
                Self::parse_numeric_string(s).0
            }
            Val::Array(arr) => if arr.is_empty() { 0 } else { 1 },
            Val::Object(_) | Val::ObjPayload(_) => 1,
            Val::Resource(_) => 0, // Resources typically convert to their ID
            Val::AppendPlaceholder => 0,
        }
    }

    /// Convert to float following PHP's convert_to_double semantics
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - convert_to_double
    pub fn to_float(&self) -> f64 {
        match self {
            Val::Null => 0.0,
            Val::Bool(b) => if *b { 1.0 } else { 0.0 },
            Val::Int(i) => *i as f64,
            Val::Float(f) => *f,
            Val::String(s) => {
                // Parse numeric string
                let (int_val, is_float) = Self::parse_numeric_string(s);
                if is_float {
                    // Re-parse as float for precision
                    if let Ok(s_str) = std::str::from_utf8(s) {
                        s_str.trim().parse::<f64>().unwrap_or(int_val as f64)
                    } else {
                        int_val as f64
                    }
                } else {
                    int_val as f64
                }
            }
            Val::Array(arr) => if arr.is_empty() { 0.0 } else { 1.0 },
            Val::Object(_) | Val::ObjPayload(_) => 1.0,
            Val::Resource(_) => 0.0,
            Val::AppendPlaceholder => 0.0,
        }
    }

    /// Parse numeric string to int, returning (value, is_float)
    /// Reference: $PHP_SRC_PATH/Zend/zend_operators.c - is_numeric_string_ex
    fn parse_numeric_string(s: &[u8]) -> (i64, bool) {
        if s.is_empty() {
            return (0, false);
        }

        // Trim leading whitespace
        let trimmed = s.iter()
            .skip_while(|&&b| b == b' ' || b == b'\t' || b == b'\n' || b == b'\r')
            .copied()
            .collect::<Vec<u8>>();
        
        if trimmed.is_empty() {
            return (0, false);
        }

        if let Ok(s_str) = std::str::from_utf8(&trimmed) {
            // Try parsing as int first
            if let Ok(i) = s_str.parse::<i64>() {
                return (i, false);
            }
            // Try as float
            if let Ok(f) = s_str.parse::<f64>() {
                return (f as i64, true);
            }
        }

        // Non-numeric string
        (0, false)
    }
}

#[derive(Debug, Clone)]
pub struct ObjectData {
    // Placeholder for object data
    pub class: Symbol,
    pub properties: IndexMap<Symbol, Handle>,
    pub internal: Option<Rc<dyn Any>>, // For internal classes like Closure
}

impl PartialEq for ObjectData {
    fn eq(&self, other: &Self) -> bool {
        self.class == other.class && self.properties == other.properties
        // Ignore internal for equality for now, or check ptr_eq
    }
}


#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ArrayKey {
    Int(i64),
    Str(Rc<Vec<u8>>)
}

// The Container (Zval equivalent)
#[derive(Debug, Clone)]
pub struct Zval {
    pub value: Val,
    pub is_ref: bool, // Explicit Reference Flag (&$a)
}
