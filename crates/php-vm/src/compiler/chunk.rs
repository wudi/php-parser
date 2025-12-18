use crate::core::value::{Handle, Symbol, Val};
use crate::vm::opcode::OpCode;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct UserFunc {
    pub params: Vec<FuncParam>,
    pub uses: Vec<Symbol>,
    pub chunk: Rc<CodeChunk>,
    pub is_static: bool,
    pub is_generator: bool,
    pub statics: Rc<RefCell<HashMap<Symbol, Handle>>>,
    pub return_type: Option<ReturnType>,
}

#[derive(Debug, Clone)]
pub enum ReturnType {
    // Simple types
    Int,
    Float,
    String,
    Bool,
    Array,
    Object,
    Void,
    Never,
    Mixed,
    Null,
    True,
    False,
    Callable,
    Iterable,
    // Named class/interface
    Named(Symbol),
    // Union type (e.g., int|string)
    Union(Vec<ReturnType>),
    // Intersection type (e.g., A&B)
    Intersection(Vec<ReturnType>),
    // Nullable type (e.g., ?int)
    Nullable(Box<ReturnType>),
    // Static return type
    Static,
}

#[derive(Debug, Clone)]
pub struct FuncParam {
    pub name: Symbol,
    pub by_ref: bool,
    pub param_type: Option<ReturnType>,
}

#[derive(Debug, Clone)]
pub struct ClosureData {
    pub func: Rc<UserFunc>,
    pub captures: IndexMap<Symbol, Handle>,
    pub this: Option<Handle>,
}

#[derive(Debug, Clone)]
pub struct CatchEntry {
    pub start: u32,
    pub end: u32,
    pub target: u32,
    pub catch_type: Option<Symbol>,  // None for catch-all
    pub finally_target: Option<u32>, // Finally block target
}

#[derive(Debug, Default)]
pub struct CodeChunk {
    pub name: Symbol,              // File/Func name
    pub file_path: Option<String>, // Source file path
    pub returns_ref: bool,         // Function returns by reference
    pub code: Vec<OpCode>,         // Instructions
    pub constants: Vec<Val>,       // Literals (Ints, Strings)
    pub lines: Vec<u32>,           // Line numbers for debug
    pub catch_table: Vec<CatchEntry>,
}
