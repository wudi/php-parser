use crate::core::value::{Symbol, Val, Handle};
use crate::vm::opcode::OpCode;
use std::rc::Rc;
use indexmap::IndexMap;

#[derive(Debug, Clone)]
pub struct UserFunc {
    pub params: Vec<FuncParam>,
    pub uses: Vec<Symbol>,
    pub chunk: Rc<CodeChunk>,
    pub is_static: bool,
    pub is_generator: bool,
}

#[derive(Debug, Clone)]
pub struct FuncParam {
    pub name: Symbol,
    pub by_ref: bool,
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
    pub catch_type: Option<Symbol>, // None for catch-all or finally?
}

#[derive(Debug, Default)]
pub struct CodeChunk {
    pub name: Symbol,         // File/Func name
    pub returns_ref: bool,    // Function returns by reference
    pub code: Vec<OpCode>,    // Instructions
    pub constants: Vec<Val>,  // Literals (Ints, Strings)
    pub lines: Vec<u32>,      // Line numbers for debug
    pub catch_table: Vec<CatchEntry>,
}
