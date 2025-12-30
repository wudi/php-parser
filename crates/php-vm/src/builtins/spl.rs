use crate::core::value::{Handle, Val};
use crate::vm::engine::VM;
use std::rc::Rc;

/// spl_autoload_register() - Register a function for autoloading classes
pub fn php_spl_autoload_register(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        // Matching native behavior: registering the default autoloader succeeds
        return Ok(vm.arena.alloc(Val::Bool(true)));
    }

    let callback_handle = args[0];
    let callback_val = vm.arena.get(callback_handle);

    // Optional: throw argument (defaults to true)
    let throw_on_failure = args
        .get(1)
        .and_then(|handle| match vm.arena.get(*handle).value {
            Val::Bool(b) => Some(b),
            _ => None,
        })
        .unwrap_or(true);

    // Optional: prepend argument (defaults to false)
    let prepend = args
        .get(2)
        .and_then(|handle| match vm.arena.get(*handle).value {
            Val::Bool(b) => Some(b),
            _ => None,
        })
        .unwrap_or(false);

    let is_valid_callback = match &callback_val.value {
        Val::Null => false,
        Val::String(_) | Val::Array(_) | Val::Object(_) => true,
        _ => false,
    };

    if !is_valid_callback {
        if throw_on_failure {
            return Err(
                "spl_autoload_register(): Argument #1 must be a valid callback".to_string(),
            );
        } else {
            return Ok(vm.arena.alloc(Val::Bool(false)));
        }
    }

    // Avoid duplicate registrations of the same handle
    let already_registered = vm
        .context
        .autoloaders
        .iter()
        .any(|existing| existing == &callback_handle);

    if !already_registered {
        if prepend {
            vm.context.autoloaders.insert(0, callback_handle);
        } else {
            vm.context.autoloaders.push(callback_handle);
        }
    }

    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// spl_object_hash() - Retrieve a unique identifier for an object
pub fn php_spl_object_hash(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("spl_object_hash() expects at least 1 parameter".to_string());
    }

    let target_handle = args[0];
    let target_val = vm.arena.get(target_handle);

    let object_handle = match &target_val.value {
        Val::Object(payload_handle) => *payload_handle,
        _ => {
            return Err(format!(
                "spl_object_hash() expects parameter 1 to be object, {} given",
                target_val.value.type_name()
            ))
        }
    };

    let hash = format!("{:016x}", object_handle.0);
    let hash_bytes = Rc::new(hash.into_bytes());
    Ok(vm.arena.alloc(Val::String(hash_bytes)))
}
