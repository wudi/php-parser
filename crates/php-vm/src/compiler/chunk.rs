use crate::core::value::{Symbol, Val};
use crate::vm::opcode::OpCode;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct UserFunc {
    pub params: Vec<Symbol>,
    pub chunk: Rc<CodeChunk>,
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
    pub code: Vec<OpCode>,    // Instructions
    pub constants: Vec<Val>,  // Literals (Ints, Strings)
    pub lines: Vec<u32>,      // Line numbers for debug
    pub catch_table: Vec<CatchEntry>,
}
