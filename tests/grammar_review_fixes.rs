use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn test_function_by_ref() {
    let code = b"<?php function &getRef() { return $x; }";
    let arena = Bump::new();
    let lexer = Lexer::new(code);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
    assert!(
        program.statements.len() >= 2,
        "Program should have at least 2 statements (open tag + function)"
    );

    // Skip the Nop statement from <?php tag and check the function
    let func_stmt = program
        .statements
        .iter()
        .find(|stmt| matches!(**stmt, php_parser_rs::ast::Stmt::Function { .. }))
        .expect("Should find a Function statement");

    if let php_parser_rs::ast::Stmt::Function { by_ref, .. } = **func_stmt {
        assert!(by_ref, "Function should have by_ref=true");
    }
}

#[test]
fn test_function_readonly_name() {
    let code = b"<?php function readonly() { }";
    let arena = Bump::new();
    let lexer = Lexer::new(code);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(program.errors.is_empty());
}

#[test]
fn test_anonymous_class_modifiers() {
    let code = b"<?php $x = new readonly class { };";
    let arena = Bump::new();
    let lexer = Lexer::new(code);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(program.errors.is_empty());

    // Find the expression statement (skip Nop from open tag)
    let expr_stmt = program
        .statements
        .iter()
        .find(|stmt| matches!(**stmt, php_parser_rs::ast::Stmt::Expression { .. }))
        .expect("Should find an Expression statement");

    // Check modifiers are parsed
    if let php_parser_rs::ast::Stmt::Expression { expr, .. } = **expr_stmt {
        if let php_parser_rs::ast::Expr::Assign { expr: right, .. } = *expr {
            if let php_parser_rs::ast::Expr::New { class, .. } = *right {
                if let php_parser_rs::ast::Expr::AnonymousClass { modifiers, .. } = *class {
                    assert_eq!(modifiers.len(), 1, "Should have one modifier");
                    assert_eq!(
                        modifiers[0].kind,
                        php_parser_rs::lexer::token::TokenKind::Readonly
                    );
                } else {
                    panic!("Expected AnonymousClass");
                }
            } else {
                panic!("Expected New expression");
            }
        } else {
            panic!("Expected Assignment");
        }
    }
}

#[test]
fn test_switch_leading_semicolon() {
    let code = b"<?php switch ($x) {; case 1: break; }";
    let arena = Bump::new();
    let lexer = Lexer::new(code);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(program.errors.is_empty());
}

#[test]
fn test_attribute_trailing_comma() {
    let code = b"<?php #[Attr1, Attr2,] class Foo {}";
    let arena = Bump::new();
    let lexer = Lexer::new(code);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(program.errors.is_empty());
}

#[test]
fn test_halt_compiler_requires_parens() {
    let code = b"<?php __halt_compiler;";
    let arena = Bump::new();
    let lexer = Lexer::new(code);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    // Should have error for missing parentheses
    assert!(!program.errors.is_empty());
    assert!(program.errors[0].message.contains("'('"));
}

#[test]
fn test_halt_compiler_with_parens() {
    let code = b"<?php __halt_compiler();";
    let arena = Bump::new();
    let lexer = Lexer::new(code);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    assert!(program.errors.is_empty());
}
