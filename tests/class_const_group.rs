use bumpalo::Bump;
use php_parser_rs::ast::{ClassMember, Stmt};
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn parses_class_const_group_with_modifiers() {
    let code = "<?php class C { public const FOO = 1, BAR = 2; }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    let class_stmt = program
        .statements
        .iter()
        .find(|s| matches!(***s, Stmt::Class { .. }))
        .expect("expected class");

    let members = match class_stmt {
        Stmt::Class { members, .. } => *members,
        _ => unreachable!(),
    };

    let const_member = members
        .iter()
        .find_map(|m| match m {
            ClassMember::Const { consts, .. } => Some(*consts),
            _ => None,
        })
        .expect("expected const member");

    assert_eq!(const_member.len(), 2);
}
