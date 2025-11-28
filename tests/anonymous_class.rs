use bumpalo::Bump;
use php_parser_rs::ast::{Expr, Stmt};
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn parses_anonymous_class_expression() {
    let code = "<?php $obj = new class($arg) extends Foo implements Bar { public function run() {} };";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    assert!(program.errors.is_empty(), "unexpected errors: {:?}", program.errors);

    let stmt = program
        .statements
        .iter()
        .find(|s| !matches!(***s, Stmt::Nop { .. }))
        .expect("expected statement");

    let expr = match **stmt {
        Stmt::Expression { expr, .. } => expr,
        _ => panic!("expected expression stmt"),
    };

    match expr {
        Expr::Assign { expr: value, .. } => match value {
            Expr::New { class, args, .. } => {
                assert_eq!(args.len(), 1);
                match class {
                    Expr::AnonymousClass { args: ctor_args, extends, implements, members, .. } => {
                        assert_eq!(ctor_args.len(), 1);
                        assert!(extends.is_some());
                        assert_eq!(implements.len(), 1);
                        assert!(!members.is_empty());
                    }
                    other => panic!("expected anonymous class, got {:?}", other),
                }
            }
            other => panic!("expected new expr, got {:?}", other),
        },
        other => panic!("expected assignment, got {:?}", other),
    }
}

#[test]
fn parses_static_scope_resolution() {
    let code = "<?php return static::NAVIGATION_POST_TYPE;";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    assert!(program.errors.is_empty(), "unexpected errors: {:?}", program.errors);

    let expr = program
        .statements
        .iter()
        .find_map(|stmt| match **stmt {
            Stmt::Return { expr: Some(expr), .. } => Some(expr),
            _ => None,
        })
        .expect("expected return statement");

    match expr {
        Expr::ClassConstFetch { .. } => {}
        other => panic!("expected class const fetch, got {:?}", other),
    }
}
