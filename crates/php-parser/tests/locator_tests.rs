use bumpalo::Bump;
use php_parser::ast::locator::{AstNode, Locator};
use php_parser::ast::{Expr, Stmt};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn test_locate_function() {
    let code = "<?php function foo() { echo 1; }";
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();

    let target = 16; // inside "foo"
    let path = Locator::find(&result, target);

    assert!(!path.is_empty());
    let node = path.last().unwrap();

    match node {
        AstNode::Stmt(Stmt::Function { name, .. }) => {
            let name_span = name.span;
            assert!(name_span.start <= target && target <= name_span.end);
        }
        _ => panic!("Expected function, got {:?}", node),
    }
}

#[test]
fn test_locate_expr_inside_function() {
    let code = "<?php function foo() { echo 1; }";
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();

    let target = 28;
    let path = Locator::find(&result, target);

    assert!(!path.is_empty());
    let node = path.last().unwrap();

    match node {
        AstNode::Expr(Expr::Integer { .. }) => {}
        _ => panic!("Expected Expr::Integer, got {:?}", node),
    }

    // Check parent chain
    // path[0] should be Function.
    match path[0] {
        AstNode::Stmt(Stmt::Function { .. }) => {}
        _ => panic!("Expected Function at root"),
    }
}

#[test]
fn test_locate_nested_expr() {
    let code = "<?php $a = 1 + 2;";
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();

    let target = 15;
    let path = Locator::find(&result, target);

    assert!(!path.is_empty());
    let node = path.last().unwrap();

    match node {
        AstNode::Expr(Expr::Integer { .. }) => {}
        _ => panic!("Expected Expr::Integer for '2', got {:?}", node),
    }
}
