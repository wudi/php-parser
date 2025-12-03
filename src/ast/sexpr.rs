use crate::ast::visitor::Visitor;
use crate::ast::*;

pub struct SExprFormatter {
    output: String,
    indent: usize,
}

impl SExprFormatter {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    pub fn finish(self) -> String {
        self.output
    }

    fn write(&mut self, s: &str) {
        self.output.push_str(s);
    }

    fn newline(&mut self) {
        self.output.push('\n');
        for _ in 0..self.indent {
            self.output.push_str("  ");
        }
    }
}

impl<'ast> Visitor<'ast> for SExprFormatter {
    fn visit_program(&mut self, program: &'ast Program<'ast>) {
        self.write("(program");
        self.indent += 1;
        for stmt in program.statements {
            self.newline();
            self.visit_stmt(stmt);
        }
        self.indent -= 1;
        self.write(")");
    }

    fn visit_stmt(&mut self, stmt: StmtId<'ast>) {
        match stmt {
            Stmt::Block { statements, .. } => {
                self.write("(block");
                self.indent += 1;
                for stmt in *statements {
                    self.newline();
                    self.visit_stmt(stmt);
                }
                self.indent -= 1;
                self.write(")");
            }
            Stmt::If {
                condition,
                then_block,
                else_block,
                ..
            } => {
                self.write("(if ");
                self.visit_expr(condition);
                self.indent += 1;
                self.newline();
                self.write("(then");
                self.indent += 1;
                for stmt in *then_block {
                    self.newline();
                    self.visit_stmt(stmt);
                }
                self.indent -= 1;
                self.write(")");
                if let Some(else_block) = else_block {
                    self.newline();
                    self.write("(else");
                    self.indent += 1;
                    for stmt in *else_block {
                        self.newline();
                        self.visit_stmt(stmt);
                    }
                    self.indent -= 1;
                    self.write(")");
                }
                self.indent -= 1;
                self.write(")");
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.write("(while ");
                self.visit_expr(condition);
                self.indent += 1;
                self.newline();
                self.write("(body");
                self.indent += 1;
                for stmt in *body {
                    self.newline();
                    self.visit_stmt(stmt);
                }
                self.indent -= 1;
                self.write("))");
                self.indent -= 1;
            }
            Stmt::Echo { exprs, .. } => {
                self.write("(echo");
                for expr in *exprs {
                    self.write(" ");
                    self.visit_expr(expr);
                }
                self.write(")");
            }
            Stmt::Expression { expr, .. } => {
                self.visit_expr(expr);
            }
            Stmt::Nop { .. } => self.write("(nop)"),
            _ => {
                self.write("(unknown-stmt)");
            }
        }
    }

    fn visit_expr(&mut self, expr: ExprId<'ast>) {
        match expr {
            Expr::Assign { var, expr, .. } => {
                self.write("(assign ");
                self.visit_expr(var);
                self.write(" ");
                self.visit_expr(expr);
                self.write(")");
            }
            Expr::Integer { value, .. } => {
                self.write("(integer ");
                self.write(&String::from_utf8_lossy(value));
                self.write(")");
            }
            Expr::String { value, .. } => {
                self.write("(string \"");
                self.write(&String::from_utf8_lossy(value)); // TODO: Escape
                self.write("\")");
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                self.write("(");
                self.write(match op {
                    BinaryOp::Plus => "+",
                    BinaryOp::Minus => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Div => "/",
                    _ => "unknown-op",
                });
                self.write(" ");
                self.visit_expr(left);
                self.write(" ");
                self.visit_expr(right);
                self.write(")");
            }
            Expr::Variable { .. } => {
                self.write("(variable)");
            }
            _ => {
                self.write("(unknown-expr)");
            }
        }
    }
}
