use bumpalo::Bump;
use php_parser::ast::Stmt;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn test_class_doc_comment() {
    let code = b"<?php
/**
 * My Class
 */
class Foo {}";
    let bump = Bump::new();
    let lexer = Lexer::new(code);
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();

    let stmt = result
        .statements
        .iter()
        .find(|s| matches!(s, Stmt::Class { .. }))
        .expect("Expected Class");

    match stmt {
        Stmt::Class { doc_comment, .. } => {
            assert!(doc_comment.is_some(), "Doc comment is None");
            let span = doc_comment.unwrap();
            let text = &code[span.start..span.end];
            assert_eq!(std::str::from_utf8(text).unwrap(), "/**\n * My Class\n */");
        }
        _ => panic!("Expected Class"),
    }
}

#[test]
fn test_function_doc_comment() {
    let code = b"<?php
/** My Function */
function foo() {}";
    let bump = Bump::new();
    let lexer = Lexer::new(code);
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();

    let stmt = result
        .statements
        .iter()
        .find(|s| matches!(s, Stmt::Function { .. }))
        .expect("Expected Function");

    match stmt {
        Stmt::Function { doc_comment, .. } => {
            assert!(doc_comment.is_some(), "Doc comment is None");
            let span = doc_comment.unwrap();
            let text = &code[span.start..span.end];
            assert_eq!(std::str::from_utf8(text).unwrap(), "/** My Function */");
        }
        _ => panic!("Expected Function"),
    }
}

#[test]
fn test_property_doc_comment() {
    let code = b"<?php
class Foo {
    /** My Property */
    public $bar;
}";
    let bump = Bump::new();
    let lexer = Lexer::new(code);
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();

    let stmt = result
        .statements
        .iter()
        .find(|s| matches!(s, Stmt::Class { .. }))
        .expect("Expected Class");

    match stmt {
        Stmt::Class { members, .. } => match &members[0] {
            php_parser::ast::ClassMember::Property { doc_comment, .. } => {
                assert!(doc_comment.is_some(), "Doc comment is None");
                let span = doc_comment.unwrap();
                let text = &code[span.start..span.end];
                assert_eq!(std::str::from_utf8(text).unwrap(), "/** My Property */");
            }
            member => panic!("Expected Property, got {:?}", member),
        },
        _ => panic!("Expected Class"),
    }
}

#[test]
fn test_method_doc_comment() {
    let code = b"<?php
class Foo {
    /** My Method */
    public function bar() {}
}";
    let bump = Bump::new();
    let lexer = Lexer::new(code);
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();

    let stmt = result
        .statements
        .iter()
        .find(|s| matches!(s, Stmt::Class { .. }))
        .expect("Expected Class");

    match stmt {
        Stmt::Class { members, .. } => match &members[0] {
            php_parser::ast::ClassMember::Method { doc_comment, .. } => {
                assert!(doc_comment.is_some(), "Doc comment is None");
                let span = doc_comment.unwrap();
                let text = &code[span.start..span.end];
                assert_eq!(std::str::from_utf8(text).unwrap(), "/** My Method */");
            }
            member => panic!("Expected Method, got {:?}", member),
        },
        _ => panic!("Expected Class"),
    }
}
