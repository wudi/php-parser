use bumpalo::Bump;
use php_parser_rs::ast::{Expr, MagicConstKind, Stmt};
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn test_magic_constants() {
    let source = b"<?php
        __LINE__;
        __FILE__;
        __DIR__;
        __FUNCTION__;
        __CLASS__;
        __TRAIT__;
        __METHOD__;
        __NAMESPACE__;
        __PROPERTY__;
    ";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());

    let statements: Vec<&Stmt> = program
        .statements
        .iter()
        .map(|s| *s)
        .filter(|s| !matches!(s, Stmt::Nop { .. }))
        .collect();
    assert_eq!(statements.len(), 9);

    let check_magic = |stmt: &Stmt, expected: MagicConstKind| {
        if let Stmt::Expression { expr, .. } = stmt {
            if let Expr::MagicConst { kind, .. } = expr {
                assert_eq!(*kind, expected);
            } else {
                panic!("Expected MagicConst, got {:?}", expr);
            }
        } else {
            panic!("Expected Expression statement, got {:?}", stmt);
        }
    };

    check_magic(statements[0], MagicConstKind::Line);
    check_magic(statements[1], MagicConstKind::File);
    check_magic(statements[2], MagicConstKind::Dir);
    check_magic(statements[3], MagicConstKind::Function);
    check_magic(statements[4], MagicConstKind::Class);
    check_magic(statements[5], MagicConstKind::Trait);
    check_magic(statements[6], MagicConstKind::Method);
    check_magic(statements[7], MagicConstKind::Namespace);
    check_magic(statements[8], MagicConstKind::Property);
}
