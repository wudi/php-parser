use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn test_list_destructuring_basic() {
    let source = b"<?php
list($a, $b, $c) = $array;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_short_array_destructuring() {
    let source = b"<?php
[$a, $b, $c] = $array;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_nested_list_destructuring() {
    let source = b"<?php
list($a, list($b, $c), $d) = $array;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_nested_short_array_destructuring() {
    let source = b"<?php
[$a, [$b, $c], $d] = $array;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_keyed_destructuring() {
    let source = b"<?php
['name' => $name, 'age' => $age] = $person;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_keyed_list_destructuring() {
    let source = b"<?php
list('name' => $name, 'age' => $age) = $person;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_destructuring_with_skip() {
    let source = b"<?php
[$a, , $c] = $array;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_destructuring_with_references() {
    let source = b"<?php
list(&$a, $b, &$c) = $array;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_foreach_with_list_destructuring() {
    let source = b"<?php
foreach ($array as list($a, $b)) {
    echo $a . $b;
}
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_foreach_with_short_array_destructuring() {
    let source = b"<?php
foreach ($array as [$a, $b]) {
    echo $a . $b;
}
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_foreach_with_keyed_destructuring() {
    let source = b"<?php
foreach ($people as ['name' => $name, 'age' => $age]) {
    echo $name . ' is ' . $age;
}
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_foreach_with_key_and_destructuring() {
    let source = b"<?php
foreach ($array as $key => [$a, $b]) {
    echo $key . $a . $b;
}
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_nested_foreach_destructuring() {
    let source = b"<?php
foreach ($matrix as [[$a, $b], [$c, $d]]) {
    echo $a + $b + $c + $d;
}
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_destructuring_with_spread() {
    let source = b"<?php
[$first, ...$rest] = $array;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_mixed_nested_destructuring() {
    let source = b"<?php
[
    'user' => ['name' => $name, 'email' => $email],
    'posts' => [$firstPost, ...$otherPosts]
] = $data;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_destructuring_in_function_param() {
    let source = b"<?php
function foo([$a, $b]) {
    return $a + $b;
}
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}
