use bumpalo::Bump;
use php_parser_rs::ast::{Stmt, Type};
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn parses_namespaced_types_in_params_and_returns() {
    let code = "<?php function foo(\\A\\B $a): C\\D {}";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    let func = program
        .statements
        .iter()
        .find_map(|s| match **s {
            Stmt::Function { params, return_type: Some(ret), .. } => Some((params, ret)),
            _ => None,
        })
        .expect("expected function");

    // Param type is namespaced
    assert!(matches!(func.0[0].ty, Some(Type::Name(_))));
    // Return type is namespaced (relative)
    assert!(matches!(*func.1, Type::Name(_)));
    assert!(program.errors.is_empty());
}

#[test]
fn parses_union_with_namespaced_types() {
    let code = "<?php class C { public function bar(\\Foo\\Bar|Baz $x) {} }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    let union = program
        .statements
        .iter()
        .find_map(|s| match **s {
            Stmt::Class { members, .. } => members.iter().find_map(|m| match m {
                php_parser_rs::ast::ClassMember::Method { params, .. } => params[0].ty,
                _ => None,
            }),
            _ => None,
        })
        .expect("expected union type");

    if let Type::Union(types) = *union {
        assert_eq!(types.len(), 2);
        assert!(matches!(types[0], Type::Name(_)));
        assert!(matches!(types[1], Type::Name(_)));
    } else {
        panic!("expected union type");
    }
    assert!(program.errors.is_empty());
}

#[test]
fn parses_intersection_with_leading_separator() {
    let code = "<?php class C { public function baz(\\Foo & Bar $x) {} }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    let ty = program
        .statements
        .iter()
        .find_map(|s| match **s {
            Stmt::Class { members, .. } => members.iter().find_map(|m| match m {
                php_parser_rs::ast::ClassMember::Method { params, .. } => params[0].ty,
                _ => None,
            }),
            _ => None,
        })
        .expect("expected type");

    if let Type::Intersection(types) = *ty {
        assert_eq!(types.len(), 2);
        assert!(matches!(types[0], Type::Name(_)));
        assert!(matches!(types[1], Type::Name(_)));
    } else {
        panic!("expected intersection type");
    }
    assert!(program.errors.is_empty());
}

#[test]
fn parses_static_return_type() {
    let code = "<?php class A { public function foo(): static {} }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    assert!(program.errors.is_empty(), "unexpected errors: {:?}", program.errors);

    let return_type = program
        .statements
        .iter()
        .find_map(|s| match **s {
            Stmt::Class { members, .. } => members.iter().find_map(|m| match m {
                php_parser_rs::ast::ClassMember::Method { return_type, .. } => *return_type,
                _ => None,
            }),
            _ => None,
        })
        .expect("expected class method return type");

    if let Type::Simple(tok) = return_type {
        assert_eq!(tok.kind, php_parser_rs::lexer::token::TokenKind::Static);
    } else {
        panic!("expected static return type, got {:?}", return_type);
    }
}
