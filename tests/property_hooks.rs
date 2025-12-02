use bumpalo::Bump;
use php_parser_rs::ast::{ClassMember, PropertyHookBody, Stmt};
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn test_property_hooks() {
    let source = b"<?php
        class Foo {
            public $bar {
                get { return $this->bar; }
                set { $this->bar = $value; }
            }
            public $baz {
                get => $this->baz;
                set => $value;
            }
        }
    ";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());

    let statements: Vec<&Stmt> = program
        .statements
        .iter().copied()
        .filter(|s| !matches!(s, Stmt::Nop { .. }))
        .collect();

    if let Stmt::Class { members, .. } = statements[0] {
        assert_eq!(members.len(), 2);

        // Check first property (block hooks)
        if let ClassMember::PropertyHook { name, hooks, .. } = &members[0] {
            assert_eq!(name.text(source), b"$bar");
            assert_eq!(hooks.len(), 2);

            assert_eq!(hooks[0].name.text(source), b"get");
            if let PropertyHookBody::Statements(stmts) = hooks[0].body {
                assert_eq!(stmts.len(), 1);
            } else {
                panic!("Expected Statements body for get");
            }

            assert_eq!(hooks[1].name.text(source), b"set");
            if let PropertyHookBody::Statements(stmts) = hooks[1].body {
                assert_eq!(stmts.len(), 1);
            } else {
                panic!("Expected Statements body for set");
            }
        } else {
            panic!("Expected PropertyHook member");
        }

        // Check second property (arrow hooks)
        if let ClassMember::PropertyHook { name, hooks, .. } = &members[1] {
            assert_eq!(name.text(source), b"$baz");
            assert_eq!(hooks.len(), 2);

            assert_eq!(hooks[0].name.text(source), b"get");
            if let PropertyHookBody::Expr(_) = hooks[0].body {
                // OK
            } else {
                panic!("Expected Expr body for get");
            }

            assert_eq!(hooks[1].name.text(source), b"set");
            if let PropertyHookBody::Expr(_) = hooks[1].body {
                // OK
            } else {
                panic!("Expected Expr body for set");
            }
        } else {
            panic!("Expected PropertyHook member");
        }
    } else {
        panic!("Expected Class statement");
    }
}
