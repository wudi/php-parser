use std::collections::HashMap;
use crate::core::value::Symbol;

#[derive(Debug, Default)]
pub struct Interner {
    map: HashMap<Vec<u8>, Symbol>,
    vec: Vec<Vec<u8>>,
}

impl Interner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn intern(&mut self, s: &[u8]) -> Symbol {
        if let Some(&sym) = self.map.get(s) {
            return sym;
        }
        let sym = Symbol(self.vec.len() as u32);
        self.vec.push(s.to_vec());
        self.map.insert(s.to_vec(), sym);
        sym
    }

    pub fn lookup(&self, sym: Symbol) -> Option<&[u8]> {
        self.vec.get(sym.0 as usize).map(|v| v.as_slice())
    }
}
