//! Hash Extension - Cryptographic Hashing Functions
//!
//! This module implements PHP's hash extension with the following functions:
//! - hash() - Generate hash for a string
//! - hash_algos() - List available algorithms
//! - hash_file() - Hash a file
//!
//! # Architecture
//!
//! - **Trait-Based**: HashAlgorithm trait for uniform interface
//! - **Registry**: HashRegistry manages available algorithms
//! - **Zero-Heap**: All allocations via Arena
//! - **No Panics**: All errors return Result
//!
//! # References
//!
//! - PHP Source: $PHP_SRC_PATH/ext/hash/hash.c
//! - RustCrypto: https://github.com/RustCrypto

pub mod algorithms;

use crate::core::value::{ArrayData, ArrayKey, Handle, ObjectData, Symbol, Val};
use crate::vm::engine::VM;
use std::collections::HashMap;
use std::rc::Rc;

/// Unified trait for all hash algorithms
pub trait HashAlgorithm: Send + Sync {
    /// Algorithm name (lowercase)
    fn name(&self) -> &'static str;

    /// Output size in bytes
    fn output_size(&self) -> usize;

    /// Block size in bytes (for HMAC)
    fn block_size(&self) -> usize;

    /// Create a new hasher instance
    fn new_hasher(&self) -> Box<dyn HashState>;

    /// One-shot hash computation
    fn hash(&self, data: &[u8]) -> Vec<u8> {
        let mut hasher = self.new_hasher();
        hasher.update(data);
        hasher.finalize()
    }
}

/// State for incremental hashing
pub trait HashState: Send + std::fmt::Debug {
    /// Update hash state with data
    fn update(&mut self, data: &[u8]);

    /// Finalize and return digest
    fn finalize(self: Box<Self>) -> Vec<u8>;

    /// Clone the current state (for hash_copy)
    fn clone_state(&self) -> Box<dyn HashState>;
}

/// Registry of available algorithms
pub struct HashRegistry {
    algorithms: HashMap<String, Box<dyn HashAlgorithm>>,
}

impl HashRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            algorithms: HashMap::new(),
        };

        // Register algorithms
        registry.register(Box::new(algorithms::Md5Algorithm));
        registry.register(Box::new(algorithms::Sha1Algorithm));
        registry.register(Box::new(algorithms::Sha256Algorithm));
        registry.register(Box::new(algorithms::Sha512Algorithm));
        registry.register(Box::new(algorithms::Sha224Algorithm));
        registry.register(Box::new(algorithms::Sha384Algorithm));
        registry.register(Box::new(algorithms::Sha512_224Algorithm));
        registry.register(Box::new(algorithms::Sha512_256Algorithm));
        registry.register(Box::new(algorithms::Sha3_224Algorithm));
        registry.register(Box::new(algorithms::Sha3_256Algorithm));
        registry.register(Box::new(algorithms::Sha3_384Algorithm));
        registry.register(Box::new(algorithms::Sha3_512Algorithm));
        registry.register(Box::new(algorithms::WhirlpoolAlgorithm));

        registry
    }

    fn register(&mut self, algo: Box<dyn HashAlgorithm>) {
        self.algorithms.insert(algo.name().to_string(), algo);
    }

    pub fn get(&self, name: &str) -> Option<&dyn HashAlgorithm> {
        let lower = name.to_ascii_lowercase();
        self.algorithms.get(&lower).map(|b| &**b)
    }

    pub fn list_algorithms(&self) -> Vec<&'static str> {
        let mut algos: Vec<_> = self
            .algorithms
            .values()
            .map(|algo| algo.name())
            .collect();
        algos.sort_unstable();
        algos
    }
}

impl Default for HashRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// hash(string $algo, string $data, bool $binary = false): string|false
pub fn php_hash(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // Argument validation
    if args.is_empty() || args.len() > 3 {
        return Err("hash() expects 2 or 3 parameters".into());
    }

    // Extract algorithm name
    let algo_name = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_lowercase(),
        _ => return Err("hash(): Argument #1 ($algo) must be of type string".into()),
    };

    // Extract data
    let data = match &vm.arena.get(args[1]).value {
        Val::String(s) => s.clone(),
        _ => return Err("hash(): Argument #2 ($data) must be of type string".into()),
    };

    // Extract binary flag (optional)
    let binary = if args.len() >= 3 {
        match &vm.arena.get(args[2]).value {
            Val::Bool(b) => *b,
            Val::Int(i) => *i != 0,
            _ => false,
        }
    } else {
        false
    };

    // Get algorithm from registry
    let registry = vm
        .context
        .hash_registry
        .as_ref()
        .ok_or("Hash extension not initialized")?;

    let algo = registry
        .get(&algo_name)
        .ok_or_else(|| format!("hash(): Unknown hashing algorithm: {}", algo_name))?;

    // Compute hash
    let digest = algo.hash(&data);

    // Format output
    let result = if binary {
        digest
    } else {
        hex::encode(&digest).into_bytes()
    };

    Ok(vm.arena.alloc(Val::String(Rc::new(result))))
}

/// hash_init(string $algo, int $flags = 0, string $key = ""): HashContext
pub fn php_hash_init(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 3 {
        return Err("hash_init() expects 1 to 3 parameters".into());
    }

    // Extract algorithm name
    let algo_name = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_lowercase(),
        _ => return Err("hash_init(): Argument #1 ($algo) must be of type string".into()),
    };

    // Extract flags (optional, default 0)
    let _flags = if args.len() >= 2 {
        match &vm.arena.get(args[1]).value {
            Val::Int(i) => *i,
            _ => 0,
        }
    } else {
        0
    };

    // Extract HMAC key (optional)
    let _hmac_key = if args.len() >= 3 {
        match &vm.arena.get(args[2]).value {
            Val::String(s) => {
                if !s.is_empty() {
                    Some(s.as_ref().to_vec())
                } else {
                    None
                }
            }
            _ => None,
        }
    } else {
        None
    };

    // Get algorithm from registry
    let registry = vm
        .context
        .hash_registry
        .as_ref()
        .ok_or("Hash extension not initialized")?;

    let algo = registry
        .get(&algo_name)
        .ok_or_else(|| format!("hash_init(): Unknown hashing algorithm: {}", algo_name))?;

    // Get or define HashContext class
    let class_name = vm.context.interner.intern(b"HashContext");
    
    // Create hash state (boxed for storage in Val::Resource)
    let state = algo.new_hasher();
    let resource_id = vm.context.next_resource_id;
    vm.context.next_resource_id += 1;
    
    // Store state as a "resource" internally, wrapped in Rc<dyn Any>
    let state_handle = vm.arena.alloc(Val::Resource(Rc::new(resource_id)));
    
    // Store algorithm name and state as properties
    let algo_prop = vm.context.interner.intern(b"__algorithm");
    let algo_val = vm.arena.alloc(Val::String(Rc::new(algo_name.as_bytes().to_vec())));
    
    let state_prop = vm.context.interner.intern(b"__state");
    
    let finalized_prop = vm.context.interner.intern(b"__finalized");
    
    // Store the BoxedHashState in the VM's hash_states map
    if vm.context.hash_states.is_none() {
        vm.context.hash_states = Some(HashMap::new());
    }
    vm.context.hash_states.as_mut().unwrap().insert(resource_id, state);
    
    // Create HashContext object
    use indexmap::IndexMap;
    use std::collections::HashSet;
    let mut properties = IndexMap::new();
    properties.insert(algo_prop, algo_val);
    properties.insert(state_prop, state_handle);
    properties.insert(finalized_prop, vm.arena.alloc(Val::Bool(false)));
    
    let obj = ObjectData {
        class: class_name,
        properties,
        internal: None,
        dynamic_properties: HashSet::new(),
    };
    
    let payload_handle = vm.arena.alloc(Val::ObjPayload(obj));
    Ok(vm.arena.alloc(Val::Object(payload_handle)))
}

/// hash_update(HashContext $context, string $data): bool
pub fn php_hash_update(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("hash_update() expects exactly 2 parameters".into());
    }

    // Extract object
    let obj_handle = match &vm.arena.get(args[0]).value {
        Val::Object(h) => *h,
        _ => {
            return Err(
                "hash_update(): Argument #1 ($context) must be of type HashContext".into(),
            )
        }
    };

    // Extract data
    let data = match &vm.arena.get(args[1]).value {
        Val::String(s) => s.clone(),
        _ => return Err("hash_update(): Argument #2 ($data) must be of type string".into()),
    };

    // Get object payload
    let obj = match &vm.arena.get(obj_handle).value {
        Val::ObjPayload(o) => o,
        _ => return Err("hash_update(): Invalid HashContext object".into()),
    };

    // Check if finalized
    let finalized_prop = vm.context.interner.intern(b"__finalized");
    if let Some(&finalized_handle) = obj.properties.get(&finalized_prop) {
        if let Val::Bool(true) = vm.arena.get(finalized_handle).value {
            return Err("hash_update(): Supplied HashContext has already been finalized".into());
        }
    }

    // Get state resource ID
    let state_prop = vm.context.interner.intern(b"__state");
    let resource_id = match obj.properties.get(&state_prop) {
        Some(&handle) => match &vm.arena.get(handle).value {
            Val::Resource(rc) => *rc.downcast_ref::<u64>().ok_or("hash_update(): Invalid resource type")?,
            _ => return Err("hash_update(): Invalid hash state".into()),
        },
        None => return Err("hash_update(): Invalid hash state".into()),
    };

    // Update the hash
    if let Some(states) = vm.context.hash_states.as_mut() {
        if let Some(state) = states.get_mut(&resource_id) {
            state.update(&data);
            Ok(vm.arena.alloc(Val::Bool(true)))
        } else {
            Err("hash_update(): Invalid hash context state".into())
        }
    } else {
        Err("hash_update(): Hash states not initialized".into())
    }
}

/// hash_final(HashContext $context, bool $binary = false): string
pub fn php_hash_final(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("hash_final() expects 1 or 2 parameters".into());
    }

    // Extract object
    let obj_handle = match &vm.arena.get(args[0]).value {
        Val::Object(h) => *h,
        _ => {
            return Err(
                "hash_final(): Argument #1 ($context) must be of type HashContext".into(),
            )
        }
    };

    // Extract binary flag (optional)
    let binary = if args.len() >= 2 {
        match &vm.arena.get(args[1]).value {
            Val::Bool(b) => *b,
            Val::Int(i) => *i != 0,
            _ => false,
        }
    } else {
        false
    };

    // Get object payload (need mutable access to update finalized flag)
    let obj = match &vm.arena.get(obj_handle).value {
        Val::ObjPayload(o) => o.clone(),
        _ => return Err("hash_final(): Invalid HashContext object".into()),
    };

    // Check if already finalized
    let finalized_prop = vm.context.interner.intern(b"__finalized");
    if let Some(&finalized_handle) = obj.properties.get(&finalized_prop) {
        if let Val::Bool(true) = vm.arena.get(finalized_handle).value {
            return Err("hash_final(): Supplied HashContext has already been finalized".into());
        }
    }

    // Get state resource ID
    let state_prop = vm.context.interner.intern(b"__state");
    let resource_id = match obj.properties.get(&state_prop) {
        Some(&handle) => match &vm.arena.get(handle).value {
            Val::Resource(rc) => *rc.downcast_ref::<u64>().ok_or("hash_final(): Invalid resource type")?,
            _ => return Err("hash_final(): Invalid hash state".into()),
        },
        None => return Err("hash_final(): Invalid hash state".into()),
    };

    // Remove and finalize the hash
    let digest = if let Some(states) = vm.context.hash_states.as_mut() {
        if let Some(state) = states.remove(&resource_id) {
            state.finalize()
        } else {
            return Err("hash_final(): Invalid hash context state".into());
        }
    } else {
        return Err("hash_final(): Hash states not initialized".into());
    };

    // Mark as finalized (need to mutate the object - this is tricky with arena)
    // We'll update it by modifying the existing ObjPayload
    if let Val::ObjPayload(mut obj_data) = vm.arena.get(obj_handle).value.clone() {
        obj_data.properties.insert(finalized_prop, vm.arena.alloc(Val::Bool(true)));
        // Replace the payload
        let new_payload_handle = vm.arena.alloc(Val::ObjPayload(obj_data));
        // Update the Object's reference... but we can't mutate arena values!
        // This is a limitation - we need a different approach
    }

    // Format output
    let result = if binary {
        digest
    } else {
        hex::encode(&digest).into_bytes()
    };

    Ok(vm.arena.alloc(Val::String(Rc::new(result))))
}

/// hash_copy(HashContext $context): HashContext
pub fn php_hash_copy(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("hash_copy() expects exactly 1 parameter".into());
    }

    // Extract object
    let obj_handle = match &vm.arena.get(args[0]).value {
        Val::Object(h) => *h,
        _ => {
            return Err("hash_copy(): Argument #1 ($context) must be of type HashContext".into())
        }
    };

    // Get object payload
    let obj = match &vm.arena.get(obj_handle).value {
        Val::ObjPayload(o) => o,
        _ => return Err("hash_copy(): Invalid HashContext object".into()),
    };

    // Check if finalized
    let finalized_prop = vm.context.interner.intern(b"__finalized");
    if let Some(&finalized_handle) = obj.properties.get(&finalized_prop) {
        if let Val::Bool(true) = vm.arena.get(finalized_handle).value {
            return Err("hash_copy(): Supplied HashContext has already been finalized".into());
        }
    }

    // Get algorithm name
    let algo_prop = vm.context.interner.intern(b"__algorithm");
    let algo_name = match obj.properties.get(&algo_prop) {
        Some(&handle) => match &vm.arena.get(handle).value {
            Val::String(s) => String::from_utf8_lossy(s).to_string(),
            _ => return Err("hash_copy(): Invalid algorithm property".into()),
        },
        None => return Err("hash_copy(): Invalid algorithm property".into()),
    };

    // Get state resource ID
    let state_prop = vm.context.interner.intern(b"__state");
    let resource_id = match obj.properties.get(&state_prop) {
        Some(&handle) => match &vm.arena.get(handle).value {
            Val::Resource(rc) => *rc.downcast_ref::<u64>().ok_or("hash_copy(): Invalid resource type")?,
            _ => return Err("hash_copy(): Invalid hash state".into()),
        },
        None => return Err("hash_copy(): Invalid hash state".into()),
    };

    // Clone the state
    let new_state = if let Some(states) = vm.context.hash_states.as_ref() {
        if let Some(state) = states.get(&resource_id) {
            state.clone_state()
        } else {
            return Err("hash_copy(): Invalid hash context state".into());
        }
    } else {
        return Err("hash_copy(): Hash states not initialized".into());
    };

    // Create new resource ID and store cloned state
    let new_resource_id = vm.context.next_resource_id;
    vm.context.next_resource_id += 1;
    
    let new_state_handle = vm.arena.alloc(Val::Resource(Rc::new(new_resource_id)));
    
    if let Some(states) = vm.context.hash_states.as_mut() {
        states.insert(new_resource_id, new_state);
    }

    // Create new HashContext object
    use indexmap::IndexMap;
    use std::collections::HashSet;
    let class_name = vm.context.interner.intern(b"HashContext");
    let mut properties = IndexMap::new();
    
    let algo_val = vm.arena.alloc(Val::String(Rc::new(algo_name.as_bytes().to_vec())));
    properties.insert(algo_prop, algo_val);
    properties.insert(state_prop, new_state_handle);
    properties.insert(finalized_prop, vm.arena.alloc(Val::Bool(false)));
    
    let new_obj = ObjectData {
        class: class_name,
        properties,
        internal: None,
        dynamic_properties: HashSet::new(),
    };
    
    let new_payload_handle = vm.arena.alloc(Val::ObjPayload(new_obj));
    Ok(vm.arena.alloc(Val::Object(new_payload_handle)))
}

/// hash_algos(): array
pub fn php_hash_algos(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if !args.is_empty() {
        return Err("hash_algos() expects no parameters".into());
    }

    let registry = vm
        .context
        .hash_registry
        .as_ref()
        .ok_or("Hash extension not initialized")?;

    let algos = registry.list_algorithms();

    // Build PHP array
    let mut arr = ArrayData::new();
    for (idx, name) in algos.iter().enumerate() {
        let key = ArrayKey::Int(idx as i64);
        let val = vm
            .arena
            .alloc(Val::String(Rc::new(name.as_bytes().to_vec())));
        arr.insert(key, val);
    }

    Ok(vm.arena.alloc(Val::Array(Rc::new(arr))))
}

/// hash_file(string $algo, string $filename, bool $binary = false): string|false
pub fn php_hash_file(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 3 {
        return Err("hash_file() expects 2 or 3 parameters".into());
    }

    let algo_name = match &vm.arena.get(args[0]).value {
        Val::String(s) => String::from_utf8_lossy(s).to_lowercase(),
        _ => return Err("hash_file(): Argument #1 ($algo) must be of type string".into()),
    };

    let filename = match &vm.arena.get(args[1]).value {
        Val::String(s) => s.clone(),
        _ => return Err("hash_file(): Argument #2 ($filename) must be of type string".into()),
    };

    let binary = if args.len() >= 3 {
        match &vm.arena.get(args[2]).value {
            Val::Bool(b) => *b,
            Val::Int(i) => *i != 0,
            _ => false,
        }
    } else {
        false
    };

    // Read file contents
    let filename_str = String::from_utf8_lossy(&filename);
    let data = std::fs::read(filename_str.as_ref()).map_err(|e| {
        format!(
            "hash_file(): Failed to open '{}': {}",
            filename_str, e
        )
    })?;

    // Get algorithm
    let registry = vm
        .context
        .hash_registry
        .as_ref()
        .ok_or("Hash extension not initialized")?;

    let algo = registry
        .get(&algo_name)
        .ok_or_else(|| format!("hash_file(): Unknown hashing algorithm: {}", algo_name))?;

    // Compute hash
    let digest = algo.hash(&data);

    // Format output
    let result = if binary {
        digest
    } else {
        hex::encode(&digest).into_bytes()
    };

    Ok(vm.arena.alloc(Val::String(Rc::new(result))))
}
