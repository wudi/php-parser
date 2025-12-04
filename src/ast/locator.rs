use crate::ast::visitor::{walk_expr, walk_stmt, Visitor};
use crate::ast::*;
use crate::span::Span;

#[derive(Debug, Clone, Copy)]
pub enum AstNode<'ast> {
    Stmt(StmtId<'ast>),
    Expr(ExprId<'ast>),
}

impl<'ast> AstNode<'ast> {
    pub fn span(&self) -> Span {
        match self {
            AstNode::Stmt(s) => s.span(),
            AstNode::Expr(e) => e.span(),
        }
    }
}

pub struct Locator<'ast> {
    target: usize,
    path: Vec<AstNode<'ast>>,
}

impl<'ast> Locator<'ast> {
    pub fn new(target: usize) -> Self {
        Self {
            target,
            path: Vec::new(),
        }
    }

    pub fn find(program: &'ast Program<'ast>, target: usize) -> Vec<AstNode<'ast>> {
        let mut locator = Self::new(target);
        locator.visit_program(program);
        locator.path
    }
}

impl<'ast> Visitor<'ast> for Locator<'ast> {
    fn visit_stmt(&mut self, stmt: StmtId<'ast>) {
        let span = stmt.span();
        if span.start <= self.target && self.target <= span.end {
            self.path.push(AstNode::Stmt(stmt));
            walk_stmt(self, stmt);
        }
    }

    fn visit_expr(&mut self, expr: ExprId<'ast>) {
        let span = expr.span();
        if span.start <= self.target && self.target <= span.end {
            self.path.push(AstNode::Expr(expr));
            walk_expr(self, expr);
        }
    }
}
