use crate::core::value::{Handle, Val, Zval};

#[derive(Debug, Default)]
pub struct Arena {
    storage: Vec<Zval>,
    free_slots: Vec<usize>,
}

impl Arena {
    pub fn new() -> Self {
        Self {
            storage: Vec::with_capacity(1024),
            free_slots: Vec::new(),
        }
    }

    pub fn alloc(&mut self, val: Val) -> Handle {
        let zval = Zval {
            value: val,
            is_ref: false,
        };

        if let Some(idx) = self.free_slots.pop() {
            self.storage[idx] = zval;
            return Handle(idx as u32);
        }

        let idx = self.storage.len();
        self.storage.push(zval);
        Handle(idx as u32)
    }

    pub fn get(&self, h: Handle) -> &Zval {
        &self.storage[h.0 as usize]
    }

    pub fn get_mut(&mut self, h: Handle) -> &mut Zval {
        &mut self.storage[h.0 as usize]
    }

    pub fn free(&mut self, h: Handle) {
        self.free_slots.push(h.0 as usize);
    }
}
