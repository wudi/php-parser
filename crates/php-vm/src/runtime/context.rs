use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use indexmap::IndexMap;
use crate::core::value::{Symbol, Val, Handle, Visibility};
use crate::core::interner::Interner;
use crate::vm::engine::VM;
use crate::compiler::chunk::{CodeChunk, UserFunc, FuncParam};

pub type NativeHandler = fn(&mut VM, args: &[Handle]) -> Result<Handle, String>;

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub name: Symbol,
    pub parent: Option<Symbol>,
    pub methods: HashMap<Symbol, (Rc<UserFunc>, Visibility, bool)>, // (func, visibility, is_static)
    pub properties: IndexMap<Symbol, (Val, Visibility)>, // Default values
    pub constants: HashMap<Symbol, (Val, Visibility)>,
    pub static_properties: HashMap<Symbol, (Val, Visibility)>,
}

pub struct EngineContext {
    pub functions: HashMap<Vec<u8>, NativeHandler>,
    pub constants: HashMap<Symbol, Val>,
}

impl EngineContext {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            constants: HashMap::new(),
        }
    }
}

pub struct RequestContext {
    pub engine: Arc<EngineContext>,
    pub globals: HashMap<Symbol, Handle>,
    pub user_functions: HashMap<Symbol, Rc<UserFunc>>,
    pub classes: HashMap<Symbol, ClassDef>,
    pub included_files: HashSet<String>,
    pub interner: Interner,
}

impl RequestContext {
    pub fn new(engine: Arc<EngineContext>) -> Self {
        Self {
            engine,
            globals: HashMap::new(),
            user_functions: HashMap::new(),
            classes: HashMap::new(),
            included_files: HashSet::new(),
            interner: Interner::new(),
        }
    }
}
