use bumpalo::Bump;
use php_parser_rs::ast::{ClassMember, Stmt, TraitAdaptation, TraitMethodRef};
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn parses_trait_use_adaptations() {
    let code =
        "<?php class C { use A, B { A::foo insteadof B; B::foo as public bar; foo as baz; } }";
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

    let trait_use = members
        .iter()
        .find_map(|m| match m {
            ClassMember::TraitUse {
                traits,
                adaptations,
                ..
            } => Some((traits, adaptations)),
            _ => None,
        })
        .expect("expected trait use");

    let (_traits, adaptations) = trait_use;
    assert_eq!(adaptations.len(), 3);

    match adaptations[0] {
        TraitAdaptation::Precedence {
            method:
                TraitMethodRef {
                    trait_name: Some(_),
                    ..
                },
            insteadof,
            ..
        } => {
            assert!(!insteadof.is_empty());
        }
        other => panic!("expected precedence, got {:?}", other),
    }

    match adaptations[1] {
        TraitAdaptation::Alias {
            visibility, alias, ..
        } => {
            assert!(visibility.is_some());
            assert!(alias.is_some());
        }
        other => panic!("expected alias with visibility, got {:?}", other),
    }

    match adaptations[2] {
        TraitAdaptation::Alias {
            method: TraitMethodRef {
                trait_name: None, ..
            },
            alias,
            ..
        } => {
            assert!(alias.is_some());
        }
        other => panic!("expected unqualified alias, got {:?}", other),
    }
}
