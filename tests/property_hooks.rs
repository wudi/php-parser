use bumpalo::Bump;
use php_parser_rs::ast::{ClassMember, PropertyHookBody, Stmt};
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn parses_property_hooks_get_set() {
    let code = "<?php class C { public int $x { get => $this->x; set($v) { $this->x = $v; } } }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    let class_stmt = program
        .statements
        .iter()
        .find(|s| matches!(**s, Stmt::Class { .. }))
        .expect("expected class");

    let members = match class_stmt {
        Stmt::Class { members, .. } => *members,
        _ => unreachable!(),
    };

    let hooks = members
        .iter()
        .find_map(|m| match m {
            ClassMember::PropertyHook { hooks, .. } => Some(*hooks),
            _ => None,
        })
        .expect("expected hooked property");

    assert_eq!(hooks.len(), 2);
    matches!(hooks[0].body, PropertyHookBody::Expr(_));
    matches!(hooks[1].body, PropertyHookBody::Statements(_));
}
