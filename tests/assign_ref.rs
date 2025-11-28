use bumpalo::Bump;
use php_parser_rs::ast::{Expr, Stmt};
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn parses_assignment_by_reference() {
    let code = "<?php $a =& $b;";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    let stmt = program
        .statements
        .iter()
        .find(|s| !matches!(***s, Stmt::Nop { .. }))
        .expect("expected assignment stmt");

    match *stmt {
        Stmt::Expression { expr, .. } => match *expr {
            Expr::AssignRef { var, expr, .. } => {
                assert!(matches!(*var, Expr::Variable { .. }));
                assert!(matches!(*expr, Expr::Variable { .. }));
            }
            other => panic!("expected AssignRef, got {:?}", other),
        },
        other => panic!("expected expression stmt, got {:?}", other),
    }
}
