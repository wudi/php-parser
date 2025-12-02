use bumpalo::Bump;
use php_parser_rs::ast::{Arg, Expr, Stmt};
use php_parser_rs::lexer::Lexer;
use php_parser_rs::lexer::token::TokenKind;
use php_parser_rs::parser::Parser;

#[test]
fn parses_named_and_unpack_args() {
    let code = "<?php foo(a: 1, ...$xs, b: 2);";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    let stmt = program
        .statements
        .iter()
        .find(|s| !matches!(***s, Stmt::Nop { .. }))
        .expect("expected expression statement");

    let call = match *stmt {
        Stmt::Expression { expr, .. } => match *expr {
            Expr::Call { func, args, .. } => {
                // func should be variable/identifier 'foo'
                if let Expr::Variable { .. } = *func {
                } else {
                    panic!(
                        "expected call target to be variable/identifier, got {:?}",
                        func
                    );
                }
                args
            }
            other => panic!("expected call expression, got {:?}", other),
        },
        other => panic!("expected expression statement, got {:?}", other),
    };

    assert_eq!(call.len(), 3);

    match call[0] {
        Arg {
            name: Some(n),
            unpack,
            ..
        } => {
            assert_eq!(n.kind, TokenKind::Identifier);
            assert!(!unpack);
        }
        other => panic!("expected named arg, got {:?}", other),
    }

    match call[1] {
        Arg {
            name: None,
            unpack: true,
            ..
        } => {}
        other => panic!("expected unpacked arg, got {:?}", other),
    }

    match call[2] {
        Arg {
            name: Some(n),
            unpack,
            ..
        } => {
            assert_eq!(n.kind, TokenKind::Identifier);
            assert!(!unpack);
        }
        other => panic!("expected trailing named arg, got {:?}", other),
    }
}

#[test]
fn arrow_function_supports_by_ref_and_attributes() {
    let code = "<?php $f = #[A] fn & (int $a): int => $a;";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();
    println!("{:#?}", program);
    assert!(
        program.errors.is_empty(),
        "Expected arrow function with attributes and by-ref to parse"
    );
}
