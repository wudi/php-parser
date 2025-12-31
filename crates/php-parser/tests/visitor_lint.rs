use bumpalo::Bump;
use php_parser::ast::visitor::{walk_expr, walk_stmt, Visitor};
use php_parser::ast::{Expr, Stmt};
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;
use php_parser::Span;

#[derive(Default)]
struct LintVisitor {
    gotos: Vec<Span>,
    evals: Vec<Span>,
}

impl<'ast> Visitor<'ast> for LintVisitor {
    fn visit_stmt(&mut self, stmt: php_parser::ast::StmtId<'ast>) {
        if let Stmt::Goto { span, .. } = stmt {
            self.gotos.push(*span);
        }

        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: php_parser::ast::ExprId<'ast>) {
        if let Expr::Eval { span, .. } = expr {
            self.evals.push(*span);
        }

        walk_expr(self, expr);
    }
}

#[test]
fn visitor_drives_simple_lint() {
    let code = r#"<?php
function demo($items) {
    foreach ($items as $item) {
        if ($item) {
            goto end;
        }
    }

    $value = eval('2 + 2');
    $closure = function() use ($items) {
        return eval('3');
    };
    $matches = match ($value) {
        4 => eval('4'),
        default => $value,
    };
    end:
        return $matches;
}
"#;

    let arena = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let mut visitor = LintVisitor::default();
    visitor.visit_program(&program);

    assert_eq!(visitor.gotos.len(), 1);
    assert_eq!(visitor.evals.len(), 3);
}
