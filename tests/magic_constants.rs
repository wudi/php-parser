use bumpalo::Bump;
use php_parser_rs::ast::{Expr, MagicConstKind, Stmt};
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn test_magic_constants() {
    let source = "<?php
    $a = __DIR__;
    $b = __FILE__;
    $c = __LINE__;
    $d = __FUNCTION__;
    $e = __CLASS__;
    $f = __TRAIT__;
    $g = __METHOD__;
    $h = __NAMESPACE__;
    ";
    let arena = Bump::new();
    let lexer = Lexer::new(source.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let statements = program.statements;
    assert_eq!(statements.len(), 9); // Nop + 8 assignments

    // Helper to check assignment
    let check_assign = |stmt: &Stmt, expected_kind: MagicConstKind| {
        if let Stmt::Expression {
            expr:
                Expr::Assign {
                    expr: Expr::MagicConst { kind, .. },
                    ..
                },
            ..
        } = stmt
        {
            assert_eq!(kind, &expected_kind);
            return;
        }
        panic!("Expected assignment to magic const, got {:?}", stmt);
    };

    // Skip Nop
    check_assign(statements[1], MagicConstKind::Dir);
    check_assign(statements[2], MagicConstKind::File);
    check_assign(statements[3], MagicConstKind::Line);
    check_assign(statements[4], MagicConstKind::Function);
    check_assign(statements[5], MagicConstKind::Class);
    check_assign(statements[6], MagicConstKind::Trait);
    check_assign(statements[7], MagicConstKind::Method);
    check_assign(statements[8], MagicConstKind::Namespace);
}

#[test]
fn test_magic_constant_in_expression() {
    let source = "<?php
    require __DIR__ . '/file.php';
    ";
    let arena = Bump::new();
    let lexer = Lexer::new(source.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    // Just ensure it parses without error
    assert!(program.errors.is_empty());
}
