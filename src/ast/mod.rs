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
    DoWhile {
        body: &'ast [StmtId<'ast>],
        condition: ExprId<'ast>,
        span: Span,
    },
    For {
        init: &'ast [ExprId<'ast>],
        condition: &'ast [ExprId<'ast>], // Can be multiple expressions separated by comma, but usually one. PHP allows empty.
        loop_expr: &'ast [ExprId<'ast>],
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
        attributes: &'ast [AttributeGroup<'ast>],
        name: &'ast Token, 
        params: &'ast [Param<'ast>],
        body: &'ast [StmtId<'ast>],
        span: Span,
    },
    Class {
        attributes: &'ast [AttributeGroup<'ast>],
        name: &'ast Token,
        extends: Option<Name<'ast>>,
        implements: &'ast [Name<'ast>],
        members: &'ast [ClassMember<'ast>],
        span: Span,
    },
    Interface {
        attributes: &'ast [AttributeGroup<'ast>],
        name: &'ast Token,
        extends: &'ast [Name<'ast>],
        members: &'ast [ClassMember<'ast>],
        span: Span,
    },
    Trait {
        attributes: &'ast [AttributeGroup<'ast>],
        name: &'ast Token,
        members: &'ast [ClassMember<'ast>],
        span: Span,
    },
    Enum {
        attributes: &'ast [AttributeGroup<'ast>],
        name: &'ast Token,
        members: &'ast [ClassMember<'ast>],
        span: Span,
    },
    Namespace {
        name: Option<Name<'ast>>,
        body: Option<&'ast [StmtId<'ast>]>,
        span: Span,
    },
    Use {
        uses: &'ast [UseItem<'ast>],
        kind: UseKind,
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
    Break {
        level: Option<ExprId<'ast>>,
        span: Span,
    },
    Continue {
        level: Option<ExprId<'ast>>,
        span: Span,
    },
    Global {
        vars: &'ast [ExprId<'ast>],
        span: Span,
    },
    Static {
        vars: &'ast [StaticVar<'ast>],
        span: Span,
    },
    Unset {
        vars: &'ast [ExprId<'ast>],
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
pub struct StaticVar<'ast> {
    pub var: ExprId<'ast>,
    pub default: Option<ExprId<'ast>>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub struct Param<'ast> {
    pub attributes: &'ast [AttributeGroup<'ast>],
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
    Float {
        value: &'ast [u8],
        span: Span,
    },
    Boolean {
        value: bool,
        span: Span,
    },
    Null {
        span: Span,
    },
    String {
        value: &'ast [u8],
        span: Span,
    },
    PostInc {
        var: ExprId<'ast>,
        span: Span,
    },
    PostDec {
        var: ExprId<'ast>,
        span: Span,
    },
    Ternary {
        condition: ExprId<'ast>,
        if_true: Option<ExprId<'ast>>,
        if_false: ExprId<'ast>,
        span: Span,
    },
    Match {
        condition: ExprId<'ast>,
        arms: &'ast [MatchArm<'ast>],
        span: Span,
    },
    Cast {
        kind: CastKind,
        expr: ExprId<'ast>,
        span: Span,
    },
    Empty {
        expr: ExprId<'ast>,
        span: Span,
    },
    Isset {
        vars: &'ast [ExprId<'ast>],
        span: Span,
    },
    Eval {
        expr: ExprId<'ast>,
        span: Span,
    },
    Die {
        expr: Option<ExprId<'ast>>,
        span: Span,
    },
    Exit {
        expr: Option<ExprId<'ast>>,
        span: Span,
    },
    Closure {
        attributes: &'ast [AttributeGroup<'ast>],
        params: &'ast [Param<'ast>],
        uses: &'ast [ClosureUse<'ast>],
        return_type: Option<&'ast Token>,
        body: &'ast [StmtId<'ast>],
        span: Span,
    },
    ArrowFunction {
        attributes: &'ast [AttributeGroup<'ast>],
        params: &'ast [Param<'ast>],
        return_type: Option<&'ast Token>,
        expr: ExprId<'ast>,
        span: Span,
    },
    Error {
        span: Span,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct ClosureUse<'ast> {
    pub var: &'ast Token,
    pub by_ref: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastKind {
    Int,
    Bool,
    Float,
    String,
    Array,
    Object,
    Unset,
}

#[derive(Debug, Clone, Copy)]
pub struct MatchArm<'ast> {
    pub conditions: Option<&'ast [ExprId<'ast>]>, // None for default
    pub body: ExprId<'ast>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
    BitNot,
    PreInc,
    PreDec,
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
            Expr::Float { span, .. } => *span,
            Expr::Boolean { span, .. } => *span,
            Expr::Null { span, .. } => *span,
            Expr::String { span, .. } => *span,
            Expr::PostInc { span, .. } => *span,
            Expr::PostDec { span, .. } => *span,
            Expr::Ternary { span, .. } => *span,
            Expr::Match { span, .. } => *span,
            Expr::Cast { span, .. } => *span,
            Expr::Empty { span, .. } => *span,
            Expr::Isset { span, .. } => *span,
            Expr::Eval { span, .. } => *span,
            Expr::Die { span, .. } => *span,
            Expr::Exit { span, .. } => *span,
            Expr::Closure { span, .. } => *span,
            Expr::ArrowFunction { span, .. } => *span,
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
            Stmt::DoWhile { span, .. } => *span,
            Stmt::For { span, .. } => *span,
            Stmt::Foreach { span, .. } => *span,
            Stmt::Block { span, .. } => *span,
            Stmt::Function { span, .. } => *span,
            Stmt::Class { span, .. } => *span,
            Stmt::Interface { span, .. } => *span,
            Stmt::Trait { span, .. } => *span,
            Stmt::Enum { span, .. } => *span,
            Stmt::Namespace { span, .. } => *span,
            Stmt::Use { span, .. } => *span,
            Stmt::Switch { span, .. } => *span,
            Stmt::Try { span, .. } => *span,
            Stmt::Throw { span, .. } => *span,
            Stmt::Break { span, .. } => *span,
            Stmt::Continue { span, .. } => *span,
            Stmt::Global { span, .. } => *span,
            Stmt::Static { span, .. } => *span,
            Stmt::Unset { span, .. } => *span,
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
    Coalesce,
    Spaceship,
    Pow,
    ShiftLeft,
    ShiftRight,
    LogicalAnd,
    LogicalOr,
    LogicalXor,
    Instanceof,
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
    pub by_ref: bool,
    pub unpack: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub enum ClassMember<'ast> {
    Property {
        attributes: &'ast [AttributeGroup<'ast>],
        modifiers: &'ast [Token],
        ty: Option<&'ast Token>,
        name: &'ast Token,
        default: Option<ExprId<'ast>>,
        span: Span,
    },
    Method {
        attributes: &'ast [AttributeGroup<'ast>],
        modifiers: &'ast [Token],
        name: &'ast Token,
        params: &'ast [Param<'ast>],
        body: &'ast [StmtId<'ast>],
        span: Span,
    },
    Const {
        attributes: &'ast [AttributeGroup<'ast>],
        name: &'ast Token,
        value: ExprId<'ast>,
        span: Span,
    },
    TraitUse {
        attributes: &'ast [AttributeGroup<'ast>],
        traits: &'ast [Name<'ast>],
        span: Span,
    },
    Case {
        attributes: &'ast [AttributeGroup<'ast>],
        name: &'ast Token,
        value: Option<ExprId<'ast>>,
        span: Span,
    },
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

#[derive(Debug, Clone, Copy)]
pub struct Name<'ast> {
    pub parts: &'ast [Token],
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub struct UseItem<'ast> {
    pub name: Name<'ast>,
    pub alias: Option<&'ast Token>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UseKind {
    Normal,
    Function,
    Const,
}

#[derive(Debug, Clone, Copy)]
pub struct Attribute<'ast> {
    pub name: Name<'ast>,
    pub args: &'ast [Arg<'ast>],
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub struct AttributeGroup<'ast> {
    pub attributes: &'ast [Attribute<'ast>],
    pub span: Span,
}

