use std::rc::Rc;
use std::collections::HashMap;
use crate::compiler::chunk::CodeChunk;
use crate::core::value::{Symbol, Handle};

#[derive(Debug)]
pub struct CallFrame {
    pub chunk: Rc<CodeChunk>,
    pub ip: usize,
    pub locals: HashMap<Symbol, Handle>,
    pub this: Option<Handle>,
    pub is_constructor: bool,
    pub class_scope: Option<Symbol>,
    pub called_scope: Option<Symbol>,
}

impl CallFrame {
    pub fn new(chunk: Rc<CodeChunk>) -> Self {
        Self {
            chunk,
            ip: 0,
            locals: HashMap::new(),
            this: None,
            is_constructor: false,
            class_scope: None,
            called_scope: None,
        }
    }
}
