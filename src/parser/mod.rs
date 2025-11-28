use bumpalo::Bump;
use crate::lexer::{Lexer, LexerMode, token::{Token, TokenKind}};
use crate::ast::{Program, Stmt, StmtId, Expr, ExprId, BinaryOp, UnaryOp, AssignOp, Param, Arg, ArrayItem, ClassMember, Case, Catch, StaticVar, MatchArm, CastKind, ClosureUse, UseKind, UseItem, Name, Attribute, AttributeGroup, Type, ParseError, IncludeKind, TraitMethodRef, PropertyHook, PropertyHookBody, ClassConst};

use crate::span::Span;

#[allow(dead_code)]
enum ModifierContext {
    Method,
    Property,
    Other,
}

    enum ClassMemberCtx {
        Class { is_abstract: bool, is_readonly: bool },
        Interface,
        Trait,
        Enum { backed: bool },
    }

pub trait TokenSource<'src> {
    fn current(&self) -> &Token;
    fn lookahead(&self, n: usize) -> &Token;
    fn bump(&mut self);
    fn set_mode(&mut self, mode: LexerMode);
}

pub struct Parser<'src, 'ast> {
    lexer: Lexer<'src>, // In real impl, this would be wrapped in a TokenSource
    arena: &'ast Bump,
    current_token: Token,
    next_token: Token,
    errors: std::vec::Vec<ParseError>,
}

impl<'src, 'ast> Parser<'src, 'ast> {
    pub fn new(lexer: Lexer<'src>, arena: &'ast Bump) -> Self {
        let mut parser = Self {
            lexer,
            arena,
            current_token: Token { kind: TokenKind::Eof, span: Span::default() },
            next_token: Token { kind: TokenKind::Eof, span: Span::default() },
            errors: std::vec::Vec::new(),
        };
        parser.bump();
        parser.bump();
        parser
    }

    fn bump(&mut self) {
        self.current_token = self.next_token;
        loop {
            let token = self.lexer.next().unwrap_or(Token {
                kind: TokenKind::Eof,
                span: Span::default(),
            });
            if token.kind != TokenKind::Comment && token.kind != TokenKind::DocComment {
                self.next_token = token;
                break;
            }
        }
    }

    fn expect_semicolon(&mut self) {
        if self.current_token.kind == TokenKind::SemiColon {
            self.bump();
        } else if self.current_token.kind == TokenKind::CloseTag {
            // Implicit semicolon at close tag
        } else if self.current_token.kind == TokenKind::Eof {
            // Implicit semicolon at EOF
        } else {
            // Error: Missing semicolon
            self.errors.push(ParseError {
                span: self.current_token.span,
                message: "Missing semicolon",
            });
            // Recovery: Assume it was there and continue.
            // We do NOT bump the current token because it belongs to the next statement.
            self.sync_to_statement_end();
        }
    }

    fn parse_name(&mut self) -> Name<'ast> {
        let start = self.current_token.span.start;
        let mut parts = std::vec::Vec::new();
        
        if self.current_token.kind == TokenKind::NsSeparator || self.current_token.kind == TokenKind::Namespace {
            parts.push(self.current_token);
            self.bump();
        }
        
        loop {
            if self.current_token.kind == TokenKind::Identifier || self.current_token.kind.is_semi_reserved() {
                parts.push(self.current_token);
                self.bump();
            } else {
                break;
            }
            
            if self.current_token.kind == TokenKind::NsSeparator {
                parts.push(self.current_token);
                self.bump();
            } else {
                break;
            }
        }
        
        let end = if parts.is_empty() {
            start
        } else {
            parts.last().unwrap().span.end
        };
        
        Name {
            parts: self.arena.alloc_slice_copy(&parts),
            span: Span::new(start, end),
        }
    }

    pub fn parse_program(&mut self) -> Program<'ast> {
        let mut statements = std::vec::Vec::new(); // Temporary vec, will be moved to arena
        
        while self.current_token.kind != TokenKind::Eof {
            statements.push(self.parse_stmt());
        }

        let span = if let (Some(first), Some(last)) = (statements.first(), statements.last()) {
            Span::new(first.span().start, last.span().end)
        } else {
            Span::default()
        };

        Program {
            statements: self.arena.alloc_slice_copy(&statements),
            errors: self.arena.alloc_slice_copy(&self.errors),
            span,
        }
    }

    fn parse_stmt(&mut self) -> StmtId<'ast> {
        self.lexer.set_mode(LexerMode::Standard);

        if self.current_token.kind == TokenKind::Identifier && self.next_token.kind == TokenKind::Colon {
            let label_token = self.arena.alloc(self.current_token);
            let start = label_token.span.start;
            let colon_span = self.next_token.span;
            self.bump(); // identifier
            self.bump(); // colon
            let span = Span::new(start, colon_span.end);
            return self.arena.alloc(Stmt::Label {
                name: label_token,
                span,
            });
        }
        
        match self.current_token.kind {
            TokenKind::Attribute => {
                let attributes = self.parse_attributes();
                match self.current_token.kind {
                    TokenKind::Function => self.parse_function(attributes),
                    TokenKind::Class => self.parse_class(attributes, &[]),
                    TokenKind::Interface => self.parse_interface(attributes),
                    TokenKind::Trait => self.parse_trait(attributes),
                    TokenKind::Enum => self.parse_enum(attributes),
                    TokenKind::Const => self.parse_const_stmt(attributes),
                    TokenKind::Final | TokenKind::Abstract | TokenKind::Readonly => {
                        let mut modifiers = std::vec::Vec::new();
                        while matches!(self.current_token.kind, TokenKind::Final | TokenKind::Abstract | TokenKind::Readonly) {
                            modifiers.push(self.current_token);
                            self.bump();
                        }
                        
                        if self.current_token.kind == TokenKind::Class {
                            self.parse_class(attributes, self.arena.alloc_slice_copy(&modifiers))
                        } else {
                             self.arena.alloc(Stmt::Error { span: self.current_token.span })
                        }
                    }
                    _ => {
                        self.arena.alloc(Stmt::Error { span: self.current_token.span })
                    }
                }
            }
            TokenKind::Final | TokenKind::Abstract | TokenKind::Readonly => {
                let mut modifiers = std::vec::Vec::new();
                while matches!(self.current_token.kind, TokenKind::Final | TokenKind::Abstract | TokenKind::Readonly) {
                    modifiers.push(self.current_token);
                    self.bump();
                }
                
                if self.current_token.kind == TokenKind::Class {
                    self.parse_class(&[], self.arena.alloc_slice_copy(&modifiers))
                } else {
                     self.arena.alloc(Stmt::Error { span: self.current_token.span })
                }
            }
            TokenKind::HaltCompiler => {
                let start = self.current_token.span.start;
                self.bump();
                if self.current_token.kind == TokenKind::OpenParen {
                    self.bump();
                }
                if self.current_token.kind == TokenKind::CloseParen {
                    self.bump();
                }
                self.expect_semicolon();
                
                let end = self.current_token.span.end;
                self.arena.alloc(Stmt::HaltCompiler { span: Span::new(start, end) })
            }
            TokenKind::Echo | TokenKind::OpenTagEcho => self.parse_echo(),
            TokenKind::Return => self.parse_return(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::Do => self.parse_do_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Foreach => self.parse_foreach(),
            TokenKind::Function => self.parse_function(&[]),
            TokenKind::Class => self.parse_class(&[], &[]),
            TokenKind::Interface => self.parse_interface(&[]),
            TokenKind::Trait => self.parse_trait(&[]),
            TokenKind::Enum => self.parse_enum(&[]),
            TokenKind::Namespace => self.parse_namespace(),
            TokenKind::Use => self.parse_use(),
            TokenKind::Switch => self.parse_switch(),
            TokenKind::Try => self.parse_try(),
            TokenKind::Throw => self.parse_throw(),
            TokenKind::Const => self.parse_const_stmt(&[]),
            TokenKind::Goto => self.parse_goto(),
            TokenKind::Break => self.parse_break(),
            TokenKind::Continue => self.parse_continue(),
            TokenKind::Declare => self.parse_declare(),
            TokenKind::Global => self.parse_global(),
            TokenKind::Static => {
                if matches!(
                    self.next_token.kind,
                    TokenKind::Variable
                        | TokenKind::AmpersandFollowedByVarOrVararg
                        | TokenKind::AmpersandNotFollowedByVarOrVararg
                ) {
                    self.parse_static()
                } else {
                    let start = self.current_token.span.start;
                    let expr = self.parse_expr(0);
                    self.expect_semicolon();
                    let end = self.current_token.span.end;
                    self.arena.alloc(Stmt::Expression {
                        expr,
                        span: Span::new(start, end),
                    })
                }
            }
            TokenKind::Unset => self.parse_unset(),
            TokenKind::OpenBrace => self.parse_block(),
            TokenKind::SemiColon => {
                let span = self.current_token.span;
                self.bump();
                self.arena.alloc(Stmt::Nop { span })
            }
            TokenKind::CloseTag => {
                let span = self.current_token.span;
                self.bump();
                self.arena.alloc(Stmt::Nop { span })
            }
            TokenKind::OpenTag => {
                let span = self.current_token.span;
                self.bump();
                self.arena.alloc(Stmt::Nop { span })
            }
            TokenKind::InlineHtml => {
                let start = self.current_token.span.start;
                let value = self.arena.alloc_slice_copy(self.lexer.slice(self.current_token.span));
                self.bump();
                let end = self.current_token.span.end;
                self.arena.alloc(Stmt::InlineHtml {
                    value,
                    span: Span::new(start, end),
                })
            }
            _ => {
                // Assume expression statement
                let start = self.current_token.span.start;
                let expr = self.parse_expr(0);
                self.expect_semicolon();
                let end = self.current_token.span.end; // Approximate
                
                self.arena.alloc(Stmt::Expression {
                    expr,
                    span: Span::new(start, end),
                })
            }
        }
    }

    fn parse_echo(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump();
        
        let mut exprs = std::vec::Vec::new();
        exprs.push(self.parse_expr(0));
        
        while self.current_token.kind == TokenKind::Comma {
             self.bump();
             exprs.push(self.parse_expr(0));
        }

        self.expect_semicolon();

        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::Echo {
            exprs: self.arena.alloc_slice_copy(&exprs),
            span: Span::new(start, end),
        })
    }

    fn parse_return(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump();
        
        let expr = if matches!(self.current_token.kind, TokenKind::SemiColon | TokenKind::CloseTag | TokenKind::Eof | TokenKind::CloseBrace) {
            None
        } else {
            Some(self.parse_expr(0))
        };

        self.expect_semicolon();
        
        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::Return {
            expr,
            span: Span::new(start, end),
        })
    }

    fn parse_block(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        
        if self.current_token.kind == TokenKind::OpenBrace {
            self.bump(); // Eat {
        } else {
            self.errors.push(ParseError {
                span: self.current_token.span,
                message: "Expected '{'",
            });
            return self.arena.alloc(Stmt::Error { span: self.current_token.span });
        }

        let mut statements = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
            statements.push(self.parse_stmt());
        }

        if self.current_token.kind == TokenKind::CloseBrace {
            self.bump();
        } else {
            self.errors.push(ParseError {
                span: self.current_token.span,
                message: "Missing '}'",
            });
        }

        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::Block {
            statements: self.arena.alloc_slice_copy(&statements),
            span: Span::new(start, end),
        })
    }

    fn parse_if(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat if

        self.parse_if_common(start)
    }

    fn parse_if_common(&mut self, start: usize) -> StmtId<'ast> {
        if self.current_token.kind == TokenKind::OpenParen {
            self.bump();
        }
        let condition = self.parse_expr(0);
        if self.current_token.kind == TokenKind::CloseParen {
            self.bump();
        }

        let is_alt = self.current_token.kind == TokenKind::Colon;
        
        let then_block = if is_alt {
            self.bump();
            let mut stmts = std::vec::Vec::new();
            while self.current_token.kind != TokenKind::EndIf && self.current_token.kind != TokenKind::Else && self.current_token.kind != TokenKind::ElseIf && self.current_token.kind != TokenKind::Eof {
                stmts.push(self.parse_stmt());
            }
            self.arena.alloc_slice_copy(&stmts) as &'ast [StmtId<'ast>]
        } else {
            let stmt = self.parse_stmt();
            match stmt {
                Stmt::Block { statements, .. } => *statements,
                _ => self.arena.alloc_slice_copy(&[stmt]) as &'ast [StmtId<'ast>],
            }
        };

        let mut consumed_endif = false;
        let else_block = if self.current_token.kind == TokenKind::ElseIf {
            let start_elseif = self.current_token.span.start;
            self.bump();
            let elseif_stmt = self.parse_if_common(start_elseif);
            consumed_endif = true;
            Some(self.arena.alloc_slice_copy(&[elseif_stmt]) as &'ast [StmtId<'ast>])
        } else if self.current_token.kind == TokenKind::Else {
            self.bump();
            if is_alt {
                if self.current_token.kind == TokenKind::Colon {
                    self.bump();
                }
                let mut stmts = std::vec::Vec::new();
                while self.current_token.kind != TokenKind::EndIf && self.current_token.kind != TokenKind::Eof {
                    stmts.push(self.parse_stmt());
                }
                Some(self.arena.alloc_slice_copy(&stmts) as &'ast [StmtId<'ast>])
            } else {
                let stmt = self.parse_stmt();
                match stmt {
                    Stmt::Block { statements, .. } => Some(*statements),
                    _ => Some(self.arena.alloc_slice_copy(&[stmt]) as &'ast [StmtId<'ast>]),
                }
            }
        } else {
            None
        };

        if is_alt && !consumed_endif && self.current_token.kind == TokenKind::EndIf {
            self.bump();
            self.expect_semicolon();
        }

        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::If {
            condition,
            then_block,
            else_block,
            span: Span::new(start, end),
        })
    }

    fn parse_while(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat while

        if self.current_token.kind == TokenKind::OpenParen {
            self.bump();
        }
        let condition = self.parse_expr(0);
        if self.current_token.kind == TokenKind::CloseParen {
            self.bump();
        }

        let body = if self.current_token.kind == TokenKind::Colon {
            self.bump();
            let mut stmts = std::vec::Vec::new();
            while self.current_token.kind != TokenKind::EndWhile && self.current_token.kind != TokenKind::Eof {
                stmts.push(self.parse_stmt());
            }
            if self.current_token.kind == TokenKind::EndWhile {
                self.bump();
            }
            self.expect_semicolon();
            self.arena.alloc_slice_copy(&stmts) as &'ast [StmtId<'ast>]
        } else {
            let body_stmt = self.parse_stmt();
            match body_stmt {
                Stmt::Block { statements, .. } => *statements,
                _ => self.arena.alloc_slice_copy(&[body_stmt]) as &'ast [StmtId<'ast>],
            }
        };

        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::While {
            condition,
            body,
            span: Span::new(start, end),
        })
    }

    fn parse_do_while(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat do

        let body_stmt = self.parse_stmt();
        let body: &'ast [StmtId<'ast>] = match body_stmt {
            Stmt::Block { statements, .. } => *statements,
            _ => self.arena.alloc_slice_copy(&[body_stmt]) as &'ast [StmtId<'ast>],
        };

        if self.current_token.kind == TokenKind::While {
            self.bump();
        }

        if self.current_token.kind == TokenKind::OpenParen {
            self.bump();
        }
        let condition = self.parse_expr(0);
        if self.current_token.kind == TokenKind::CloseParen {
            self.bump();
        }

        self.expect_semicolon();

        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::DoWhile {
            condition,
            body,
            span: Span::new(start, end),
        })
    }

    fn parse_for(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat for

        if self.current_token.kind == TokenKind::OpenParen {
            self.bump();
        }

        // Init expressions
        let mut init = std::vec::Vec::new();
        if self.current_token.kind != TokenKind::SemiColon {
            init.push(self.parse_expr(0));
            while self.current_token.kind == TokenKind::Comma {
                self.bump();
                init.push(self.parse_expr(0));
            }
        }
        if self.current_token.kind == TokenKind::SemiColon {
            self.bump();
        }

        // Condition expressions
        let mut condition = std::vec::Vec::new();
        if self.current_token.kind != TokenKind::SemiColon {
            condition.push(self.parse_expr(0));
            while self.current_token.kind == TokenKind::Comma {
                self.bump();
                condition.push(self.parse_expr(0));
            }
        }
        if self.current_token.kind == TokenKind::SemiColon {
            self.bump();
        }

        // Loop expressions
        let mut loop_expr = std::vec::Vec::new();
        if self.current_token.kind != TokenKind::CloseParen {
            loop_expr.push(self.parse_expr(0));
            while self.current_token.kind == TokenKind::Comma {
                self.bump();
                loop_expr.push(self.parse_expr(0));
            }
        }
        if self.current_token.kind == TokenKind::CloseParen {
            self.bump();
        }

        let body = if self.current_token.kind == TokenKind::Colon {
            self.bump();
            let mut stmts = std::vec::Vec::new();
            while self.current_token.kind != TokenKind::EndFor && self.current_token.kind != TokenKind::Eof {
                stmts.push(self.parse_stmt());
            }
            if self.current_token.kind == TokenKind::EndFor {
                self.bump();
            }
            self.expect_semicolon();
            self.arena.alloc_slice_copy(&stmts) as &'ast [StmtId<'ast>]
        } else {
            let body_stmt = self.parse_stmt();
            match body_stmt {
                Stmt::Block { statements, .. } => *statements,
                _ => self.arena.alloc_slice_copy(&[body_stmt]) as &'ast [StmtId<'ast>],
            }
        };

        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::For {
            init: self.arena.alloc_slice_copy(&init),
            condition: self.arena.alloc_slice_copy(&condition),
            loop_expr: self.arena.alloc_slice_copy(&loop_expr),
            body,
            span: Span::new(start, end),
        })
    }

    fn parse_foreach(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat foreach

        if self.current_token.kind == TokenKind::OpenParen {
            self.bump();
        }
        
        let expr = self.parse_expr(0);
        
        if self.current_token.kind == TokenKind::As {
            self.bump();
        }
        
        let mut key_var = None;
        let mut value_var = self.parse_expr(0); // This might be key if => follows
        
        if self.current_token.kind == TokenKind::DoubleArrow {
            self.bump();
            key_var = Some(value_var);
            value_var = self.parse_expr(0);
        }
        
        if self.current_token.kind == TokenKind::CloseParen {
            self.bump();
        }

        let body = if self.current_token.kind == TokenKind::Colon {
            self.bump();
            let mut stmts = std::vec::Vec::new();
            while self.current_token.kind != TokenKind::EndForeach && self.current_token.kind != TokenKind::Eof {
                stmts.push(self.parse_stmt());
            }
            if self.current_token.kind == TokenKind::EndForeach {
                self.bump();
            }
            self.expect_semicolon();
            self.arena.alloc_slice_copy(&stmts) as &'ast [StmtId<'ast>]
        } else {
            let body_stmt = self.parse_stmt();
            match body_stmt {
                Stmt::Block { statements, .. } => *statements,
                _ => self.arena.alloc_slice_copy(&[body_stmt]) as &'ast [StmtId<'ast>],
            }
        };

        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::Foreach {
            expr,
            key_var,
            value_var,
            body,
            span: Span::new(start, end),
        })
    }

    fn parse_class(&mut self, attributes: &'ast [AttributeGroup<'ast>], modifiers: &'ast [Token]) -> StmtId<'ast> {
        let start = if let Some(first) = attributes.first() {
            first.span.start
        } else if let Some(first) = modifiers.first() {
            first.span.start
        } else {
            self.current_token.span.start
        };
        self.bump(); // Eat class
        
        let name = if matches!(self.current_token.kind, TokenKind::Identifier | TokenKind::Enum) {
            let token = self.arena.alloc(self.current_token);
            self.bump();
            token
        } else {
            // Error recovery
            self.arena.alloc(Token { kind: TokenKind::Error, span: Span::default() })
        };

        let mut extends = None;
        if self.current_token.kind == TokenKind::Extends {
            self.bump();
            let parent = self.parse_name();
            if self.name_eq_token(&parent, name) {
                self.errors.push(ParseError { span: parent.span, message: "class cannot extend itself" });
            }
            extends = Some(parent);
        }

        let mut implements = std::vec::Vec::new();
        if self.current_token.kind == TokenKind::Implements {
            self.bump();
            loop {
                implements.push(self.parse_name());
                if self.current_token.kind == TokenKind::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
            for (i, n) in implements.iter().enumerate() {
                if self.name_eq_token(n, name) {
                    self.errors.push(ParseError { span: n.span, message: "class cannot implement itself" });
                }
                for prev in implements.iter().take(i) {
                    if self.name_eq(prev, n) {
                        self.errors.push(ParseError { span: n.span, message: "duplicate interface in implements list" });
                        break;
                    }
                }
            }
        }
        
        if self.current_token.kind == TokenKind::OpenBrace {
            self.bump();
        } else {
            self.errors.push(ParseError { span: self.current_token.span, message: "Expected '{'" });
            return self.arena.alloc(Stmt::Class {
                attributes,
                modifiers,
                name,
                extends,
                implements: self.arena.alloc_slice_copy(&implements),
                members: &[],
                span: Span::new(start, self.current_token.span.end),
            });
        }
        
        let class_is_abstract = modifiers.iter().any(|m| m.kind == TokenKind::Abstract);
        let class_is_readonly = modifiers.iter().any(|m| m.kind == TokenKind::Readonly);
        self.validate_class_modifiers(modifiers);

        let mut members = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
            members.push(self.parse_class_member(ClassMemberCtx::Class { is_abstract: class_is_abstract, is_readonly: class_is_readonly }));
        }
        
        if self.current_token.kind == TokenKind::CloseBrace {
            self.bump();
        } else {
            self.errors.push(ParseError { span: self.current_token.span, message: "Missing '}'" });
        }
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Class {
            attributes,
            modifiers,
            name,
            extends,
            implements: self.arena.alloc_slice_copy(&implements),
            members: self.arena.alloc_slice_copy(&members),
            span: Span::new(start, end),
        })
    }

    fn parse_anonymous_class(&mut self) -> (ExprId<'ast>, &'ast [Arg<'ast>]) {
        let start = self.current_token.span.start; // points at 'class'
        self.bump(); // eat class

        let (ctor_args, ctor_end) = if self.current_token.kind == TokenKind::OpenParen {
            let (args, span) = self.parse_call_arguments();
            (args, span.end)
        } else {
            (&[] as &[Arg], self.current_token.span.start)
        };

        let mut extends = None;
        if self.current_token.kind == TokenKind::Extends {
            self.bump();
            extends = Some(self.parse_name());
        }

        let mut implements = std::vec::Vec::new();
        if self.current_token.kind == TokenKind::Implements {
            self.bump();
            loop {
                implements.push(self.parse_name());
                if self.current_token.kind == TokenKind::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
            for i in 0..implements.len() {
                for prev in implements.iter().take(i) {
                    if self.name_eq(prev, &implements[i]) {
                        self.errors.push(ParseError { span: implements[i].span, message: "duplicate interface in implements list" });
                        break;
                    }
                }
            }
        }

        if self.current_token.kind == TokenKind::OpenBrace {
            self.bump();
        } else {
            self.errors.push(ParseError { span: self.current_token.span, message: "Expected '{'" });
            let span = Span::new(start, self.current_token.span.end);
            return (self.arena.alloc(Expr::AnonymousClass {
                args: ctor_args,
                extends,
                implements: self.arena.alloc_slice_copy(&implements),
                members: &[],
                span,
            }), ctor_args);
        }

        let mut members = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
            members.push(self.parse_class_member(ClassMemberCtx::Class { is_abstract: false, is_readonly: false }));
        }

        if self.current_token.kind == TokenKind::CloseBrace {
            self.bump();
        } else {
            self.errors.push(ParseError { span: self.current_token.span, message: "Missing '}'" });
        }

        let end = self.current_token.span.end.max(ctor_end);

        (
            self.arena.alloc(Expr::AnonymousClass {
                args: ctor_args,
                extends,
                implements: self.arena.alloc_slice_copy(&implements),
                members: self.arena.alloc_slice_copy(&members),
                span: Span::new(start, end),
            }),
            ctor_args,
        )
    }

    fn parse_interface(&mut self, attributes: &'ast [AttributeGroup<'ast>]) -> StmtId<'ast> {
        let start = if let Some(first) = attributes.first() {
            first.span.start
        } else {
            self.current_token.span.start
        };
        self.bump(); // Eat interface
        
        let name = if self.current_token.kind == TokenKind::Identifier {
            let token = self.arena.alloc(self.current_token);
            self.bump();
            token
        } else {
            self.arena.alloc(Token { kind: TokenKind::Error, span: Span::default() })
        };

        let mut extends = std::vec::Vec::new();
        if self.current_token.kind == TokenKind::Extends {
            self.bump();
            loop {
                extends.push(self.parse_name());
                if self.current_token.kind == TokenKind::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
            for (i, n) in extends.iter().enumerate() {
                if self.name_eq_token(n, name) {
                    self.errors.push(ParseError { span: n.span, message: "interface cannot extend itself" });
                }
                for prev in extends.iter().take(i) {
                    if self.name_eq(prev, n) {
                        self.errors.push(ParseError { span: n.span, message: "duplicate interface in extends list" });
                        break;
                    }
                }
            }
        }
        
        if self.current_token.kind == TokenKind::OpenBrace {
            self.bump();
        } else {
            self.errors.push(ParseError { span: self.current_token.span, message: "Expected '{'" });
            return self.arena.alloc(Stmt::Interface {
                attributes,
                name,
                extends: self.arena.alloc_slice_copy(&extends),
                members: &[],
                span: Span::new(start, self.current_token.span.end),
            });
        }
        
        let mut members = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
            members.push(self.parse_class_member(ClassMemberCtx::Interface));
        }
        
        if self.current_token.kind == TokenKind::CloseBrace {
            self.bump();
        } else {
            self.errors.push(ParseError { span: self.current_token.span, message: "Missing '}'" });
        }
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Interface {
            attributes,
            name,
            extends: self.arena.alloc_slice_copy(&extends),
            members: self.arena.alloc_slice_copy(&members),
            span: Span::new(start, end),
        })
    }

    fn parse_trait(&mut self, attributes: &'ast [AttributeGroup<'ast>]) -> StmtId<'ast> {
        let start = if let Some(first) = attributes.first() {
            first.span.start
        } else {
            self.current_token.span.start
        };
        self.bump(); // Eat trait
        
        let name = if self.current_token.kind == TokenKind::Identifier {
            let token = self.arena.alloc(self.current_token);
            self.bump();
            token
        } else {
            self.arena.alloc(Token { kind: TokenKind::Error, span: Span::default() })
        };
        
        if self.current_token.kind == TokenKind::OpenBrace {
            self.bump();
        } else {
            self.errors.push(ParseError { span: self.current_token.span, message: "Expected '{'" });
            return self.arena.alloc(Stmt::Trait {
                attributes,
                name,
                members: &[],
                span: Span::new(start, self.current_token.span.end),
            });
        }
        
        let mut members = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
            members.push(self.parse_class_member(ClassMemberCtx::Trait));
        }
        
        if self.current_token.kind == TokenKind::CloseBrace {
            self.bump();
        } else {
            self.errors.push(ParseError { span: self.current_token.span, message: "Missing '}'" });
        }
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Trait {
            attributes,
            name,
            members: self.arena.alloc_slice_copy(&members),
            span: Span::new(start, end),
        })
    }

    fn parse_namespace(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat namespace
        
        let name = if self.current_token.kind == TokenKind::Identifier || self.current_token.kind == TokenKind::NsSeparator || self.current_token.kind == TokenKind::Namespace {
            Some(self.parse_name())
        } else {
            None
        };
        
        let body = if self.current_token.kind == TokenKind::OpenBrace {
            self.bump();
            let mut statements = std::vec::Vec::new();
            while self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
                statements.push(self.parse_stmt());
            }
            if self.current_token.kind == TokenKind::CloseBrace {
                self.bump();
            } else {
                self.errors.push(ParseError { span: self.current_token.span, message: "Missing '}'" });
            }
            Some(self.arena.alloc_slice_copy(&statements) as &'ast [StmtId<'ast>])
        } else if self.current_token.kind == TokenKind::Colon {
            self.bump();
            let mut statements = std::vec::Vec::new();
            while !matches!(self.current_token.kind, TokenKind::EndDeclare | TokenKind::Eof) {
                statements.push(self.parse_stmt());
            }
            if self.current_token.kind == TokenKind::EndDeclare {
                self.bump();
                self.expect_semicolon();
            } else {
                self.errors.push(ParseError { span: self.current_token.span, message: "Missing enddeclare" });
            }
            Some(self.arena.alloc_slice_copy(&statements) as &'ast [StmtId<'ast>])
        } else {
            self.expect_semicolon();
            None
        };
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Namespace {
            name,
            body,
            span: Span::new(start, end),
        })
    }

    fn parse_use(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat use
        
        let kind = if self.current_token.kind == TokenKind::Function {
            self.bump();
            UseKind::Function
        } else if self.current_token.kind == TokenKind::Const {
            self.bump();
            UseKind::Const
        } else {
            UseKind::Normal
        };
        
        let mut uses = std::vec::Vec::new();
        loop {
            let mut item_kind = kind;
            if matches!(self.current_token.kind, TokenKind::Function | TokenKind::Const) {
                item_kind = if self.current_token.kind == TokenKind::Function {
                    self.bump();
                    UseKind::Function
                } else {
                    self.bump();
                    UseKind::Const
                };
            }

            let prefix = self.parse_name();
            
            if self.current_token.kind == TokenKind::OpenBrace {
                self.bump(); // Eat {
                while self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
                    let mut element_kind = item_kind;
                    if matches!(self.current_token.kind, TokenKind::Function | TokenKind::Const) {
                        element_kind = if self.current_token.kind == TokenKind::Function {
                            self.bump();
                            UseKind::Function
                        } else {
                            self.bump();
                            UseKind::Const
                        };
                    }
                    let suffix = self.parse_name();
                    
                    let alias = if self.current_token.kind == TokenKind::As {
                        self.bump();
                        if self.current_token.kind == TokenKind::Identifier {
                            let token = self.arena.alloc(self.current_token);
                            self.bump();
                            Some(token as &Token)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    
                    let mut full_parts = std::vec::Vec::new();
                    full_parts.extend_from_slice(prefix.parts);
                    full_parts.extend_from_slice(suffix.parts);
                    
                    let full_name = Name {
                        parts: self.arena.alloc_slice_copy(&full_parts),
                        span: Span::new(prefix.span.start, suffix.span.end),
                    };
                    
                    uses.push(UseItem {
                        name: full_name,
                        alias,
                        kind: element_kind,
                        span: Span::new(prefix.span.start, alias.map(|a| a.span.end).unwrap_or(suffix.span.end)),
                    });
                    
                    if self.current_token.kind == TokenKind::Comma {
                        self.bump();
                    } else {
                        break;
                    }
                }
                if self.current_token.kind == TokenKind::CloseBrace {
                    self.bump();
                } else {
                    self.errors.push(ParseError { span: self.current_token.span, message: "Missing '}'" });
                }
            } else {
                let alias = if self.current_token.kind == TokenKind::As {
                    self.bump();
                    if self.current_token.kind == TokenKind::Identifier {
                        let token = self.arena.alloc(self.current_token);
                        self.bump();
                        Some(token as &Token)
                    } else {
                        None
                    }
                } else {
                    None
                };
                
                uses.push(UseItem {
                    name: prefix,
                    alias,
                    kind: item_kind,
                    span: Span::new(prefix.span.start, alias.map(|a| a.span.end).unwrap_or(prefix.span.end)),
                });
            }
            
            if self.current_token.kind == TokenKind::Comma {
                self.bump();
            } else {
                break;
            }
        }
        
        self.expect_semicolon();
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Use {
            uses: self.arena.alloc_slice_copy(&uses),
            kind,
            span: Span::new(start, end),
        })
    }

    fn parse_enum(&mut self, attributes: &'ast [AttributeGroup<'ast>]) -> StmtId<'ast> {
        let start = if let Some(first) = attributes.first() {
            first.span.start
        } else {
            self.current_token.span.start
        };
        self.bump(); // Eat enum
        
        let name = if self.current_token.kind == TokenKind::Identifier {
            let token = self.arena.alloc(self.current_token);
            self.bump();
            token
        } else {
            self.arena.alloc(Token { kind: TokenKind::Error, span: Span::default() })
        };
        
        let backed_type = if self.current_token.kind == TokenKind::Colon {
            self.bump();
            self.parse_type().map(|t| self.arena.alloc(t) as &'ast Type<'ast>)
        } else {
            None
        };

        let mut implements = std::vec::Vec::new();
        if self.current_token.kind == TokenKind::Implements {
            self.bump();
            loop {
                implements.push(self.parse_name());
                if self.current_token.kind == TokenKind::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
            for (i, n) in implements.iter().enumerate() {
                if self.name_eq_token(n, name) {
                    self.errors.push(ParseError { span: n.span, message: "enum cannot implement itself" });
                }
                for prev in implements.iter().take(i) {
                    if self.name_eq(prev, n) {
                        self.errors.push(ParseError { span: n.span, message: "duplicate interface in implements list" });
                        break;
                    }
                }
            }
        }
        
        if self.current_token.kind == TokenKind::OpenBrace {
            self.bump();
        } else {
            self.errors.push(ParseError { span: self.current_token.span, message: "Expected '{'" });
            return self.arena.alloc(Stmt::Enum {
                attributes,
                name,
                backed_type,
                implements: self.arena.alloc_slice_copy(&implements),
                members: &[],
                span: Span::new(start, self.current_token.span.end),
            });
        }
        
        let mut members = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
            members.push(self.parse_class_member(ClassMemberCtx::Enum { backed: backed_type.is_some() }));
        }
        
        if self.current_token.kind == TokenKind::CloseBrace {
            self.bump();
        } else {
            self.errors.push(ParseError { span: self.current_token.span, message: "Missing '}'" });
        }
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Enum {
            attributes,
            name,
            backed_type,
            implements: self.arena.alloc_slice_copy(&implements),
            members: self.arena.alloc_slice_copy(&members),
            span: Span::new(start, end),
        })
    }

    fn parse_class_member(&mut self, ctx: ClassMemberCtx) -> ClassMember<'ast> {
        let mut attributes = &[] as &'ast [AttributeGroup<'ast>];
        if self.current_token.kind == TokenKind::Attribute {
            attributes = self.parse_attributes();
        }
        
        let start = if let Some(first) = attributes.first() {
            first.span.start
        } else {
            self.current_token.span.start
        };
        
        let mut modifiers = std::vec::Vec::new();
        
        while matches!(self.current_token.kind, 
            TokenKind::Public | TokenKind::Protected | TokenKind::Private | TokenKind::Static | TokenKind::Abstract | TokenKind::Final | TokenKind::Readonly) {
            modifiers.push(self.current_token);
            self.bump();
        }
        self.validate_modifiers(&modifiers, ModifierContext::Other);
        if self.current_token.kind == TokenKind::Case {
             self.bump();
             let name = if self.current_token.kind == TokenKind::Identifier || self.current_token.kind.is_semi_reserved() {
                let token = self.arena.alloc(self.current_token);
                self.bump();
                token
            } else {
                self.arena.alloc(Token { kind: TokenKind::Error, span: Span::default() })
            };
            
            let value = if self.current_token.kind == TokenKind::Eq {
                self.bump();
                Some(self.parse_expr(0))
            } else {
                None
            };

            if !matches!(ctx, ClassMemberCtx::Enum { .. }) {
                self.errors.push(ParseError { span: name.span, message: "case not allowed here" });
            } else if matches!(ctx, ClassMemberCtx::Enum { backed: true }) && value.is_none() {
                self.errors.push(ParseError { span: name.span, message: "backed enum cases require a value" });
            } else if matches!(ctx, ClassMemberCtx::Enum { backed: false }) && value.is_some() {
                self.errors.push(ParseError { span: name.span, message: "pure enum cases cannot have values" });
            }
            
            self.expect_semicolon();
            
            let end = self.current_token.span.end;
            return ClassMember::Case {
                attributes,
                name,
                value,
                span: Span::new(start, end),
            };
        }

        if self.current_token.kind == TokenKind::Use {
            self.bump();
            let mut traits = std::vec::Vec::new();
            loop {
                traits.push(self.parse_name());
                if self.current_token.kind == TokenKind::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
            let mut adaptations = std::vec::Vec::new();

            if self.current_token.kind == TokenKind::OpenBrace {
                self.bump();
                while self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
                    let method = self.parse_trait_method_ref();
                    let adapt_span_start = method.span.start;

                    if self.current_token.kind == TokenKind::Insteadof {
                        self.bump();
                        let mut insteads = std::vec::Vec::new();
                        loop {
                            insteads.push(self.parse_name());
                            if self.current_token.kind == TokenKind::Comma {
                                self.bump();
                                continue;
                            }
                            break;
                        }
                        adaptations.push(crate::ast::TraitAdaptation::Precedence {
                            method,
                            insteadof: self.arena.alloc_slice_copy(&insteads),
                            span: Span::new(adapt_span_start, self.current_token.span.end),
                        });
                    } else if self.current_token.kind == TokenKind::As {
                        self.bump();
                        let visibility = if matches!(self.current_token.kind, TokenKind::Public | TokenKind::Protected | TokenKind::Private) {
                            let v = self.arena.alloc(self.current_token);
                            self.bump();
                            Some(v)
                        } else {
                            None
                        };

                        let alias = if self.current_token.kind == TokenKind::Identifier {
                            let a = self.arena.alloc(self.current_token);
                            self.bump();
                            Some(a)
                        } else {
                            None
                        };

                        adaptations.push(crate::ast::TraitAdaptation::Alias {
                            method,
                            alias: alias.map(|t| &*t),
                            visibility: visibility.map(|t| &*t),
                            span: Span::new(adapt_span_start, self.current_token.span.end),
                        });
                    } else {
                        self.errors.push(ParseError { span: self.current_token.span, message: "Expected insteadof or as in trait adaptation" });
                        // try to recover to next semicolon
                    }

                    if self.current_token.kind == TokenKind::SemiColon {
                        self.bump();
                    } else {
                        self.expect_semicolon();
                    }
                }
                if self.current_token.kind == TokenKind::CloseBrace {
                    self.bump();
                }
            } else {
                self.expect_semicolon();
            }
            
            let end = self.current_token.span.end;
            return ClassMember::TraitUse {
                attributes,
                traits: self.arena.alloc_slice_copy(&traits),
                adaptations: self.arena.alloc_slice_copy(&adaptations),
                span: Span::new(start, end),
            };
        }
        
        if self.current_token.kind == TokenKind::Function {
            self.bump();
            let name = if self.current_token.kind == TokenKind::Identifier || self.current_token.kind.is_semi_reserved() {
                let token = self.arena.alloc(self.current_token);
                self.bump();
                token
            } else {
                self.arena.alloc(Token { kind: TokenKind::Error, span: Span::default() })
            };
            
            if self.current_token.kind == TokenKind::OpenParen {
                self.bump();
            }
            
            let mut params = std::vec::Vec::new();
            while self.current_token.kind != TokenKind::CloseParen && self.current_token.kind != TokenKind::Eof {
                params.push(self.parse_param());
                if self.current_token.kind == TokenKind::Comma {
                    self.bump();
                }
            }
            // Promotion modifier validation for interface/trait already handled at method level
            
            if self.current_token.kind == TokenKind::CloseParen {
                self.bump();
            }

            let return_type = if self.current_token.kind == TokenKind::Colon {
                self.bump();
                if let Some(t) = self.parse_type() {
                    Some(self.arena.alloc(t) as &'ast Type<'ast>)
                } else {
                    None
                }
            } else {
                None
            };
            
            let mut has_body_flag = false;
            let body = if self.current_token.kind == TokenKind::OpenBrace {
                has_body_flag = true;
                let body_stmt = self.parse_block();
                match body_stmt {
                    Stmt::Block { statements, .. } => *statements,
                    _ => self.arena.alloc_slice_copy(&[body_stmt]) as &'ast [StmtId<'ast>],
                }
            } else {
                self.expect_semicolon();
                &[] as &'ast [StmtId<'ast>]
            };
            
            let end = if body.is_empty() {
                self.current_token.span.end
            } else {
                body.last().unwrap().span().end
            };

            self.validate_modifiers(&modifiers, ModifierContext::Method);

            let mut method_is_abstract = modifiers.iter().any(|m| m.kind == TokenKind::Abstract);
            if matches!(ctx, ClassMemberCtx::Interface) {
                method_is_abstract = true; // interfaces imply abstract
            }
            let has_body = has_body_flag || !body.is_empty();
            if method_is_abstract && has_body {
                self.errors.push(ParseError { span: Span::new(start, start), message: "abstract method cannot have a body" });
            }
            if matches!(ctx, ClassMemberCtx::Interface) {
                if has_body {
                    self.errors.push(ParseError { span: Span::new(start, start), message: "interface methods cannot have a body" });
                }
                if modifiers.iter().any(|m| matches!(m.kind, TokenKind::Protected | TokenKind::Private | TokenKind::Final)) {
                    self.errors.push(ParseError { span: Span::new(start, start), message: "invalid modifier in interface method" });
                }
            }
            if let ClassMemberCtx::Class { is_abstract, .. } = ctx {
                if method_is_abstract && !is_abstract {
                    self.errors.push(ParseError { span: Span::new(start, start), message: "abstract method in non-abstract class" });
                }
                if !method_is_abstract && !has_body {
                    self.errors.push(ParseError { span: Span::new(start, start), message: "non-abstract method must have a body" });
                }
            }
            if matches!(ctx, ClassMemberCtx::Enum { .. }) && method_is_abstract {
                self.errors.push(ParseError { span: Span::new(start, start), message: "abstract methods not allowed in enums" });
            }

            if self.token_eq_ident(name, b"__construct") {
                for param in params.iter() {
                    if param.modifiers.is_empty() {
                        continue;
                    }
                    let has_visibility = param.modifiers.iter().any(|m| matches!(m.kind, TokenKind::Public | TokenKind::Protected | TokenKind::Private));
                    let vis_count = param.modifiers.iter().filter(|m| matches!(m.kind, TokenKind::Public | TokenKind::Protected | TokenKind::Private)).count();
                    let has_readonly = param.modifiers.iter().any(|m| m.kind == TokenKind::Readonly);
                    let readonly_count = param.modifiers.iter().filter(|m| m.kind == TokenKind::Readonly).count();
                    let _by_ref = param.by_ref;

                    if matches!(ctx, ClassMemberCtx::Interface) {
                        self.errors.push(ParseError { span: param.span, message: "property promotion not allowed in interfaces/traits" });
                        continue;
                    }

                    if vis_count > 1 {
                        self.errors.push(ParseError { span: param.span, message: "multiple visibilities in promoted parameter" });
                    }
                    if !has_visibility {
                        self.errors.push(ParseError { span: param.span, message: "promoted parameter requires visibility" });
                    }
                    if has_readonly && !has_visibility {
                        self.errors.push(ParseError { span: param.span, message: "readonly promotion requires visibility" });
                    }
                    if has_readonly && param.ty.is_none() {
                        self.errors.push(ParseError { span: param.span, message: "readonly promoted property requires a type" });
                    }
                    if param.ty.is_none() && matches!(ctx, ClassMemberCtx::Class { is_readonly: true, .. }) {
                        self.errors.push(ParseError { span: param.span, message: "readonly property requires a type" });
                    }
                    if readonly_count > 1 {
                        self.errors.push(ParseError { span: param.span, message: "Duplicate readonly modifier" });
                    }
                    // if by_ref {
                    //     self.errors.push(ParseError { span: param.span, message: "promoted parameter cannot be by-reference" });
                    // }
                }
            }
            
            ClassMember::Method {
                attributes,
                modifiers: self.arena.alloc_slice_copy(&modifiers),
                name,
                params: self.arena.alloc_slice_copy(&params),
                return_type,
                body,
                span: Span::new(start, end),
            }
        } else if self.current_token.kind == TokenKind::Const {
            self.bump();
            
            let ty = self.parse_type();
            let mut const_type = None;
            let mut first_name = None;

            if let Some(t) = ty {
                if self.current_token.kind == TokenKind::Identifier || self.current_token.kind.is_semi_reserved() {
                    const_type = Some(self.arena.alloc(t) as &'ast Type<'ast>);
                } else {
                    match t {
                        Type::Simple(token) => {
                            first_name = Some(token);
                        },
                        Type::Name(name) => {
                             if name.parts.len() == 1 {
                                 first_name = Some(&name.parts[0]);
                             } else {
                                 self.errors.push(ParseError { span: name.span, message: "Class constant must be an identifier" });
                                 first_name = Some(&name.parts[0]);
                             }
                        },
                        _ => {
                             self.errors.push(ParseError { span: self.current_token.span, message: "Expected identifier" });
                             first_name = Some(self.arena.alloc(Token { kind: TokenKind::Error, span: Span::default() }));
                        }
                    }
                }
            }
            
            let mut consts = std::vec::Vec::new();
            let mut first = true;
            
            loop {
                let name = if first && first_name.is_some() {
                    first_name.unwrap()
                } else {
                    if self.current_token.kind == TokenKind::Identifier {
                        let token = self.arena.alloc(self.current_token);
                        self.bump();
                        token
                    } else {
                        self.arena.alloc(Token { kind: TokenKind::Error, span: Span::default() })
                    }
                };
                first = false;
                
                if self.current_token.kind == TokenKind::Eq {
                    self.bump();
                }
                
                let value = self.parse_expr(0);
                consts.push(crate::ast::ClassConst {
                    name,
                    value,
                    span: Span::new(name.span.start, value.span().end),
                });

                if self.current_token.kind == TokenKind::Comma {
                    self.bump();
                    continue;
                } else {
                    break;
                }
            }
            
            self.expect_semicolon();
            
            self.validate_const_modifiers(&modifiers, ctx);
            let end = self.current_token.span.end;
            
            ClassMember::Const {
                attributes,
                modifiers: self.arena.alloc_slice_copy(&modifiers),
                ty: const_type,
                consts: self.arena.alloc_slice_copy(&consts),
                span: Span::new(start, end),
            }
        } else {
            // Property
            self.validate_modifiers(&modifiers, ModifierContext::Property);
            if matches!(ctx, ClassMemberCtx::Interface) {
                self.errors.push(ParseError { span: Span::new(start, start), message: "interfaces cannot declare properties" });
            }
            if matches!(ctx, ClassMemberCtx::Enum { .. }) {
                self.errors.push(ParseError { span: Span::new(start, start), message: "enums cannot declare properties" });
            }
            let class_is_readonly = matches!(ctx, ClassMemberCtx::Class { is_readonly: true, .. });
            let mut ty = None;
            if self.current_token.kind != TokenKind::Variable {
                 if let Some(t) = self.parse_type() {
                     ty = Some(self.arena.alloc(t) as &'ast Type<'ast>);
                 }
            }

            let name = if self.current_token.kind == TokenKind::Variable {
                let token = self.arena.alloc(self.current_token);
                self.bump();
                token
            } else {
                self.bump(); // Skip bad token
                self.arena.alloc(Token { kind: TokenKind::Error, span: Span::default() })
            };
            
            let default = if self.current_token.kind == TokenKind::Eq {
                self.bump();
                Some(self.parse_expr(0))
            } else {
                None
            };

            if modifiers.iter().any(|m| m.kind == TokenKind::Readonly) && ty.is_none() {
                self.errors.push(ParseError { span: Span::new(start, start), message: "readonly property requires a type" });
            }
            if class_is_readonly && ty.is_none() {
                self.errors.push(ParseError { span: Span::new(start, start), message: "readonly property requires a type" });
            }
            
            // Property hooks
            if self.current_token.kind == TokenKind::OpenBrace {
                let hooks = self.parse_property_hooks();
                self.expect_semicolon();
                let end = self.current_token.span.end;
                ClassMember::PropertyHook {
                    attributes,
                    modifiers: self.arena.alloc_slice_copy(&modifiers),
                    ty,
                    name,
                    default,
                    hooks: self.arena.alloc_slice_copy(&hooks),
                    span: Span::new(start, end),
                }
            } else {
                self.expect_semicolon();
                
                let end = self.current_token.span.end;
                
                ClassMember::Property {
                    attributes,
                    modifiers: self.arena.alloc_slice_copy(&modifiers),
                    ty,
                    name,
                    default,
                    span: Span::new(start, end),
                }
            }
        }
    }

    fn parse_trait_method_ref(&mut self) -> TraitMethodRef<'ast> {
        let start = self.current_token.span.start;
        let mut trait_name = None;

        // Try qualified reference: TraitName::method
        if matches!(self.current_token.kind, TokenKind::Identifier | TokenKind::NsSeparator | TokenKind::Namespace) {
            // Peek to see if this is a qualified name followed by ::
            let lookahead = self.current_token;
            if lookahead.kind == TokenKind::Identifier && self.next_token.kind == TokenKind::DoubleColon
                || lookahead.kind == TokenKind::NsSeparator
            {
                let name = self.parse_name();
                if self.current_token.kind == TokenKind::DoubleColon {
                    trait_name = Some(name);
                    self.bump();
                } else {
                    // Not actually qualified; roll back trait name usage
                    trait_name = None;
                }
            }
        }

        let method = if self.current_token.kind == TokenKind::Identifier {
            let t = self.arena.alloc(self.current_token);
            self.bump();
            &*t
        } else {
            self.errors.push(ParseError { span: self.current_token.span, message: "Expected method name" });
            let t = self.arena.alloc(Token { kind: TokenKind::Error, span: self.current_token.span });
            self.bump();
            &*t
        };

        TraitMethodRef {
            trait_name,
            method,
            span: Span::new(start, method.span.end),
        }
    }

    fn parse_property_hooks(&mut self) -> Vec<PropertyHook<'ast>> {
        // current token is '{'
        let mut hooks = std::vec::Vec::new();
        self.bump(); // eat {
        while self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
            let mut attributes = &[] as &'ast [AttributeGroup<'ast>];
            if self.current_token.kind == TokenKind::Attribute {
                attributes = self.parse_attributes();
            }

            let mut modifiers = std::vec::Vec::new();
            while matches!(self.current_token.kind, TokenKind::Public | TokenKind::Protected | TokenKind::Private | TokenKind::Static | TokenKind::Abstract | TokenKind::Final | TokenKind::Readonly) {
                modifiers.push(self.current_token);
                self.bump();
            }
            self.validate_modifiers(&modifiers, ModifierContext::Method);

            let by_ref = if matches!(self.current_token.kind, TokenKind::Ampersand | TokenKind::AmpersandFollowedByVarOrVararg | TokenKind::AmpersandNotFollowedByVarOrVararg) {
                self.bump();
                true
            } else {
                false
            };

            let start = self.current_token.span.start;
            let name = if self.current_token.kind == TokenKind::Identifier {
                let t = self.arena.alloc(self.current_token);
                self.bump();
                t
            } else {
                self.errors.push(ParseError { span: self.current_token.span, message: "Expected hook name" });
                let t = self.arena.alloc(Token { kind: TokenKind::Error, span: self.current_token.span });
                self.bump();
                t
            };

            let mut params = std::vec::Vec::new();
            if self.current_token.kind == TokenKind::OpenParen {
                self.bump();
                while self.current_token.kind != TokenKind::CloseParen && self.current_token.kind != TokenKind::Eof {
                    params.push(self.parse_param());
                    if self.current_token.kind == TokenKind::Comma {
                        self.bump();
                    }
                }
                if self.current_token.kind == TokenKind::CloseParen {
                    self.bump();
                }
            }

            let body = match self.current_token.kind {
                TokenKind::SemiColon => {
                    self.bump();
                    PropertyHookBody::None
                }
                TokenKind::OpenBrace => {
                    let stmt = self.parse_block();
                    match stmt {
                        Stmt::Block { statements, .. } => PropertyHookBody::Statements(*statements),
                        _ => PropertyHookBody::Statements(self.arena.alloc_slice_copy(&[stmt])),
                    }
                }
                TokenKind::DoubleArrow => {
                    self.bump();
                    let expr = self.parse_expr(0);
                    if self.current_token.kind == TokenKind::SemiColon {
                        self.bump();
                    }
                    PropertyHookBody::Expr(expr)
                }
                _ => {
                    self.errors.push(ParseError { span: self.current_token.span, message: "Invalid property hook body" });
                    PropertyHookBody::None
                }
            };

            let end = match body {
                PropertyHookBody::None => name.span.end,
                PropertyHookBody::Expr(e) => e.span().end,
                PropertyHookBody::Statements(stmts) => {
                    if let Some(last) = stmts.last() {
                        last.span().end
                    } else {
                        self.current_token.span.end
                    }
                }
            };

            hooks.push(PropertyHook {
                attributes,
                modifiers: self.arena.alloc_slice_copy(&modifiers),
                name,
                params: self.arena.alloc_slice_copy(&params),
                by_ref,
                body,
                span: Span::new(start, end),
            });
        }
        if self.current_token.kind == TokenKind::CloseBrace {
            self.bump();
        }
        hooks
    }

    fn parse_switch(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat switch
        
        if self.current_token.kind == TokenKind::OpenParen {
            self.bump();
        }
        let condition = self.parse_expr(0);
        if self.current_token.kind == TokenKind::CloseParen {
            self.bump();
        }
        
        let is_alt = if self.current_token.kind == TokenKind::Colon {
            self.bump();
            true
        } else {
            if self.current_token.kind == TokenKind::OpenBrace {
                self.bump();
            }
            false
        };
        
        let mut cases = std::vec::Vec::new();
        let end_token = if is_alt { TokenKind::EndSwitch } else { TokenKind::CloseBrace };

        while self.current_token.kind != end_token && self.current_token.kind != TokenKind::Eof {
            let case_start = self.current_token.span.start;
            let condition = if self.current_token.kind == TokenKind::Case {
                self.bump();
                let expr = self.parse_expr(0);
                if self.current_token.kind == TokenKind::Colon || self.current_token.kind == TokenKind::SemiColon {
                    self.bump();
                }
                Some(expr)
            } else if self.current_token.kind == TokenKind::Default {
                self.bump();
                if self.current_token.kind == TokenKind::Colon || self.current_token.kind == TokenKind::SemiColon {
                    self.bump();
                }
                None
            } else {
                // Error or end of switch
                break;
            };
            
            let mut body_stmts = std::vec::Vec::new();
            while self.current_token.kind != TokenKind::Case && self.current_token.kind != TokenKind::Default && self.current_token.kind != end_token && self.current_token.kind != TokenKind::Eof {
                body_stmts.push(self.parse_stmt());
            }
            
            let case_end = if body_stmts.is_empty() {
                self.current_token.span.start
            } else {
                body_stmts.last().unwrap().span().end
            };
            
            cases.push(Case {
                condition,
                body: self.arena.alloc_slice_copy(&body_stmts),
                span: Span::new(case_start, case_end),
            });
        }
        
        if self.current_token.kind == end_token {
            self.bump();
        }
        if is_alt {
            self.expect_semicolon();
        }
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Switch {
            condition,
            cases: self.arena.alloc_slice_copy(&cases),
            span: Span::new(start, end),
        })
    }

    fn parse_try(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat try
        
        let body_stmt = self.parse_block();
        let body: &'ast [StmtId<'ast>] = match body_stmt {
            Stmt::Block { statements, .. } => *statements,
            _ => self.arena.alloc_slice_copy(&[body_stmt]) as &'ast [StmtId<'ast>],
        };
        
        let mut catches = std::vec::Vec::new();
        while self.current_token.kind == TokenKind::Catch {
            let catch_start = self.current_token.span.start;
            self.bump();
            
            if self.current_token.kind == TokenKind::OpenParen {
                self.bump();
            }
            
            // Types
            let mut types = std::vec::Vec::new();
            loop {
                types.push(self.parse_name());
                if self.current_token.kind == TokenKind::Pipe {
                    self.bump();
                    continue;
                }
                break;
            }
            
            let var = if self.current_token.kind == TokenKind::Variable {
                let t = self.arena.alloc(self.current_token);
                self.bump();
                Some(&*t)
            } else {
                None
            };
            
            if self.current_token.kind == TokenKind::CloseParen {
                self.bump();
            }
            
            let catch_body_stmt = self.parse_block();
            let catch_body: &'ast [StmtId<'ast>] = match catch_body_stmt {
                Stmt::Block { statements, .. } => *statements,
                _ => self.arena.alloc_slice_copy(&[catch_body_stmt]) as &'ast [StmtId<'ast>],
            };
            
            let catch_end = self.current_token.span.end; // Approximate
            
            catches.push(Catch {
                types: self.arena.alloc_slice_copy(&types),
                var,
                body: catch_body,
                span: Span::new(catch_start, catch_end),
            });
        }
        
        let finally = if self.current_token.kind == TokenKind::Finally {
            self.bump();
            let finally_stmt = self.parse_block();
            match finally_stmt {
                Stmt::Block { statements, .. } => Some(*statements),
                _ => Some(self.arena.alloc_slice_copy(&[finally_stmt]) as &'ast [StmtId<'ast>]),
            }
        } else {
            None
        };
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Try {
            body,
            catches: self.arena.alloc_slice_copy(&catches),
            finally,
            span: Span::new(start, end),
        })
    }

    fn parse_throw(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat throw
        
        let expr = self.parse_expr(0);
        
        self.expect_semicolon();
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Throw {
            expr,
            span: Span::new(start, end),
        })
    }

    fn parse_const_stmt(&mut self, attributes: &'ast [AttributeGroup<'ast>]) -> StmtId<'ast> {
        let start = if let Some(first) = attributes.first() {
            first.span.start
        } else {
            self.current_token.span.start
        };
        self.bump(); // const

        let mut consts = std::vec::Vec::new();
        loop {
            let name = if self.current_token.kind == TokenKind::Identifier {
                let tok = self.arena.alloc(self.current_token);
                self.bump();
                tok
            } else {
                self.errors.push(ParseError { span: self.current_token.span, message: "Expected identifier" });
                self.arena.alloc(Token { kind: TokenKind::Error, span: self.current_token.span })
            };

            if self.current_token.kind == TokenKind::Eq {
                self.bump();
            } else {
                self.errors.push(ParseError { span: self.current_token.span, message: "Expected '='" });
            }
            let value = self.parse_expr(0);
            let span = Span::new(name.span.start, value.span().end);
            consts.push(ClassConst { name, value, span });

            if self.current_token.kind == TokenKind::Comma {
                self.bump();
                continue;
            }
            break;
        }

        self.expect_semicolon();
        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::Const {
            attributes,
            consts: self.arena.alloc_slice_copy(&consts),
            span: Span::new(start, end),
        })
    }

    fn parse_break(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat break
        
        let level = if self.current_token.kind != TokenKind::SemiColon && self.current_token.kind != TokenKind::CloseTag && self.current_token.kind != TokenKind::Eof && self.current_token.kind != TokenKind::CloseBrace {
            let expr = self.parse_expr(0);
            self.validate_break_continue_level(expr);
            Some(expr)
        } else {
            None
        };
        
        self.expect_semicolon();
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Break {
            level,
            span: Span::new(start, end),
        })
    }

    fn parse_continue(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat continue
        
        let level = if self.current_token.kind != TokenKind::SemiColon && self.current_token.kind != TokenKind::CloseTag && self.current_token.kind != TokenKind::Eof && self.current_token.kind != TokenKind::CloseBrace {
            let expr = self.parse_expr(0);
            self.validate_break_continue_level(expr);
            Some(expr)
        } else {
            None
        };
        
        self.expect_semicolon();
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Continue {
            level,
            span: Span::new(start, end),
        })
    }

    fn validate_break_continue_level(&mut self, expr: ExprId<'ast>) {
        if let Expr::Integer { value, span } = expr {
            if value.is_empty() {
                self.errors.push(ParseError { span: *span, message: "break/continue level must be a positive integer" });
                return;
            }
            let mut num: usize = 0;
            for b in *value {
                if !b.is_ascii_digit() {
                    num = 0;
                    break;
                }
                num = num.saturating_mul(10).saturating_add((b - b'0') as usize);
            }
            if num == 0 {
                self.errors.push(ParseError { span: *span, message: "break/continue level must be a positive integer" });
            }
        } else {
            self.errors.push(ParseError { span: expr.span(), message: "break/continue level must be a positive integer literal" });
        }
    }

    fn parse_goto(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat goto

        let label = if self.current_token.kind == TokenKind::Identifier {
            let tok = self.arena.alloc(self.current_token);
            self.bump();
            tok
        } else {
            self.errors.push(ParseError {
                span: self.current_token.span,
                message: "Expected label after goto",
            });
            let tok = self.arena.alloc(self.current_token);
            self.bump();
            tok
        };

        self.expect_semicolon();

        let end = self.current_token.span.end;
        self.arena.alloc(Stmt::Goto {
            label,
            span: Span::new(start, end),
        })
    }

    fn parse_declare(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat declare

        if self.current_token.kind == TokenKind::OpenParen {
            self.bump();
        }

        let mut declares = std::vec::Vec::new();
        loop {
            let key = if self.current_token.kind == TokenKind::Identifier {
                let token = self.arena.alloc(self.current_token);
                self.bump();
                token
            } else {
                self.arena.alloc(Token { kind: TokenKind::Error, span: Span::default() })
            };

            if self.current_token.kind == TokenKind::Eq {
                self.bump();
            }

            let value = self.parse_expr(0);
            self.validate_declare_item(key, value);

            declares.push(crate::ast::DeclareItem {
                key,
                value,
                span: Span::new(key.span.start, value.span().end),
            });

            if self.current_token.kind == TokenKind::Comma {
                self.bump();
            } else {
                break;
            }
        }

        if self.current_token.kind == TokenKind::CloseParen {
            self.bump();
        }

        let body = if self.current_token.kind == TokenKind::Colon {
            self.bump();
            let mut stmts = std::vec::Vec::new();
            while self.current_token.kind != TokenKind::EndDeclare && self.current_token.kind != TokenKind::Eof {
                stmts.push(self.parse_stmt());
            }
            if self.current_token.kind == TokenKind::EndDeclare {
                self.bump();
            }
            self.expect_semicolon();
            self.arena.alloc_slice_copy(&stmts) as &'ast [StmtId<'ast>]
        } else if self.current_token.kind == TokenKind::SemiColon {
            self.bump();
            &[] as &'ast [StmtId<'ast>]
        } else {
            let stmt = self.parse_stmt();
            match stmt {
                Stmt::Block { statements, .. } => *statements,
                _ => self.arena.alloc_slice_copy(&[stmt]) as &'ast [StmtId<'ast>],
            }
        };

        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::Declare {
            declares: self.arena.alloc_slice_copy(&declares),
            body,
            span: Span::new(start, end),
        })
    }

    fn validate_declare_item(&mut self, key: &Token, value: ExprId<'ast>) {
        if self.token_eq_ident(key, b"strict_types") {
            if let Some(num) = self.int_literal_value(value) {
                if num != 0 && num != 1 {
                    self.errors.push(ParseError { span: value.span(), message: "strict_types must be 0 or 1" });
                }
            } else {
                self.errors.push(ParseError { span: value.span(), message: "strict_types must be an integer literal" });
            }
        } else if self.token_eq_ident(key, b"ticks") {
            if let Some(num) = self.int_literal_value(value) {
                if num == 0 {
                    self.errors.push(ParseError { span: value.span(), message: "ticks must be a positive integer" });
                }
            } else {
                self.errors.push(ParseError { span: value.span(), message: "ticks must be an integer literal" });
            }
        } else if self.token_eq_ident(key, b"encoding") {
            match value {
                Expr::String { .. } => {}
                _ => self.errors.push(ParseError { span: value.span(), message: "encoding must be a string literal" }),
            }
        }
    }

    fn int_literal_value(&self, expr: ExprId<'ast>) -> Option<u64> {
        if let Expr::Integer { value, .. } = expr {
            let mut num: u64 = 0;
            for b in *value {
                if *b == b'_' {
                    continue;
                }
                if !b.is_ascii_digit() {
                    return None;
                }
                num = num.saturating_mul(10).saturating_add((*b - b'0') as u64);
            }
            Some(num)
        } else {
            None
        }
    }

    fn parse_global(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat global
        
        let mut vars = std::vec::Vec::new();
        loop {
            vars.push(self.parse_expr(0));
            if self.current_token.kind == TokenKind::Comma {
                self.bump();
            } else {
                break;
            }
        }
        
        self.expect_semicolon();
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Global {
            vars: self.arena.alloc_slice_copy(&vars),
            span: Span::new(start, end),
        })
    }

    fn parse_static(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat static
        
        let mut vars = std::vec::Vec::new();
        loop {
            let var = self.parse_expr(0);
            let default = if self.current_token.kind == TokenKind::Eq {
                self.bump();
                Some(self.parse_expr(0))
            } else {
                None
            };
            
            let span = if let Some(def) = default {
                Span::new(var.span().start, def.span().end)
            } else {
                var.span()
            };

            vars.push(StaticVar { var, default, span });

            if self.current_token.kind == TokenKind::Comma {
                self.bump();
            } else {
                break;
            }
        }
        
        self.expect_semicolon();
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Static {
            vars: self.arena.alloc_slice_copy(&vars),
            span: Span::new(start, end),
        })
    }

    fn parse_unset(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat unset
        
        if self.current_token.kind == TokenKind::OpenParen {
            self.bump();
        }
        
        let mut vars = std::vec::Vec::new();
        loop {
            vars.push(self.parse_expr(0));
            if self.current_token.kind == TokenKind::Comma {
                self.bump();
            } else {
                break;
            }
        }
        
        if self.current_token.kind == TokenKind::CloseParen {
            self.bump();
        }
        
        self.expect_semicolon();
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Unset {
            vars: self.arena.alloc_slice_copy(&vars),
            span: Span::new(start, end),
        })
    }


    fn parse_call_arguments(&mut self) -> (&'ast [Arg<'ast>], Span) {
        let start = self.current_token.span.start;
        if self.current_token.kind != TokenKind::OpenParen {
            return (&[], Span::default());
        }
        self.bump(); // consume (

        let mut args = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseParen && self.current_token.kind != TokenKind::Eof {
            let mut name: Option<&'ast Token> = None;
            let mut unpack = false;
            let start = self.current_token.span.start;

            // Named argument: identifier-like token followed by :
            if (self.current_token.kind == TokenKind::Identifier || self.current_token.kind.is_semi_reserved()) && self.next_token.kind == TokenKind::Colon {
                name = Some(self.arena.alloc(self.current_token.clone()));
                self.bump(); // Identifier
                self.bump(); // Colon
            } else if self.current_token.kind == TokenKind::Ellipsis {
                if self.next_token.kind == TokenKind::CloseParen {
                    let span = self.current_token.span;
                    self.bump(); // Eat ...
                    let value = self.arena.alloc(Expr::VariadicPlaceholder { span });
                    args.push(Arg {
                        name: None,
                        value,
                        unpack: false,
                        span,
                    });
                    continue;
                }
                unpack = true;
                self.bump();
            }

            let value = self.parse_expr(0);
            
            args.push(Arg {
                name,
                value,
                unpack,
                span: Span { start, end: value.span().end },
            });

            if self.current_token.kind == TokenKind::Comma {
                self.bump();
                // Allow trailing comma in argument list
                if self.current_token.kind == TokenKind::CloseParen {
                    break;
                }
            } else if self.current_token.kind != TokenKind::CloseParen {
                break;
            }
        }
        let end = self.current_token.span.end;
        if self.current_token.kind == TokenKind::CloseParen {
            self.bump();
        }
        (self.arena.alloc_slice_copy(&args), Span::new(start, end))
    }

    fn parse_closure_expr(&mut self, attributes: &'ast [AttributeGroup<'ast>], is_static: bool, start: usize) -> ExprId<'ast> {
        let _returns_by_ref = if self.current_token.kind == TokenKind::Ampersand {
            self.bump();
            true
        } else {
            false
        };

        // Anonymous functions should not have a name, but allow an identifier for recovery
        if self.current_token.kind == TokenKind::Identifier {
            self.bump();
        }

        if self.current_token.kind == TokenKind::OpenParen {
            self.bump();
        }
        let mut params = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseParen && self.current_token.kind != TokenKind::Eof {
            params.push(self.parse_param());
            if self.current_token.kind == TokenKind::Comma {
                self.bump();
            }
        }
        if self.current_token.kind == TokenKind::CloseParen {
            self.bump();
        }

        let mut uses = std::vec::Vec::new();
        if self.current_token.kind == TokenKind::Use {
            self.bump();
            if self.current_token.kind == TokenKind::OpenParen {
                self.bump();
            }
            while self.current_token.kind != TokenKind::CloseParen && self.current_token.kind != TokenKind::Eof {
                let by_ref = if matches!(self.current_token.kind, TokenKind::Ampersand | TokenKind::AmpersandFollowedByVarOrVararg) {
                    self.bump();
                    true
                } else {
                    false
                };
                
                let var = if self.current_token.kind == TokenKind::Variable {
                    let t = self.arena.alloc(self.current_token);
                    self.bump();
                    t
                } else {
                    self.arena.alloc(Token { kind: TokenKind::Error, span: Span::default() })
                };
                
                uses.push(ClosureUse {
                    var,
                    by_ref,
                    span: var.span,
                });
                
                if self.current_token.kind == TokenKind::Comma {
                    self.bump();
                }
            }
            if self.current_token.kind == TokenKind::CloseParen {
                self.bump();
            }
        }

        let return_type = if self.current_token.kind == TokenKind::Colon {
            self.bump();
            if let Some(t) = self.parse_type() {
                Some(self.arena.alloc(t) as &'ast Type<'ast>)
            } else {
                None
            }
        } else {
            None
        };

        let body_stmt = self.parse_block();
        let body: &'ast [StmtId<'ast>] = match body_stmt {
            Stmt::Block { statements, .. } => *statements,
            _ => self.arena.alloc_slice_copy(&[body_stmt]) as &'ast [StmtId<'ast>],
        };
        
        let end = self.current_token.span.end;
        self.arena.alloc(Expr::Closure {
            attributes,
            is_static,
            params: self.arena.alloc_slice_copy(&params),
            uses: self.arena.alloc_slice_copy(&uses),
            return_type,
            body,
            span: Span::new(start, end),
        })
    }

    fn parse_arrow_function(&mut self, attributes: &'ast [AttributeGroup<'ast>], is_static: bool, start: usize) -> ExprId<'ast> {
        let _returns_by_ref = if self.current_token.kind == TokenKind::Ampersand {
            self.bump();
            true
        } else {
            false
        };

        if self.current_token.kind == TokenKind::OpenParen {
            self.bump();
        }
        let mut params = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseParen && self.current_token.kind != TokenKind::Eof {
            params.push(self.parse_param());
            if self.current_token.kind == TokenKind::Comma {
                self.bump();
            }
        }
        if self.current_token.kind == TokenKind::CloseParen {
            self.bump();
        }

        let return_type = if self.current_token.kind == TokenKind::Colon {
            self.bump();
            if let Some(t) = self.parse_type() {
                Some(self.arena.alloc(t) as &'ast Type<'ast>)
            } else {
                None
            }
        } else {
            None
        };

        if self.current_token.kind == TokenKind::DoubleArrow {
            self.bump();
        }
        let expr = self.parse_expr(0);
        
        let end = expr.span().end;
        self.arena.alloc(Expr::ArrowFunction {
            attributes,
            is_static,
            params: self.arena.alloc_slice_copy(&params),
            return_type,
            expr,
            span: Span::new(start, end),
        })
    }

    fn parse_expr(&mut self, min_bp: u8) -> ExprId<'ast> {
        let mut left = self.parse_nud();

        loop {
            let op = match self.current_token.kind {
                TokenKind::Plus => BinaryOp::Plus,
                TokenKind::Minus => BinaryOp::Minus,
                TokenKind::Asterisk => BinaryOp::Mul,
                TokenKind::Slash => BinaryOp::Div,
                TokenKind::Percent => BinaryOp::Mod,
                TokenKind::Dot => BinaryOp::Concat,
                TokenKind::EqEq => BinaryOp::EqEq,
                TokenKind::EqEqEq => BinaryOp::EqEqEq,
                TokenKind::BangEq => BinaryOp::NotEq,
                TokenKind::BangEqEq => BinaryOp::NotEqEq,
                TokenKind::Lt => BinaryOp::Lt,
                TokenKind::LtEq => BinaryOp::LtEq,
                TokenKind::Gt => BinaryOp::Gt,
                TokenKind::GtEq => BinaryOp::GtEq,
                TokenKind::AmpersandAmpersand => BinaryOp::And,
                TokenKind::PipePipe => BinaryOp::Or,
                TokenKind::Ampersand | TokenKind::AmpersandFollowedByVarOrVararg | TokenKind::AmpersandNotFollowedByVarOrVararg => BinaryOp::BitAnd,
                TokenKind::Pipe => BinaryOp::BitOr,
                TokenKind::Caret => BinaryOp::BitXor,
                TokenKind::LogicalAnd => BinaryOp::LogicalAnd,
                TokenKind::LogicalOr | TokenKind::Insteadof => BinaryOp::LogicalOr,
                TokenKind::LogicalXor => BinaryOp::LogicalXor,
                TokenKind::Coalesce => BinaryOp::Coalesce,
                TokenKind::Spaceship => BinaryOp::Spaceship,
                TokenKind::Pow => BinaryOp::Pow,
                TokenKind::Sl => BinaryOp::ShiftLeft,
                TokenKind::Sr => BinaryOp::ShiftRight,
                TokenKind::InstanceOf => BinaryOp::Instanceof,
                TokenKind::Question => {
                    // Ternary: a ? b : c
                    let (l_bp, r_bp) = (40, 41);
                    if l_bp < min_bp {
                        break;
                    }
                    self.bump();
                    
                    let if_true = if self.current_token.kind != TokenKind::Colon {
                        Some(self.parse_expr(0))
                    } else {
                        None
                    };
                    
                    if self.current_token.kind == TokenKind::Colon {
                        self.bump();
                    }
                    
                    let if_false = self.parse_expr(r_bp);
                    
                    let span = Span::new(left.span().start, if_false.span().end);
                    left = self.arena.alloc(Expr::Ternary {
                        condition: left,
                        if_true,
                        if_false,
                        span,
                    });
                    continue;
                }
                TokenKind::PlusEq | TokenKind::MinusEq | TokenKind::MulEq | TokenKind::DivEq | 
                TokenKind::ModEq | TokenKind::ConcatEq | TokenKind::AndEq | TokenKind::OrEq | 
                TokenKind::XorEq | TokenKind::SlEq | TokenKind::SrEq | TokenKind::PowEq | 
                TokenKind::CoalesceEq => {
                    let op = match self.current_token.kind {
                        TokenKind::PlusEq => AssignOp::Plus,
                        TokenKind::MinusEq => AssignOp::Minus,
                        TokenKind::MulEq => AssignOp::Mul,
                        TokenKind::DivEq => AssignOp::Div,
                        TokenKind::ModEq => AssignOp::Mod,
                        TokenKind::ConcatEq => AssignOp::Concat,
                        TokenKind::AndEq => AssignOp::BitAnd,
                        TokenKind::OrEq => AssignOp::BitOr,
                        TokenKind::XorEq => AssignOp::BitXor,
                        TokenKind::SlEq => AssignOp::ShiftLeft,
                        TokenKind::SrEq => AssignOp::ShiftRight,
                        TokenKind::PowEq => AssignOp::Pow,
                        TokenKind::CoalesceEq => AssignOp::Coalesce,
                        _ => unreachable!(),
                    };
                    
                    let l_bp = 35; // Same as Assignment
                    if l_bp < min_bp {
                        break;
                    }
                    self.bump();
                    let right = self.parse_expr(l_bp - 1);
                    let span = Span::new(left.span().start, right.span().end);
                    left = self.arena.alloc(Expr::AssignOp {
                        var: left,
                        op,
                        expr: right,
                        span,
                    });
                    continue;
                }
                TokenKind::Eq => {
                    // Assignment: $a = 1
                    let l_bp = 35; // Higher than 'and' (30), lower than 'ternary' (40)
                    if l_bp < min_bp {
                        break;
                    }
                    self.bump();

                    // Assignment by reference: $a =& $b
                    if matches!(self.current_token.kind, TokenKind::Ampersand | TokenKind::AmpersandFollowedByVarOrVararg | TokenKind::AmpersandNotFollowedByVarOrVararg) {
                        self.bump();
                        let right = self.parse_expr(l_bp - 1);
                        let span = Span::new(left.span().start, right.span().end);
                        left = self.arena.alloc(Expr::AssignRef {
                            var: left,
                            expr: right,
                            span,
                        });
                        continue;
                    }

                    // Right associative
                    let right = self.parse_expr(l_bp - 1);
                    
                    let span = Span::new(left.span().start, right.span().end);
                    left = self.arena.alloc(Expr::Assign {
                        var: left,
                        expr: right,
                        span,
                    });
                    continue;
                }
                TokenKind::OpenBracket => {
                    // Array Dimension Fetch: $a[1]
                    let l_bp = 210; // Very high
                    if l_bp < min_bp {
                        break;
                    }
                    self.bump();
                    
                    let dim = if self.current_token.kind == TokenKind::CloseBracket {
                        None
                    } else {
                        Some(self.parse_expr(0))
                    };
                    
                    if self.current_token.kind == TokenKind::CloseBracket {
                        self.bump();
                    }
                    
                    let span = Span::new(left.span().start, self.current_token.span.end);
                    left = self.arena.alloc(Expr::ArrayDimFetch {
                        array: left,
                        dim,
                        span,
                    });
                    continue;
                }
                TokenKind::NullSafeArrow => {
                    let l_bp = 210;
                    if l_bp < min_bp {
                        break;
                    }
                    self.bump();

                    let prop_or_method = if matches!(self.current_token.kind, TokenKind::OpenBrace | TokenKind::DollarOpenCurlyBraces) {
                        self.bump();
                        let expr = self.parse_expr(0);
                        if self.current_token.kind == TokenKind::CloseBrace {
                            self.bump();
                        }
                        expr
                    } else if self.current_token.kind == TokenKind::Dollar {
                        let start = self.current_token.span.start;
                        self.bump();
                        if self.current_token.kind == TokenKind::OpenBrace {
                            self.bump();
                            let expr = self.parse_expr(0);
                            if self.current_token.kind == TokenKind::CloseBrace {
                                self.bump();
                            }
                            expr
                        } else if self.current_token.kind == TokenKind::Variable {
                            let token = self.current_token;
                            self.bump();
                            let span = Span::new(start, token.span.end);
                            self.arena.alloc(Expr::Variable {
                                name: span,
                                span,
                            })
                        } else {
                            self.arena.alloc(Expr::Error { span: Span::new(start, self.current_token.span.end) })
                        }
                    } else if self.current_token.kind == TokenKind::Identifier || self.current_token.kind == TokenKind::Variable || self.current_token.kind.is_semi_reserved() {
                        let token = self.current_token;
                        self.bump();
                        self.arena.alloc(Expr::Variable {
                            name: token.span,
                            span: token.span,
                        })
                    } else {
                        self.arena.alloc(Expr::Error { span: self.current_token.span })
                    };

                    if self.current_token.kind == TokenKind::OpenParen {
                        let (args, args_span) = self.parse_call_arguments();
                        let span = Span::new(left.span().start, args_span.end);
                        left = self.arena.alloc(Expr::NullsafeMethodCall {
                            target: left,
                            method: prop_or_method,
                            args,
                            span,
                        });
                    } else {
                        let span = Span::new(left.span().start, prop_or_method.span().end);
                        left = self.arena.alloc(Expr::NullsafePropertyFetch {
                            target: left,
                            property: prop_or_method,
                            span,
                        });
                    }
                    continue;
                }
                TokenKind::Arrow => {
                    // Property Fetch or Method Call: $a->b or $a->b()
                    let l_bp = 210;
                    if l_bp < min_bp {
                        break;
                    }
                    self.bump();
                    
                    // Expect identifier or variable (for dynamic property)
                    // For now assume identifier
                    let prop_or_method = if matches!(self.current_token.kind, TokenKind::OpenBrace | TokenKind::DollarOpenCurlyBraces) {
                        self.bump();
                        let expr = self.parse_expr(0);
                        if self.current_token.kind == TokenKind::CloseBrace {
                            self.bump();
                        }
                        expr
                    } else if self.current_token.kind == TokenKind::Dollar {
                        let start = self.current_token.span.start;
                        self.bump();
                        if self.current_token.kind == TokenKind::OpenBrace {
                            self.bump();
                            let expr = self.parse_expr(0);
                            if self.current_token.kind == TokenKind::CloseBrace {
                                self.bump();
                            }
                            expr
                        } else if self.current_token.kind == TokenKind::Variable {
                            let token = self.current_token;
                            self.bump();
                            let span = Span::new(start, token.span.end);
                            self.arena.alloc(Expr::Variable {
                                name: span,
                                span,
                            })
                        } else {
                            self.arena.alloc(Expr::Error { span: Span::new(start, self.current_token.span.end) })
                        }
                    } else if self.current_token.kind == TokenKind::Identifier || self.current_token.kind == TokenKind::Variable || self.current_token.kind.is_semi_reserved() {
                        // We need to wrap this token in an Expr
                        // Reusing Variable/Identifier logic from parse_nud would be good but we need to call it explicitly or just handle it here
                        let token = self.current_token;
                        self.bump();
                        self.arena.alloc(Expr::Variable { // Using Variable for now, should be Identifier if it's a name
                            name: token.span,
                            span: token.span,
                        })
                    } else {
                        // Error
                        self.arena.alloc(Expr::Error { span: self.current_token.span })
                    };
                    
                    // Check for method call
                    if self.current_token.kind == TokenKind::OpenParen {
                        let (args, args_span) = self.parse_call_arguments();
                        
                        let span = Span::new(left.span().start, args_span.end);
                        left = self.arena.alloc(Expr::MethodCall {
                            target: left,
                            method: prop_or_method,
                            args,
                            span,
                        });
                    } else {
                        // Property Fetch
                        let span = Span::new(left.span().start, prop_or_method.span().end);
                        left = self.arena.alloc(Expr::PropertyFetch {
                            target: left,
                            property: prop_or_method,
                            span,
                        });
                    }
                    continue;
                }
                TokenKind::DoubleColon => {
                    // Static Property/Method/Const: A::b, A::b(), A::CONST
                    let l_bp = 210;
                    if l_bp < min_bp {
                        break;
                    }
                    self.bump();
                    
                    let member = if matches!(self.current_token.kind, TokenKind::OpenBrace | TokenKind::DollarOpenCurlyBraces) {
                        self.bump();
                        let expr = self.parse_expr(0);
                        if self.current_token.kind == TokenKind::CloseBrace {
                            self.bump();
                        }
                        expr
                    } else if self.current_token.kind == TokenKind::Dollar {
                        let start = self.current_token.span.start;
                        self.bump();
                        if self.current_token.kind == TokenKind::OpenBrace {
                            self.bump();
                            let expr = self.parse_expr(0);
                            if self.current_token.kind == TokenKind::CloseBrace {
                                self.bump();
                            }
                            expr
                        } else if self.current_token.kind == TokenKind::Variable {
                            let token = self.current_token;
                            self.bump();
                            let span = Span::new(start, token.span.end);
                            self.arena.alloc(Expr::Variable {
                                name: span,
                                span,
                            })
                        } else {
                            self.arena.alloc(Expr::Error { span: Span::new(start, self.current_token.span.end) })
                        }
                    } else if self.current_token.kind == TokenKind::Identifier || self.current_token.kind == TokenKind::Variable || self.current_token.kind.is_semi_reserved() {
                        let token = self.current_token;
                        self.bump();
                        self.arena.alloc(Expr::Variable { 
                            name: token.span,
                            span: token.span,
                        })
                    } else {
                        self.arena.alloc(Expr::Error { span: self.current_token.span })
                    };
                    
                    if self.current_token.kind == TokenKind::OpenParen {
                        // Static Method Call
                        let (args, args_span) = self.parse_call_arguments();
                        let span = Span::new(left.span().start, args_span.end);
                        left = self.arena.alloc(Expr::StaticCall {
                            class: left,
                            method: member,
                            args,
                            span,
                        });
                    } else {
                        // Class Const Fetch (or static property if it starts with $)
                        // For now assume const fetch if identifier
                        let span = Span::new(left.span().start, member.span().end);
                        left = self.arena.alloc(Expr::ClassConstFetch {
                            class: left,
                            constant: member,
                            span,
                        });
                    }
                    continue;
                }
                TokenKind::OpenParen => {
                    // Function Call
                    let l_bp = 190;
                    if l_bp < min_bp {
                        break;
                    }
                    
                    let (args, args_span) = self.parse_call_arguments();
                    
                    let span = Span::new(left.span().start, args_span.end);
                    left = self.arena.alloc(Expr::Call {
                        func: left,
                        args,
                        span,
                    });
                    continue;
                }
                TokenKind::Inc => {
                    let l_bp = 180;
                    if l_bp < min_bp {
                        break;
                    }
                    let end = self.current_token.span.end;
                    self.bump();
                    
                    let span = Span::new(left.span().start, end);
                    left = self.arena.alloc(Expr::PostInc {
                        var: left,
                        span,
                    });
                    continue;
                }
                TokenKind::Dec => {
                    let l_bp = 180;
                    if l_bp < min_bp {
                        break;
                    }
                    let end = self.current_token.span.end;
                    self.bump();
                    
                    let span = Span::new(left.span().start, end);
                    left = self.arena.alloc(Expr::PostDec {
                        var: left,
                        span,
                    });
                    continue;
                }
                _ => break,
            };

            let (l_bp, r_bp) = self.infix_binding_power(op);
            if l_bp < min_bp {
                break;
            }

            self.bump();
            let right = self.parse_expr(r_bp);
            
            let span = Span::new(left.span().start, right.span().end);
            left = self.arena.alloc(Expr::Binary {
                left,
                op,
                right,
                span,
            });
        }

        left
    }

    fn validate_modifiers(&mut self, modifiers: &[Token], ctx: ModifierContext) {
        let mut has_public = false;
        let mut has_protected = false;
        let mut has_private = false;
        let mut has_abstract = false;
        let mut has_final = false;
        let mut has_static = false;
        let mut has_readonly = false;

        for m in modifiers {
            match m.kind {
                TokenKind::Public => {
                    if has_public || has_protected || has_private {
                        self.errors.push(ParseError { span: m.span, message: "Multiple visibility modifiers" });
                    }
                    has_public = true;
                }
                TokenKind::Protected => {
                    if has_public || has_protected || has_private {
                        self.errors.push(ParseError { span: m.span, message: "Multiple visibility modifiers" });
                    }
                    has_protected = true;
                }
                TokenKind::Private => {
                    if has_public || has_protected || has_private {
                        self.errors.push(ParseError { span: m.span, message: "Multiple visibility modifiers" });
                    }
                    has_private = true;
                }
                TokenKind::Abstract => {
                    if has_abstract {
                        self.errors.push(ParseError { span: m.span, message: "Duplicate abstract modifier" });
                    }
                    has_abstract = true;
                }
                TokenKind::Final => {
                    if has_final {
                        self.errors.push(ParseError { span: m.span, message: "Duplicate final modifier" });
                    }
                    has_final = true;
                }
                TokenKind::Static => {
                    if has_static {
                        self.errors.push(ParseError { span: m.span, message: "Duplicate static modifier" });
                    }
                    has_static = true;
                }
                TokenKind::Readonly => {
                    if has_readonly {
                        self.errors.push(ParseError { span: m.span, message: "Duplicate readonly modifier" });
                    }
                    has_readonly = true;
                }
                _ => {}
            }
        }

        if has_abstract && has_final {
            self.errors.push(ParseError { span: modifiers.first().map(|t| t.span).unwrap_or_default(), message: "abstract and final cannot be combined" });
        }

        // readonly is only valid on properties; flag when used on methods
        if matches!(ctx, ModifierContext::Method) && modifiers.iter().any(|m| m.kind == TokenKind::Readonly) {
            self.errors.push(ParseError { span: modifiers.first().map(|t| t.span).unwrap_or_default(), message: "readonly not allowed on methods" });
        }

        if matches!(ctx, ModifierContext::Property) {
            if modifiers.iter().any(|m| matches!(m.kind, TokenKind::Abstract | TokenKind::Final)) {
                self.errors.push(ParseError { span: modifiers.first().map(|t| t.span).unwrap_or_default(), message: "abstract/final not allowed on properties" });
            }
            let has_static = modifiers.iter().any(|m| m.kind == TokenKind::Static);
            if has_static && modifiers.iter().any(|m| m.kind == TokenKind::Readonly) {
                self.errors.push(ParseError { span: modifiers.first().map(|t| t.span).unwrap_or_default(), message: "readonly properties cannot be static" });
            }
            // promotion and visibility rules will be enforced at constructor parsing time; placeholder here.
        }
    }

    fn validate_class_modifiers(&mut self, modifiers: &[Token]) {
        let mut seen_abstract = false;
        let mut seen_final = false;
        let mut seen_readonly = false;

        for m in modifiers {
            match m.kind {
                TokenKind::Abstract => {
                    if seen_abstract {
                        self.errors.push(ParseError { span: m.span, message: "Duplicate abstract modifier" });
                    }
                    seen_abstract = true;
                }
                TokenKind::Final => {
                    if seen_final {
                        self.errors.push(ParseError { span: m.span, message: "Duplicate final modifier" });
                    }
                    seen_final = true;
                }
                TokenKind::Readonly => {
                    if seen_readonly {
                        self.errors.push(ParseError { span: m.span, message: "Duplicate readonly modifier" });
                    }
                    seen_readonly = true;
                }
                _ => {}
            }
        }

        if seen_abstract && seen_final {
            self.errors.push(ParseError { span: modifiers.first().map(|t| t.span).unwrap_or_default(), message: "abstract and final cannot be combined" });
        }
    }

    fn validate_const_modifiers(&mut self, modifiers: &[Token], ctx: ClassMemberCtx) {
        let mut seen_visibility: Option<TokenKind> = None;
        let mut seen_final = false;

        for m in modifiers {
            match m.kind {
                TokenKind::Public | TokenKind::Protected | TokenKind::Private => {
                    if seen_visibility.is_some() {
                        self.errors.push(ParseError { span: m.span, message: "Multiple visibility modifiers" });
                    }
                    if matches!(ctx, ClassMemberCtx::Interface) && m.kind != TokenKind::Public {
                        self.errors.push(ParseError { span: m.span, message: "Interface constants must be public" });
                    }
                    seen_visibility = Some(m.kind);
                }
                TokenKind::Final => {
                    if seen_final {
                        self.errors.push(ParseError { span: m.span, message: "Duplicate final modifier" });
                    }
                    seen_final = true;
                }
                TokenKind::Abstract => {
                    self.errors.push(ParseError { span: m.span, message: "abstract not allowed on class constants" });
                }
                TokenKind::Static => {
                    self.errors.push(ParseError { span: m.span, message: "static not allowed on class constants" });
                }
                TokenKind::Readonly => {
                    self.errors.push(ParseError { span: m.span, message: "readonly not allowed on class constants" });
                }
                _ => {}
            }
        }
    }

    fn token_eq_ident(&self, token: &Token, ident: &[u8]) -> bool {
        let slice = self.lexer.slice(token.span);
        slice.eq_ignore_ascii_case(ident)
    }

    fn name_eq(&self, a: &Name<'ast>, b: &Name<'ast>) -> bool {
        if a.parts.len() != b.parts.len() {
            return false;
        }
        a.parts.iter().zip(b.parts.iter()).all(|(x, y)| {
            self.lexer.slice(x.span).eq_ignore_ascii_case(self.lexer.slice(y.span))
        })
    }

    fn name_eq_token(&self, name: &Name<'ast>, tok: &Token) -> bool {
        if name.parts.len() != 1 {
            return false;
        }
        self.lexer.slice(name.parts[0].span).eq_ignore_ascii_case(self.lexer.slice(tok.span))
    }

    fn sync_to_statement_end(&mut self) {
        while !matches!(self.current_token.kind, TokenKind::SemiColon | TokenKind::CloseBrace | TokenKind::CloseTag | TokenKind::Eof) {
            self.bump();
        }
        if self.current_token.kind == TokenKind::SemiColon {
            self.bump();
        }
    }

    fn parse_nud(&mut self) -> ExprId<'ast> {
        let mut attributes = &[] as &'ast [AttributeGroup<'ast>];
        if self.current_token.kind == TokenKind::Attribute {
            attributes = self.parse_attributes();
        }

        let token = self.current_token;
        match token.kind {
            TokenKind::Empty => {
                let start = token.span.start;
                self.bump();
                if self.current_token.kind == TokenKind::OpenParen {
                    self.bump();
                }
                let expr = self.parse_expr(0);
                if self.current_token.kind == TokenKind::CloseParen {
                    self.bump();
                }
                let end = self.current_token.span.end;
                self.arena.alloc(Expr::Empty {
                    expr,
                    span: Span::new(start, end),
                })
            }
            TokenKind::Isset | TokenKind::LogicalOr | TokenKind::Insteadof | TokenKind::LogicalAnd | TokenKind::LogicalXor => {
                let start = token.span.start;
                self.bump();
                if self.current_token.kind == TokenKind::OpenParen {
                    self.bump();
                }
                let mut vars = std::vec::Vec::new();
                vars.push(self.parse_expr(0));
                while self.current_token.kind == TokenKind::Comma {
                    self.bump();
                    vars.push(self.parse_expr(0));
                }
                if self.current_token.kind == TokenKind::CloseParen {
                    self.bump();
                }
                let end = self.current_token.span.end;
                self.arena.alloc(Expr::Isset {
                    vars: self.arena.alloc_slice_copy(&vars),
                    span: Span::new(start, end),
                })
            }
            TokenKind::Eval => {
                let start = token.span.start;
                self.bump();
                if self.current_token.kind == TokenKind::OpenParen {
                    self.bump();
                }
                let expr = self.parse_expr(0);
                if self.current_token.kind == TokenKind::CloseParen {
                    self.bump();
                }
                let end = self.current_token.span.end;
                self.arena.alloc(Expr::Eval {
                    expr,
                    span: Span::new(start, end),
                })
            }
            TokenKind::Die | TokenKind::Exit => {
                let start = token.span.start;
                let is_die = token.kind == TokenKind::Die;
                self.bump();
                let expr = if self.current_token.kind == TokenKind::OpenParen {
                    self.bump();
                    let e = if self.current_token.kind == TokenKind::CloseParen {
                        None
                    } else {
                        Some(self.parse_expr(0))
                    };
                    if self.current_token.kind == TokenKind::CloseParen {
                        self.bump();
                    }
                    e
                } else {
                    None
                };
                let end = self.current_token.span.end;
                let span = Span::new(start, end);
                if is_die {
                    self.arena.alloc(Expr::Die { expr, span })
                } else {
                    self.arena.alloc(Expr::Exit { expr, span })
                }
            }
            TokenKind::Include | TokenKind::IncludeOnce | TokenKind::Require | TokenKind::RequireOnce => {
                let start = token.span.start;
                self.bump();
                let expr = self.parse_expr(0);
                let end = expr.span().end;
                self.arena.alloc(Expr::Include {
                    kind: match token.kind {
                        TokenKind::Include => IncludeKind::Include,
                        TokenKind::IncludeOnce => IncludeKind::IncludeOnce,
                        TokenKind::Require => IncludeKind::Require,
                        TokenKind::RequireOnce => IncludeKind::RequireOnce,
                        _ => unreachable!(),
                    },
                    expr,
                    span: Span::new(start, end),
                })
            }
            TokenKind::Print => {
                let start = token.span.start;
                self.bump();
                let expr = if self.current_token.kind == TokenKind::OpenParen {
                    self.bump();
                    let inner = self.parse_expr(0);
                    if self.current_token.kind == TokenKind::CloseParen {
                        self.bump();
                    }
                    inner
                } else {
                    self.parse_expr(0)
                };
                let span = Span::new(start, expr.span().end);
                self.arena.alloc(Expr::Print { expr, span })
            }
            TokenKind::Yield | TokenKind::YieldFrom => {
                let start = token.span.start;
                self.bump();

                let mut is_from = token.kind == TokenKind::YieldFrom;
                if !is_from && self.current_token.kind == TokenKind::Identifier {
                    let text = self.lexer.slice(self.current_token.span);
                    let mut lowered = text.to_vec();
                    lowered.make_ascii_lowercase();
                    if lowered == b"from" {
                        is_from = true;
                        self.bump(); // consume 'from'
                    }
                }

                if is_from {
                    let value = self.parse_expr(30);
                    let span = Span::new(start, value.span().end);
                    return self.arena.alloc(Expr::Yield {
                        key: None,
                        value: Some(value),
                        from: true,
                        span,
                    });
                }

                if matches!(self.current_token.kind, TokenKind::SemiColon | TokenKind::CloseTag | TokenKind::Eof | TokenKind::CloseBrace | TokenKind::Comma) {
                    let span = Span::new(start, self.current_token.span.start);
                    return self.arena.alloc(Expr::Yield {
                        key: None,
                        value: None,
                        from: false,
                        span,
                    });
                }

                let first = self.parse_expr(30);
                let (key, value) = if self.current_token.kind == TokenKind::DoubleArrow {
                    self.bump();
                    let val = self.parse_expr(30);
                    (Some(first), val)
                } else {
                    (None, first)
                };
                let span = Span::new(start, value.span().end);
                self.arena.alloc(Expr::Yield {
                    key,
                    value: Some(value),
                    from: false,
            span,
        })
   }

            TokenKind::Throw => {
                // Throw expression (PHP 8+): reuse error node to avoid a new variant
                let start = token.span.start;
                self.bump();
                let expr = self.parse_expr(0);
                let span = Span::new(start, expr.span().end);
                self.arena.alloc(Expr::Error { span })
            }

            TokenKind::Function => {
                let start = if let Some(first) = attributes.first() {
                    first.span.start
                } else {
                    token.span.start
                };
                self.bump();
                self.parse_closure_expr(attributes, false, start)
            }
            TokenKind::Fn => {
                let start = if let Some(first) = attributes.first() {
                    first.span.start
                } else {
                    token.span.start
                };
                self.bump();
                self.parse_arrow_function(attributes, false, start)
            }
            TokenKind::Static => {
                let start = if let Some(first) = attributes.first() {
                    first.span.start
                } else {
                    token.span.start
                };
                self.bump();
                match self.current_token.kind {
                    TokenKind::Function => {
                        self.bump();
                        self.parse_closure_expr(attributes, true, start)
                    }
                    TokenKind::Fn => {
                        self.bump();
                        self.parse_arrow_function(attributes, true, start)
                    }
                    TokenKind::DoubleColon => {
                        // static scope resolution (e.g., static::CONST)
                        self.arena.alloc(Expr::Variable {
                            name: token.span,
                            span: token.span,
                        })
                    }
                    _ => {
                        self.arena.alloc(Expr::Variable {
                            name: token.span,
                            span: token.span,
                        })
                    }
                }
            }
            TokenKind::New => {
                self.bump();
                if self.current_token.kind == TokenKind::Class {
                    let (class, args) = self.parse_anonymous_class();
                    let span = Span::new(token.span.start, class.span().end);
                    self.arena.alloc(Expr::New { class, args, span })
                } else {
                    let class = self.parse_expr(200); // High binding power to grab the class name
                    
                    let (args, end_pos) = if self.current_token.kind == TokenKind::OpenParen {
                        let (a, s) = self.parse_call_arguments();
                        (a, s.end)
                    } else {
                        (&[] as &[Arg], class.span().end)
                    };
                    
                    let span = Span::new(token.span.start, end_pos);
                    self.arena.alloc(Expr::New {
                        class,
                        args,
                        span,
                    })
                }
            }
            TokenKind::Clone => {
                self.bump();
                let expr = self.parse_expr(200);
                let span = Span::new(token.span.start, expr.span().end);
                self.arena.alloc(Expr::Clone {
                    expr,
                    span,
                })
            }
            TokenKind::Match => {
                let start = token.span.start;
                self.bump(); // Eat match
                
                if self.current_token.kind == TokenKind::OpenParen {
                    self.bump();
                }
                let condition = self.parse_expr(0);
                if self.current_token.kind == TokenKind::CloseParen {
                    self.bump();
                }
                
                if self.current_token.kind == TokenKind::OpenBrace {
                    self.bump();
                }
                
                let mut arms = std::vec::Vec::new();
                while self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
                    let arm_start = self.current_token.span.start;
                    
                    let conditions = if self.current_token.kind == TokenKind::Default {
                        self.bump();
                        None
                    } else {
                        let mut conds = std::vec::Vec::new();
                        conds.push(self.parse_expr(0));
                        while self.current_token.kind == TokenKind::Comma {
                            // Lookahead for double arrow
                            let mut lookahead_lexer = self.lexer.clone();
                            let next_token = lookahead_lexer.next().unwrap_or(Token { kind: TokenKind::Eof, span: Span::default() });
                            if next_token.kind == TokenKind::DoubleArrow {
                                break;
                            }
                            self.bump();
                            conds.push(self.parse_expr(0));
                        }
                        Some(self.arena.alloc_slice_copy(&conds) as &'ast [ExprId<'ast>])
                    };
                    
                    if self.current_token.kind == TokenKind::DoubleArrow {
                        self.bump();
                    }
                    
                    let body = self.parse_expr(0);
                    
                    if self.current_token.kind == TokenKind::Comma {
                        self.bump();
                    }
                    
                    let arm_end = body.span().end;
                    
                    arms.push(MatchArm {
                        conditions,
                        body,
                        span: Span::new(arm_start, arm_end),
                    });
                }
                
                if self.current_token.kind == TokenKind::CloseBrace {
                    self.bump();
                }
                
                let end = self.current_token.span.end;
                self.arena.alloc(Expr::Match {
                    condition,
                    arms: self.arena.alloc_slice_copy(&arms),
                    span: Span::new(start, end),
                })
            }
            TokenKind::Dollar => {
                let start = self.current_token.span.start;
                self.bump();
                let expr = self.parse_expr(200);
                let span = Span::new(start, expr.span().end);
                self.arena.alloc(Expr::Variable {
                    name: expr.span(),
                    span,
                })
            }
            TokenKind::Variable => {
                self.bump();
                self.arena.alloc(Expr::Variable {
                    name: token.span,
                    span: token.span,
                })
            }
            TokenKind::LNumber => {
                self.bump();
                self.arena.alloc(Expr::Integer {
                    value: self.arena.alloc_slice_copy(self.lexer.slice(token.span)),
                    span: token.span,
                })
            }
            TokenKind::DNumber => {
                self.bump();
                self.arena.alloc(Expr::Float {
                    value: self.arena.alloc_slice_copy(self.lexer.slice(token.span)),
                    span: token.span,
                })
            }
            TokenKind::StringLiteral => {
                self.bump();
                self.arena.alloc(Expr::String {
                    value: self.arena.alloc_slice_copy(self.lexer.slice(token.span)),
                    span: token.span,
                })
            }
            TokenKind::DoubleQuote => self.parse_interpolated_string(TokenKind::DoubleQuote),
            TokenKind::StartHeredoc => self.parse_interpolated_string(TokenKind::EndHeredoc),
            TokenKind::Backtick => self.parse_interpolated_string(TokenKind::Backtick),
            TokenKind::TypeTrue => {
                self.bump();
                self.arena.alloc(Expr::Boolean {
                    value: true,
                    span: token.span,
                })
            }
            TokenKind::TypeFalse => {
                self.bump();
                self.arena.alloc(Expr::Boolean {
                    value: false,
                    span: token.span,
                })
            }
            TokenKind::TypeNull => {
                self.bump();
                self.arena.alloc(Expr::Null {
                    span: token.span,
                })
            }
            TokenKind::Identifier | TokenKind::Namespace | TokenKind::NsSeparator => {
                let name = self.parse_name();
                self.arena.alloc(Expr::Variable {
                    name: name.span,
                    span: name.span,
                })
            }
            TokenKind::Bang => {
                self.bump();
                let expr = self.parse_expr(160); // BP for !
                let span = Span::new(token.span.start, expr.span().end);
                self.arena.alloc(Expr::Unary {
                    op: UnaryOp::Not,
                    expr,
                    span,
                })
            }
            TokenKind::Minus | TokenKind::Plus | TokenKind::BitNot | TokenKind::At | TokenKind::Inc | TokenKind::Dec | TokenKind::Ampersand | TokenKind::AmpersandFollowedByVarOrVararg | TokenKind::AmpersandNotFollowedByVarOrVararg => {
                let op = match token.kind {
                    TokenKind::Minus => UnaryOp::Minus,
                    TokenKind::Plus => UnaryOp::Plus,
                    TokenKind::BitNot => UnaryOp::BitNot,
                    TokenKind::At => UnaryOp::ErrorSuppress,
                    TokenKind::Inc => UnaryOp::PreInc,
                    TokenKind::Dec => UnaryOp::PreDec,
                    TokenKind::Ampersand | TokenKind::AmpersandFollowedByVarOrVararg | TokenKind::AmpersandNotFollowedByVarOrVararg => UnaryOp::Reference,
                    _ => unreachable!(),
                };
                self.bump();
                let expr = self.parse_expr(180); // BP for unary +, -, ~, ++, --
                let span = Span::new(token.span.start, expr.span().end);
                self.arena.alloc(Expr::Unary {
                    op,
                    expr,
                    span,
                })
            }
            TokenKind::IntCast | TokenKind::BoolCast | TokenKind::FloatCast | TokenKind::StringCast | TokenKind::ArrayCast | TokenKind::ObjectCast | TokenKind::UnsetCast => {
                let kind = match token.kind {
                    TokenKind::IntCast => CastKind::Int,
                    TokenKind::BoolCast => CastKind::Bool,
                    TokenKind::FloatCast => CastKind::Float,
                    TokenKind::StringCast => CastKind::String,
                    TokenKind::ArrayCast => CastKind::Array,
                    TokenKind::ObjectCast => CastKind::Object,
                    TokenKind::UnsetCast => CastKind::Unset,
                    _ => unreachable!(),
                };
                self.bump();
                let expr = self.parse_expr(180); // BP for casts (same as unary)
                let span = Span::new(token.span.start, expr.span().end);
                self.arena.alloc(Expr::Cast {
                    kind,
                    expr,
                    span,
                })
            }
            TokenKind::Array => {
                let start = token.span.start;
                self.bump();
                if self.current_token.kind == TokenKind::OpenParen {
                    self.bump();
                }
                let mut items = std::vec::Vec::new();
                while self.current_token.kind != TokenKind::CloseParen && self.current_token.kind != TokenKind::Eof {
                    items.push(self.parse_array_item());
                    if self.current_token.kind == TokenKind::Comma {
                        self.bump();
                    }
                }
                if self.current_token.kind == TokenKind::CloseParen {
                    self.bump();
                }
                let end = self.current_token.span.end;
                self.arena.alloc(Expr::Array {
                    items: self.arena.alloc_slice_copy(&items),
                    span: Span::new(start, end),
                })
            }
            TokenKind::List => {
                let start = token.span.start;
                self.bump();
                if self.current_token.kind == TokenKind::OpenParen {
                    self.bump();
                }
                let mut items = std::vec::Vec::new();
                while self.current_token.kind != TokenKind::CloseParen && self.current_token.kind != TokenKind::Eof {
                    if self.current_token.kind == TokenKind::Comma {
                        // Empty slot in list()
                        items.push(ArrayItem {
                            key: None,
                            value: self.arena.alloc(Expr::Error { span: self.current_token.span }),
                            by_ref: false,
                            unpack: false,
                            span: self.current_token.span,
                        });
                        self.bump();
                        continue;
                    }
                    items.push(self.parse_array_item());
                    if self.current_token.kind == TokenKind::Comma {
                        self.bump();
                        // allow trailing comma
                        if self.current_token.kind == TokenKind::CloseParen {
                            break;
                        }
                    }
                }
                if self.current_token.kind == TokenKind::CloseParen {
                    self.bump();
                }
                let end = self.current_token.span.end;
                self.arena.alloc(Expr::Array {
                    items: self.arena.alloc_slice_copy(&items),
                    span: Span::new(start, end),
                })
            }
            TokenKind::OpenBracket => { // Short array syntax [1, 2, 3]
                let start = token.span.start;
                self.bump();
                let mut items = std::vec::Vec::new();
                while self.current_token.kind != TokenKind::CloseBracket && self.current_token.kind != TokenKind::Eof {
                    items.push(self.parse_array_item());
                    if self.current_token.kind == TokenKind::Comma {
                        self.bump();
                    }
                }
                if self.current_token.kind == TokenKind::CloseBracket {
                    self.bump();
                }
                let end = self.current_token.span.end;
                self.arena.alloc(Expr::Array {
                    items: self.arena.alloc_slice_copy(&items),
                    span: Span::new(start, end),
                })
            }
            TokenKind::OpenParen => {
                self.bump();
                let expr = self.parse_expr(0);
                if self.current_token.kind == TokenKind::CloseParen {
                    self.bump();
                }
                expr
            }
            _ => {
                // Error recovery: consume token and return Error expr
                self.bump();
                self.arena.alloc(Expr::Error { span: token.span })
            }
        }
    }

    fn infix_binding_power(&self, op: BinaryOp) -> (u8, u8) {
        match op {
            BinaryOp::LogicalOr => (10, 11),
            BinaryOp::LogicalXor => (20, 21),
            BinaryOp::LogicalAnd => (30, 31),
            
            BinaryOp::Coalesce => (51, 50), // Right associative
            
            BinaryOp::Or => (60, 61), // ||
            BinaryOp::And => (70, 71), // &&
            
            BinaryOp::BitOr => (80, 81),
            BinaryOp::BitXor => (90, 91),
            BinaryOp::BitAnd => (100, 101),
            
            BinaryOp::EqEq | BinaryOp::NotEq | BinaryOp::EqEqEq | BinaryOp::NotEqEq | BinaryOp::Spaceship => (110, 111),
            BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => (120, 121),
            
            BinaryOp::ShiftLeft | BinaryOp::ShiftRight => (130, 131),
            
            BinaryOp::Plus | BinaryOp::Minus | BinaryOp::Concat => (140, 141),
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => (150, 151),
            
            BinaryOp::Instanceof => (170, 171), // Non-associative usually, but let's say left for now
            
            BinaryOp::Pow => (191, 190), // Right associative
            
            _ => (0, 0),
        }
    }

    fn parse_param(&mut self) -> Param<'ast> {
        let mut attributes = &[] as &'ast [AttributeGroup<'ast>];
        if self.current_token.kind == TokenKind::Attribute {
            attributes = self.parse_attributes();
        }

        let start = if let Some(first) = attributes.first() {
            first.span.start
        } else {
            self.current_token.span.start
        };
        
        let mut modifiers = std::vec::Vec::new();
        while matches!(self.current_token.kind, 
            TokenKind::Public | TokenKind::Protected | TokenKind::Private | TokenKind::Readonly) {
            modifiers.push(self.current_token);
            self.bump();
        }

        // Type hint?
        let ty = if let Some(t) = self.parse_type() {
             Some(self.arena.alloc(t) as &'ast Type<'ast>)
        } else {
            None
        };
        
        let by_ref = if matches!(self.current_token.kind, TokenKind::Ampersand | TokenKind::AmpersandFollowedByVarOrVararg) {
            self.bump();
            true
        } else {
            false
        };

        let variadic = if self.current_token.kind == TokenKind::Ellipsis {
            self.bump();
            true
        } else {
            false
        };
        
        if self.current_token.kind == TokenKind::Variable {
            let param_name = self.arena.alloc(self.current_token);
            self.bump();
            
            let default = if self.current_token.kind == TokenKind::Eq {
                self.bump();
                Some(self.parse_expr(0))
            } else {
                None
            };
            
            let end = if let Some(expr) = default {
                expr.span().end
            } else {
                param_name.span.end
            };
            
            Param {
                attributes,
                modifiers: self.arena.alloc_slice_copy(&modifiers),
                name: param_name,
                ty,
                default,
                by_ref,
                variadic,
                span: Span::new(start, end),
            }
        } else {
            // Error
            let span = Span::new(start, self.current_token.span.end);
            self.bump();
            Param {
                attributes,
                modifiers: self.arena.alloc_slice_copy(&modifiers),
                name: self.arena.alloc(Token { kind: TokenKind::Error, span }),
                ty: None,
                default: None,
                by_ref,
                variadic,
                span,
            }
        }
    }

    fn parse_function(&mut self, attributes: &'ast [AttributeGroup<'ast>]) -> StmtId<'ast> {
        let start = if let Some(first) = attributes.first() {
            first.span.start
        } else {
            self.current_token.span.start
        };
        self.bump(); // Eat function

        // Name
        let name = if self.current_token.kind == TokenKind::Identifier {
            let token = self.arena.alloc(self.current_token);
            self.bump();
            token
        } else {
            // Error: expected identifier
            self.arena.alloc(self.current_token)
        };

        // Params
        if self.current_token.kind == TokenKind::OpenParen {
            self.bump();
        }
        
        let mut params = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseParen && self.current_token.kind != TokenKind::Eof {
            params.push(self.parse_param());

            if self.current_token.kind == TokenKind::Comma {
                self.bump();
            }
        }

        if self.current_token.kind == TokenKind::CloseParen {
            self.bump();
        }

        let return_type = if self.current_token.kind == TokenKind::Colon {
            self.bump();
            if let Some(t) = self.parse_type() {
                Some(self.arena.alloc(t) as &'ast Type<'ast>)
            } else {
                None
            }
        } else {
            None
        };

        // Body
        let body_stmt = self.parse_stmt(); // Should be a block
        let body: &'ast [StmtId<'ast>] = match body_stmt {
            Stmt::Block { statements, .. } => *statements,
            _ => self.arena.alloc_slice_copy(&[body_stmt]) as &'ast [StmtId<'ast>],
        };

        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::Function {
            attributes,
            name,
            params: self.arena.alloc_slice_copy(&params),
            return_type,
            body,
            span: Span::new(start, end),
        })
    }

    fn parse_attributes(&mut self) -> &'ast [AttributeGroup<'ast>] {
        let mut groups = std::vec::Vec::new();
        while self.current_token.kind == TokenKind::Attribute {
            let start = self.current_token.span.start;
            self.bump(); // Eat #[
            
            let mut attributes = std::vec::Vec::new();
            loop {
                let name = self.parse_name();
                
                let args = if self.current_token.kind == TokenKind::OpenParen {
                    self.parse_call_arguments().0
                } else {
                    &[]
                };
                
                attributes.push(Attribute {
                    name,
                    args,
                    span: Span::new(name.span.start, self.current_token.span.end),
                });
                
                if self.current_token.kind == TokenKind::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
            
            if self.current_token.kind == TokenKind::CloseBracket {
                self.bump();
            }
            
            let end = self.current_token.span.end;
            groups.push(AttributeGroup {
                attributes: self.arena.alloc_slice_copy(&attributes),
                span: Span::new(start, end),
            });
        }
        self.arena.alloc_slice_copy(&groups)
    }

    fn parse_array_item(&mut self) -> ArrayItem<'ast> {
        let unpack = if self.current_token.kind == TokenKind::Ellipsis {
            self.bump();
            true
        } else {
            false
        };

        let by_ref = if matches!(self.current_token.kind, TokenKind::Ampersand | TokenKind::AmpersandFollowedByVarOrVararg | TokenKind::AmpersandNotFollowedByVarOrVararg) {
            self.bump();
            true
        } else {
            false
        };

        let expr1 = self.parse_expr(0);
        
        if self.current_token.kind == TokenKind::DoubleArrow {
            self.bump();
            let value_by_ref = if matches!(self.current_token.kind, TokenKind::Ampersand | TokenKind::AmpersandFollowedByVarOrVararg | TokenKind::AmpersandNotFollowedByVarOrVararg) {
                self.bump();
                true
            } else {
                false
            };
            let value = self.parse_expr(0);
            ArrayItem {
                key: Some(expr1),
                value,
                by_ref: value_by_ref,
                unpack,
                span: Span::new(expr1.span().start, value.span().end),
            }
        } else {
            ArrayItem {
                key: None,
                value: expr1,
                by_ref,
                unpack,
                span: expr1.span(),
            }
        }
    }

    fn parse_type_atomic(&mut self) -> Option<Type<'ast>> {
        if self.current_token.kind == TokenKind::Question {
            self.bump();
            let ty = self.parse_type_atomic()?;
            Some(Type::Nullable(self.arena.alloc(ty)))
        } else if self.current_token.kind == TokenKind::OpenParen {
            self.bump();
            let ty = self.parse_type()?;
            if self.current_token.kind == TokenKind::CloseParen {
                self.bump();
            }
            Some(ty)
        } else if matches!(self.current_token.kind, TokenKind::Namespace | TokenKind::NsSeparator | TokenKind::Identifier) {
            let name = self.parse_name();
            Some(Type::Name(name))
        } else if matches!(self.current_token.kind, 
            TokenKind::Array | 
            TokenKind::Static |
            TokenKind::TypeInt | 
            TokenKind::TypeString | 
            TokenKind::TypeBool | 
            TokenKind::TypeFloat | 
            TokenKind::TypeVoid | 
            TokenKind::TypeObject | 
            TokenKind::TypeMixed | 
            TokenKind::TypeNever | 
            TokenKind::TypeNull | 
            TokenKind::TypeFalse | 
            TokenKind::TypeTrue | 
            TokenKind::TypeIterable | 
            TokenKind::TypeCallable | TokenKind::LogicalOr | TokenKind::Insteadof | TokenKind::LogicalAnd | TokenKind::LogicalXor) {
             let t = self.arena.alloc(self.current_token);
             self.bump();
             Some(Type::Simple(t))
        } else {
            None
        }
       }

    fn parse_type_intersection(&mut self) -> Option<Type<'ast>> {
        let mut left = self.parse_type_atomic()?;
        
        if matches!(self.current_token.kind, TokenKind::Ampersand | TokenKind::AmpersandNotFollowedByVarOrVararg) {
             // Check lookahead to distinguish from by-ref param
             if !(self.next_token.kind == TokenKind::Identifier 
                || self.next_token.kind == TokenKind::Question 
                || self.next_token.kind == TokenKind::OpenParen 
                || self.next_token.kind == TokenKind::NsSeparator 
                || self.next_token.kind.is_semi_reserved()) {
                 return Some(left);
             }

            let mut types = std::vec::Vec::new();
            types.push(left);
            while matches!(self.current_token.kind, TokenKind::Ampersand | TokenKind::AmpersandNotFollowedByVarOrVararg) {
                 if !(self.next_token.kind == TokenKind::Identifier 
                    || self.next_token.kind == TokenKind::Question 
                    || self.next_token.kind == TokenKind::OpenParen 
                    || self.next_token.kind == TokenKind::NsSeparator 
                    || self.next_token.kind.is_semi_reserved()) {
                     break;
                 }

                self.bump();
                if let Some(right) = self.parse_type_atomic() {
                    types.push(right);
                } else {
                    break;
                }
            }
            left = Type::Intersection(self.arena.alloc_slice_copy(&types));
        }
        Some(left)
    }

    fn parse_type(&mut self) -> Option<Type<'ast>> {
        let mut left = self.parse_type_intersection()?;
        
        if self.current_token.kind == TokenKind::Pipe {
            let mut types = std::vec::Vec::new();
            types.push(left);
            while self.current_token.kind == TokenKind::Pipe {
                self.bump();
                if let Some(right) = self.parse_type_intersection() {
                    types.push(right);
                } else {
                    break;
                }
            }
            left = Type::Union(self.arena.alloc_slice_copy(&types));
        }
        Some(left)
    }

    fn parse_interpolated_string(&mut self, end_token: TokenKind) -> ExprId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat opening token

        let mut parts: std::vec::Vec<&'ast Expr<'ast>> = std::vec::Vec::new();
        
        while self.current_token.kind != end_token && self.current_token.kind != TokenKind::Eof {
            match self.current_token.kind {
                TokenKind::EncapsedAndWhitespace => {
                    let token = self.current_token;
                    self.bump();
                    parts.push(self.arena.alloc(Expr::String {
                        value: self.arena.alloc_slice_copy(self.lexer.slice(token.span)),
                        span: token.span,
                    }));
                }
                TokenKind::Variable => {
                    let token = self.current_token;
                    self.bump();
                    let var_expr = self.arena.alloc(Expr::Variable {
                        name: token.span,
                        span: token.span,
                    }) as &'ast Expr<'ast>;
                    
                    // Check for array offset
                    if self.current_token.kind == TokenKind::OpenBracket {
                        self.bump(); // [
                        
                        // Key
                        let key = match self.current_token.kind {
                            TokenKind::Identifier => {
                                let t = self.current_token;
                                self.bump();
                                self.arena.alloc(Expr::String { value: self.arena.alloc_slice_copy(self.lexer.slice(t.span)), span: t.span }) as &'ast Expr<'ast>
                            },
                            TokenKind::NumString => {
                                let t = self.current_token;
                                self.bump();
                                self.arena.alloc(Expr::Integer { value: self.arena.alloc_slice_copy(self.lexer.slice(t.span)), span: t.span }) as &'ast Expr<'ast>
                            },
                            TokenKind::Variable => {
                                let t = self.current_token;
                                self.bump();
                                self.arena.alloc(Expr::Variable { name: t.span, span: t.span }) as &'ast Expr<'ast>
                            },
                            TokenKind::Minus => {
                                // Handle negative number?
                                self.bump();
                                if self.current_token.kind == TokenKind::NumString {
                                    let t = self.current_token;
                                    self.bump();
                                    // TODO: Combine minus and number
                                    self.arena.alloc(Expr::Integer { value: self.arena.alloc_slice_copy(self.lexer.slice(t.span)), span: t.span }) as &'ast Expr<'ast>
                                } else {
                                    self.arena.alloc(Expr::Error { span: self.current_token.span }) as &'ast Expr<'ast>
                                }
                            }
                            _ => {
                                // Error
                                self.arena.alloc(Expr::Error { span: self.current_token.span }) as &'ast Expr<'ast>
                            }
                        };
                        
                        if self.current_token.kind == TokenKind::CloseBracket {
                            self.bump();
                        }
                        
                        parts.push(self.arena.alloc(Expr::ArrayDimFetch {
                            array: var_expr,
                            dim: Some(key),
                            span: Span::new(token.span.start, self.current_token.span.end),
                        }));
                    } else if self.current_token.kind == TokenKind::Arrow {
                         // Property fetch $foo->bar
                         self.bump();
                         if self.current_token.kind == TokenKind::Identifier {
                             let prop_name = self.current_token;
                             self.bump();
                             
                             parts.push(self.arena.alloc(Expr::PropertyFetch {
                                 target: var_expr,
                                 property: self.arena.alloc(Expr::Variable { name: prop_name.span, span: prop_name.span }),
                                 span: Span::new(token.span.start, prop_name.span.end),
                             }));
                         } else {
                             parts.push(var_expr);
                         }
                    } else {
                        parts.push(var_expr);
                    }
                }
                TokenKind::CurlyOpen => {
                    self.bump();
                    let expr = self.parse_expr(0);
                    if self.current_token.kind == TokenKind::CloseBrace {
                        self.bump();
                    }
                    parts.push(expr);
                }
                TokenKind::DollarOpenCurlyBraces => {
                    self.bump();
                    // ${expr}
                    let expr = self.parse_expr(0);
                    if self.current_token.kind == TokenKind::CloseBrace {
                        self.bump();
                    }
                    parts.push(expr);
                }
                _ => {
                    // Unexpected token inside string
                    let token = self.current_token;
                    self.bump();
                    parts.push(self.arena.alloc(Expr::Error { span: token.span }));
                }
            }
        }

        let end = if self.current_token.kind == end_token {
            let end = self.current_token.span.end;
            self.bump();
            end
        } else {
            self.current_token.span.start
        };

        let span = Span::new(start, end);

        if end_token == TokenKind::Backtick {
             self.arena.alloc(Expr::ShellExec {
                parts: self.arena.alloc_slice_copy(&parts),
                span,
            })
        } else {
            self.arena.alloc(Expr::InterpolatedString {
                parts: self.arena.alloc_slice_copy(&parts),
                span,
            })
        }
    }
}
