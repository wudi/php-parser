use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Unified resource manager for type-safe resource handling across extensions
///
/// Resources are identified by a unique u64 ID and stored with type information.
/// This provides a centralized, type-safe way for extensions to manage resources
/// (database connections, file handles, etc.) without direct HashMap manipulation.
///
/// # Example
/// ```ignore
/// // Register a resource
/// let id = ctx.next_resource_id;
/// ctx.next_resource_id += 1;
/// resource_manager.register(id, Rc::new(RefCell::new(connection)));
///
/// // Retrieve a resource
/// if let Some(conn) = resource_manager.get::<MysqliConnection>(id) {
///     // Use connection
/// }
///
/// // Remove a resource
/// resource_manager.remove::<MysqliConnection>(id);
/// ```
#[derive(Default)]
pub struct ResourceManager {
    /// Storage for resources by type - each TypeId maps to a HashMap<u64, T>
    storage: HashMap<TypeId, Box<dyn Any>>,
}

impl ResourceManager {
    /// Create a new empty resource manager
    pub fn new() -> Self {
        Self {
            storage: HashMap::new(),
        }
    }

    /// Register a resource with a given ID and type
    ///
    /// If a resource with the same ID and type already exists, it will be replaced.
    pub fn register<T: 'static>(&mut self, id: u64, resource: Rc<RefCell<T>>) {
        let type_id = TypeId::of::<T>();
        let map = self
            .storage
            .entry(type_id)
            .or_insert_with(|| Box::new(HashMap::<u64, Rc<RefCell<T>>>::new()))
            .downcast_mut::<HashMap<u64, Rc<RefCell<T>>>>()
            .expect("TypeId mismatch - this should never happen");

        map.insert(id, resource);
    }

    /// Get a reference to a resource by ID and type
    ///
    /// Returns None if the resource doesn't exist or has the wrong type.
    pub fn get<T: 'static>(&self, id: u64) -> Option<Rc<RefCell<T>>> {
        let type_id = TypeId::of::<T>();
        let map = self
            .storage
            .get(&type_id)?
            .downcast_ref::<HashMap<u64, Rc<RefCell<T>>>>()?;

        map.get(&id).cloned()
    }

    /// Remove a resource by ID and type
    ///
    /// Returns the resource if it existed, None otherwise.
    pub fn remove<T: 'static>(&mut self, id: u64) -> Option<Rc<RefCell<T>>> {
        let type_id = TypeId::of::<T>();
        let map = self
            .storage
            .get_mut(&type_id)?
            .downcast_mut::<HashMap<u64, Rc<RefCell<T>>>>()?;

        map.remove(&id)
    }

    /// Check if a resource with the given ID and type exists
    pub fn contains<T: 'static>(&self, id: u64) -> bool {
        let type_id = TypeId::of::<T>();
        if let Some(map) = self.storage.get(&type_id) {
            if let Some(map) = map.downcast_ref::<HashMap<u64, Rc<RefCell<T>>>>() {
                return map.contains_key(&id);
            }
        }
        false
    }

    /// Get all resource IDs of a specific type
    pub fn ids_of_type<T: 'static>(&self) -> Vec<u64> {
        let type_id = TypeId::of::<T>();
        if let Some(map) = self.storage.get(&type_id) {
            if let Some(map) = map.downcast_ref::<HashMap<u64, Rc<RefCell<T>>>>() {
                return map.keys().copied().collect();
            }
        }
        Vec::new()
    }

    /// Clear all resources of a specific type
    pub fn clear_type<T: 'static>(&mut self) {
        let type_id = TypeId::of::<T>();
        if let Some(map) = self.storage.get_mut(&type_id) {
            if let Some(map) = map.downcast_mut::<HashMap<u64, Rc<RefCell<T>>>>() {
                map.clear();
            }
        }
    }

    /// Clear all resources
    pub fn clear_all(&mut self) {
        self.storage.clear();
    }

    /// Count of resources of a specific type
    pub fn count_of_type<T: 'static>(&self) -> usize {
        let type_id = TypeId::of::<T>();
        if let Some(map) = self.storage.get(&type_id) {
            if let Some(map) = map.downcast_ref::<HashMap<u64, Rc<RefCell<T>>>>() {
                return map.len();
            }
        }
        0
    }
}

// Implement Debug manually since dyn Any doesn't implement Debug
impl std::fmt::Debug for ResourceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceManager")
            .field("type_count", &self.storage.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestResource {
        value: i32,
    }

    struct AnotherResource {
        name: String,
    }

    #[test]
    fn test_register_and_get() {
        let mut manager = ResourceManager::new();
        let resource = Rc::new(RefCell::new(TestResource { value: 42 }));

        manager.register(1, resource.clone());

        let retrieved = manager.get::<TestResource>(1).unwrap();
        assert_eq!(retrieved.borrow().value, 42);
    }

    #[test]
    fn test_remove() {
        let mut manager = ResourceManager::new();
        let resource = Rc::new(RefCell::new(TestResource { value: 42 }));

        manager.register(1, resource);
        assert!(manager.contains::<TestResource>(1));

        let removed = manager.remove::<TestResource>(1);
        assert!(removed.is_some());
        assert!(!manager.contains::<TestResource>(1));
    }

    #[test]
    fn test_type_isolation() {
        let mut manager = ResourceManager::new();

        manager.register(1, Rc::new(RefCell::new(TestResource { value: 42 })));
        manager.register(
            1,
            Rc::new(RefCell::new(AnotherResource {
                name: "test".to_string(),
            })),
        );

        // Both IDs coexist for different types
        assert!(manager.contains::<TestResource>(1));
        assert!(manager.contains::<AnotherResource>(1));

        // Getting with wrong type returns None
        assert_eq!(manager.get::<TestResource>(1).unwrap().borrow().value, 42);
        assert_eq!(
            manager.get::<AnotherResource>(1).unwrap().borrow().name,
            "test"
        );
    }

    #[test]
    fn test_ids_of_type() {
        let mut manager = ResourceManager::new();

        manager.register(1, Rc::new(RefCell::new(TestResource { value: 1 })));
        manager.register(2, Rc::new(RefCell::new(TestResource { value: 2 })));
        manager.register(
            3,
            Rc::new(RefCell::new(AnotherResource {
                name: "test".to_string(),
            })),
        );

        let test_ids = manager.ids_of_type::<TestResource>();
        assert_eq!(test_ids.len(), 2);
        assert!(test_ids.contains(&1));
        assert!(test_ids.contains(&2));

        let another_ids = manager.ids_of_type::<AnotherResource>();
        assert_eq!(another_ids.len(), 1);
        assert!(another_ids.contains(&3));
    }

    #[test]
    fn test_clear_type() {
        let mut manager = ResourceManager::new();

        manager.register(1, Rc::new(RefCell::new(TestResource { value: 1 })));
        manager.register(
            2,
            Rc::new(RefCell::new(AnotherResource {
                name: "test".to_string(),
            })),
        );

        manager.clear_type::<TestResource>();

        assert!(!manager.contains::<TestResource>(1));
        assert!(manager.contains::<AnotherResource>(2));
    }

    #[test]
    fn test_count_of_type() {
        let mut manager = ResourceManager::new();

        manager.register(1, Rc::new(RefCell::new(TestResource { value: 1 })));
        manager.register(2, Rc::new(RefCell::new(TestResource { value: 2 })));

        assert_eq!(manager.count_of_type::<TestResource>(), 2);
        assert_eq!(manager.count_of_type::<AnotherResource>(), 0);
    }
}
