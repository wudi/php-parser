use bumpalo::Bump;
use php_parser::ast::{Expr, Stmt};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn parses_print_expression_with_and_without_parens() {
    let code = "<?php print $a; print($b);";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    let exprs: Vec<_> = program
        .statements
        .iter()
        .filter_map(|s| match **s {
            Stmt::Expression { expr, .. } => Some(expr),
            _ => None,
        })
        .collect();

    assert_eq!(exprs.len(), 2);
    for e in exprs {
        match e {
            Expr::Print { expr, .. } => assert!(matches!(*expr, Expr::Variable { .. })),
            other => panic!("expected print expr, got {:?}", other),
        }
    }
}
