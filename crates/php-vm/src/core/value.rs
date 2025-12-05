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
    String(Vec<u8>), // PHP strings are byte arrays
    Array(IndexMap<ArrayKey, Handle>), // Recursive handles
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
    Str(Vec<u8>)
}

// The Container (Zval equivalent)
#[derive(Debug, Clone)]
pub struct Zval {
    pub value: Val,
    pub is_ref: bool, // Explicit Reference Flag (&$a)
}
