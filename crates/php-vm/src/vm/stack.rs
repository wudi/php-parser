use crate::core::value::Handle;

#[derive(Debug, Default)]
pub struct Stack {
    values: Vec<Handle>,
}

impl Stack {
    pub fn new() -> Self {
        Self { values: Vec::with_capacity(1024) }
    }

    pub fn push(&mut self, h: Handle) {
        self.values.push(h);
    }

    pub fn pop(&mut self) -> Option<Handle> {
        self.values.pop()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }
    
    pub fn peek(&self) -> Option<Handle> {
        self.values.last().copied()
    }
    
    pub fn peek_at(&self, offset: usize) -> Option<Handle> {
        if offset >= self.values.len() {
            None
        } else {
            Some(self.values[self.values.len() - 1 - offset])
        }
    }
    
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}
