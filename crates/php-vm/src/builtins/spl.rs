use crate::core::value::{Handle, Val};
use crate::vm::engine::VM;

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
