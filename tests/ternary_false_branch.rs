use bumpalo::Bump;
use insta::assert_debug_snapshot;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn test_false_branch_with_assignment() {
    // PHP allows low-precedence operators in ternary branches
    // This should parse as: $a ? 10 : ($b = 5)
    // NOT as: ($a ? 10 : $b) = 5
    let input = b"<?php $a ? 10 : $b = 5;";
    let arena = Bump::new();
    let lexer = Lexer::new(input);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    assert_debug_snapshot!(program);
}

#[test]
fn test_false_branch_with_and() {
    // PHP allows 'and' operator in ternary branches
    // This should parse as: $a ? 10 : ($b and $c)
    // NOT as: ($a ? 10 : $b) and $c
    let input = b"<?php $a ? 10 : $b and $c;";
    let arena = Bump::new();
    let lexer = Lexer::new(input);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    assert_debug_snapshot!(program);
}

#[test]
fn test_true_branch_with_assignment() {
    // Similarly, true branch should also allow low-precedence operators
    // This should parse as: $a ? ($b = 5) : 10
    let input = b"<?php $a ? $b = 5 : 10;";
    let arena = Bump::new();
    let lexer = Lexer::new(input);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    assert_debug_snapshot!(program);
}

#[test]
fn test_true_branch_with_or() {
    // True branch with 'or' operator
    // This should parse as: $a ? ($b or $c) : $d
    let input = b"<?php $a ? $b or $c : $d;";
    let arena = Bump::new();
    let lexer = Lexer::new(input);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    assert_debug_snapshot!(program);
}


#[test]
fn test_ternary_precedence_in_assignment() {
    // $res = $a ? 10 : 1 and 0;
    // Should parse as: ($res = ($a ? 10 : 1)) and 0
    // Because '=' (35) > 'and' (10).
    // So the assignment consumes the ternary, but stops at 'and'.
    // The ternary false branch must also stop at 'and'.
    
    let input = b"<?php $res = $a ? 10 : 1 and 0;";
    let arena = Bump::new();
    let lexer = Lexer::new(input);
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    assert_debug_snapshot!(program);
}
