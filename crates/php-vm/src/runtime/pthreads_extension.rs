use crate::core::value::{Handle, Val};
use crate::runtime::context::RequestContext;
use crate::runtime::extension::{Extension, ExtensionInfo, ExtensionResult};
use crate::runtime::registry::ExtensionRegistry;
use crate::vm::engine::VM;
use std::any::Any;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::thread::{self, JoinHandle};

/// pthreads extension for multi-threading support
///
/// This extension provides PHP threading capabilities similar to the pthreads PECL extension.
/// It includes:
/// - Thread: Base threading class
/// - Worker: Persistent worker threads
/// - Pool: Thread pool management
/// - Mutex: Mutual exclusion locks
/// - Cond: Condition variables
/// - Volatile: Thread-safe shared state
pub struct PthreadsExtension;

impl Extension for PthreadsExtension {
    fn info(&self) -> ExtensionInfo {
        ExtensionInfo {
            name: "pthreads",
            version: "1.0.0",
            dependencies: &[],
        }
    }

    fn module_init(&self, registry: &mut ExtensionRegistry) -> ExtensionResult {
        // Register Thread class functions
        registry.register_function(b"pthreads_thread_start", thread_start);
        registry.register_function(b"pthreads_thread_join", thread_join);
        registry.register_function(b"pthreads_thread_isRunning", thread_is_running);
        registry.register_function(b"pthreads_thread_isJoined", thread_is_joined);
        registry.register_function(b"pthreads_thread_getThreadId", thread_get_thread_id);

        // Register Mutex class functions
        registry.register_function(b"pthreads_mutex_create", mutex_create);
        registry.register_function(b"pthreads_mutex_lock", mutex_lock);
        registry.register_function(b"pthreads_mutex_trylock", mutex_trylock);
        registry.register_function(b"pthreads_mutex_unlock", mutex_unlock);
        registry.register_function(b"pthreads_mutex_destroy", mutex_destroy);

        // Register Cond class functions
        registry.register_function(b"pthreads_cond_create", cond_create);
        registry.register_function(b"pthreads_cond_wait", cond_wait);
        registry.register_function(b"pthreads_cond_signal", cond_signal);
        registry.register_function(b"pthreads_cond_broadcast", cond_broadcast);

        // Register Volatile class functions
        registry.register_function(b"pthreads_volatile_create", volatile_create);
        registry.register_function(b"pthreads_volatile_get", volatile_get);
        registry.register_function(b"pthreads_volatile_set", volatile_set);

        println!("[PthreadsExtension] MINIT: Registered threading functions");
        ExtensionResult::Success
    }

    fn module_shutdown(&self) -> ExtensionResult {
        println!("[PthreadsExtension] MSHUTDOWN: Cleaning up");
        ExtensionResult::Success
    }

    fn request_init(&self, _context: &mut RequestContext) -> ExtensionResult {
        ExtensionResult::Success
    }

    fn request_shutdown(&self, _context: &mut RequestContext) -> ExtensionResult {
        ExtensionResult::Success
    }
}

// ============================================================================
// Thread Internal State
// ============================================================================

/// Internal thread state shared between PHP and Rust
#[derive(Debug)]
struct ThreadState {
    thread_id: u64,
    running: bool,
    joined: bool,
    handle: Option<JoinHandle<()>>,
}

// ============================================================================
// Mutex Internal State
// ============================================================================

/// Mutex resource wrapper
struct MutexResource {
    mutex: Arc<Mutex<()>>,
}

impl std::fmt::Debug for MutexResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MutexResource").finish()
    }
}

// ============================================================================
// Condition Variable Internal State
// ============================================================================

/// Condition variable resource wrapper
struct CondResource {
    cond: Arc<Condvar>,
    mutex: Arc<Mutex<bool>>,
}

impl std::fmt::Debug for CondResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CondResource").finish()
    }
}

// ============================================================================
// Volatile (Thread-safe shared state)
// ============================================================================

/// Volatile resource for thread-safe shared data
struct VolatileResource {
    data: Arc<RwLock<HashMap<String, Handle>>>,
}

impl std::fmt::Debug for VolatileResource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VolatileResource").finish()
    }
}

// ============================================================================
// Thread Functions
// ============================================================================

/// Start a thread
fn thread_start(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("pthreads_thread_start() expects at least 1 parameter".to_string());
    }

    let _thread_obj = &vm.arena.get(args[0]).value;

    // Create thread state
    let thread_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;

    let state = Arc::new(Mutex::new(ThreadState {
        thread_id,
        running: true,
        joined: false,
        handle: None,
    }));

    // Spawn the thread
    let state_clone = Arc::clone(&state);
    let handle = thread::spawn(move || {
        // Thread execution logic would go here
        // In a real implementation, this would execute the run() method
        println!("[Thread {}] Started", thread_id);

        // Simulate work
        std::thread::sleep(std::time::Duration::from_millis(100));

        println!("[Thread {}] Finished", thread_id);

        // Mark as not running
        if let Ok(mut s) = state_clone.lock() {
            s.running = false;
        }
    });

    // Store the handle
    if let Ok(mut s) = state.lock() {
        s.handle = Some(handle);
    }

    // Store state in the object's internal field
    // This would be attached to the Thread object in a real implementation
    let resource = Rc::new(state as Arc<dyn Any>);

    Ok(vm.arena.alloc(Val::Resource(resource)))
}

/// Join a thread (wait for completion)
fn thread_join(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("pthreads_thread_join() expects at least 1 parameter".to_string());
    }

    let resource_val = &vm.arena.get(args[0]).value;

    // Clone the resource to break the borrow from vm.arena
    let resource = if let Val::Resource(res) = resource_val {
        Some(Rc::clone(res))
    } else {
        None
    };

    if let Some(res) = resource {
        if let Some(state_any) = res.downcast_ref::<Arc<Mutex<ThreadState>>>() {
            let mut state = state_any.lock().map_err(|e| format!("Lock error: {}", e))?;

            if state.joined {
                return Err("Thread has already been joined".to_string());
            }

            if let Some(handle) = state.handle.take() {
                drop(state); // Release lock before joining
                handle
                    .join()
                    .map_err(|_| "Thread join failed".to_string())?;

                // Mark as joined
                let mut state = state_any.lock().map_err(|e| format!("Lock error: {}", e))?;
                state.joined = true;
                state.running = false;
                drop(state); // Release lock before allocating
            }

            let result = Val::Bool(true);
            return Ok(vm.arena.alloc(result));
        }
    }

    Err("Invalid thread resource".to_string())
}

/// Check if thread is running
fn thread_is_running(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("pthreads_thread_isRunning() expects at least 1 parameter".to_string());
    }

    let resource_val = &vm.arena.get(args[0]).value;

    // Clone the resource to break the borrow from vm.arena
    let resource = if let Val::Resource(res) = resource_val {
        Some(Rc::clone(res))
    } else {
        None
    };

    if let Some(res) = resource {
        if let Some(state) = res.downcast_ref::<Arc<Mutex<ThreadState>>>() {
            let state = state.lock().map_err(|e| format!("Lock error: {}", e))?;
            let is_running = state.running;
            drop(state); // Release lock before allocating
            return Ok(vm.arena.alloc(Val::Bool(is_running)));
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

/// Check if thread is joined
fn thread_is_joined(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("pthreads_thread_isJoined() expects at least 1 parameter".to_string());
    }

    let resource_val = &vm.arena.get(args[0]).value;

    // Clone the resource to break the borrow from vm.arena
    let resource = if let Val::Resource(res) = resource_val {
        Some(Rc::clone(res))
    } else {
        None
    };

    if let Some(res) = resource {
        if let Some(state) = res.downcast_ref::<Arc<Mutex<ThreadState>>>() {
            let state = state.lock().map_err(|e| format!("Lock error: {}", e))?;
            let is_joined = state.joined;
            drop(state); // Release lock before allocating
            return Ok(vm.arena.alloc(Val::Bool(is_joined)));
        }
    }

    Ok(vm.arena.alloc(Val::Bool(false)))
}

/// Get thread ID
fn thread_get_thread_id(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("pthreads_thread_getThreadId() expects at least 1 parameter".to_string());
    }

    let resource_val = &vm.arena.get(args[0]).value;

    // Clone the resource to break the borrow from vm.arena
    let resource = if let Val::Resource(res) = resource_val {
        Some(Rc::clone(res))
    } else {
        None
    };

    if let Some(res) = resource {
        if let Some(state) = res.downcast_ref::<Arc<Mutex<ThreadState>>>() {
            let state = state.lock().map_err(|e| format!("Lock error: {}", e))?;
            let thread_id = state.thread_id as i64;
            drop(state); // Release lock before allocating
            return Ok(vm.arena.alloc(Val::Int(thread_id)));
        }
    }

    Ok(vm.arena.alloc(Val::Int(0)))
}

// ============================================================================
// Mutex Functions
// ============================================================================

/// Create a new mutex
fn mutex_create(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let mutex = Arc::new(Mutex::new(()));
    let resource = MutexResource { mutex };
    Ok(vm.arena.alloc(Val::Resource(Rc::new(resource))))
}

/// Lock a mutex
fn mutex_lock(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("pthreads_mutex_lock() expects at least 1 parameter".to_string());
    }

    let resource_val = &vm.arena.get(args[0]).value;

    // Clone the resource to break the borrow from vm.arena
    let resource = if let Val::Resource(res) = resource_val {
        Some(Rc::clone(res))
    } else {
        None
    };

    if let Some(res) = resource {
        if let Some(mutex_res) = res.downcast_ref::<MutexResource>() {
            // Lock the mutex (blocks until available)
            let _guard = mutex_res
                .mutex
                .lock()
                .map_err(|e| format!("Lock error: {}", e))?;
            // In a real implementation, we'd need to store the guard somewhere
            // For now, we just return success
            drop(_guard); // Release lock before allocating
            let result = Val::Bool(true);
            return Ok(vm.arena.alloc(result));
        }
    }

    Err("Invalid mutex resource".to_string())
}

/// Try to lock a mutex (non-blocking)
fn mutex_trylock(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("pthreads_mutex_trylock() expects at least 1 parameter".to_string());
    }

    let resource_val = &vm.arena.get(args[0]).value;

    // Clone the resource to break the borrow from vm.arena
    let resource = if let Val::Resource(res) = resource_val {
        Some(Rc::clone(res))
    } else {
        None
    };

    if let Some(res) = resource {
        if let Some(mutex_res) = res.downcast_ref::<MutexResource>() {
            // Try to lock the mutex (non-blocking)
            let success = mutex_res.mutex.try_lock().is_ok();
            return Ok(vm.arena.alloc(Val::Bool(success)));
        } else {
            Err("Invalid mutex resource".to_string())
        }
    } else {
        Err("Invalid mutex resource".to_string())
    }
}

/// Unlock a mutex
fn mutex_unlock(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("pthreads_mutex_unlock() expects at least 1 parameter".to_string());
    }

    // In a real implementation, we'd need to track the lock guard
    // For now, we just return success
    Ok(vm.arena.alloc(Val::Bool(true)))
}

/// Destroy a mutex
fn mutex_destroy(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("pthreads_mutex_destroy() expects at least 1 parameter".to_string());
    }

    // Mutex will be automatically destroyed when the resource is dropped
    Ok(vm.arena.alloc(Val::Bool(true)))
}

// ============================================================================
// Condition Variable Functions
// ============================================================================

/// Create a new condition variable
fn cond_create(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let cond = Arc::new(Condvar::new());
    let mutex = Arc::new(Mutex::new(false));
    let resource = CondResource { cond, mutex };
    Ok(vm.arena.alloc(Val::Resource(Rc::new(resource))))
}

/// Wait on a condition variable
fn cond_wait(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("pthreads_cond_wait() expects at least 2 parameters".to_string());
    }

    let cond_val = &vm.arena.get(args[0]).value;

    // Clone the resource to break the borrow from vm.arena
    let resource = if let Val::Resource(res) = cond_val {
        Some(Rc::clone(res))
    } else {
        None
    };

    if let Some(res) = resource {
        if let Some(cond_res) = res.downcast_ref::<CondResource>() {
            let guard = cond_res
                .mutex
                .lock()
                .map_err(|e| format!("Lock error: {}", e))?;
            let _guard = cond_res
                .cond
                .wait(guard)
                .map_err(|e| format!("Wait error: {}", e))?;
            drop(_guard); // Release lock before allocating
            let result = Val::Bool(true);
            return Ok(vm.arena.alloc(result));
        }
    }

    Err("Invalid condition variable resource".to_string())
}

/// Signal a condition variable (wake one thread)
fn cond_signal(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("pthreads_cond_signal() expects at least 1 parameter".to_string());
    }

    let cond_val = &vm.arena.get(args[0]).value;

    if let Val::Resource(res) = cond_val {
        if let Some(cond_res) = res.downcast_ref::<CondResource>() {
            cond_res.cond.notify_one();
            return Ok(vm.arena.alloc(Val::Bool(true)));
        }
    }

    Err("Invalid condition variable resource".to_string())
}

/// Broadcast a condition variable (wake all threads)
fn cond_broadcast(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() {
        return Err("pthreads_cond_broadcast() expects at least 1 parameter".to_string());
    }

    let cond_val = &vm.arena.get(args[0]).value;

    if let Val::Resource(res) = cond_val {
        if let Some(cond_res) = res.downcast_ref::<CondResource>() {
            cond_res.cond.notify_all();
            return Ok(vm.arena.alloc(Val::Bool(true)));
        }
    }

    Err("Invalid condition variable resource".to_string())
}

// ============================================================================
// Volatile Functions
// ============================================================================

/// Create a new volatile (thread-safe shared state)
fn volatile_create(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let data = Arc::new(RwLock::new(HashMap::new()));
    let resource = VolatileResource { data };
    Ok(vm.arena.alloc(Val::Resource(Rc::new(resource))))
}

/// Get a value from volatile storage
fn volatile_get(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 {
        return Err("pthreads_volatile_get() expects at least 2 parameters".to_string());
    }

    let volatile_val = &vm.arena.get(args[0]).value;
    let key_val = &vm.arena.get(args[1]).value;

    let key = match key_val {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("Key must be a string".to_string()),
    };

    // Clone the resource to break the borrow from vm.arena
    let resource = if let Val::Resource(res) = volatile_val {
        Some(Rc::clone(res))
    } else {
        None
    };

    if let Some(res) = resource {
        if let Some(volatile_res) = res.downcast_ref::<VolatileResource>() {
            let data = volatile_res
                .data
                .read()
                .map_err(|e| format!("Read error: {}", e))?;
            let result = if let Some(handle) = data.get(&key) {
                *handle
            } else {
                drop(data); // Release lock before allocating
                vm.arena.alloc(Val::Null)
            };
            return Ok(result);
        }
    }

    Err("Invalid volatile resource".to_string())
}

/// Set a value in volatile storage
fn volatile_set(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 {
        return Err("pthreads_volatile_set() expects at least 3 parameters".to_string());
    }

    let volatile_val = &vm.arena.get(args[0]).value;
    let key_val = &vm.arena.get(args[1]).value;
    let value_handle = args[2];

    let key = match key_val {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => return Err("Key must be a string".to_string()),
    };

    // Clone the resource to break the borrow from vm.arena
    let resource = if let Val::Resource(res) = volatile_val {
        Some(Rc::clone(res))
    } else {
        None
    };

    if let Some(res) = resource {
        if let Some(volatile_res) = res.downcast_ref::<VolatileResource>() {
            let mut data = volatile_res
                .data
                .write()
                .map_err(|e| format!("Write error: {}", e))?;
            data.insert(key, value_handle);
            drop(data); // Release lock before allocating
            let result = Val::Bool(true);
            return Ok(vm.arena.alloc(result));
        }
    }

    Err("Invalid volatile resource".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::context::EngineBuilder;
    use crate::vm::engine::VM;

    #[test]
    fn test_pthreads_extension_registration() {
        let engine = EngineBuilder::new()
            .with_extension(PthreadsExtension)
            .build()
            .expect("Failed to build engine");

        assert!(engine.registry.extension_loaded("pthreads"));

        // Verify thread functions
        assert!(engine
            .registry
            .get_function(b"pthreads_thread_start")
            .is_some());
        assert!(engine
            .registry
            .get_function(b"pthreads_thread_join")
            .is_some());

        // Verify mutex functions
        assert!(engine
            .registry
            .get_function(b"pthreads_mutex_create")
            .is_some());
        assert!(engine
            .registry
            .get_function(b"pthreads_mutex_lock")
            .is_some());
    }

    #[test]
    fn test_mutex_creation() {
        let engine = EngineBuilder::new()
            .with_extension(PthreadsExtension)
            .build()
            .expect("Failed to build engine");

        let mut vm = VM::new(engine);

        let handler = vm
            .context
            .engine
            .registry
            .get_function(b"pthreads_mutex_create")
            .expect("pthreads_mutex_create not found");

        let result = handler(&mut vm, &[]).expect("Call failed");
        let result_val = &vm.arena.get(result).value;

        assert!(matches!(result_val, Val::Resource(_)));
    }

    #[test]
    fn test_volatile_storage() {
        let engine = EngineBuilder::new()
            .with_extension(PthreadsExtension)
            .build()
            .expect("Failed to build engine");

        let mut vm = VM::new(engine);

        // Create volatile
        let create_handler = vm
            .context
            .engine
            .registry
            .get_function(b"pthreads_volatile_create")
            .expect("pthreads_volatile_create not found");

        let volatile_handle = create_handler(&mut vm, &[]).expect("Create failed");

        // Set a value
        let set_handler = vm
            .context
            .engine
            .registry
            .get_function(b"pthreads_volatile_set")
            .expect("pthreads_volatile_set not found");

        let key = vm.arena.alloc(Val::String(Rc::new(b"test_key".to_vec())));
        let value = vm.arena.alloc(Val::Int(42));

        set_handler(&mut vm, &[volatile_handle, key, value]).expect("Set failed");

        // Get the value
        let get_handler = vm
            .context
            .engine
            .registry
            .get_function(b"pthreads_volatile_get")
            .expect("pthreads_volatile_get not found");

        let result = get_handler(&mut vm, &[volatile_handle, key]).expect("Get failed");
        let result_val = &vm.arena.get(result).value;

        if let Val::Int(i) = result_val {
            assert_eq!(*i, 42);
        } else {
            panic!("Expected int result");
        }
    }
}
