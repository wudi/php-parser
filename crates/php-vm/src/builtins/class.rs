use crate::core::value::{ArrayKey, Handle, Val};
use crate::vm::engine::{PropertyCollectionMode, VM};
use indexmap::IndexMap;
use std::rc::Rc;

pub fn php_get_object_vars(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("get_object_vars() expects exactly 1 parameter".into());
    }

    let obj_handle = args[0];
    let obj_val = vm.arena.get(obj_handle);

    if let Val::Object(payload_handle) = &obj_val.value {
        let payload = vm.arena.get(*payload_handle);
        if let Val::ObjPayload(obj_data) = &payload.value {
            let mut result_map = IndexMap::new();
            let class_sym = obj_data.class;
            let current_scope = vm.get_current_class();

            let properties: Vec<(crate::core::value::Symbol, Handle)> =
                obj_data.properties.iter().map(|(k, v)| (*k, *v)).collect();

            for (prop_sym, val_handle) in properties {
                if vm
                    .check_prop_visibility(class_sym, prop_sym, current_scope)
                    .is_ok()
                {
                    let prop_name_bytes =
                        vm.context.interner.lookup(prop_sym).unwrap_or(b"").to_vec();
                    let key = ArrayKey::Str(Rc::new(prop_name_bytes));
                    result_map.insert(key, val_handle);
                }
            }

            return Ok(vm.arena.alloc(Val::Array(
                crate::core::value::ArrayData::from(result_map).into(),
            )));
        }
    }

    Err("get_object_vars() expects parameter 1 to be object".into())
}

pub fn php_get_class(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        if let Some(frame) = vm.frames.last() {
            if let Some(class_scope) = frame.class_scope {
                let name = vm
                    .context
                    .interner
                    .lookup(class_scope)
                    .unwrap_or(b"")
                    .to_vec();
                return Ok(vm.arena.alloc(Val::String(name.into())));
            }
        }
        return Err("get_class() called without object from outside a class".into());
    }

    let val = vm.arena.get(args[0]);
    if let Val::Object(h) = val.value {
        let obj_zval = vm.arena.get(h);
        if let Val::ObjPayload(obj_data) = &obj_zval.value {
            let class_name = vm
                .context
                .interner
                .lookup(obj_data.class)
                .unwrap_or(b"")
                .to_vec();
            return Ok(vm.arena.alloc(Val::String(class_name.into())));
        }
    }

    Err("get_class() called on non-object".into())
}

pub fn php_get_parent_class(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let class_name_sym = if args.is_empty() {
        if let Some(frame) = vm.frames.last() {
            if let Some(class_scope) = frame.class_scope {
                class_scope
            } else {
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        } else {
            return Ok(vm.arena.alloc(Val::Bool(false)));
        }
    } else {
        let val = vm.arena.get(args[0]);
        match &val.value {
            Val::Object(h) => {
                let obj_zval = vm.arena.get(*h);
                if let Val::ObjPayload(obj_data) = &obj_zval.value {
                    obj_data.class
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            Val::String(s) => {
                if let Some(sym) = vm.context.interner.find(s) {
                    sym
                } else {
                    return Ok(vm.arena.alloc(Val::Bool(false)));
                }
            }
            _ => return Ok(vm.arena.alloc(Val::Bool(false))),
        }
    };

    if let Some(def) = vm.context.classes.get(&class_name_sym) {
        if let Some(parent_sym) = def.parent {
            let parent_name = vm
                .context
                .interner
                .lookup(parent_sym)
                .unwrap_or(b"")
                .to_vec();
            return Ok(vm.arena.alloc(Val::String(parent_name.into())));
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_is_subclass_of(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("is_subclass_of() expects at least 2 parameters".into());
    }

    let object_or_class = vm.arena.get(args[0]);
    let class_name_val = vm.arena.get(args[1]);

    let child_sym = match &object_or_class.value {
        Val::Object(h) => {
            let obj_zval = vm.arena.get(*h);
            if let Val::ObjPayload(obj_data) = &obj_zval.value {
                obj_data.class
            } else {
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        }
        Val::String(s) => {
            if let Some(sym) = vm.context.interner.find(s) {
                sym
            } else {
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        }
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let parent_sym = match &class_name_val.value {
        Val::String(s) => {
            if let Some(sym) = vm.context.interner.find(s) {
                sym
            } else {
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        }
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    if child_sym == parent_sym {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }

    let result = vm.is_subclass_of(child_sym, parent_sym);
    Ok(vm.arena.alloc(Val::Bool(result)))
}

pub fn php_is_a(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("is_a() expects at least 2 parameters".into());
    }

    let object_or_class = vm.arena.get(args[0]);
    let class_name_val = vm.arena.get(args[1]);

    let child_sym = match &object_or_class.value {
        Val::Object(h) => {
            let obj_zval = vm.arena.get(*h);
            if let Val::ObjPayload(obj_data) = &obj_zval.value {
                obj_data.class
            } else {
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        }
        Val::String(s) => {
            if let Some(sym) = vm.context.interner.find(s) {
                sym
            } else {
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        }
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let parent_sym = match &class_name_val.value {
        Val::String(s) => {
            if let Some(sym) = vm.context.interner.find(s) {
                sym
            } else {
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        }
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    if child_sym == parent_sym {
        return Ok(vm.arena.alloc(Val::Bool(true)));
    }

    let result = vm.is_subclass_of(child_sym, parent_sym);
    Ok(vm.arena.alloc(Val::Bool(result)))
}

pub fn php_class_exists(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("class_exists() expects at least 1 parameter".into());
    }

    let val = vm.arena.get(args[0]);
    if let Val::String(s) = &val.value {
        if let Some(sym) = vm.context.interner.find(s) {
            if let Some(def) = vm.context.classes.get(&sym) {
                return Ok(vm
                    .arena
                    .alloc(Val::Bool(!def.is_interface && !def.is_trait)));
            }
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_interface_exists(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("interface_exists() expects at least 1 parameter".into());
    }

    let val = vm.arena.get(args[0]);
    if let Val::String(s) = &val.value {
        if let Some(sym) = vm.context.interner.find(s) {
            if let Some(def) = vm.context.classes.get(&sym) {
                return Ok(vm.arena.alloc(Val::Bool(def.is_interface)));
            }
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_trait_exists(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("trait_exists() expects at least 1 parameter".into());
    }

    let val = vm.arena.get(args[0]);
    if let Val::String(s) = &val.value {
        if let Some(sym) = vm.context.interner.find(s) {
            if let Some(def) = vm.context.classes.get(&sym) {
                return Ok(vm.arena.alloc(Val::Bool(def.is_trait)));
            }
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_method_exists(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("method_exists() expects exactly 2 parameters".into());
    }

    let object_or_class = vm.arena.get(args[0]);
    let method_name_val = vm.arena.get(args[1]);

    let class_sym = match &object_or_class.value {
        Val::Object(h) => {
            let obj_zval = vm.arena.get(*h);
            if let Val::ObjPayload(obj_data) = &obj_zval.value {
                obj_data.class
            } else {
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        }
        Val::String(s) => {
            if let Some(sym) = vm.context.interner.find(s) {
                sym
            } else {
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        }
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let method_sym = match &method_name_val.value {
        Val::String(s) => {
            if let Some(sym) = vm.context.interner.find(s) {
                sym
            } else {
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        }
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    let exists = vm.find_method(class_sym, method_sym).is_some();
    Ok(vm.arena.alloc(Val::Bool(exists)))
}

pub fn php_property_exists(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("property_exists() expects exactly 2 parameters".into());
    }

    let object_or_class = vm.arena.get(args[0]);
    let prop_name_val = vm.arena.get(args[1]);

    let prop_sym = match &prop_name_val.value {
        Val::String(s) => {
            if let Some(sym) = vm.context.interner.find(s) {
                sym
            } else {
                return Ok(vm.arena.alloc(Val::Bool(false)));
            }
        }
        _ => return Ok(vm.arena.alloc(Val::Bool(false))),
    };

    match &object_or_class.value {
        Val::Object(h) => {
            let obj_zval = vm.arena.get(*h);
            if let Val::ObjPayload(obj_data) = &obj_zval.value {
                // Check dynamic properties first
                if obj_data.properties.contains_key(&prop_sym) {
                    return Ok(vm.arena.alloc(Val::Bool(true)));
                }
                // Check class definition
                let exists = vm.has_property(obj_data.class, prop_sym);
                return Ok(vm.arena.alloc(Val::Bool(exists)));
            }
        }
        Val::String(s) => {
            if let Some(class_sym) = vm.context.interner.find(s) {
                let exists = vm.has_property(class_sym, prop_sym);
                return Ok(vm.arena.alloc(Val::Bool(exists)));
            }
        }
        _ => {}
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_get_class_methods(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("get_class_methods() expects exactly 1 parameter".into());
    }

    let val = vm.arena.get(args[0]);
    let class_sym = match &val.value {
        Val::Object(h) => {
            let obj_zval = vm.arena.get(*h);
            if let Val::ObjPayload(obj_data) = &obj_zval.value {
                obj_data.class
            } else {
                return Ok(vm
                    .arena
                    .alloc(Val::Array(crate::core::value::ArrayData::new().into())));
            }
        }
        Val::String(s) => {
            if let Some(sym) = vm.context.interner.find(s) {
                sym
            } else {
                return Ok(vm.arena.alloc(Val::Null));
            }
        }
        _ => return Ok(vm.arena.alloc(Val::Null)),
    };

    let caller_scope = vm.get_current_class();
    let methods = vm.collect_methods(class_sym, caller_scope);
    let mut array = IndexMap::new();

    for (i, method_sym) in methods.iter().enumerate() {
        let name = vm
            .context
            .interner
            .lookup(*method_sym)
            .unwrap_or(b"")
            .to_vec();
        let val_handle = vm.arena.alloc(Val::String(name.into()));
        array.insert(ArrayKey::Int(i as i64), val_handle);
    }

    Ok(vm.arena.alloc(Val::Array(
        crate::core::value::ArrayData::from(array).into(),
    )))
}

pub fn php_get_class_vars(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("get_class_vars() expects exactly 1 parameter".into());
    }

    let val = vm.arena.get(args[0]);
    let class_sym = match &val.value {
        Val::String(s) => {
            if let Some(sym) = vm.context.interner.find(s) {
                sym
            } else {
                return Err("Class does not exist".into());
            }
        }
        _ => return Err("get_class_vars() expects a string".into()),
    };

    let caller_scope = vm.get_current_class();
    let properties =
        vm.collect_properties(class_sym, PropertyCollectionMode::VisibleTo(caller_scope));
    let mut array = IndexMap::new();

    for (prop_sym, val_handle) in properties {
        let name = vm.context.interner.lookup(prop_sym).unwrap_or(b"").to_vec();
        let key = ArrayKey::Str(Rc::new(name));
        array.insert(key, val_handle);
    }

    Ok(vm.arena.alloc(Val::Array(
        crate::core::value::ArrayData::from(array).into(),
    )))
}

pub fn php_get_called_class(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let frame = vm
        .frames
        .last()
        .ok_or("get_called_class() called from outside a function".to_string())?;

    if let Some(scope) = frame.called_scope {
        let name = vm.context.interner.lookup(scope).unwrap_or(b"").to_vec();
        Ok(vm.arena.alloc(Val::String(name.into())))
    } else {
        Err("get_called_class() called from outside a class".into())
    }
}
