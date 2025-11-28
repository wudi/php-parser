use bumpalo::Bump;
use php_parser_rs::ast::{Expr, Stmt};
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn parses_list_destructuring_with_by_ref_and_skips() {
    let code = "<?php list($a, &$b, , $c) = $value;";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    let stmt = program
        .statements
        .iter()
        .find(|s| !matches!(***s, Stmt::Nop { .. }))
        .expect("expected assignment statement");

    let (lhs, rhs) = match *stmt {
        Stmt::Expression { expr, .. } => match *expr {
            Expr::Assign { var, expr, .. } => (var, expr),
            other => panic!("expected assignment, got {:?}", other),
        },
        other => panic!("expected expression statement, got {:?}", other),
    };

    match *lhs {
        Expr::Array { items, .. } => {
            assert_eq!(items.len(), 4);
            // $a
            assert!(!items[0].by_ref);
            // &$b
            assert!(items[1].by_ref);
            // placeholder error node for skipped slot
            assert!(matches!(*items[2].value, Expr::Error { .. }));
            // $c
            assert!(!items[3].by_ref);
        }
        other => panic!("expected list array structure, got {:?}", other),
    }

    // RHS should be the value expression
    assert!(matches!(*rhs, Expr::Variable { .. }));
}
