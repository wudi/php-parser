use bumpalo::Bump;
use php_parser::ast::Stmt;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn flags_invalid_modifiers() {
    let code = "<?php class C { abstract final function foo(); readonly function bar(); public private $x; }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    assert!(program.errors.len() >= 2, "expected modifier errors");

    // Ensure class parsed
    assert!(
        program
            .statements
            .iter()
            .any(|s| matches!(**s, Stmt::Class { .. }))
    );
}

#[test]
fn detects_class_modifier_conflicts_and_duplicates() {
    let code = "<?php abstract final abstract class C {}";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    assert!(
        program
            .errors
            .iter()
            .any(|e| e.message.contains("abstract and final"))
    );
    assert!(
        program
            .errors
            .iter()
            .any(|e| e.message.contains("Duplicate abstract"))
    );
}

#[test]
fn detects_duplicate_modifiers_on_members_and_promotions() {
    let code = "<?php class C { public static static function f() {} public static static int $x; function __construct(public readonly readonly int $y) {} }";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    assert!(
        program.errors.len() >= 3,
        "expected duplicate modifier errors"
    );
}

#[test]
fn validates_class_const_modifiers() {
    let code = "<?php
    class C { abstract const A = 1; static const B = 2; readonly const C = 3; public private const D = 4; }
    interface I { protected const X = 1; }
    ";
    let arena = Bump::new();
    let mut parser = Parser::new(Lexer::new(code.as_bytes()), &arena);
    let program = parser.parse_program();

    assert!(
        program.errors.len() >= 4,
        "expected constant modifier errors"
    );
}
