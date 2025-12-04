use php_parser::line_index::LineIndex;
use php_parser::span::Span;

#[test]
fn test_line_index_basic() {
    let code = b"line1\nline2\nline3";
    let index = LineIndex::new(code);
    
    // "line1" -> 0..5
    // "\n" -> 5..6
    // "line2" -> 6..11
    // "\n" -> 11..12
    // "line3" -> 12..17
    
    assert_eq!(index.line_col(0), (0, 0)); // 'l'
    assert_eq!(index.line_col(5), (0, 5)); // '\n'
    assert_eq!(index.line_col(6), (1, 0)); // 'l' of line2
    assert_eq!(index.line_col(11), (1, 5)); // '\n'
    assert_eq!(index.line_col(12), (2, 0)); // 'l' of line3
    assert_eq!(index.line_col(17), (2, 5)); // EOF
}

#[test]
fn test_line_index_offset() {
    let code = b"abc\ndef";
    let index = LineIndex::new(code);
    
    assert_eq!(index.offset(0, 0), Some(0));
    assert_eq!(index.offset(0, 3), Some(3)); // '\n'
    assert_eq!(index.offset(1, 0), Some(4)); // 'd'
    assert_eq!(index.offset(1, 3), Some(7)); // EOF
    
    assert_eq!(index.offset(2, 0), None); // Out of bounds
}

#[test]
fn test_lsp_range() {
    let code = b"function foo() {}";
    let index = LineIndex::new(code);
    
    // "foo" is at 9..12
    let span = Span::new(9, 12);
    let (start_line, start_col, end_line, end_col) = index.to_lsp_range(span);
    
    assert_eq!(start_line, 0);
    assert_eq!(start_col, 9);
    assert_eq!(end_line, 0);
    assert_eq!(end_col, 12);
}
