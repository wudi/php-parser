use crate::span::Span;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn text<'a>(&self, source: &'a [u8]) -> &'a [u8] {
        self.span.as_str(source)
    }

    pub fn line(&self, source: &[u8]) -> usize {
        source
            .get(..self.span.start)
            .unwrap_or_default()
            .iter()
            .filter(|&&b| b == b'\n')
            .count()
            + 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy, Serialize)]
pub enum TokenKind {
    // Keywords
    Function,
    Class,
    Interface,
    Trait,
    Extends,
    Implements,
    Enum,
    If,
    Else,
    ElseIf,
    EndIf,
    Return,
    Echo,
    Print,
    While,
    Do,
    For,
    Foreach,
    EndWhile,
    EndFor,
    EndForeach,
    As,
    Switch,
    EndSwitch,
    Case,
    Default,
    Break,
    Continue,
    Goto,
    Insteadof,
    Try,
    Catch,
    Finally,
    Throw,
    Public,
    Protected,
    Private,
    Static,
    Abstract,
    Final,
    Readonly,
    PublicSet,
    ProtectedSet,
    PrivateSet,
    Namespace,
    Use,
    Global,
    New,
    Clone,
    InstanceOf,
    Array,
    Const,
    Include,
    IncludeOnce,
    Require,
    RequireOnce,
    Eval,
    Exit,
    Die,
    Empty,
    Isset,
    Unset,
    List,
    Yield,
    YieldFrom,
    Declare,
    EndDeclare,
    Match,
    Fn,
    HaltCompiler, // __halt_compiler
    Attribute,    // #[

    // Magic Constants
    Line,
    File,
    Dir,
    ClassC,
    TraitC,
    MethodC,
    FuncC,
    NsC,
    PropertyC,

    // Types (for type hints)
    TypeBool,
    TypeInt,
    TypeFloat,
    TypeString,
    TypeObject,
    TypeVoid,
    TypeIterable,
    TypeCallable,
    TypeMixed,
    TypeNever,
    TypeNull,
    TypeFalse,
    TypeTrue,

    // Casts
    IntCast,
    FloatCast,
    StringCast,
    ArrayCast,
    ObjectCast,
    BoolCast,
    UnsetCast,

    // Identifiers & Literals
    Identifier,
    LNumber,
    DNumber,
    StringLiteral,
    NumString,     // For array offset in string
    StringVarname, // For ${var} in string
    Variable,
    InlineHtml,
    EncapsedAndWhitespace,
    DollarOpenCurlyBraces, // ${
    CurlyOpen,             // {$
    Backtick,              // `
    DoubleQuote,           // "
    StartHeredoc,          // <<<
    EndHeredoc,            // The closing identifier
    Dollar,                // $ (for variable variables like $$a)
    NsSeparator,           // \

    // Comments
    Comment,
    DocComment,

    // Symbols
    Arrow,         // ->
    NullSafeArrow, // ?->
    DoubleArrow,   // =>
    DoubleColon,   // ::
    Ellipsis,      // ...

    Plus,
    Minus,
    Asterisk,
    Slash,
    Percent,
    Dot,
    Pow, // **
    Inc,
    Dec, // ++, --

    Eq, // =
    PlusEq,
    MinusEq,
    MulEq,
    DivEq,
    ModEq,
    ConcatEq,
    PowEq,
    AndEq,
    OrEq,
    XorEq,
    SlEq,
    SrEq,
    CoalesceEq,

    EqEq,      // ==
    EqEqEq,    // ===
    Bang,      // !
    BangEq,    // !=
    BangEqEq,  // !==
    Lt,        // <
    LtEq,      // <=
    Gt,        // >
    GtEq,      // >=
    Spaceship, // <=>

    Ampersand, // &
    AmpersandFollowedByVarOrVararg,
    AmpersandNotFollowedByVarOrVararg,
    Pipe,   // |
    Caret,  // ^
    BitNot, // ~
    Sl,     // <<
    Sr,     // >>

    AmpersandAmpersand, // &&
    PipePipe,           // ||
    LogicalAnd,         // and
    LogicalOr,          // or
    LogicalXor,         // xor
    Question,           // ?
    Coalesce,           // ??
    At,                 // @

    SemiColon,
    Colon,
    Comma,
    OpenBrace,
    CloseBrace,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,

    OpenTag,     // <?php
    OpenTagEcho, // <?=
    CloseTag,    // ?>

    Eof,

    // Error token for lexing failures
    Error,
    AmpersandFollowedByVar,
    AmpersandNotFollowedByVar,
}

impl TokenKind {
    pub fn is_semi_reserved(self) -> bool {
        matches!(
            self,
            TokenKind::New
                | TokenKind::Static
                | TokenKind::Class
                | TokenKind::Trait
                | TokenKind::Interface
                | TokenKind::Extends
                | TokenKind::Implements
                | TokenKind::Enum
                | TokenKind::Namespace
                | TokenKind::TypeInt
                | TokenKind::TypeFloat
                | TokenKind::TypeBool
                | TokenKind::TypeString
                | TokenKind::TypeVoid
                | TokenKind::TypeNever
                | TokenKind::TypeNull
                | TokenKind::TypeFalse
                | TokenKind::TypeTrue
                | TokenKind::TypeMixed
                | TokenKind::TypeIterable
                | TokenKind::TypeObject
                | TokenKind::TypeCallable
                | TokenKind::LogicalOr
                | TokenKind::Insteadof
                | TokenKind::LogicalAnd
                | TokenKind::LogicalXor
                | TokenKind::As
                | TokenKind::Empty
                | TokenKind::Isset
                | TokenKind::Default
                | TokenKind::Switch
                | TokenKind::Case
                | TokenKind::For
                | TokenKind::Foreach
                | TokenKind::While
                | TokenKind::Do
                | TokenKind::If
                | TokenKind::Else
                | TokenKind::ElseIf
                | TokenKind::EndIf
                | TokenKind::Try
                | TokenKind::Catch
                | TokenKind::Finally
                | TokenKind::Throw
                | TokenKind::Return
                | TokenKind::Break
                | TokenKind::Continue
                | TokenKind::Goto
                | TokenKind::Echo
                | TokenKind::Print
                | TokenKind::List
                | TokenKind::Clone
                | TokenKind::Include
                | TokenKind::IncludeOnce
                | TokenKind::Require
                | TokenKind::RequireOnce
                | TokenKind::Global
                | TokenKind::Unset
                | TokenKind::Exit
                | TokenKind::Die
                | TokenKind::Eval
                | TokenKind::Yield
                | TokenKind::YieldFrom
                | TokenKind::Declare
                | TokenKind::EndDeclare
                | TokenKind::Match
                | TokenKind::Fn
                | TokenKind::Const
                | TokenKind::Use
                | TokenKind::Public
                | TokenKind::Protected
                | TokenKind::Private
                | TokenKind::Abstract
                | TokenKind::Final
                | TokenKind::Readonly
                | TokenKind::Array
                | TokenKind::Function
                | TokenKind::HaltCompiler
                | TokenKind::InstanceOf
                | TokenKind::Line
                | TokenKind::File
                | TokenKind::Dir
                | TokenKind::ClassC
                | TokenKind::PropertyC
                | TokenKind::TraitC
                | TokenKind::MethodC
                | TokenKind::FuncC
                | TokenKind::NsC
        )
    }
}
