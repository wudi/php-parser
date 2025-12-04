use bumpalo::Bump;
use php_parser::ast::{Expr, Stmt};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn parses_yield_and_nullsafe_access() {
    let code = "<?php
function gen() {
    yield 1;
    yield from foo();
    yield $k => $v;
}
$x?->prop;
$x?->method(1);
";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    let mut non_nop = program
        .statements
        .iter()
        .filter(|stmt| !matches!(***stmt, Stmt::Nop { .. }));

    let func_body: &[&Stmt] = match non_nop.next().expect("expected function statement") {
        Stmt::Function { body, .. } => body,
        other => panic!("expected function first, got {:?}", other),
    };

    match func_body[0] {
        Stmt::Expression { expr, .. } => match *expr {
            Expr::Yield {
                from, key, value, ..
            } => {
                assert!(!from);
                assert!(key.is_none());
                assert!(value.is_some());
            }
            other => panic!("expected yield expr, got {:?}", other),
        },
        other => panic!("expected expression stmt, got {:?}", other),
    }

    match func_body[1] {
        Stmt::Expression { expr, .. } => match *expr {
            Expr::Yield {
                from, key, value, ..
            } => {
                assert!(from);
                assert!(key.is_none());
                assert!(value.is_some());
            }
            other => panic!("expected yield from expr, got {:?}", other),
        },
        other => panic!("expected expression stmt, got {:?}", other),
    }

    match func_body[2] {
        Stmt::Expression { expr, .. } => match *expr {
            Expr::Yield {
                from, key, value, ..
            } => {
                assert!(!from);
                assert!(key.is_some());
                assert!(value.is_some());
            }
            other => panic!("expected keyed yield expr, got {:?}", other),
        },
        other => panic!("expected expression stmt, got {:?}", other),
    }

    // Skip function decl; remaining top-level expressions are nullsafe property/method
    let top_level: Vec<_> = non_nop.collect();

    match *top_level[0] {
        Stmt::Expression { expr, .. } => match *expr {
            Expr::NullsafePropertyFetch { .. } => {}
            other => panic!("expected nullsafe property fetch, got {:?}", other),
        },
        other => panic!("expected expression stmt, got {:?}", other),
    }

    match *top_level[1] {
        Stmt::Expression { expr, .. } => match *expr {
            Expr::NullsafeMethodCall { .. } => {}
            other => panic!("expected nullsafe method call, got {:?}", other),
        },
        other => panic!("expected expression stmt, got {:?}", other),
    }
}
