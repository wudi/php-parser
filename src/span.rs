use std::fmt;
use std::cell::RefCell;
use serde::Serialize;

thread_local! {
    static DEBUG_SOURCE: RefCell<Option<&'static [u8]>> = RefCell::new(None);
}

/// Execute a closure with a source code context for Span debugging.
/// This allows Spans to print their line number and text content when Debug formatted.
pub fn with_session_globals<F, R>(source: &[u8], f: F) -> R
where
    F: FnOnce() -> R,
{
    // SAFETY: We are extending the lifetime of the slice to 'static to store it in a thread_local.
    // We ensure that the thread_local is cleared before this function returns, so the reference
    // never outlives the actual data.
    let source_static: &'static [u8] = unsafe { std::mem::transmute(source) };
    
    DEBUG_SOURCE.with(|s| *s.borrow_mut() = Some(source_static));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    DEBUG_SOURCE.with(|s| *s.borrow_mut() = None);
    
    match result {
        Ok(r) => r,
        Err(e) => std::panic::resume_unwind(e),
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Default, Hash, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut builder = f.debug_struct("Span");
        builder.field("start", &self.start);
        builder.field("end", &self.end);

        DEBUG_SOURCE.with(|source_cell| {
            if let Some(source) = *source_cell.borrow() {
                if self.start <= self.end && self.end <= source.len() {
                    let line = source[..self.start].iter().filter(|&&b| b == b'\n').count() + 1;
                    builder.field("line", &line);
                    
                    let text = &source[self.start..self.end];
                    let text_str = String::from_utf8_lossy(text);
                    builder.field("text", &text_str);
                }
            }
        });

        builder.finish()
    }
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
