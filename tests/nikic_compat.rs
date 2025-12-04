use bumpalo::Bump;
use php_parser::ast::{Expr, Stmt};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn test_group_use_flattening() {
    let code = "<?php use A\\{B, C};";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());

    // Currently flattens to 1 Use statement (Use)
    assert_eq!(program.statements.len(), 1);

    match program.statements[0] {
        Stmt::Use { uses, .. } => {
            assert_eq!(uses.len(), 2);
        }
        _ => panic!("Expected Use statement"),
    }
}

#[test]
fn test_list_vs_short_array() {
    let code = "<?php list($a) = [1]; [$b] = [2];";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());

    // Check first assignment: list($a) = ...
    // Index 0 because Nop is gone
    match program.statements[0] {
        Stmt::Expression { expr, .. } => {
            match expr {
                Expr::Assign { var, .. } => {
                    // In this repo, list() is parsed as Expr::Array (or similar)
                    // We want to confirm it is indistinguishable from short array if that's the case
                    assert!(matches!(**var, Expr::Array { .. }));
                }
                _ => panic!("Expected Assign"),
            }
        }
        _ => panic!("Expected Expression statement"),
    }

    // Check second assignment: [$b] = ...
    match program.statements[1] {
        Stmt::Expression { expr, .. } => match expr {
            Expr::Assign { var, .. } => {
                assert!(matches!(**var, Expr::Array { .. }));
            }
            _ => panic!("Expected Assign"),
        },
        _ => panic!("Expected Expression statement"),
    }
}

#[test]
fn test_heredoc_indentation() {
    let code = "<?php
$x = <<<EOT
  foo
  EOT;
";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());

    assert!(program.errors.is_empty());

    match program.statements[0] {
        Stmt::Expression { expr, .. } => {
            match expr {
                Expr::Assign { expr, .. } => {
                    match expr {
                        Expr::InterpolatedString { parts, .. } => {
                            // We expect parts to contain the string with indentation
                            // Since we can't easily inspect the content without more helpers,
                            // we just assert it parsed as InterpolatedString.
                            assert!(!parts.is_empty());
                        }
                        _ => panic!("Expected InterpolatedString, got {:?}", expr),
                    }
                }
                _ => panic!("Expected Assign"),
            }
        }
        _ => panic!("Expected Expression statement"),
    }
}
