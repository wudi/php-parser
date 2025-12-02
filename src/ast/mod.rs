use crate::lexer::token::Token;
use crate::span::{LineInfo, Span};
use serde::Serialize;

pub mod visitor;

pub type ExprId<'ast> = &'ast Expr<'ast>;
pub type StmtId<'ast> = &'ast Stmt<'ast>;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ParseError {
    pub span: Span,
    pub message: &'static str,
}

impl ParseError {
    pub fn to_human_readable(&self, source: &[u8]) -> String {
        self.to_human_readable_with_path(source, None)
    }

    pub fn to_human_readable_with_path(&self, source: &[u8], path: Option<&str>) -> String {
        let Some(LineInfo {
            line,
            column,
            line_text,
        }) = self.span.line_info(source)
        else {
            return format!("error: {}", self.message);
        };

        let line_str = String::from_utf8_lossy(line_text);
        let gutter_width = line.to_string().len();
        let padding = std::cmp::min(line_text.len(), column.saturating_sub(1));
        let highlight_len = std::cmp::max(
            1,
            std::cmp::min(self.span.len(), line_text.len().saturating_sub(padding)),
        );

        let mut marker = String::new();
        marker.push_str(&" ".repeat(padding));
        marker.push_str(&"^".repeat(highlight_len));

        let location = match path {
            Some(path) => format!("{path}:{line}:{column}"),
            None => format!("line {line}, column {column}"),
        };

        format!(
            "error: {}\n --> {}\n{gutter}|\n{line_no:>width$} | {line_src}\n{gutter}| {marker}",
            self.message,
            location,
            gutter = " ".repeat(gutter_width + 1),
            line_no = line,
            width = gutter_width,
            line_src = line_str,
            marker = marker,
        )
    }
}

#[derive(Debug, Serialize)]
pub struct Program<'ast> {
    pub statements: &'ast [StmtId<'ast>],
    pub errors: &'ast [ParseError],
    pub span: Span,
}

#[derive(Debug, Serialize)]
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
        by_ref: bool,
        params: &'ast [Param<'ast>],
        return_type: Option<&'ast Type<'ast>>,
        body: &'ast [StmtId<'ast>],
        span: Span,
    },
    Class {
        attributes: &'ast [AttributeGroup<'ast>],
        modifiers: &'ast [Token],
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
        backed_type: Option<&'ast Type<'ast>>,
        implements: &'ast [Name<'ast>],
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
    Const {
        attributes: &'ast [AttributeGroup<'ast>],
        consts: &'ast [ClassConst<'ast>],
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
    InlineHtml {
        value: &'ast [u8],
        span: Span,
    },
    Nop {
        span: Span,
    },
    Label {
        name: &'ast Token,
        span: Span,
    },
    Goto {
        label: &'ast Token,
        span: Span,
    },
    Error {
        span: Span,
    },
    Declare {
        declares: &'ast [DeclareItem<'ast>],
        body: &'ast [StmtId<'ast>],
        span: Span,
    },
    HaltCompiler {
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct StaticVar<'ast> {
    pub var: ExprId<'ast>,
    pub default: Option<ExprId<'ast>>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Param<'ast> {
    pub attributes: &'ast [AttributeGroup<'ast>],
    pub modifiers: &'ast [Token],
    pub name: &'ast Token,
    pub ty: Option<&'ast Type<'ast>>,
    pub default: Option<ExprId<'ast>>,
    pub by_ref: bool,
    pub variadic: bool,
    pub hooks: Option<&'ast [PropertyHook<'ast>]>,
    pub span: Span,
}

#[derive(Debug, Serialize)]
pub enum Expr<'ast> {
    Assign {
        var: ExprId<'ast>,
        expr: ExprId<'ast>,
        span: Span,
    },
    AssignRef {
        var: ExprId<'ast>,
        expr: ExprId<'ast>,
        span: Span,
    },
    AssignOp {
        var: ExprId<'ast>,
        op: AssignOp,
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
    InterpolatedString {
        parts: &'ast [ExprId<'ast>],
        span: Span,
    },
    ShellExec {
        parts: &'ast [ExprId<'ast>],
        span: Span,
    },
    Include {
        kind: IncludeKind,
        expr: ExprId<'ast>,
        span: Span,
    },
    MagicConst {
        kind: MagicConstKind,
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
    AnonymousClass {
        attributes: &'ast [AttributeGroup<'ast>],
        modifiers: &'ast [Token],
        args: &'ast [Arg<'ast>],
        extends: Option<Name<'ast>>,
        implements: &'ast [Name<'ast>],
        members: &'ast [ClassMember<'ast>],
        span: Span,
    },
    Print {
        expr: ExprId<'ast>,
        span: Span,
    },
    Yield {
        key: Option<ExprId<'ast>>,
        value: Option<ExprId<'ast>>,
        from: bool,
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
        is_static: bool,
        by_ref: bool,
        params: &'ast [Param<'ast>],
        uses: &'ast [ClosureUse<'ast>],
        return_type: Option<&'ast Type<'ast>>,
        body: &'ast [StmtId<'ast>],
        span: Span,
    },
    ArrowFunction {
        attributes: &'ast [AttributeGroup<'ast>],
        is_static: bool,
        by_ref: bool,
        params: &'ast [Param<'ast>],
        return_type: Option<&'ast Type<'ast>>,
        expr: ExprId<'ast>,
        span: Span,
    },
    Clone {
        expr: ExprId<'ast>,
        span: Span,
    },
    NullsafePropertyFetch {
        target: ExprId<'ast>,
        property: ExprId<'ast>,
        span: Span,
    },
    NullsafeMethodCall {
        target: ExprId<'ast>,
        method: ExprId<'ast>,
        args: &'ast [Arg<'ast>],
        span: Span,
    },
    VariadicPlaceholder {
        span: Span,
    },
    Error {
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ClosureUse<'ast> {
    pub var: &'ast Token,
    pub by_ref: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CastKind {
    Int,
    Bool,
    Float,
    String,
    Array,
    Object,
    Unset,
    Void,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct MatchArm<'ast> {
    pub conditions: Option<&'ast [ExprId<'ast>]>, // None for default
    pub body: ExprId<'ast>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct AnonymousClass<'ast> {
    pub args: &'ast [Arg<'ast>],
    pub extends: Option<Name<'ast>>,
    pub implements: &'ast [Name<'ast>],
    pub members: &'ast [ClassMember<'ast>],
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum UnaryOp {
    Plus,
    Minus,
    Not,
    BitNot,
    PreInc,
    PreDec,
    ErrorSuppress,
    Reference,
}

impl<'ast> Expr<'ast> {
    pub fn span(&self) -> Span {
        match self {
            Expr::Assign { span, .. } => *span,
            Expr::AssignRef { span, .. } => *span,
            Expr::AssignOp { span, .. } => *span,
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
            Expr::InterpolatedString { span, .. } => *span,
            Expr::ShellExec { span, .. } => *span,
            Expr::Include { span, .. } => *span,
            Expr::MagicConst { span, .. } => *span,
            Expr::PostInc { span, .. } => *span,
            Expr::PostDec { span, .. } => *span,
            Expr::Ternary { span, .. } => *span,
            Expr::Match { span, .. } => *span,
            Expr::AnonymousClass { span, .. } => *span,
            Expr::Yield { span, .. } => *span,
            Expr::Cast { span, .. } => *span,
            Expr::Empty { span, .. } => *span,
            Expr::Isset { span, .. } => *span,
            Expr::Eval { span, .. } => *span,
            Expr::Die { span, .. } => *span,
            Expr::Exit { span, .. } => *span,
            Expr::Closure { span, .. } => *span,
            Expr::ArrowFunction { span, .. } => *span,
            Expr::Clone { span, .. } => *span,
            Expr::Print { span, .. } => *span,
            Expr::NullsafePropertyFetch { span, .. } => *span,
            Expr::NullsafeMethodCall { span, .. } => *span,
            Expr::VariadicPlaceholder { span } => *span,
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
            Stmt::Const { span, .. } => *span,
            Stmt::Break { span, .. } => *span,
            Stmt::Continue { span, .. } => *span,
            Stmt::Global { span, .. } => *span,
            Stmt::Static { span, .. } => *span,
            Stmt::Unset { span, .. } => *span,
            Stmt::Expression { span, .. } => *span,
            Stmt::InlineHtml { span, .. } => *span,
            Stmt::Declare { span, .. } => *span,
            Stmt::HaltCompiler { span } => *span,
            Stmt::Label { span, .. } => *span,
            Stmt::Goto { span, .. } => *span,
            Stmt::Error { span } => *span,
            Stmt::Nop { span } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AssignOp {
    Plus,       // +=
    Minus,      // -=
    Mul,        // *=
    Div,        // /=
    Mod,        // %=
    Concat,     // .=
    BitAnd,     // &=
    BitOr,      // |=
    BitXor,     // ^=
    ShiftLeft,  // <<=
    ShiftRight, // >>=
    Pow,        // **=
    Coalesce,   // ??=
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Arg<'ast> {
    pub name: Option<&'ast Token>,
    pub value: ExprId<'ast>,
    pub unpack: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ArrayItem<'ast> {
    pub key: Option<ExprId<'ast>>,
    pub value: ExprId<'ast>,
    pub by_ref: bool,
    pub unpack: bool,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct PropertyEntry<'ast> {
    pub name: &'ast Token,
    pub default: Option<ExprId<'ast>>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum ClassMember<'ast> {
    Property {
        attributes: &'ast [AttributeGroup<'ast>],
        modifiers: &'ast [Token],
        ty: Option<&'ast Type<'ast>>,
        entries: &'ast [PropertyEntry<'ast>],
        span: Span,
    },
    PropertyHook {
        attributes: &'ast [AttributeGroup<'ast>],
        modifiers: &'ast [Token],
        ty: Option<&'ast Type<'ast>>,
        name: &'ast Token,
        default: Option<ExprId<'ast>>,
        hooks: &'ast [PropertyHook<'ast>],
        span: Span,
    },
    Method {
        attributes: &'ast [AttributeGroup<'ast>],
        modifiers: &'ast [Token],
        name: &'ast Token,
        params: &'ast [Param<'ast>],
        return_type: Option<&'ast Type<'ast>>,
        body: &'ast [StmtId<'ast>],
        span: Span,
    },
    Const {
        attributes: &'ast [AttributeGroup<'ast>],
        modifiers: &'ast [Token],
        ty: Option<&'ast Type<'ast>>,
        consts: &'ast [ClassConst<'ast>],
        span: Span,
    },
    TraitUse {
        attributes: &'ast [AttributeGroup<'ast>],
        traits: &'ast [Name<'ast>],
        adaptations: &'ast [TraitAdaptation<'ast>],
        span: Span,
    },
    Case {
        attributes: &'ast [AttributeGroup<'ast>],
        name: &'ast Token,
        value: Option<ExprId<'ast>>,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Case<'ast> {
    pub condition: Option<ExprId<'ast>>, // None for default
    pub body: &'ast [StmtId<'ast>],
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ClassConst<'ast> {
    pub name: &'ast Token,
    pub value: ExprId<'ast>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum PropertyHookBody<'ast> {
    None,
    Statements(&'ast [StmtId<'ast>]),
    Expr(ExprId<'ast>),
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct PropertyHook<'ast> {
    pub attributes: &'ast [AttributeGroup<'ast>],
    pub modifiers: &'ast [Token],
    pub name: &'ast Token,
    pub params: &'ast [Param<'ast>],
    pub by_ref: bool,
    pub body: PropertyHookBody<'ast>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct TraitMethodRef<'ast> {
    pub trait_name: Option<Name<'ast>>,
    pub method: &'ast Token,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum TraitAdaptation<'ast> {
    Precedence {
        method: TraitMethodRef<'ast>,
        insteadof: &'ast [Name<'ast>],
        span: Span,
    },
    Alias {
        method: TraitMethodRef<'ast>,
        alias: Option<&'ast Token>,
        visibility: Option<&'ast Token>,
        span: Span,
    },
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Catch<'ast> {
    pub types: &'ast [Name<'ast>], // Multi-catch: TryCatch|Exception
    pub var: Option<&'ast Token>,  // Variable may be omitted in PHP 8+
    pub body: &'ast [StmtId<'ast>],
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Name<'ast> {
    pub parts: &'ast [Token],
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct UseItem<'ast> {
    pub name: Name<'ast>,
    pub alias: Option<&'ast Token>,
    pub kind: UseKind,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum UseKind {
    Normal,
    Function,
    Const,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Attribute<'ast> {
    pub name: Name<'ast>,
    pub args: &'ast [Arg<'ast>],
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct AttributeGroup<'ast> {
    pub attributes: &'ast [Attribute<'ast>],
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum Type<'ast> {
    Simple(&'ast Token),
    Name(Name<'ast>),
    Union(&'ast [Type<'ast>]),
    Intersection(&'ast [Type<'ast>]),
    Nullable(&'ast Type<'ast>),
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct DeclareItem<'ast> {
    pub key: &'ast Token,
    pub value: ExprId<'ast>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum IncludeKind {
    Include,
    IncludeOnce,
    Require,
    RequireOnce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum MagicConstKind {
    Dir,
    File,
    Line,
    Function,
    Class,
    Trait,
    Method,
    Namespace,
    Property,
}
