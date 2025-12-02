use crate::ast::{Name, ParseError, Program};
use crate::lexer::{
    Lexer, LexerMode,
    token::{Token, TokenKind},
};
use bumpalo::Bump;

use crate::span::Span;

mod attributes;
mod control_flow;
mod definitions;
mod expr;
mod stmt;
mod types;

#[allow(dead_code)]
pub trait TokenSource<'src> {
    fn current(&self) -> &Token;
    fn lookahead(&self, n: usize) -> &Token;
    fn bump(&mut self);
    fn set_mode(&mut self, mode: LexerMode);
}

pub struct Parser<'src, 'ast> {
    pub(super) lexer: Lexer<'src>, // In real impl, this would be wrapped in a TokenSource
    pub(super) arena: &'ast Bump,
    pub(super) current_token: Token,
    pub(super) next_token: Token,
    pub(super) errors: std::vec::Vec<ParseError>,
}

impl<'src, 'ast> Parser<'src, 'ast> {
    pub fn new(lexer: Lexer<'src>, arena: &'ast Bump) -> Self {
        let mut parser = Self {
            lexer,
            arena,
            current_token: Token {
                kind: TokenKind::Eof,
                span: Span::default(),
            },
            next_token: Token {
                kind: TokenKind::Eof,
                span: Span::default(),
            },
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

    pub(super) fn parse_name(&mut self) -> Name<'ast> {
        let start = self.current_token.span.start;
        let mut parts = std::vec::Vec::new();

        if self.current_token.kind == TokenKind::NsSeparator {
            parts.push(self.current_token);
            self.bump();
        } else if self.current_token.kind == TokenKind::Namespace {
            parts.push(self.current_token);
            self.bump();
            if self.current_token.kind == TokenKind::NsSeparator {
                parts.push(self.current_token);
                self.bump();
            }
        }

        loop {
            if self.current_token.kind == TokenKind::Identifier
                || self.current_token.kind.is_semi_reserved()
            {
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
            statements.push(self.parse_top_stmt());
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

    fn sync_to_statement_end(&mut self) {
        while !matches!(
            self.current_token.kind,
            TokenKind::SemiColon | TokenKind::CloseBrace | TokenKind::CloseTag | TokenKind::Eof
        ) {
            self.bump();
        }
        if self.current_token.kind == TokenKind::SemiColon {
            self.bump();
        }
    }
}
