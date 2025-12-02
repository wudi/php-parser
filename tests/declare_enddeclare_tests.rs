use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn test_declare_enddeclare_strict_types() {
    let code = r#"<?php
declare(strict_types=1):
    function add(int $a, int $b): int {
        return $a + $b;
    }
enddeclare;
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_declare_enddeclare_ticks() {
    let code = r#"<?php
declare(ticks=1):
    $counter = 0;
    register_tick_function(function() use (&$counter) {
        $counter++;
    });
enddeclare;
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_declare_enddeclare_encoding() {
    let code = r#"<?php
declare(encoding='UTF-8'):
    $text = "Hello World";
enddeclare;
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_declare_enddeclare_multiple_directives() {
    let code = r#"<?php
declare(strict_types=1, ticks=1):
    function foo() {
        return 42;
    }
enddeclare;
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_declare_enddeclare_nested() {
    let code = r#"<?php
declare(strict_types=1):
    declare(ticks=1):
        $x = 10;
    enddeclare;
enddeclare;
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_declare_enddeclare_with_class() {
    let code = r#"<?php
declare(strict_types=1):
    class Calculator {
        public function add(int $a, int $b): int {
            return $a + $b;
        }
    }
enddeclare;
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_declare_enddeclare_empty() {
    let code = r#"<?php
declare(strict_types=1):
enddeclare;
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_declare_enddeclare_mixed_with_regular_code() {
    let code = r#"<?php
$before = true;

declare(strict_types=1):
    function typed(int $x): int {
        return $x * 2;
    }
enddeclare;

$after = false;
"#;
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    insta::assert_debug_snapshot!(program);
}
