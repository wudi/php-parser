use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;
use php_parser_rs::ast::Stmt;

#[test]
fn parses_union_catch_and_optional_variable() {
    let code = "<?php try {} catch (\\Foo\\Bar|Baz $e) {} catch (RuntimeException) {}";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    let try_stmt = program
        .statements
        .iter()
        .find_map(|s| match **s {
            Stmt::Try { catches, .. } => Some(catches),
            _ => None,
        })
        .expect("expected try statement");

    assert_eq!(try_stmt.len(), 2);
    assert_eq!(try_stmt[0].types.len(), 2);
    assert!(try_stmt[0].var.is_some());
    assert_eq!(try_stmt[1].types.len(), 1);
    assert!(try_stmt[1].var.is_none());
    assert!(program.errors.is_empty());
}
