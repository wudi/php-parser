use bumpalo::Bump;
use php_parser_rs::ast::Stmt;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn parses_backed_enum_with_implements() {
    let code = "<?php enum Status: string implements JsonSerializable { case Ok; case Err = \"err\"; }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    let enum_stmt = program
        .statements
        .iter()
        .find(|s| matches!(**s, Stmt::Enum { .. }))
        .expect("expected enum");

    match enum_stmt {
        Stmt::Enum { backed_type, implements, members, .. } => {
            assert!(backed_type.is_some());
            assert!(!implements.is_empty());
            assert!(members.iter().any(|m| matches!(m, php_parser_rs::ast::ClassMember::Case { .. })));
        }
        _ => panic!("expected enum stmt"),
    }
}
