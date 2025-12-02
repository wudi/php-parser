use bumpalo::Bump;
use insta::assert_debug_snapshot;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn test_declare() {
    let code = "<?php declare(strict_types=1);";
    let lexer = Lexer::new(code.as_bytes());
    let bump = Bump::new();
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    assert_debug_snapshot!("declare_strict_types", program);
}

#[test]
fn test_declare_block() {
    let code = "<?php declare(ticks=1) { echo 1; }";
    let lexer = Lexer::new(code.as_bytes());
    let bump = Bump::new();
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    assert_debug_snapshot!("declare_block", program);
}

#[test]
fn test_if_alt() {
    let code = "<?php if ($a): echo 1; elseif ($b): echo 2; else: echo 3; endif;";
    let lexer = Lexer::new(code.as_bytes());
    let bump = Bump::new();
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    assert_debug_snapshot!("if_alt", program);
}

#[test]
fn test_while_alt() {
    let code = "<?php while ($a): echo 1; endwhile;";
    let lexer = Lexer::new(code.as_bytes());
    let bump = Bump::new();
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    assert_debug_snapshot!("while_alt", program);
}

#[test]
fn test_for_alt() {
    let code = "<?php for ($i=0; $i<10; $i++): echo $i; endfor;";
    let lexer = Lexer::new(code.as_bytes());
    let bump = Bump::new();
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    assert_debug_snapshot!("for_alt", program);
}

#[test]
fn test_foreach_alt() {
    let code = "<?php foreach ($arr as $k => $v): echo $v; endforeach;";
    let lexer = Lexer::new(code.as_bytes());
    let bump = Bump::new();
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    assert_debug_snapshot!("foreach_alt", program);
}

#[test]
fn test_switch_alt() {
    let code = "<?php switch ($a): case 1: echo 1; break; default: echo 2; endswitch;";
    let lexer = Lexer::new(code.as_bytes());
    let bump = Bump::new();
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    assert_debug_snapshot!("switch_alt", program);
}
