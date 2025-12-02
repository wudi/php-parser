use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn test_match_basic() {
    let source = b"<?php
$result = match($value) {
    1 => 'one',
    2 => 'two',
    default => 'other',
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_match_multiple_conditions() {
    let source = b"<?php
$result = match($value) {
    1, 2, 3 => 'small',
    4, 5, 6 => 'medium',
    7, 8, 9 => 'large',
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_match_trailing_comma_in_conditions() {
    let source = b"<?php
$result = match($value) {
    1, 2, 3, => 'numbers',
    default => 'other',
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_match_trailing_comma_in_arms() {
    let source = b"<?php
$result = match($value) {
    1 => 'one',
    2 => 'two',
    default => 'other',
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_match_no_default() {
    let source = b"<?php
$result = match($value) {
    1 => 'one',
    2 => 'two',
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_match_empty() {
    let source = b"<?php
$result = match($value) {
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_match_only_default() {
    let source = b"<?php
$result = match($value) {
    default => 'always',
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_match_complex_expressions() {
    let source = b"<?php
$result = match(true) {
    $x > 0 => 'positive',
    $x < 0 => 'negative',
    default => 'zero',
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_match_with_function_calls() {
    let source = b"<?php
$result = match($value) {
    1 => doOne(),
    2 => doTwo(),
    default => doDefault(),
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_match_nested() {
    let source = b"<?php
$result = match($outer) {
    1 => match($inner) {
        'a' => 'one-a',
        'b' => 'one-b',
    },
    2 => 'two',
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_match_with_array_creation() {
    let source = b"<?php
$result = match($value) {
    1 => ['a', 'b'],
    2 => ['c', 'd'],
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_match_with_ternary() {
    let source = b"<?php
$result = match($value) {
    1 => $x ? 'yes' : 'no',
    default => 'maybe',
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_match_with_null_coalesce() {
    let source = b"<?php
$result = match($value) {
    1 => $x ?? 'default',
    2 => $y ?? 'fallback',
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_match_string_keys() {
    let source = b"<?php
$result = match($value) {
    'foo' => 1,
    'bar' => 2,
    'baz' => 3,
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_match_mixed_condition_types() {
    let source = b"<?php
$result = match($value) {
    1, 'one', true => 'match',
    default => 'no match',
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}
