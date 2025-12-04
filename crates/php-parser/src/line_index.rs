use crate::span::Span;

#[derive(Debug, Clone)]
pub struct LineIndex {
    /// Offset of the start of each line.
    line_starts: Vec<usize>,
    len: usize,
}

impl LineIndex {
    pub fn new(source: &[u8]) -> Self {
        let mut line_starts = vec![0];
        for (i, &b) in source.iter().enumerate() {
            if b == b'\n' {
                line_starts.push(i + 1);
            }
        }
        Self {
            line_starts,
            len: source.len(),
        }
    }

    /// Returns (line, column) for a given byte offset.
    /// Both line and column are 0-based.
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        if offset > self.len {
            // Fallback or panic? For robustness, clamp to end.
            let last_line = self.line_starts.len() - 1;
            let last_start = self.line_starts[last_line];
            return (last_line, self.len.saturating_sub(last_start));
        }

        // Binary search to find the line
        match self.line_starts.binary_search(&offset) {
            Ok(line) => (line, 0),
            Err(insert_idx) => {
                let line = insert_idx - 1;
                let col = offset - self.line_starts[line];
                (line, col)
            }
        }
    }

    /// Returns the byte offset for a given (line, column).
    /// Both line and column are 0-based.
    pub fn offset(&self, line: usize, col: usize) -> Option<usize> {
        if line >= self.line_starts.len() {
            return None;
        }
        let start = self.line_starts[line];
        let offset = start + col;

        // Check if offset is within the line (or at least within file bounds)
        // We don't strictly check if col goes beyond the line length here,
        // but we should check if it goes beyond the next line start.
        if line + 1 < self.line_starts.len() {
            if offset >= self.line_starts[line + 1] {
                // Column is too large for this line
                // But maybe we allow it if it points to the newline char?
                // LSP allows pointing past the end of line.
                // But strictly speaking, it shouldn't cross into the next line.
                // For now, let's just check total length.
            }
        }

        if offset > self.len {
            None
        } else {
            Some(offset)
        }
    }

    pub fn to_lsp_range(&self, span: Span) -> (usize, usize, usize, usize) {
        let (start_line, start_col) = self.line_col(span.start);
        let (end_line, end_col) = self.line_col(span.end);
        (start_line, start_col, end_line, end_col)
    }
}
