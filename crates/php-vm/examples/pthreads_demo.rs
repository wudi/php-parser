use php_vm::core::value::Val;
use php_vm::runtime::context::EngineBuilder;
use php_vm::runtime::pthreads_extension::PthreadsExtension;
use php_vm::vm::engine::VM;
use std::rc::Rc;

fn main() {
    println!("=== pthreads Extension Demo ===\n");

    // Build engine with pthreads extension
    let engine = EngineBuilder::new()
        .with_extension(PthreadsExtension)
        .build()
        .expect("Failed to build engine");

    println!("✓ Engine built with pthreads extension\n");

    let mut vm = VM::new(engine);

    // Demo 1: Mutex Creation and Operations
    println!("--- Demo 1: Mutex Operations ---");
    demo_mutex(&mut vm);
    println!();

    // Demo 2: Volatile Storage
    println!("--- Demo 2: Volatile Storage ---");
    demo_volatile(&mut vm);
    println!();

    // Demo 3: Condition Variables
    println!("--- Demo 3: Condition Variables ---");
    demo_condition_variables(&mut vm);
    println!();

    println!("=== All Demos Completed Successfully ===");
}

fn demo_mutex(vm: &mut VM) {
    // Create a mutex
    let create_handler = vm
        .context
        .engine
        .registry
        .get_function(b"pthreads_mutex_create")
        .expect("pthreads_mutex_create not found");

    let mutex = create_handler(vm, &[]).expect("Failed to create mutex");
    println!("✓ Created mutex");

    // Try to lock it
    let trylock_handler = vm
        .context
        .engine
        .registry
        .get_function(b"pthreads_mutex_trylock")
        .expect("pthreads_mutex_trylock not found");

    let result = trylock_handler(vm, &[mutex]).expect("Failed to trylock");
    let result_val = &vm.arena.get(result).value;

    if let Val::Bool(success) = result_val {
        println!("✓ Trylock result: {}", success);
    }

    // Lock it (blocking)
    let lock_handler = vm
        .context
        .engine
        .registry
        .get_function(b"pthreads_mutex_lock")
        .expect("pthreads_mutex_lock not found");

    let _result = lock_handler(vm, &[mutex]).expect("Failed to lock");
    println!("✓ Acquired lock");

    // Unlock it
    let unlock_handler = vm
        .context
        .engine
        .registry
        .get_function(b"pthreads_mutex_unlock")
        .expect("pthreads_mutex_unlock not found");

    let _result = unlock_handler(vm, &[mutex]).expect("Failed to unlock");
    println!("✓ Released lock");
}

fn demo_volatile(vm: &mut VM) {
    // Create volatile storage
    let create_handler = vm
        .context
        .engine
        .registry
        .get_function(b"pthreads_volatile_create")
        .expect("pthreads_volatile_create not found");

    let volatile = create_handler(vm, &[]).expect("Failed to create volatile");
    println!("✓ Created volatile storage");

    // Set some values
    let set_handler = vm
        .context
        .engine
        .registry
        .get_function(b"pthreads_volatile_set")
        .expect("pthreads_volatile_set not found");

    let key1 = vm.arena.alloc(Val::String(Rc::new(b"counter".to_vec())));
    let value1 = vm.arena.alloc(Val::Int(42));
    set_handler(vm, &[volatile, key1, value1]).expect("Failed to set counter");
    println!("✓ Set counter = 42");

    let key2 = vm.arena.alloc(Val::String(Rc::new(b"name".to_vec())));
    let value2 = vm.arena.alloc(Val::String(Rc::new(b"pthreads".to_vec())));
    set_handler(vm, &[volatile, key2, value2]).expect("Failed to set name");
    println!("✓ Set name = \"pthreads\"");

    // Get values back
    let get_handler = vm
        .context
        .engine
        .registry
        .get_function(b"pthreads_volatile_get")
        .expect("pthreads_volatile_get not found");

    let result = get_handler(vm, &[volatile, key1]).expect("Failed to get counter");
    let result_val = &vm.arena.get(result).value;
    if let Val::Int(i) = result_val {
        println!("✓ Got counter = {}", i);
    }

    let result = get_handler(vm, &[volatile, key2]).expect("Failed to get name");
    let result_val = &vm.arena.get(result).value;
    if let Val::String(s) = result_val {
        println!("✓ Got name = \"{}\"", String::from_utf8_lossy(s));
    }

    // Try to get non-existent key
    let key3 = vm
        .arena
        .alloc(Val::String(Rc::new(b"nonexistent".to_vec())));
    let result = get_handler(vm, &[volatile, key3]).expect("Failed to get nonexistent");
    let result_val = &vm.arena.get(result).value;
    if let Val::Null = result_val {
        println!("✓ Non-existent key returns null");
    }
}

fn demo_condition_variables(vm: &mut VM) {
    // Create condition variable
    let create_handler = vm
        .context
        .engine
        .registry
        .get_function(b"pthreads_cond_create")
        .expect("pthreads_cond_create not found");

    let cond = create_handler(vm, &[]).expect("Failed to create cond");
    println!("✓ Created condition variable");

    // Signal it
    let signal_handler = vm
        .context
        .engine
        .registry
        .get_function(b"pthreads_cond_signal")
        .expect("pthreads_cond_signal not found");

    let _result = signal_handler(vm, &[cond]).expect("Failed to signal");
    println!("✓ Signaled condition variable");

    // Broadcast it
    let broadcast_handler = vm
        .context
        .engine
        .registry
        .get_function(b"pthreads_cond_broadcast")
        .expect("pthreads_cond_broadcast not found");

    let _result = broadcast_handler(vm, &[cond]).expect("Failed to broadcast");
    println!("✓ Broadcast condition variable");
}
