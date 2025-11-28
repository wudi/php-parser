#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self { Self { start, end } }
    
    pub fn len(&self) -> usize { self.end - self.start }

    pub fn is_empty(&self) -> bool { self.start == self.end }
    
    /// Safely slice the source. Returns None if indices are out of bounds.
    /// In this project we assume the parser manages bounds correctly, but for safety we could return Option.
    /// For now, following ARCHITECTURE.md, we return slice.
    pub fn as_str<'src>(&self, source: &'src [u8]) -> &'src [u8] {
        &source[self.start..self.end]
    }
}
