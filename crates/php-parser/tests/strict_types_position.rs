use php_parser::ast::ParseError;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn test_strict_types_as_first_statement() {
    let code = r#"<?php
declare(strict_types=1);

function foo() {}
"#;
    let arena = bumpalo::Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    // Should have no errors
    assert_eq!(program.errors.len(), 0, "Expected no errors, got: {:?}", program.errors);
}

#[test]
fn test_strict_types_not_first_statement_errors() {
    let code = r#"<?php
$x = 1;
declare(strict_types=1);
"#;
    let arena = bumpalo::Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    // Should have position error
    assert!(
        program.errors.len() > 0,
        "Expected position error"
    );
    
    let has_position_error = program.errors.iter().any(|e| {
        e.message.contains("first statement")
    });
    
    assert!(
        has_position_error,
        "Expected 'first statement' error, got: {:?}",
        program.errors
    );
}

#[test]
fn test_strict_types_after_function_errors() {
    let code = r#"<?php
function foo() {}
declare(strict_types=1);
"#;
    let arena = bumpalo::Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    // Should have position error
    assert!(program.errors.len() > 0);
    let has_position_error = program.errors.iter().any(|e| {
        e.message.contains("first statement")
    });
    assert!(has_position_error);
}

#[test]
fn test_multiple_declares_allowed_at_start() {
    let code = r#"<?php
declare(ticks=1);
declare(strict_types=1);
declare(encoding='UTF-8');

function foo() {}
"#;
    let arena = bumpalo::Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    // Multiple declares at the start are allowed
    // strict_types should not have position error
    let has_position_error = program.errors.iter().any(|e| {
        e.message.contains("first statement")
    });
    assert!(!has_position_error, "Should not have position error, got: {:?}", program.errors);
}

#[test]
fn test_strict_types_after_other_declare_allowed() {
    let code = r#"<?php
declare(ticks=1);
declare(strict_types=1);
"#;
    let arena = bumpalo::Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    // Should have no position errors
    let has_position_error = program.errors.iter().any(|e| {
        e.message.contains("first statement")
    });
    assert!(!has_position_error);
}

#[test]
fn test_strict_types_with_namespace_before() {
    let code = r#"<?php
namespace Foo;
declare(strict_types=1);
"#;
    let arena = bumpalo::Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    // Namespace is a statement, so strict_types after it should error
    let has_position_error = program.errors.iter().any(|e| {
        e.message.contains("first statement")
    });
    assert!(has_position_error, "Should have position error after namespace");
}
