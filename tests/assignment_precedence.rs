use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;
use bumpalo::Bump;
use php_parser_rs::ast::Expr;

#[test]
fn test_bang_assignment() {
    let code = "<?php ! $a = 1;";
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
    
    println!("Statements: {}", program.statements.len());
    for s in program.statements.iter() {
        println!("Stmt: {:?}", s);
    }
    let stmt = &program.statements[program.statements.len() - 1];
    // Should be ExprStmt(UnaryOp(Not, Assign(a, 1)))
    
    match stmt {
        php_parser_rs::ast::Stmt::Expression { expr, .. } => {
            match expr {
                Expr::Unary { op, expr: inner, .. } => {
                    assert!(matches!(op, php_parser_rs::ast::UnaryOp::Not));
                    match inner {
                        Expr::Assign { var, expr: _right, .. } => {
                            match var {
                                Expr::Variable { .. } => {
                                    // OK
                                }
                                _ => panic!("Expected variable"),
                            }
                        }
                        _ => panic!("Expected assignment inside unary op"),
                    }
                }
                _ => panic!("Expected unary op"),
            }
        }
        _ => panic!("Expected expression statement"),
    }
}

#[test]
fn test_cast_assignment() {
    let code = "<?php (int) $a = 1;";
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
}

#[test]
fn test_at_assignment() {
    let code = "<?php @ $a = 1;";
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
}

#[test]
fn test_binary_assignment() {
    let code = "<?php $a + $b = 1;";
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
}

#[test]
fn test_ternary_assignment() {
    let code = "<?php $a ? $b : $c = 1;";
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
}

#[test]
fn test_assign_op_reassociation() {
    let code = "<?php $a + $b += 1;";
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
}

#[test]
fn test_clone_assignment() {
    let code = "<?php clone $a = 1;";
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();

    assert!(program.errors.is_empty());
}
