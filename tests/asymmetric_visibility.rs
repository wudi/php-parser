use bumpalo::Bump;
use php_parser_rs::ast::{ClassMember, Stmt};
use php_parser_rs::lexer::Lexer;
use php_parser_rs::lexer::token::TokenKind;
use php_parser_rs::parser::Parser;

#[test]
fn test_asymmetric_visibility() {
    let code = r#"<?php
class Test {
    private(set) $a;
    protected(set) int $b;
    public private(set) string $c;
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();

    assert!(result.errors.is_empty());

    let stmt = result
        .statements
        .iter()
        .find(|s| matches!(s, Stmt::Class { .. }))
        .expect("Expected class");
    if let Stmt::Class { members, .. } = stmt {
        assert_eq!(members.len(), 3);

        // private(set) $a;
        if let ClassMember::Property { modifiers, .. } = &members[0] {
            assert_eq!(modifiers.len(), 1);
            assert_eq!(modifiers[0].kind, TokenKind::PrivateSet);
        } else {
            panic!("Expected property");
        }

        // protected(set) int $b;
        if let ClassMember::Property { modifiers, .. } = &members[1] {
            assert_eq!(modifiers.len(), 1);
            assert_eq!(modifiers[0].kind, TokenKind::ProtectedSet);
        } else {
            panic!("Expected property");
        }

        // public private(set) string $c;
        if let ClassMember::Property { modifiers, .. } = &members[2] {
            assert_eq!(modifiers.len(), 2);
            assert_eq!(modifiers[0].kind, TokenKind::Public);
            assert_eq!(modifiers[1].kind, TokenKind::PrivateSet);
        } else {
            panic!("Expected property");
        }
    } else {
        panic!("Expected class, got {:?}", stmt);
    }
}

#[test]
fn test_asymmetric_visibility_constructor_promotion() {
    let code = r#"
<?php
class Test {
    public function __construct(
        private(set) $a,
        public protected(set) int $b
    ) {}
}
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();

    assert!(result.errors.is_empty());
}
