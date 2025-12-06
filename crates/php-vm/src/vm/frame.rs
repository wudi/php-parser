use std::rc::Rc;
use std::collections::HashMap;
use crate::compiler::chunk::{CodeChunk, UserFunc};
use crate::core::value::{Symbol, Handle};

#[derive(Debug, Clone)]
pub struct CallFrame {
    pub chunk: Rc<CodeChunk>,
    pub func: Option<Rc<UserFunc>>,
    pub ip: usize,
    pub locals: HashMap<Symbol, Handle>,
    pub this: Option<Handle>,
    pub is_constructor: bool,
    pub class_scope: Option<Symbol>,
    pub called_scope: Option<Symbol>,
    pub generator: Option<Handle>,
    pub discard_return: bool,
}

impl CallFrame {
    pub fn new(chunk: Rc<CodeChunk>) -> Self {
        Self {
            chunk,
            func: None,
            ip: 0,
            locals: HashMap::new(),
            this: None,
            is_constructor: false,
            class_scope: None,
            called_scope: None,
            generator: None,
            discard_return: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SubGenState {
    Initial,
    Yielded,
    Resuming,
}

#[derive(Debug, Clone)]
pub enum SubIterator {
    Array { handle: Handle, index: usize },
    Generator { handle: Handle, state: SubGenState },
}

#[derive(Debug, Clone)]
pub enum GeneratorState {
    Created(CallFrame),
    Running,
    Suspended(CallFrame),
    Finished,
    Delegating(CallFrame),
}

#[derive(Debug, Clone)]
pub struct GeneratorData {
    pub state: GeneratorState,
    pub current_val: Option<Handle>,
    pub current_key: Option<Handle>,
    pub auto_key: i64,
    pub sub_iter: Option<SubIterator>,
    pub sent_val: Option<Handle>,
}
