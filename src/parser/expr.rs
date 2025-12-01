use crate::ast::{
    Arg, ArrayItem, AssignOp, AttributeGroup, BinaryOp, CastKind, ClosureUse, Expr, ExprId,
    IncludeKind, MatchArm, ParseError, Stmt, StmtId, Type, UnaryOp,
};
use crate::lexer::token::{Token, TokenKind};
use crate::parser::Parser;
use crate::span::Span;

impl<'src, 'ast> Parser<'src, 'ast> {
    pub(super) fn parse_call_arguments(&mut self) -> (&'ast [Arg<'ast>], Span) {
        let start = self.current_token.span.start;
        if self.current_token.kind != TokenKind::OpenParen {
            return (&[], Span::default());
        }
        self.bump(); // consume (

        let mut args = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseParen
            && self.current_token.kind != TokenKind::Eof
        {
            let mut name: Option<&'ast Token> = None;
            let mut unpack = false;
            let start = self.current_token.span.start;

            // Named argument: identifier-like token followed by :
            if (self.current_token.kind == TokenKind::Identifier
                || self.current_token.kind.is_semi_reserved())
                && self.next_token.kind == TokenKind::Colon
            {
                name = Some(self.arena.alloc(self.current_token));
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
                span: Span {
                    start,
                    end: value.span().end,
                },
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

    pub(super) fn parse_closure_expr(
        &mut self,
        attributes: &'ast [AttributeGroup<'ast>],
        is_static: bool,
        start: usize,
    ) -> ExprId<'ast> {
        let _returns_by_ref = if matches!(
            self.current_token.kind,
            TokenKind::Ampersand | TokenKind::AmpersandNotFollowedByVarOrVararg
        ) {
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
        while self.current_token.kind != TokenKind::CloseParen
            && self.current_token.kind != TokenKind::Eof
        {
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
            while self.current_token.kind != TokenKind::CloseParen
                && self.current_token.kind != TokenKind::Eof
            {
                let by_ref = if matches!(
                    self.current_token.kind,
                    TokenKind::Ampersand | TokenKind::AmpersandFollowedByVarOrVararg
                ) {
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
                    self.arena.alloc(Token {
                        kind: TokenKind::Error,
                        span: Span::default(),
                    })
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
            Stmt::Block { statements, .. } => statements,
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

    pub(super) fn parse_arrow_function(
        &mut self,
        attributes: &'ast [AttributeGroup<'ast>],
        is_static: bool,
        start: usize,
    ) -> ExprId<'ast> {
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
        while self.current_token.kind != TokenKind::CloseParen
            && self.current_token.kind != TokenKind::Eof
        {
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

    pub(super) fn parse_expr(&mut self, min_bp: u8) -> ExprId<'ast> {
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
                TokenKind::Ampersand
                | TokenKind::AmpersandFollowedByVarOrVararg
                | TokenKind::AmpersandNotFollowedByVarOrVararg => BinaryOp::BitAnd,
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
                TokenKind::PlusEq
                | TokenKind::MinusEq
                | TokenKind::MulEq
                | TokenKind::DivEq
                | TokenKind::ModEq
                | TokenKind::ConcatEq
                | TokenKind::AndEq
                | TokenKind::OrEq
                | TokenKind::XorEq
                | TokenKind::SlEq
                | TokenKind::SrEq
                | TokenKind::PowEq
                | TokenKind::CoalesceEq => {
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
                    if matches!(
                        self.current_token.kind,
                        TokenKind::Ampersand
                            | TokenKind::AmpersandFollowedByVarOrVararg
                            | TokenKind::AmpersandNotFollowedByVarOrVararg
                    ) {
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

                    let end = if self.current_token.kind == TokenKind::CloseBracket {
                        let end = self.current_token.span.end;
                        self.bump();
                        end
                    } else {
                        self.current_token.span.start
                    };

                    let span = Span::new(left.span().start, end);
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

                    let prop_or_method = if matches!(
                        self.current_token.kind,
                        TokenKind::OpenBrace | TokenKind::DollarOpenCurlyBraces
                    ) {
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
                            self.arena.alloc(Expr::Variable { name: span, span })
                        } else {
                            self.arena.alloc(Expr::Error {
                                span: Span::new(start, self.current_token.span.end),
                            })
                        }
                    } else if self.current_token.kind == TokenKind::Identifier
                        || self.current_token.kind == TokenKind::Variable
                        || self.current_token.kind.is_semi_reserved()
                    {
                        let token = self.current_token;
                        self.bump();
                        self.arena.alloc(Expr::Variable {
                            name: token.span,
                            span: token.span,
                        })
                    } else {
                        self.arena.alloc(Expr::Error {
                            span: self.current_token.span,
                        })
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
                    let prop_or_method = if matches!(
                        self.current_token.kind,
                        TokenKind::OpenBrace | TokenKind::DollarOpenCurlyBraces
                    ) {
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
                            self.arena.alloc(Expr::Variable { name: span, span })
                        } else {
                            self.arena.alloc(Expr::Error {
                                span: Span::new(start, self.current_token.span.end),
                            })
                        }
                    } else if self.current_token.kind == TokenKind::Identifier
                        || self.current_token.kind == TokenKind::Variable
                        || self.current_token.kind.is_semi_reserved()
                    {
                        // We need to wrap this token in an Expr
                        // Reusing Variable/Identifier logic from parse_nud would be good but we need to call it explicitly or just handle it here
                        let token = self.current_token;
                        self.bump();
                        self.arena.alloc(Expr::Variable {
                            // Using Variable for now, should be Identifier if it's a name
                            name: token.span,
                            span: token.span,
                        })
                    } else {
                        // Error
                        self.arena.alloc(Expr::Error {
                            span: self.current_token.span,
                        })
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

                    let member = if matches!(
                        self.current_token.kind,
                        TokenKind::OpenBrace | TokenKind::DollarOpenCurlyBraces
                    ) {
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
                            self.arena.alloc(Expr::Variable { name: span, span })
                        } else {
                            self.arena.alloc(Expr::Error {
                                span: Span::new(start, self.current_token.span.end),
                            })
                        }
                    } else if self.current_token.kind == TokenKind::Identifier
                        || self.current_token.kind == TokenKind::Variable
                        || self.current_token.kind.is_semi_reserved()
                    {
                        let token = self.current_token;
                        self.bump();
                        self.arena.alloc(Expr::Variable {
                            name: token.span,
                            span: token.span,
                        })
                    } else {
                        self.arena.alloc(Expr::Error {
                            span: self.current_token.span,
                        })
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
                    left = self.arena.alloc(Expr::PostInc { var: left, span });
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
                    left = self.arena.alloc(Expr::PostDec { var: left, span });
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
            TokenKind::Isset
            | TokenKind::LogicalOr
            | TokenKind::Insteadof
            | TokenKind::LogicalAnd
            | TokenKind::LogicalXor => {
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
            TokenKind::Include
            | TokenKind::IncludeOnce
            | TokenKind::Require
            | TokenKind::RequireOnce => {
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

                if matches!(
                    self.current_token.kind,
                    TokenKind::SemiColon
                        | TokenKind::CloseTag
                        | TokenKind::Eof
                        | TokenKind::CloseBrace
                        | TokenKind::Comma
                ) {
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
                    _ => self.arena.alloc(Expr::Variable {
                        name: token.span,
                        span: token.span,
                    }),
                }
            }
            TokenKind::New => {
                self.bump();

                let attributes = if self.current_token.kind == TokenKind::Attribute {
                    self.parse_attributes()
                } else {
                    &[]
                };

                if self.current_token.kind == TokenKind::Class {
                    let (class, args) = self.parse_anonymous_class(attributes);
                    let span = Span::new(token.span.start, class.span().end);
                    self.arena.alloc(Expr::New { class, args, span })
                } else {
                    if !attributes.is_empty() {
                        self.errors.push(ParseError {
                            span: Span::new(
                                attributes.first().unwrap().span.start,
                                attributes.last().unwrap().span.end,
                            ),
                            message: "Attributes are only allowed on anonymous classes in new expression",
                        });
                    }

                    let class = self.parse_expr(200); // High binding power to grab the class name

                    let (args, end_pos) = if self.current_token.kind == TokenKind::OpenParen {
                        let (a, s) = self.parse_call_arguments();
                        (a, s.end)
                    } else {
                        (&[] as &[Arg], class.span().end)
                    };

                    let span = Span::new(token.span.start, end_pos);
                    self.arena.alloc(Expr::New { class, args, span })
                }
            }
            TokenKind::Clone => {
                self.bump();
                let expr = self.parse_expr(200);
                let span = Span::new(token.span.start, expr.span().end);
                self.arena.alloc(Expr::Clone { expr, span })
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
                while self.current_token.kind != TokenKind::CloseBrace
                    && self.current_token.kind != TokenKind::Eof
                {
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
                            let next_token = lookahead_lexer.next().unwrap_or(Token {
                                kind: TokenKind::Eof,
                                span: Span::default(),
                            });
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

                if self.current_token.kind == TokenKind::OpenBrace {
                    self.bump();
                    let expr = self.parse_expr(0);
                    let end = if self.current_token.kind == TokenKind::CloseBrace {
                        let end = self.current_token.span.end;
                        self.bump();
                        end
                    } else {
                        self.current_token.span.start
                    };

                    let span = Span::new(start, end);
                    self.arena.alloc(Expr::Variable {
                        name: expr.span(),
                        span,
                    })
                } else {
                    let expr = self.parse_expr(200);
                    let span = Span::new(start, expr.span().end);
                    self.arena.alloc(Expr::Variable {
                        name: expr.span(),
                        span,
                    })
                }
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
                self.arena.alloc(Expr::Null { span: token.span })
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
            TokenKind::Minus
            | TokenKind::Plus
            | TokenKind::BitNot
            | TokenKind::At
            | TokenKind::Inc
            | TokenKind::Dec
            | TokenKind::Ampersand
            | TokenKind::AmpersandFollowedByVarOrVararg
            | TokenKind::AmpersandNotFollowedByVarOrVararg => {
                let op = match token.kind {
                    TokenKind::Minus => UnaryOp::Minus,
                    TokenKind::Plus => UnaryOp::Plus,
                    TokenKind::BitNot => UnaryOp::BitNot,
                    TokenKind::At => UnaryOp::ErrorSuppress,
                    TokenKind::Inc => UnaryOp::PreInc,
                    TokenKind::Dec => UnaryOp::PreDec,
                    TokenKind::Ampersand
                    | TokenKind::AmpersandFollowedByVarOrVararg
                    | TokenKind::AmpersandNotFollowedByVarOrVararg => UnaryOp::Reference,
                    _ => unreachable!(),
                };
                self.bump();
                let expr = self.parse_expr(180); // BP for unary +, -, ~, ++, --
                let span = Span::new(token.span.start, expr.span().end);
                self.arena.alloc(Expr::Unary { op, expr, span })
            }
            TokenKind::IntCast
            | TokenKind::BoolCast
            | TokenKind::FloatCast
            | TokenKind::StringCast
            | TokenKind::ArrayCast
            | TokenKind::ObjectCast
            | TokenKind::UnsetCast => {
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
                self.arena.alloc(Expr::Cast { kind, expr, span })
            }
            TokenKind::Array => {
                let start = token.span.start;
                self.bump();
                if self.current_token.kind == TokenKind::OpenParen {
                    self.bump();
                }
                let mut items = std::vec::Vec::new();
                while self.current_token.kind != TokenKind::CloseParen
                    && self.current_token.kind != TokenKind::Eof
                {
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
                while self.current_token.kind != TokenKind::CloseParen
                    && self.current_token.kind != TokenKind::Eof
                {
                    if self.current_token.kind == TokenKind::Comma {
                        // Empty slot in list()
                        items.push(ArrayItem {
                            key: None,
                            value: self.arena.alloc(Expr::Error {
                                span: self.current_token.span,
                            }),
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
            TokenKind::OpenBracket => {
                // Short array syntax [1, 2, 3]
                let start = token.span.start;
                self.bump();
                let mut items = std::vec::Vec::new();
                while self.current_token.kind != TokenKind::CloseBracket
                    && self.current_token.kind != TokenKind::Eof
                {
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

            BinaryOp::Or => (60, 61),  // ||
            BinaryOp::And => (70, 71), // &&

            BinaryOp::BitOr => (80, 81),
            BinaryOp::BitXor => (90, 91),
            BinaryOp::BitAnd => (100, 101),

            BinaryOp::EqEq
            | BinaryOp::NotEq
            | BinaryOp::EqEqEq
            | BinaryOp::NotEqEq
            | BinaryOp::Spaceship => (110, 111),
            BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => (120, 121),

            BinaryOp::ShiftLeft | BinaryOp::ShiftRight => (130, 131),

            BinaryOp::Plus | BinaryOp::Minus | BinaryOp::Concat => (140, 141),
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => (150, 151),

            BinaryOp::Instanceof => (170, 171), // Non-associative usually, but let's say left for now

            BinaryOp::Pow => (191, 190), // Right associative

            _ => (0, 0),
        }
    }

    fn parse_array_item(&mut self) -> ArrayItem<'ast> {
        let unpack = if self.current_token.kind == TokenKind::Ellipsis {
            self.bump();
            true
        } else {
            false
        };

        let by_ref = if matches!(
            self.current_token.kind,
            TokenKind::Ampersand
                | TokenKind::AmpersandFollowedByVarOrVararg
                | TokenKind::AmpersandNotFollowedByVarOrVararg
        ) {
            self.bump();
            true
        } else {
            false
        };

        let expr1 = self.parse_expr(0);

        if self.current_token.kind == TokenKind::DoubleArrow {
            self.bump();
            let value_by_ref = if matches!(
                self.current_token.kind,
                TokenKind::Ampersand
                    | TokenKind::AmpersandFollowedByVarOrVararg
                    | TokenKind::AmpersandNotFollowedByVarOrVararg
            ) {
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
                                self.arena.alloc(Expr::String {
                                    value: self.arena.alloc_slice_copy(self.lexer.slice(t.span)),
                                    span: t.span,
                                }) as &'ast Expr<'ast>
                            }
                            TokenKind::NumString => {
                                let t = self.current_token;
                                self.bump();
                                self.arena.alloc(Expr::Integer {
                                    value: self.arena.alloc_slice_copy(self.lexer.slice(t.span)),
                                    span: t.span,
                                }) as &'ast Expr<'ast>
                            }
                            TokenKind::Variable => {
                                let t = self.current_token;
                                self.bump();
                                self.arena.alloc(Expr::Variable {
                                    name: t.span,
                                    span: t.span,
                                }) as &'ast Expr<'ast>
                            }
                            TokenKind::Minus => {
                                // Handle negative number?
                                self.bump();
                                if self.current_token.kind == TokenKind::NumString {
                                    let t = self.current_token;
                                    self.bump();
                                    // TODO: Combine minus and number
                                    self.arena.alloc(Expr::Integer {
                                        value: self
                                            .arena
                                            .alloc_slice_copy(self.lexer.slice(t.span)),
                                        span: t.span,
                                    }) as &'ast Expr<'ast>
                                } else {
                                    self.arena.alloc(Expr::Error {
                                        span: self.current_token.span,
                                    }) as &'ast Expr<'ast>
                                }
                            }
                            _ => {
                                // Error
                                self.arena.alloc(Expr::Error {
                                    span: self.current_token.span,
                                }) as &'ast Expr<'ast>
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
                                property: self.arena.alloc(Expr::Variable {
                                    name: prop_name.span,
                                    span: prop_name.span,
                                }),
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
