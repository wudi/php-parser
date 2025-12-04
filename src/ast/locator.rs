use crate::ast::visitor::{Visitor, walk_class_member, walk_expr, walk_stmt};
use crate::ast::*;
use crate::span::Span;

#[derive(Debug, Clone, Copy)]
pub enum AstNode<'ast> {
    Stmt(StmtId<'ast>),
    Expr(ExprId<'ast>),
    ClassMember(&'ast ClassMember<'ast>),
}

impl<'ast> AstNode<'ast> {
    pub fn span(&self) -> Span {
        match self {
            AstNode::Stmt(s) => s.span(),
            AstNode::Expr(e) => e.span(),
            AstNode::ClassMember(m) => match m {
                ClassMember::Property { span, .. } => *span,
                ClassMember::PropertyHook { span, .. } => *span,
                ClassMember::Method { span, .. } => *span,
                ClassMember::Const { span, .. } => *span,
                ClassMember::TraitUse { span, .. } => *span,
                ClassMember::Case { span, .. } => *span,
            },
        }
    }
}

pub struct Locator<'ast> {
    target: usize,
    pub path: Vec<AstNode<'ast>>,
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

    fn visit_class_member(&mut self, member: &'ast ClassMember<'ast>) {
        let span = match member {
            ClassMember::Property { span, .. } => *span,
            ClassMember::PropertyHook { span, .. } => *span,
            ClassMember::Method { span, .. } => *span,
            ClassMember::Const { span, .. } => *span,
            ClassMember::TraitUse { span, .. } => *span,
            ClassMember::Case { span, .. } => *span,
        };

        if span.start <= self.target && self.target <= span.end {
            self.path.push(AstNode::ClassMember(member));
            walk_class_member(self, member);
        }
    }
}
