use crate::ast::{
    AttributeGroup, Catch, ClassConst, ParseError, StaticVar, Stmt, StmtId, UseItem, UseKind,
};
use crate::lexer::token::TokenKind;
use crate::parser::{LexerMode, Parser, Token};
use crate::span::Span;

impl<'src, 'ast> Parser<'src, 'ast> {
    pub(super) fn parse_stmt(&mut self) -> StmtId<'ast> {
        self.parse_stmt_impl(false)
    }

    pub(super) fn parse_top_stmt(&mut self) -> StmtId<'ast> {
        self.parse_stmt_impl(true)
    }

    fn parse_stmt_impl(&mut self, top_level: bool) -> StmtId<'ast> {
        self.lexer.set_mode(LexerMode::Standard);

        let doc_comment = self.current_doc_comment;

        if self.current_token.kind == TokenKind::Identifier
            && self.next_token.kind == TokenKind::Colon
        {
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
                    TokenKind::Function => self.parse_function(attributes, doc_comment),
                    TokenKind::Class => self.parse_class(attributes, &[], doc_comment),
                    TokenKind::Interface => self.parse_interface(attributes, doc_comment),
                    TokenKind::Trait => self.parse_trait(attributes, doc_comment),
                    TokenKind::Enum => self.parse_enum(attributes, doc_comment),
                    TokenKind::Const => self.parse_const_stmt(attributes, doc_comment),
                    TokenKind::Final | TokenKind::Abstract | TokenKind::Readonly => {
                        let mut modifiers = std::vec::Vec::new();
                        while matches!(
                            self.current_token.kind,
                            TokenKind::Final | TokenKind::Abstract | TokenKind::Readonly
                        ) {
                            modifiers.push(self.current_token);
                            self.bump();
                        }

                        if self.current_token.kind == TokenKind::Class {
                            self.parse_class(attributes, self.arena.alloc_slice_copy(&modifiers), doc_comment)
                        } else {
                            self.arena.alloc(Stmt::Error {
                                span: self.current_token.span,
                            })
                        }
                    }
                    _ => self.arena.alloc(Stmt::Error {
                        span: self.current_token.span,
                    }),
                }
            }
            TokenKind::Final | TokenKind::Abstract | TokenKind::Readonly => {
                let mut modifiers = std::vec::Vec::new();
                while matches!(
                    self.current_token.kind,
                    TokenKind::Final | TokenKind::Abstract | TokenKind::Readonly
                ) {
                    modifiers.push(self.current_token);
                    self.bump();
                }

                if self.current_token.kind == TokenKind::Class {
                    self.parse_class(&[], self.arena.alloc_slice_copy(&modifiers), doc_comment)
                } else {
                    self.arena.alloc(Stmt::Error {
                        span: self.current_token.span,
                    })
                }
            }
            TokenKind::HaltCompiler => {
                if !top_level {
                    self.errors.push(ParseError {
                        span: self.current_token.span,
                        message: "__halt_compiler() can only be used from the outermost scope",
                    });
                }
                let start = self.current_token.span.start;
                self.bump();
                // Parentheses are required by grammar: T_HALT_COMPILER '(' ')' ';'
                if self.current_token.kind == TokenKind::OpenParen {
                    self.bump();
                } else {
                    self.errors.push(ParseError {
                        span: self.current_token.span,
                        message: "Expected '(' after __halt_compiler",
                    });
                }
                if self.current_token.kind == TokenKind::CloseParen {
                    self.bump();
                } else {
                    self.errors.push(ParseError {
                        span: self.current_token.span,
                        message: "Expected ')' after __halt_compiler(",
                    });
                }
                self.expect_semicolon();

                let end = self.current_token.span.end;
                self.arena.alloc(Stmt::HaltCompiler {
                    span: Span::new(start, end),
                })
            }
            TokenKind::Echo | TokenKind::OpenTagEcho => self.parse_echo(),
            TokenKind::Return => self.parse_return(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::Do => self.parse_do_while(),
            TokenKind::For => self.parse_for(),
            TokenKind::Foreach => self.parse_foreach(),
            TokenKind::Function => self.parse_function(&[], doc_comment),
            TokenKind::Class => self.parse_class(&[], &[], doc_comment),
            TokenKind::Interface => self.parse_interface(&[], doc_comment),
            TokenKind::Trait => self.parse_trait(&[], doc_comment),
            TokenKind::Enum => self.parse_enum(&[], doc_comment),
            TokenKind::Namespace => {
                if !top_level {
                    self.errors.push(ParseError {
                        span: self.current_token.span,
                        message: "Namespace declaration statement has to be the very first statement or after any declare call in the script",
                    });
                }
                self.parse_namespace()
            }
            TokenKind::Use => {
                if !top_level {
                    self.errors.push(ParseError {
                        span: self.current_token.span,
                        message: "Use declarations are only allowed at the top level",
                    });
                }
                self.parse_use()
            }
            TokenKind::Switch => self.parse_switch(),
            TokenKind::Try => self.parse_try(),
            TokenKind::Throw => self.parse_throw(),
            TokenKind::Const => {
                if !top_level {
                    self.errors.push(ParseError {
                        span: self.current_token.span,
                        message: "Const declarations are only allowed at the top level",
                    });
                }
                self.parse_const_stmt(&[], doc_comment)
            }
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
            TokenKind::CloseBrace => {
                self.errors.push(ParseError {
                    span: self.current_token.span,
                    message: "Unexpected '}'",
                });
                let span = self.current_token.span;
                self.bump();
                self.arena.alloc(Stmt::Error { span })
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
                let value = self
                    .arena
                    .alloc_slice_copy(self.lexer.slice(self.current_token.span));
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

        let expr = if matches!(
            self.current_token.kind,
            TokenKind::SemiColon | TokenKind::CloseTag | TokenKind::Eof | TokenKind::CloseBrace
        ) {
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

    pub(super) fn parse_block(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;

        if self.current_token.kind == TokenKind::OpenBrace {
            self.bump(); // Eat {
        } else {
            self.errors.push(crate::ast::ParseError {
                span: self.current_token.span,
                message: "Expected '{'",
            });
            return self.arena.alloc(Stmt::Error {
                span: self.current_token.span,
            });
        }

        let mut statements = bumpalo::collections::Vec::new_in(self.arena);
        while self.current_token.kind != TokenKind::CloseBrace
            && self.current_token.kind != TokenKind::Eof
        {
            statements.push(self.parse_stmt());
        }

        if self.current_token.kind == TokenKind::CloseBrace {
            self.bump();
        } else {
            self.errors.push(crate::ast::ParseError {
                span: self.current_token.span,
                message: "Missing '}'",
            });
        }

        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::Block {
            statements: statements.into_bump_slice(),
            span: Span::new(start, end),
        })
    }

    fn parse_namespace(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat namespace

        let name = if self.current_token.kind == TokenKind::Identifier
            || self.current_token.kind == TokenKind::NsSeparator
            || self.current_token.kind == TokenKind::Namespace
        {
            Some(self.parse_name())
        } else {
            None
        };

        let body = if self.current_token.kind == TokenKind::OpenBrace {
            self.bump();
            let mut statements = bumpalo::collections::Vec::new_in(self.arena);
            while self.current_token.kind != TokenKind::CloseBrace
                && self.current_token.kind != TokenKind::Eof
            {
                statements.push(self.parse_top_stmt());
            }
            if self.current_token.kind == TokenKind::CloseBrace {
                self.bump();
            } else {
                self.errors.push(crate::ast::ParseError {
                    span: self.current_token.span,
                    message: "Missing '}'",
                });
            }
            Some(statements.into_bump_slice() as &'ast [StmtId<'ast>])
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
            if matches!(
                self.current_token.kind,
                TokenKind::Function | TokenKind::Const
            ) {
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
                while self.current_token.kind != TokenKind::CloseBrace
                    && self.current_token.kind != TokenKind::Eof
                {
                    let mut element_kind = item_kind;
                    if matches!(
                        self.current_token.kind,
                        TokenKind::Function | TokenKind::Const
                    ) {
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

                    let full_name = crate::ast::Name {
                        parts: self.arena.alloc_slice_copy(&full_parts),
                        span: Span::new(prefix.span.start, suffix.span.end),
                    };

                    uses.push(UseItem {
                        name: full_name,
                        alias,
                        kind: element_kind,
                        span: Span::new(
                            prefix.span.start,
                            alias.map(|a| a.span.end).unwrap_or(suffix.span.end),
                        ),
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
                    self.errors.push(crate::ast::ParseError {
                        span: self.current_token.span,
                        message: "Missing '}'",
                    });
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
                    span: Span::new(
                        prefix.span.start,
                        alias.map(|a| a.span.end).unwrap_or(prefix.span.end),
                    ),
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

    fn parse_try(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat try

        let body_stmt = self.parse_block();
        let body: &'ast [StmtId<'ast>] = match body_stmt {
            Stmt::Block { statements, .. } => statements,
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
                Stmt::Block { statements, .. } => statements,
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

    fn parse_const_stmt(
        &mut self,
        attributes: &'ast [AttributeGroup<'ast>],
        doc_comment: Option<Span>,
    ) -> StmtId<'ast> {
        let start = if let Some(doc) = doc_comment {
            doc.start
        } else if let Some(first) = attributes.first() {
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
                self.errors.push(crate::ast::ParseError {
                    span: self.current_token.span,
                    message: "Expected identifier",
                });
                self.arena.alloc(Token {
                    kind: TokenKind::Error,
                    span: self.current_token.span,
                })
            };

            if self.current_token.kind == TokenKind::Eq {
                self.bump();
            } else {
                self.errors.push(crate::ast::ParseError {
                    span: self.current_token.span,
                    message: "Expected '='",
                });
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
            doc_comment,
            span: Span::new(start, end),
        })
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
                if self.current_token.kind == TokenKind::CloseParen {
                    break;
                }
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
}
