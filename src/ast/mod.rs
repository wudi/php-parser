use crate::span::Span;
use crate::lexer::token::Token;

pub type ExprId<'ast> = &'ast Expr<'ast>;
pub type StmtId<'ast> = &'ast Stmt<'ast>;

#[derive(Debug)]
pub struct Program<'ast> {
    pub statements: &'ast [StmtId<'ast>],
    pub span: Span,
}

#[derive(Debug)]
pub enum Stmt<'ast> {
    Echo {
        exprs: &'ast [ExprId<'ast>],
        span: Span,
    },
    Return {
        expr: Option<ExprId<'ast>>,
        span: Span,
    },
    If {
        condition: ExprId<'ast>,
        then_block: &'ast [StmtId<'ast>],
        else_block: Option<&'ast [StmtId<'ast>]>, // Simplified: else block is just statements for now
        span: Span,
    },
    While {
        condition: ExprId<'ast>,
        body: &'ast [StmtId<'ast>],
        span: Span,
    },
    Foreach {
        expr: ExprId<'ast>,
        key_var: Option<ExprId<'ast>>,
        value_var: ExprId<'ast>,
        body: &'ast [StmtId<'ast>],
        span: Span,
    },
    Block {
        statements: &'ast [StmtId<'ast>],
        span: Span,
    },
    Function {
        name: &'ast Token, 
        params: &'ast [Param<'ast>],
        body: &'ast [StmtId<'ast>],
        span: Span,
    },
    Class {
        name: &'ast Token,
        members: &'ast [ClassMember<'ast>],
        span: Span,
    },
    Switch {
        condition: ExprId<'ast>,
        cases: &'ast [Case<'ast>],
        span: Span,
    },
    Try {
        body: &'ast [StmtId<'ast>],
        catches: &'ast [Catch<'ast>],
        finally: Option<&'ast [StmtId<'ast>]>,
        span: Span,
    },
    Throw {
        expr: ExprId<'ast>,
        span: Span,
    },
    Expression {
        expr: ExprId<'ast>,
        span: Span,
    },
    Error {
        span: Span,
    },
    Noop,
}

#[derive(Debug, Clone, Copy)]
pub struct Param<'ast> {
    pub name: &'ast Token,
    pub ty: Option<&'ast Token>, // Simplified type hint
    pub default: Option<ExprId<'ast>>,
    pub span: Span,
}

#[derive(Debug)]
pub enum Expr<'ast> {
    Assign {
        var: ExprId<'ast>,
        expr: ExprId<'ast>,
        span: Span,
    },
    Binary {

        left: ExprId<'ast>,
        op: BinaryOp,
        right: ExprId<'ast>,
        span: Span,
    },
    Unary {
        op: UnaryOp,
        expr: ExprId<'ast>,
        span: Span,
    },
    Call {
        func: ExprId<'ast>,
        args: &'ast [Arg<'ast>],
        span: Span,
    },
    Array {
        items: &'ast [ArrayItem<'ast>],
        span: Span,
    },
    ArrayDimFetch {
        array: ExprId<'ast>,
        dim: Option<ExprId<'ast>>, // None for $a[]
        span: Span,
    },
    PropertyFetch {
        target: ExprId<'ast>,
        property: ExprId<'ast>, // Usually Identifier or Variable
        span: Span,
    },
    MethodCall {
        target: ExprId<'ast>,
        method: ExprId<'ast>,
        args: &'ast [Arg<'ast>],
        span: Span,
    },
    StaticCall {
        class: ExprId<'ast>,
        method: ExprId<'ast>,
        args: &'ast [Arg<'ast>],
        span: Span,
    },
    ClassConstFetch {
        class: ExprId<'ast>,
        constant: ExprId<'ast>,
        span: Span,
    },
    New {
        class: ExprId<'ast>,
        args: &'ast [Arg<'ast>],
        span: Span,
    },
    Variable {
        name: Span,
        span: Span,
    },
    Integer {
        value: &'ast [u8], 
        span: Span,
    },
    String {
        value: &'ast [u8],
        span: Span,
    },
    Error {
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
    BitNot,
}

impl<'ast> Expr<'ast> {
    pub fn span(&self) -> Span {
        match self {
            Expr::Assign { span, .. } => *span,
            Expr::Binary { span, .. } => *span,
            Expr::Unary { span, .. } => *span,
            Expr::Call { span, .. } => *span,
            Expr::Array { span, .. } => *span,
            Expr::ArrayDimFetch { span, .. } => *span,
            Expr::PropertyFetch { span, .. } => *span,
            Expr::MethodCall { span, .. } => *span,
            Expr::StaticCall { span, .. } => *span,
            Expr::ClassConstFetch { span, .. } => *span,
            Expr::New { span, .. } => *span,
            Expr::Variable { span, .. } => *span,
            Expr::Integer { span, .. } => *span,
            Expr::String { span, .. } => *span,
            Expr::Error { span } => *span,
        }
    }
}

impl<'ast> Stmt<'ast> {
    pub fn span(&self) -> Span {
        match self {
            Stmt::Echo { span, .. } => *span,
            Stmt::Return { span, .. } => *span,
            Stmt::If { span, .. } => *span,
            Stmt::While { span, .. } => *span,
            Stmt::Foreach { span, .. } => *span,
            Stmt::Block { span, .. } => *span,
            Stmt::Function { span, .. } => *span,
            Stmt::Class { span, .. } => *span,
            Stmt::Switch { span, .. } => *span,
            Stmt::Try { span, .. } => *span,
            Stmt::Throw { span, .. } => *span,
            Stmt::Expression { span, .. } => *span,
            Stmt::Error { span } => *span,
            Stmt::Noop => Span::default(),
        }
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Plus,
    Minus,
    Mul,
    Div,
    Mod,
    Concat, // .
    Eq,
    EqEq,
    EqEqEq,
    NotEq,
    NotEqEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
}

#[derive(Debug, Clone, Copy)]
pub struct Arg<'ast> {
    pub value: ExprId<'ast>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub struct ArrayItem<'ast> {
    pub key: Option<ExprId<'ast>>,
    pub value: ExprId<'ast>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub enum ClassMember<'ast> {
    Property {
        modifiers: &'ast [Token],
        name: &'ast Token,
        default: Option<ExprId<'ast>>,
        span: Span,
    },
    Method {
        modifiers: &'ast [Token],
        name: &'ast Token,
        params: &'ast [Param<'ast>],
        body: &'ast [StmtId<'ast>],
        span: Span,
    },
    Const {
        name: &'ast Token,
        value: ExprId<'ast>,
        span: Span,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Case<'ast> {
    pub condition: Option<ExprId<'ast>>, // None for default
    pub body: &'ast [StmtId<'ast>],
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub struct Catch<'ast> {
    pub types: &'ast [Token], // Multi-catch: TryCatch|Exception
    pub var: &'ast Token,
    pub body: &'ast [StmtId<'ast>],
    pub span: Span,
}

