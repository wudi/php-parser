use crate::ast::{Case, Expr, ExprId, Stmt, StmtId};
use crate::lexer::token::TokenKind;
use crate::parser::{Parser, Token};
use crate::span::Span;

impl<'src, 'ast> Parser<'src, 'ast> {
    pub(super) fn parse_if(&mut self) -> StmtId<'ast> {
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
            while self.current_token.kind != TokenKind::EndIf
                && self.current_token.kind != TokenKind::Else
                && self.current_token.kind != TokenKind::ElseIf
                && self.current_token.kind != TokenKind::Eof
            {
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
                while self.current_token.kind != TokenKind::EndIf
                    && self.current_token.kind != TokenKind::Eof
                {
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

    pub(super) fn parse_while(&mut self) -> StmtId<'ast> {
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
            while self.current_token.kind != TokenKind::EndWhile
                && self.current_token.kind != TokenKind::Eof
            {
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

    pub(super) fn parse_do_while(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat do

        let body_stmt = self.parse_stmt();
        let body: &'ast [StmtId<'ast>] = match body_stmt {
            Stmt::Block { statements, .. } => statements,
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

    pub(super) fn parse_for(&mut self) -> StmtId<'ast> {
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
            while self.current_token.kind != TokenKind::EndFor
                && self.current_token.kind != TokenKind::Eof
            {
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

    pub(super) fn parse_foreach(&mut self) -> StmtId<'ast> {
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
            while self.current_token.kind != TokenKind::EndForeach
                && self.current_token.kind != TokenKind::Eof
            {
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

    pub(super) fn parse_switch(&mut self) -> StmtId<'ast> {
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
        let end_token = if is_alt {
            TokenKind::EndSwitch
        } else {
            TokenKind::CloseBrace
        };

        while self.current_token.kind != end_token && self.current_token.kind != TokenKind::Eof {
            let case_start = self.current_token.span.start;
            let condition = if self.current_token.kind == TokenKind::Case {
                self.bump();
                let expr = self.parse_expr(0);
                if self.current_token.kind == TokenKind::Colon
                    || self.current_token.kind == TokenKind::SemiColon
                {
                    self.bump();
                }
                Some(expr)
            } else if self.current_token.kind == TokenKind::Default {
                self.bump();
                if self.current_token.kind == TokenKind::Colon
                    || self.current_token.kind == TokenKind::SemiColon
                {
                    self.bump();
                }
                None
            } else {
                // Error or end of switch
                break;
            };

            let mut body_stmts = std::vec::Vec::new();
            while self.current_token.kind != TokenKind::Case
                && self.current_token.kind != TokenKind::Default
                && self.current_token.kind != end_token
                && self.current_token.kind != TokenKind::Eof
            {
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

    pub(super) fn parse_break(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat break

        let level = if self.current_token.kind != TokenKind::SemiColon
            && self.current_token.kind != TokenKind::CloseTag
            && self.current_token.kind != TokenKind::Eof
            && self.current_token.kind != TokenKind::CloseBrace
        {
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

    pub(super) fn parse_continue(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat continue

        let level = if self.current_token.kind != TokenKind::SemiColon
            && self.current_token.kind != TokenKind::CloseTag
            && self.current_token.kind != TokenKind::Eof
            && self.current_token.kind != TokenKind::CloseBrace
        {
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
                self.errors.push(crate::ast::ParseError {
                    span: *span,
                    message: "break/continue level must be a positive integer",
                });
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
                self.errors.push(crate::ast::ParseError {
                    span: *span,
                    message: "break/continue level must be a positive integer",
                });
            }
        } else {
            self.errors.push(crate::ast::ParseError {
                span: expr.span(),
                message: "break/continue level must be a positive integer literal",
            });
        }
    }

    pub(super) fn parse_goto(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat goto

        let label = if self.current_token.kind == TokenKind::Identifier {
            let tok = self.arena.alloc(self.current_token);
            self.bump();
            tok
        } else {
            self.errors.push(crate::ast::ParseError {
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

    pub(super) fn parse_declare(&mut self) -> StmtId<'ast> {
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
                self.arena.alloc(Token {
                    kind: TokenKind::Error,
                    span: Span::default(),
                })
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
            while self.current_token.kind != TokenKind::EndDeclare
                && self.current_token.kind != TokenKind::Eof
            {
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
                    self.errors.push(crate::ast::ParseError {
                        span: value.span(),
                        message: "strict_types must be 0 or 1",
                    });
                }
            } else {
                self.errors.push(crate::ast::ParseError {
                    span: value.span(),
                    message: "strict_types must be an integer literal",
                });
            }
        } else if self.token_eq_ident(key, b"ticks") {
            if let Some(num) = self.int_literal_value(value) {
                if num == 0 {
                    self.errors.push(crate::ast::ParseError {
                        span: value.span(),
                        message: "ticks must be a positive integer",
                    });
                }
            } else {
                self.errors.push(crate::ast::ParseError {
                    span: value.span(),
                    message: "ticks must be an integer literal",
                });
            }
        } else if self.token_eq_ident(key, b"encoding") {
            match value {
                Expr::String { .. } => {}
                _ => self.errors.push(crate::ast::ParseError {
                    span: value.span(),
                    message: "encoding must be a string literal",
                }),
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
}
