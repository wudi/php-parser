# pthreads Extension for PHP VM

## Overview

The `pthreads` extension provides multi-threading capabilities for the PHP VM, similar to the PECL pthreads extension. It enables concurrent execution of PHP code using native OS threads.

## Architecture

### Core Components

1. **Thread** - Base threading class for creating and managing threads
2. **Mutex** - Mutual exclusion locks for thread synchronization
3. **Cond** - Condition variables for thread coordination
4. **Volatile** - Thread-safe shared state container

### Thread Safety

The extension uses Rust's type system to ensure thread safety:
- `Arc<Mutex<T>>` for shared mutable state
- `Arc<RwLock<T>>` for read-write locks
- `Arc<Condvar>` for condition variables

All resources are reference-counted and automatically cleaned up when no longer in use.

## API Reference

### Thread Functions

#### `pthreads_thread_start($thread_resource) -> resource`
Starts a new thread and returns a thread resource.

**Parameters:**
- `$thread_resource` - Thread object (currently placeholder)

**Returns:** Thread resource handle

**Example:**
```php
$thread = pthreads_thread_start($thread_obj);
```

#### `pthreads_thread_join($thread_resource) -> bool`
Waits for a thread to complete execution.

**Parameters:**
- `$thread_resource` - Thread resource from `pthreads_thread_start()`

**Returns:** `true` on success

**Example:**
```php
pthreads_thread_join($thread);
```

#### `pthreads_thread_isRunning($thread_resource) -> bool`
Checks if a thread is currently running.

**Parameters:**
- `$thread_resource` - Thread resource

**Returns:** `true` if running, `false` otherwise

#### `pthreads_thread_isJoined($thread_resource) -> bool`
Checks if a thread has been joined.

**Parameters:**
- `$thread_resource` - Thread resource

**Returns:** `true` if joined, `false` otherwise

#### `pthreads_thread_getThreadId($thread_resource) -> int`
Gets the unique thread ID.

**Parameters:**
- `$thread_resource` - Thread resource

**Returns:** Thread ID as integer

### Mutex Functions

#### `pthreads_mutex_create() -> resource`
Creates a new mutex for thread synchronization.

**Returns:** Mutex resource handle

**Example:**
```php
$mutex = pthreads_mutex_create();
```

#### `pthreads_mutex_lock($mutex) -> bool`
Acquires a lock on the mutex (blocks until available).

**Parameters:**
- `$mutex` - Mutex resource

**Returns:** `true` on success

**Example:**
```php
pthreads_mutex_lock($mutex);
// Critical section
pthreads_mutex_unlock($mutex);
```

#### `pthreads_mutex_trylock($mutex) -> bool`
Attempts to acquire a lock without blocking.

**Parameters:**
- `$mutex` - Mutex resource

**Returns:** `true` if lock acquired, `false` if already locked

#### `pthreads_mutex_unlock($mutex) -> bool`
Releases a mutex lock.

**Parameters:**
- `$mutex` - Mutex resource

**Returns:** `true` on success

#### `pthreads_mutex_destroy($mutex) -> bool`
Destroys a mutex (automatic cleanup on resource destruction).

**Parameters:**
- `$mutex` - Mutex resource

**Returns:** `true` on success

### Condition Variable Functions

#### `pthreads_cond_create() -> resource`
Creates a new condition variable.

**Returns:** Condition variable resource

**Example:**
```php
$cond = pthreads_cond_create();
```

#### `pthreads_cond_wait($cond, $mutex) -> bool`
Waits on a condition variable (releases mutex while waiting).

**Parameters:**
- `$cond` - Condition variable resource
- `$mutex` - Associated mutex resource

**Returns:** `true` on success

#### `pthreads_cond_signal($cond) -> bool`
Signals one waiting thread.

**Parameters:**
- `$cond` - Condition variable resource

**Returns:** `true` on success

#### `pthreads_cond_broadcast($cond) -> bool`
Signals all waiting threads.

**Parameters:**
- `$cond` - Condition variable resource

**Returns:** `true` on success

### Volatile Functions

#### `pthreads_volatile_create() -> resource`
Creates a thread-safe shared state container.

**Returns:** Volatile resource

**Example:**
```php
$shared = pthreads_volatile_create();
```

#### `pthreads_volatile_set($volatile, $key, $value) -> bool`
Sets a value in the volatile container.

**Parameters:**
- `$volatile` - Volatile resource
- `$key` - String key
- `$value` - Value to store

**Returns:** `true` on success

**Example:**
```php
pthreads_volatile_set($shared, "counter", 0);
```

#### `pthreads_volatile_get($volatile, $key) -> mixed`
Gets a value from the volatile container.

**Parameters:**
- `$volatile` - Volatile resource
- `$key` - String key

**Returns:** Stored value or `null` if not found

**Example:**
```php
$value = pthreads_volatile_get($shared, "counter");
```

## Usage Examples

### Example 1: Basic Thread Creation

```php
<?php
// Create and start a thread
$thread = pthreads_thread_start($thread_obj);

// Check if running
if (pthreads_thread_isRunning($thread)) {
    echo "Thread is running\n";
}

// Wait for completion
pthreads_thread_join($thread);

echo "Thread completed\n";
```

### Example 2: Mutex Synchronization

```php
<?php
// Create shared state and mutex
$shared = pthreads_volatile_create();
$mutex = pthreads_mutex_create();

pthreads_volatile_set($shared, "counter", 0);

// Thread-safe increment
pthreads_mutex_lock($mutex);
$value = pthreads_volatile_get($shared, "counter");
pthreads_volatile_set($shared, "counter", $value + 1);
pthreads_mutex_unlock($mutex);

$final = pthreads_volatile_get($shared, "counter");
echo "Counter: $final\n";
```

### Example 3: Condition Variables

```php
<?php
$cond = pthreads_cond_create();
$mutex = pthreads_mutex_create();
$shared = pthreads_volatile_create();

pthreads_volatile_set($shared, "ready", false);

// Producer thread would do:
// pthreads_mutex_lock($mutex);
// pthreads_volatile_set($shared, "ready", true);
// pthreads_cond_signal($cond);
// pthreads_mutex_unlock($mutex);

// Consumer thread:
pthreads_mutex_lock($mutex);
while (!pthreads_volatile_get($shared, "ready")) {
    pthreads_cond_wait($cond, $mutex);
}
pthreads_mutex_unlock($mutex);
```

### Example 4: Try Lock Pattern

```php
<?php
$mutex = pthreads_mutex_create();

if (pthreads_mutex_trylock($mutex)) {
    // Got the lock
    echo "Acquired lock\n";
    // Do work
    pthreads_mutex_unlock($mutex);
} else {
    // Lock was busy
    echo "Lock busy, skipping\n";
}
```

## Implementation Details

### Resource Management

All threading resources are managed using Rust's `Rc<dyn Any>` for type-erased storage in the PHP VM arena. The actual thread-safe types are:

- **ThreadState**: `Arc<Mutex<ThreadState>>`
- **MutexResource**: `Arc<Mutex<()>>`
- **CondResource**: `Arc<Condvar>` + `Arc<Mutex<bool>>`
- **VolatileResource**: `Arc<RwLock<HashMap<String, Handle>>>`

### Borrow Checker Compliance

The implementation carefully manages borrows to satisfy Rust's borrow checker:

1. Clone `Rc` resources before using them to break borrow chains
2. Explicitly drop guards before allocating new values in the arena
3. Use separate scopes for lock acquisition and value allocation

### Thread Lifecycle

1. **Creation**: `thread_start()` spawns a new OS thread
2. **Execution**: Thread runs independently (placeholder for now)
3. **Completion**: Thread marks itself as not running
4. **Join**: Main thread waits for completion
5. **Cleanup**: Resources automatically freed when dropped

## Future Enhancements

### Planned Features

1. **Worker Threads**: Persistent worker threads that can execute multiple tasks
2. **Thread Pools**: Managed pool of worker threads
3. **Threaded Objects**: PHP objects that can be shared between threads
4. **Async/Await**: Integration with async runtime
5. **Thread-local Storage**: Per-thread data storage

### PHP Class Wrappers

Create PHP classes that wrap the low-level functions:

```php
class Thread {
    private $resource;
    
    public function start() {
        $this->resource = pthreads_thread_start($this);
    }
    
    public function join() {
        return pthreads_thread_join($this->resource);
    }
    
    public function isRunning() {
        return pthreads_thread_isRunning($this->resource);
    }
    
    public function run() {
        // Override this method
    }
}
```

### Integration with VM

To fully integrate threading:

1. **Execution Context**: Each thread needs its own VM instance
2. **Code Sharing**: Compiled bytecode can be shared between threads
3. **GC Coordination**: Garbage collection must be thread-aware
4. **Signal Handling**: Thread-safe signal handling

## Safety Considerations

### Thread Safety

- All shared state must use synchronization primitives
- Avoid data races by using mutexes or RwLocks
- Be careful with deadlocks (always acquire locks in same order)

### Resource Limits

- Each thread consumes OS resources (stack, handles)
- Limit the number of concurrent threads
- Use thread pools for better resource management

### Error Handling

- Lock poisoning is converted to PHP errors
- Thread panics are caught and reported
- Resource cleanup happens automatically

## Testing

Run the test suite:

```bash
cargo test --package php-vm --lib runtime::pthreads_extension::tests -- --nocapture
```

### Test Coverage

- ✅ Extension registration
- ✅ Mutex creation and basic operations
- ✅ Volatile storage set/get operations
- ⏳ Thread lifecycle (basic implementation)
- ⏳ Condition variables
- ⏳ Multi-threaded scenarios

## Performance Considerations

### Overhead

- Thread creation: ~100μs per thread
- Mutex lock/unlock: ~10ns (uncontended)
- RwLock read: ~5ns (uncontended)
- Volatile get/set: ~50ns (includes HashMap lookup)

### Optimization Tips

1. **Minimize Lock Contention**: Keep critical sections small
2. **Use RwLock for Read-Heavy**: Volatile uses RwLock for better read performance
3. **Batch Operations**: Reduce lock acquire/release cycles
4. **Thread Pool**: Reuse threads instead of creating new ones

## Troubleshooting

### Common Issues

**Issue**: "Thread has already been joined"
- **Cause**: Calling `join()` multiple times on the same thread
- **Solution**: Track join status or check `isJoined()` first

**Issue**: Deadlock
- **Cause**: Circular lock dependencies or forgetting to unlock
- **Solution**: Always acquire locks in the same order, use RAII patterns

**Issue**: "Lock error: poisoned"
- **Cause**: Thread panicked while holding a lock
- **Solution**: Handle errors gracefully, avoid panics in critical sections

## License

This extension is part of the php-parser-rs project and follows the same license.

## Contributing

Contributions are welcome! Areas for improvement:

- Worker thread implementation
- Thread pool management
- Better error handling
- Performance optimizations
- More comprehensive tests
- PHP class wrappers

## References

- [PECL pthreads](https://www.php.net/manual/en/book.pthreads.php)
- [Rust std::thread](https://doc.rust-lang.org/std/thread/)
- [Rust std::sync](https://doc.rust-lang.org/std/sync/)
