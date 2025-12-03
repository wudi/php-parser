use bumpalo::Bump;
use php_parser::ast::sexpr::SExprFormatter;
use php_parser::ast::visitor::Visitor;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn test_sexpr_basic() {
    let code = "<?php echo 1 + 2;";
    let arena = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let mut formatter = SExprFormatter::new();
    formatter.visit_program(&program);
    let output = formatter.finish();

    assert_eq!(
        output,
        "(program\n  (nop)\n  (echo (+ (integer 1) (integer 2))))"
    );
}

#[test]
fn test_sexpr_control_flow() {
    let code = "<?php if ($a) { echo 1; } else { echo 2; } while ($b) { $a = 1; }";
    let arena = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let mut formatter = SExprFormatter::new();
    formatter.visit_program(&program);
    let output = formatter.finish();

    assert_eq!(
        output,
        "(program\n  (nop)\n  (if (variable)\n    (then\n      (echo (integer 1)))\n    (else\n      (echo (integer 2))))\n  (while (variable)\n    (body\n      (assign (variable) (integer 1)))))"
    );
}
