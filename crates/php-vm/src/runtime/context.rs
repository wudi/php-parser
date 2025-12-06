use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use indexmap::IndexMap;
use crate::core::value::{Symbol, Val, Handle, Visibility};
use crate::core::interner::Interner;
use crate::vm::engine::VM;
use crate::compiler::chunk::UserFunc;
use crate::builtins::{string, array, class, variable};

pub type NativeHandler = fn(&mut VM, args: &[Handle]) -> Result<Handle, String>;

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub name: Symbol,
    pub parent: Option<Symbol>,
    pub is_interface: bool,
    pub is_trait: bool,
    pub interfaces: Vec<Symbol>,
    pub traits: Vec<Symbol>,
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
        let mut functions = HashMap::new();
        functions.insert(b"strlen".to_vec(), string::php_strlen as NativeHandler);
        functions.insert(b"str_repeat".to_vec(), string::php_str_repeat as NativeHandler);
        functions.insert(b"substr".to_vec(), string::php_substr as NativeHandler);
        functions.insert(b"strpos".to_vec(), string::php_strpos as NativeHandler);
        functions.insert(b"strtolower".to_vec(), string::php_strtolower as NativeHandler);
        functions.insert(b"strtoupper".to_vec(), string::php_strtoupper as NativeHandler);
        functions.insert(b"array_merge".to_vec(), array::php_array_merge as NativeHandler);
        functions.insert(b"array_keys".to_vec(), array::php_array_keys as NativeHandler);
        functions.insert(b"array_values".to_vec(), array::php_array_values as NativeHandler);
        functions.insert(b"var_dump".to_vec(), variable::php_var_dump as NativeHandler);
        functions.insert(b"count".to_vec(), array::php_count as NativeHandler);
        functions.insert(b"is_string".to_vec(), variable::php_is_string as NativeHandler);
        functions.insert(b"is_int".to_vec(), variable::php_is_int as NativeHandler);
        functions.insert(b"is_array".to_vec(), variable::php_is_array as NativeHandler);
        functions.insert(b"is_bool".to_vec(), variable::php_is_bool as NativeHandler);
        functions.insert(b"is_null".to_vec(), variable::php_is_null as NativeHandler);
        functions.insert(b"is_object".to_vec(), variable::php_is_object as NativeHandler);
        functions.insert(b"is_float".to_vec(), variable::php_is_float as NativeHandler);
        functions.insert(b"is_numeric".to_vec(), variable::php_is_numeric as NativeHandler);
        functions.insert(b"is_scalar".to_vec(), variable::php_is_scalar as NativeHandler);
        functions.insert(b"implode".to_vec(), string::php_implode as NativeHandler);
        functions.insert(b"explode".to_vec(), string::php_explode as NativeHandler);
        functions.insert(b"define".to_vec(), variable::php_define as NativeHandler);
        functions.insert(b"defined".to_vec(), variable::php_defined as NativeHandler);
        functions.insert(b"constant".to_vec(), variable::php_constant as NativeHandler);
        functions.insert(b"get_object_vars".to_vec(), class::php_get_object_vars as NativeHandler);
        functions.insert(b"get_class".to_vec(), class::php_get_class as NativeHandler);
        functions.insert(b"get_parent_class".to_vec(), class::php_get_parent_class as NativeHandler);
        functions.insert(b"is_subclass_of".to_vec(), class::php_is_subclass_of as NativeHandler);
        functions.insert(b"is_a".to_vec(), class::php_is_a as NativeHandler);
        functions.insert(b"class_exists".to_vec(), class::php_class_exists as NativeHandler);
        functions.insert(b"interface_exists".to_vec(), class::php_interface_exists as NativeHandler);
        functions.insert(b"trait_exists".to_vec(), class::php_trait_exists as NativeHandler);
        functions.insert(b"method_exists".to_vec(), class::php_method_exists as NativeHandler);
        functions.insert(b"property_exists".to_vec(), class::php_property_exists as NativeHandler);
        functions.insert(b"get_class_methods".to_vec(), class::php_get_class_methods as NativeHandler);
        functions.insert(b"get_class_vars".to_vec(), class::php_get_class_vars as NativeHandler);
        functions.insert(b"get_called_class".to_vec(), class::php_get_called_class as NativeHandler);
        functions.insert(b"gettype".to_vec(), variable::php_gettype as NativeHandler);
        functions.insert(b"var_export".to_vec(), variable::php_var_export as NativeHandler);

        Self {
            functions,
            constants: HashMap::new(),
        }
    }
}

pub struct RequestContext {
    pub engine: Arc<EngineContext>,
    pub globals: HashMap<Symbol, Handle>,
    pub constants: HashMap<Symbol, Val>,
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
            constants: HashMap::new(),
            user_functions: HashMap::new(),
            classes: HashMap::new(),
            included_files: HashSet::new(),
            interner: Interner::new(),
        }
    }
}
