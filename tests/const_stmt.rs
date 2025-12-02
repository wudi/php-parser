use bumpalo::Bump;
use php_parser::ast::{ClassConst, Stmt};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn parses_global_const_statement() {
    let code = "<?php const FOO = 1, BAR = 2;";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    assert!(
        program.errors.is_empty(),
        "unexpected errors: {:?}",
        program.errors
    );

    let const_stmt = program
        .statements
        .iter()
        .find_map(|stmt| match **stmt {
            Stmt::Const { consts, .. } => Some(consts),
            _ => None,
        })
        .expect("expected const statement");

    assert_eq!(const_stmt.len(), 2);
    for c in (*const_stmt).iter() {
        assert!(matches!(c, ClassConst { .. }));
    }
}
