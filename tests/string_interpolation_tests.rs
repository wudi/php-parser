use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn test_simple_variable_interpolation() {
    let source = b"<?php
echo \"Hello $name\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_array_access_in_string() {
    let source = b"<?php
echo \"Value: $array[0]\";
echo \"Key: $array['key']\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_property_access_in_string() {
    let source = b"<?php
echo \"Name: $obj->name\";
echo \"Value: $obj->value\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_nullsafe_in_string() {
    let source = b"<?php
echo \"Value: $obj?->prop\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_curly_brace_syntax() {
    let source = b"<?php
echo \"Value: {$var}\";
echo \"Array: {$array[0]}\";
echo \"Property: {$obj->prop}\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_dollar_curly_syntax() {
    let source = b"<?php
echo \"Value: ${var}\";
echo \"Expr: ${expr + 1}\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_complex_expression_in_string() {
    let source = b"<?php
echo \"Sum: {$a + $b}\";
echo \"Call: {$obj->method()}\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_nested_array_in_string() {
    let source = b"<?php
echo \"Value: $array[0][1]\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_variable_variable_in_string() {
    let source = b"<?php
echo \"Value: ${$varname}\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_mixed_interpolation() {
    let source = b"<?php
echo \"User: $user->name, Age: {$user->age}, ID: $user->id\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_escaped_variables() {
    let source = b"<?php
echo \"Literal \\$var\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_interpolation_with_methods() {
    let source = b"<?php
echo \"Result: {$obj->method()->prop}\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_negative_array_index() {
    let source = b"<?php
echo \"Value: $array[-1]\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_string_interpolation_in_heredoc() {
    let source = b"<?php
$text = <<<EOT
Hello $name
Value: $obj->prop
Array: $array[0]
EOT;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_no_interpolation_in_nowdoc() {
    let source = b"<?php
$text = <<<'EOT'
Literal $name
Not interpolated $var
EOT;
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_multiple_variables_in_string() {
    let source = b"<?php
echo \"$a $b $c $d\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_interpolation_with_concatenation() {
    let source = b"<?php
echo \"Hello $name\" . \" World\";
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_empty_string_with_no_interpolation() {
    let source = b"<?php
echo \"\";
echo '';
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}
