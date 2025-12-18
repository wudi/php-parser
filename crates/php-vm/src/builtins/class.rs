use crate::core::value::{ArrayKey, Handle, Val};
use crate::vm::engine::{PropertyCollectionMode, VM};
use indexmap::IndexMap;
use std::rc::Rc;

//=============================================================================
// Predefined Interface & Class Implementations
// Reference: $PHP_SRC_PATH/Zend/zend_interfaces.c
//=============================================================================

// Iterator interface methods (SPL)
// Reference: $PHP_SRC_PATH/Zend/zend_interfaces.c - zend_user_iterator
pub fn iterator_current(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("Iterator::current() called outside object context")?;

    // Default implementation returns null if not overridden
    Ok(vm.arena.alloc(Val::Null))
}

pub fn iterator_key(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("Iterator::key() called outside object context")?;

    Ok(vm.arena.alloc(Val::Null))
}

pub fn iterator_next(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

pub fn iterator_rewind(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

pub fn iterator_valid(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Bool(false)))
}

// IteratorAggregate interface
pub fn iterator_aggregate_get_iterator(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("IteratorAggregate::getIterator() must be implemented".into())
}

// Countable interface
// Reference: $PHP_SRC_PATH/Zend/zend_interfaces.c - spl_countable
pub fn countable_count(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("Countable::count() must be implemented".into())
}

// ArrayAccess interface methods
// Reference: $PHP_SRC_PATH/Zend/zend_interfaces.c - zend_user_arrayaccess
pub fn array_access_offset_exists(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("ArrayAccess::offsetExists() must be implemented".into())
}

pub fn array_access_offset_get(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("ArrayAccess::offsetGet() must be implemented".into())
}

pub fn array_access_offset_set(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("ArrayAccess::offsetSet() must be implemented".into())
}

pub fn array_access_offset_unset(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("ArrayAccess::offsetUnset() must be implemented".into())
}

// Serializable interface (deprecated in PHP 8.1, but still supported)
pub fn serializable_serialize(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("Serializable::serialize() must be implemented".into())
}

pub fn serializable_unserialize(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("Serializable::unserialize() must be implemented".into())
}

// Closure class methods
// Reference: $PHP_SRC_PATH/Zend/zend_closures.c
pub fn closure_bind(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // Closure::bind($closure, $newthis, $newscope = "static")
    // Returns a new closure with bound $this and/or class scope
    // For now, simplified implementation
    if args.is_empty() {
        return Err("Closure::bind() expects at least 1 parameter".into());
    }

    // Return the closure unchanged for now (full implementation would create new binding)
    Ok(args[0])
}

pub fn closure_bind_to(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // $closure->bindTo($newthis, $newscope = "static")
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("Closure::bindTo() called outside object context")?;

    // Return this unchanged for now
    Ok(this_handle)
}

pub fn closure_call(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // $closure->call($newThis, ...$args)
    Err("Closure::call() not yet fully implemented".into())
}

pub fn closure_from_callable(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // Closure::fromCallable($callable)
    if args.is_empty() {
        return Err("Closure::fromCallable() expects exactly 1 parameter".into());
    }

    // Would convert callable to Closure
    Ok(args[0])
}

// stdClass - empty class, allows dynamic properties
// Reference: $PHP_SRC_PATH/Zend/zend_builtin_functions.c
// No methods needed - pure data container

// Generator class methods
// Reference: $PHP_SRC_PATH/Zend/zend_generators.c
pub fn generator_current(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

pub fn generator_key(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

pub fn generator_next(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

pub fn generator_rewind(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    // Generators can only be rewound before first iteration
    Ok(vm.arena.alloc(Val::Null))
}

pub fn generator_send(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

pub fn generator_throw(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("Generator::throw() not yet implemented".into())
}

pub fn generator_valid(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn generator_get_return(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

// Fiber class methods (PHP 8.1+)
// Reference: $PHP_SRC_PATH/Zend/zend_fibers.c
pub fn fiber_construct(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // Fiber::__construct(callable $callback)
    if args.is_empty() {
        return Err("Fiber::__construct() expects exactly 1 parameter".into());
    }
    Ok(vm.arena.alloc(Val::Null))
}

pub fn fiber_start(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("Fiber::start() not yet implemented".into())
}

pub fn fiber_resume(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("Fiber::resume() not yet implemented".into())
}

pub fn fiber_suspend(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("Fiber::suspend() not yet implemented".into())
}

pub fn fiber_throw(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("Fiber::throw() not yet implemented".into())
}

pub fn fiber_is_started(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn fiber_is_suspended(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn fiber_is_running(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn fiber_is_terminated(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn fiber_get_return(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

pub fn fiber_get_current(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

// WeakReference class (PHP 7.4+)
// Reference: $PHP_SRC_PATH/Zend/zend_weakrefs.c
pub fn weak_reference_construct(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // WeakReference::__construct() - private, use ::create() instead
    Err("WeakReference::__construct() is private".into())
}

pub fn weak_reference_create(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // WeakReference::create(object $object): WeakReference
    if args.is_empty() {
        return Err("WeakReference::create() expects exactly 1 parameter".into());
    }

    let val = vm.arena.get(args[0]);
    if !matches!(val.value, Val::Object(_)) {
        return Err("WeakReference::create() expects parameter 1 to be object".into());
    }

    // Would create a WeakReference object
    Ok(args[0])
}

pub fn weak_reference_get(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    // Returns the referenced object or null if collected
    Ok(vm.arena.alloc(Val::Null))
}

// WeakMap class (PHP 8.0+)
// Reference: $PHP_SRC_PATH/Zend/zend_weakrefs.c
pub fn weak_map_construct(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

pub fn weak_map_offset_exists(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn weak_map_offset_get(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

pub fn weak_map_offset_set(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

pub fn weak_map_offset_unset(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

pub fn weak_map_count(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Int(0)))
}

pub fn weak_map_get_iterator(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("WeakMap::getIterator() not yet implemented".into())
}

// Stringable interface (PHP 8.0+)
// Reference: $PHP_SRC_PATH/Zend/zend_interfaces.c
pub fn stringable_to_string(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("Stringable::__toString() must be implemented".into())
}

// UnitEnum interface (PHP 8.1+)
// Reference: $PHP_SRC_PATH/Zend/zend_enum.c
pub fn unit_enum_cases(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    // Returns array of all enum cases
    Ok(vm
        .arena
        .alloc(Val::Array(crate::core::value::ArrayData::new().into())))
}

// BackedEnum interface (PHP 8.1+)
pub fn backed_enum_from(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Err("BackedEnum::from() not yet implemented".into())
}

pub fn backed_enum_try_from(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    Ok(vm.arena.alloc(Val::Null))
}

// SensitiveParameterValue class (PHP 8.2+)
// Reference: $PHP_SRC_PATH/Zend/zend_attributes.c
pub fn sensitive_parameter_value_construct(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // SensitiveParameterValue::__construct($value)
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("SensitiveParameterValue::__construct() called outside object context")?;

    let value = if args.is_empty() {
        vm.arena.alloc(Val::Null)
    } else {
        args[0]
    };

    let value_sym = vm.context.interner.intern(b"value");

    if let Val::Object(payload_handle) = &vm.arena.get(this_handle).value {
        let payload = vm.arena.get_mut(*payload_handle);
        if let Val::ObjPayload(ref mut obj_data) = payload.value {
            obj_data.properties.insert(value_sym, value);
        }
    }

    Ok(vm.arena.alloc(Val::Null))
}

pub fn sensitive_parameter_value_get_value(
    vm: &mut VM,
    _args: &[Handle],
) -> Result<Handle, String> {
    let this_handle = vm
        .frames
        .last()
        .and_then(|f| f.this)
        .ok_or("SensitiveParameterValue::getValue() called outside object context")?;

    let value_sym = vm.context.interner.intern(b"value");

    if let Val::Object(payload_handle) = &vm.arena.get(this_handle).value {
        if let Val::ObjPayload(obj_data) = &vm.arena.get(*payload_handle).value {
            if let Some(&val_handle) = obj_data.properties.get(&value_sym) {
                return Ok(val_handle);
            }
        }
    }

    Ok(vm.arena.alloc(Val::Null))
}

pub fn sensitive_parameter_value_debug_info(
    vm: &mut VM,
    _args: &[Handle],
) -> Result<Handle, String> {
    // __debugInfo() returns array with redacted value
    let mut array = IndexMap::new();
    let key = ArrayKey::Str(Rc::new(b"value".to_vec()));
    let val = vm.arena.alloc(Val::String(Rc::new(b"[REDACTED]".to_vec())));
    array.insert(key, val);

    Ok(vm.arena.alloc(Val::Array(
        crate::core::value::ArrayData::from(array).into(),
    )))
}

// __PHP_Incomplete_Class - used during unserialization
// Reference: $PHP_SRC_PATH/ext/standard/incomplete_class.c
pub fn incomplete_class_construct(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    // Should not be instantiated directly
    Err("__PHP_Incomplete_Class cannot be instantiated".into())
}

//=============================================================================
// Existing class introspection functions
//=============================================================================

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
