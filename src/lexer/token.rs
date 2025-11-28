use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum TokenKind {
    // Keywords
    Function, Class, Interface, Trait, Extends, Implements, Enum,
    If, Else, ElseIf, EndIf, Return, Echo, Print,
    While, Do, For, Foreach, EndWhile, EndFor, EndForeach, As, Switch, EndSwitch, Case, Default, Break, Continue, Goto,
    Try, Catch, Finally, Throw,
    Public, Protected, Private, Static, Abstract, Final, Readonly,
    Namespace, Use, Global,
    New, Clone, InstanceOf,
    Array, Const,
    Include, IncludeOnce, Require, RequireOnce, Eval, Exit, Die,
    Empty, Isset, Unset, List,
    Yield, YieldFrom,
    Declare, EndDeclare, Match, Fn,
    HaltCompiler, // __halt_compiler
    Attribute, // #[

    // Magic Constants
    Line, File, Dir, ClassC, TraitC, MethodC, FuncC, NsC,

    // Types (for type hints)
    TypeBool, TypeInt, TypeFloat, TypeString, TypeObject, TypeVoid, TypeIterable, TypeCallable, TypeMixed, TypeNever, TypeNull, TypeFalse, TypeTrue,

    // Casts
    IntCast, FloatCast, StringCast, ArrayCast, ObjectCast, BoolCast, UnsetCast,

    // Identifiers & Literals
    Identifier, 
    LNumber, 
    DNumber, 
    StringLiteral, 
    NumString, // For array offset in string
    Variable,
    InlineHtml,
    EncapsedAndWhitespace,
    DollarOpenCurlyBraces, // ${
    CurlyOpen, // {$
    Backtick, // `
    DoubleQuote, // "
    StartHeredoc, // <<<
    EndHeredoc, // The closing identifier
    Dollar, // $ (for variable variables like $$a)
    NsSeparator, // \
    
    // Comments
    Comment,
    DocComment,

    // Symbols
    Arrow, // ->
    NullSafeArrow, // ?->
    DoubleArrow, // =>
    DoubleColon, // ::
    Ellipsis, // ...
    
    Plus, Minus, Asterisk, Slash, Percent, Dot,
    Pow, // **
    Inc, Dec, // ++, --
    
    Eq, // =
    PlusEq, MinusEq, MulEq, DivEq, ModEq, ConcatEq, PowEq,
    AndEq, OrEq, XorEq, SlEq, SrEq, CoalesceEq,
    
    EqEq, // ==
    EqEqEq, // ===
    Bang, // !
    BangEq, // !=
    BangEqEq, // !==
    Lt, // <
    LtEq, // <=
    Gt, // >
    GtEq, // >=
    Spaceship, // <=>
    
    Ampersand, // &
    AmpersandFollowedByVarOrVararg,
    AmpersandNotFollowedByVarOrVararg,
    Pipe, // |
    Caret, // ^
    BitNot, // ~
    Sl, // <<
    Sr, // >>
    
    AmpersandAmpersand, // &&
    PipePipe, // ||
    LogicalAnd, // and
    LogicalOr, // or
    LogicalXor, // xor
    Question, // ?
    Coalesce, // ??
    At, // @
    
    SemiColon,
    Colon,
    Comma,
    OpenBrace,
    CloseBrace,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    
    OpenTag, // <?php
    OpenTagEcho, // <?=
    CloseTag, // ?>
    
    Eof,
    
    // Error token for lexing failures
    Error,
    AmpersandFollowedByVar,
    AmpersandNotFollowedByVar,
}

