use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn formats_parse_error_with_snippet_and_pointer() {
    let source = "<?php\ndeclare(strict_types=2);\n";
    let bump = Bump::new();
    let mut parser = Parser::new(Lexer::new(source.as_bytes()), &bump);

    let program = parser.parse_program();
    let error = program.errors.first().expect("expected parse error");
    let location = error
        .span
        .line_info(source.as_bytes())
        .expect("missing line info");

    assert_eq!(location.line, 2);
    assert_eq!(location.column, 22);

    let rendered = error.to_human_readable_with_path(source.as_bytes(), Some("test.php"));

    assert!(rendered.contains("strict_types must be 0 or 1"));
    assert!(rendered.contains(&format!("test.php:{}:{}", location.line, location.column)));
    assert!(
        rendered
            .lines()
            .any(|line| line.contains("declare(strict_types=2);"))
    );

    let pointer_line = rendered
        .lines()
        .last()
        .expect("pointer line missing")
        .trim_end();
    assert!(
        pointer_line.ends_with(&format!("| {}^", " ".repeat(location.column - 1))),
        "pointer not aligned: {pointer_line}"
    );
}
