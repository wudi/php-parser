use bumpalo::Bump;
use php_parser_rs::ast::Stmt;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn parses_dynamic_static_property_name() {
    let code = "<?php return get_term_link( self::${$type . '_id'} );";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    assert!(
        program.errors.is_empty(),
        "unexpected errors: {:?}",
        program.errors
    );

    let stmt = program
        .statements
        .iter()
        .find(|s| !matches!(***s, Stmt::Nop { .. }))
        .expect("expected a return statement");

    assert!(matches!(**stmt, Stmt::Return { .. }));
}
