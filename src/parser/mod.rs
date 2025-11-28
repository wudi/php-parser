use bumpalo::Bump;
use crate::lexer::{Lexer, LexerMode, token::{Token, TokenKind}};
use crate::ast::{Program, Stmt, StmtId, Expr, ExprId, BinaryOp, UnaryOp, Param, Arg, ArrayItem, ClassMember, Case, Catch};



use crate::span::Span;

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
}

impl<'src, 'ast> Parser<'src, 'ast> {
    pub fn new(mut lexer: Lexer<'src>, arena: &'ast Bump) -> Self {
        let current_token = lexer.next().unwrap_or(Token {
            kind: TokenKind::Eof,
            span: Span::default(),
        });
        
        Self {
            lexer,
            arena,
            current_token,
        }
    }

    fn bump(&mut self) {
        self.current_token = self.lexer.next().unwrap_or(Token {
            kind: TokenKind::Eof,
            span: Span::default(),
        });
    }

    pub fn parse_program(&mut self) -> Program<'ast> {
        let mut statements = std::vec::Vec::new(); // Temporary vec, will be moved to arena
        
        while self.current_token.kind != TokenKind::Eof {
            statements.push(self.parse_stmt());
        }

        Program {
            statements: self.arena.alloc_slice_copy(&statements),
            span: Span::default(), // TODO: Calculate full span
        }
    }

    fn parse_stmt(&mut self) -> StmtId<'ast> {
        match self.current_token.kind {
            TokenKind::Echo => self.parse_echo(),
            TokenKind::Return => self.parse_return(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            TokenKind::Foreach => self.parse_foreach(),
            TokenKind::Function => self.parse_function(),
            TokenKind::Class => self.parse_class(),
            TokenKind::Switch => self.parse_switch(),
            TokenKind::Try => self.parse_try(),
            TokenKind::Throw => self.parse_throw(),
            TokenKind::OpenBrace => self.parse_block(),
            TokenKind::OpenTag => {
                self.bump();
                self.parse_stmt() // Skip open tag
            }
            _ => {
                // Assume expression statement
                let start = self.current_token.span.start;
                let expr = self.parse_expr(0);
                if self.current_token.kind == TokenKind::SemiColon {
                    self.bump();
                }
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

        if self.current_token.kind == TokenKind::SemiColon {
            self.bump();
        }

        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::Echo {
            exprs: self.arena.alloc_slice_copy(&exprs),
            span: Span::new(start, end),
        })
    }

    fn parse_return(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump();
        
        let expr = if self.current_token.kind == TokenKind::SemiColon {
            None
        } else {
            Some(self.parse_expr(0))
        };

        if self.current_token.kind == TokenKind::SemiColon {
            self.bump();
        }
        
        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::Return {
            expr,
            span: Span::new(start, end),
        })
    }

    fn parse_block(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat {

        let mut statements = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
            statements.push(self.parse_stmt());
        }

        if self.current_token.kind == TokenKind::CloseBrace {
            self.bump();
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

        if self.current_token.kind == TokenKind::OpenParen {
            self.bump();
        }
        let condition = self.parse_expr(0);
        if self.current_token.kind == TokenKind::CloseParen {
            self.bump();
        }

        let then_stmt = self.parse_stmt();
        let then_block: &'ast [StmtId<'ast>] = match then_stmt {
            Stmt::Block { statements, .. } => statements,
            _ => self.arena.alloc_slice_copy(&[then_stmt]),
        };

        let else_block = if self.current_token.kind == TokenKind::Else {
            self.bump();
            let else_stmt = self.parse_stmt();
            match else_stmt {
                Stmt::Block { statements, .. } => Some(*statements),
                _ => Some(self.arena.alloc_slice_copy(&[else_stmt]) as &'ast [StmtId<'ast>]),
            }
        } else {
            None
        };

        // TODO: Handle elseif

        let end = self.current_token.span.end; // Approximate

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

        let body_stmt = self.parse_stmt();
        let body: &'ast [StmtId<'ast>] = match body_stmt {
            Stmt::Block { statements, .. } => statements,
            _ => self.arena.alloc_slice_copy(&[body_stmt]),
        };

        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::While {
            condition,
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

        let body_stmt = self.parse_stmt();
        let body: &'ast [StmtId<'ast>] = match body_stmt {
            Stmt::Block { statements, .. } => statements,
            _ => self.arena.alloc_slice_copy(&[body_stmt]),
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

    fn parse_class(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
        self.bump(); // Eat class
        
        let name = if self.current_token.kind == TokenKind::Identifier {
            let token = self.arena.alloc(self.current_token);
            self.bump();
            token
        } else {
            // Error recovery
            self.arena.alloc(Token { kind: TokenKind::Error, span: Span::default() })
        };
        
        if self.current_token.kind == TokenKind::OpenBrace {
            self.bump();
        }
        
        let mut members = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
            members.push(self.parse_class_member());
        }
        
        if self.current_token.kind == TokenKind::CloseBrace {
            self.bump();
        }
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Class {
            name,
            members: self.arena.alloc_slice_copy(&members),
            span: Span::new(start, end),
        })
    }

    fn parse_class_member(&mut self) -> ClassMember<'ast> {
        let start = self.current_token.span.start;
        let mut modifiers = std::vec::Vec::new();
        
        while matches!(self.current_token.kind, 
            TokenKind::Public | TokenKind::Protected | TokenKind::Private | TokenKind::Static | TokenKind::Abstract | TokenKind::Final) {
            modifiers.push(self.current_token);
            self.bump();
        }
        
        if self.current_token.kind == TokenKind::Function {
            self.bump();
            let name = if self.current_token.kind == TokenKind::Identifier {
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
            
            if self.current_token.kind == TokenKind::CloseParen {
                self.bump();
            }
            
            let body_stmt = self.parse_block();
            let body: &'ast [StmtId<'ast>] = match body_stmt {
                Stmt::Block { statements, .. } => statements,
                _ => self.arena.alloc_slice_copy(&[body_stmt]),
            };
            
            let end = self.current_token.span.end; // Approximate
            
            ClassMember::Method {
                modifiers: self.arena.alloc_slice_copy(&modifiers),
                name,
                params: self.arena.alloc_slice_copy(&params),
                body,
                span: Span::new(start, end),
            }
        } else if self.current_token.kind == TokenKind::Const {
            self.bump();
            let name = if self.current_token.kind == TokenKind::Identifier {
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
            
            if self.current_token.kind == TokenKind::SemiColon {
                self.bump();
            }
            
            let end = self.current_token.span.end;
            
            ClassMember::Const {
                name,
                value,
                span: Span::new(start, end),
            }
        } else {
            // Property
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
            
            if self.current_token.kind == TokenKind::SemiColon {
                self.bump();
            }
            
            let end = self.current_token.span.end;
            
            ClassMember::Property {
                modifiers: self.arena.alloc_slice_copy(&modifiers),
                name,
                default,
                span: Span::new(start, end),
            }
        }
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
        
        if self.current_token.kind == TokenKind::OpenBrace {
            self.bump();
        }
        
        let mut cases = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
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
            while self.current_token.kind != TokenKind::Case && self.current_token.kind != TokenKind::Default && self.current_token.kind != TokenKind::CloseBrace && self.current_token.kind != TokenKind::Eof {
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
        
        if self.current_token.kind == TokenKind::CloseBrace {
            self.bump();
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
            Stmt::Block { statements, .. } => statements,
            _ => self.arena.alloc_slice_copy(&[body_stmt]),
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
                if self.current_token.kind == TokenKind::Identifier {
                    types.push(self.current_token);
                    self.bump();
                }
                
                if self.current_token.kind == TokenKind::Pipe {
                    self.bump();
                } else {
                    break;
                }
            }
            
            let var = if self.current_token.kind == TokenKind::Variable {
                let t = self.arena.alloc(self.current_token);
                self.bump();
                t
            } else {
                self.arena.alloc(Token { kind: TokenKind::Error, span: Span::default() })
            };
            
            if self.current_token.kind == TokenKind::CloseParen {
                self.bump();
            }
            
            let catch_body_stmt = self.parse_block();
            let catch_body: &'ast [StmtId<'ast>] = match catch_body_stmt {
                Stmt::Block { statements, .. } => statements,
                _ => self.arena.alloc_slice_copy(&[catch_body_stmt]),
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
        
        if self.current_token.kind == TokenKind::SemiColon {
            self.bump();
        }
        
        let end = self.current_token.span.end;
        
        self.arena.alloc(Stmt::Throw {
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
                TokenKind::Ampersand => BinaryOp::BitAnd,
                TokenKind::Pipe => BinaryOp::BitOr,
                TokenKind::Caret => BinaryOp::BitXor,
                TokenKind::Eq => {
                    // Assignment: $a = 1
                    // Right associative, so we use a slightly lower binding power for the right side
                    // But wait, assignment is usually handled as an operator with low precedence.
                    // Let's check binding power.
                    // If we treat it as binary op, we need to ensure it's right associative.
                    // Or we can handle it here.
                    let l_bp = 5; // Very low binding power
                    if l_bp < min_bp {
                        break;
                    }
                    self.bump();
                    
                    // Right associative: pass l_bp - 1 (or same if we want right assoc)
                    // For right associativity: parse_expr(l_bp)
                    let right = self.parse_expr(l_bp);
                    
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
                    let l_bp = 30;
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
                TokenKind::Arrow => {
                    // Property Fetch or Method Call: $a->b or $a->b()
                    let l_bp = 30;
                    if l_bp < min_bp {
                        break;
                    }
                    self.bump();
                    
                    // Expect identifier or variable (for dynamic property)
                    // For now assume identifier
                    let prop_or_method = if self.current_token.kind == TokenKind::Identifier || self.current_token.kind == TokenKind::Variable {
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
                        self.bump();
                        let mut args = std::vec::Vec::new();
                        while self.current_token.kind != TokenKind::CloseParen && self.current_token.kind != TokenKind::Eof {
                            let arg_expr = self.parse_expr(0);
                            args.push(Arg {
                                value: arg_expr,
                                span: arg_expr.span(),
                            });
                            if self.current_token.kind == TokenKind::Comma {
                                self.bump();
                            }
                        }
                        if self.current_token.kind == TokenKind::CloseParen {
                            self.bump();
                        }
                        
                        let span = Span::new(left.span().start, self.current_token.span.end);
                        left = self.arena.alloc(Expr::MethodCall {
                            target: left,
                            method: prop_or_method,
                            args: self.arena.alloc_slice_copy(&args),
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
                    let l_bp = 30;
                    if l_bp < min_bp {
                        break;
                    }
                    self.bump();
                    
                    let member = if matches!(self.current_token.kind, TokenKind::Identifier | TokenKind::Variable | TokenKind::Const | TokenKind::Class) {
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
                        self.bump();
                        let mut args = std::vec::Vec::new();
                        while self.current_token.kind != TokenKind::CloseParen && self.current_token.kind != TokenKind::Eof {
                            let arg_expr = self.parse_expr(0);
                            args.push(Arg {
                                value: arg_expr,
                                span: arg_expr.span(),
                            });
                            if self.current_token.kind == TokenKind::Comma {
                                self.bump();
                            }
                        }
                        if self.current_token.kind == TokenKind::CloseParen {
                            self.bump();
                        }
                        let span = Span::new(left.span().start, self.current_token.span.end);
                        left = self.arena.alloc(Expr::StaticCall {
                            class: left,
                            method: member,
                            args: self.arena.alloc_slice_copy(&args),
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
                    let l_bp = 30;
                    if l_bp < min_bp {
                        break;
                    }
                    self.bump();
                    
                    let mut args = std::vec::Vec::new();
                    while self.current_token.kind != TokenKind::CloseParen && self.current_token.kind != TokenKind::Eof {
                        let arg_expr = self.parse_expr(0);
                        args.push(Arg {
                            value: arg_expr,
                            span: arg_expr.span(),
                        });
                        
                        if self.current_token.kind == TokenKind::Comma {
                            self.bump();
                        }
                    }
                    
                    if self.current_token.kind == TokenKind::CloseParen {
                        self.bump();
                    }
                    
                    let span = Span::new(left.span().start, self.current_token.span.end); // Approximate end
                    left = self.arena.alloc(Expr::Call {
                        func: left,
                        args: self.arena.alloc_slice_copy(&args),
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

    fn parse_nud(&mut self) -> ExprId<'ast> {
        let token = self.current_token;
        match token.kind {
            TokenKind::New => {
                self.bump();
                // Expect class name (Identifier or Variable or complex expr)
                // For now assume Identifier
                let class = self.parse_expr(30); // High binding power to grab the class name
                
                let mut args = std::vec::Vec::new();
                if self.current_token.kind == TokenKind::OpenParen {
                    self.bump();
                    while self.current_token.kind != TokenKind::CloseParen && self.current_token.kind != TokenKind::Eof {
                        let arg_expr = self.parse_expr(0);
                        args.push(Arg {
                            value: arg_expr,
                            span: arg_expr.span(),
                        });
                        if self.current_token.kind == TokenKind::Comma {
                            self.bump();
                        }
                    }
                    if self.current_token.kind == TokenKind::CloseParen {
                        self.bump();
                    }
                }
                
                let span = Span::new(token.span.start, self.current_token.span.end);
                self.arena.alloc(Expr::New {
                    class,
                    args: self.arena.alloc_slice_copy(&args),
                    span,
                })
            }
            TokenKind::Array | TokenKind::OpenBracket => {
                // Array creation: array(...) or [...]
                let start = token.span.start;
                let is_short_syntax = token.kind == TokenKind::OpenBracket;
                self.bump();
                
                let mut items = std::vec::Vec::new();
                let close_kind = if is_short_syntax { TokenKind::CloseBracket } else { TokenKind::CloseParen };
                
                if !is_short_syntax && self.current_token.kind == TokenKind::OpenParen {
                    self.bump();
                }

                while self.current_token.kind != close_kind && self.current_token.kind != TokenKind::Eof {
                    // Check for key => value
                    let first_expr = self.parse_expr(0);
                    
                    if self.current_token.kind == TokenKind::DoubleArrow { // =>
                        self.bump();
                        let value_expr = self.parse_expr(0);
                        items.push(ArrayItem {
                            key: Some(first_expr),
                            value: value_expr,
                            span: Span::new(first_expr.span().start, value_expr.span().end),
                        });
                    } else {
                        items.push(ArrayItem {
                            key: None,
                            value: first_expr,
                            span: first_expr.span(),
                        });
                    }
                    
                    if self.current_token.kind == TokenKind::Comma {
                        self.bump();
                    }
                }
                
                if self.current_token.kind == close_kind {
                    self.bump();
                }
                
                let end = self.current_token.span.end;
                self.arena.alloc(Expr::Array {
                    items: self.arena.alloc_slice_copy(&items),
                    span: Span::new(start, end),
                })
            }
            TokenKind::LNumber => {
                self.bump();
                self.arena.alloc(Expr::Integer {
                    value: self.arena.alloc_slice_copy(self.lexer.input_slice(token.span)),
                    span: token.span,
                })
            }
            TokenKind::StringLiteral => {
                self.bump();
                self.arena.alloc(Expr::String {
                    value: self.arena.alloc_slice_copy(self.lexer.input_slice(token.span)),
                    span: token.span,
                })
            }
            TokenKind::Variable => {
                self.bump();
                self.arena.alloc(Expr::Variable {
                    name: token.span,
                    span: token.span,
                })
            }
            TokenKind::Identifier => {
                // For now, treat identifier as a "bare word" string or potential constant/function name
                // In PHP, `foo()` parses `foo` as a name.
                // We need an Expr variant for Identifier/Name.
                // For now, let's reuse Variable but maybe add a flag or new variant?
                // Or just use String? No, String is quoted.
                // Let's add Expr::Identifier
                self.bump();
                // Temporary hack: reuse Variable but it's not quite right.
                // Better: Add Expr::Identifier
                self.arena.alloc(Expr::Variable { 
                    name: token.span,
                    span: token.span,
                })
            }
            TokenKind::Bang | TokenKind::Minus | TokenKind::Plus | TokenKind::Caret => {
                let op = match token.kind {
                    TokenKind::Bang => UnaryOp::Not,
                    TokenKind::Minus => UnaryOp::Minus,
                    TokenKind::Plus => UnaryOp::Plus,
                    TokenKind::Caret => UnaryOp::BitNot,
                    _ => unreachable!(),
                };
                self.bump();
                let expr = self.parse_expr(29); // High binding power for unary
                let span = Span::new(token.span.start, expr.span().end);
                self.arena.alloc(Expr::Unary {
                    op,
                    expr,
                    span,
                })
            }
            TokenKind::OpenParen => {
                self.bump();
                let expr = self.parse_expr(0);
                if self.current_token.kind == TokenKind::CloseParen {
                    self.bump();
                } else {
                    // TODO: Error handling for missing closing paren
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
            BinaryOp::Or => (10, 11),
            BinaryOp::And => (12, 13),
            BinaryOp::BitOr => (14, 15),
            BinaryOp::BitXor => (16, 17),
            BinaryOp::BitAnd => (18, 19),
            BinaryOp::EqEq | BinaryOp::NotEq | BinaryOp::EqEqEq | BinaryOp::NotEqEq => (20, 21),
            BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq => (22, 23),
            BinaryOp::Plus | BinaryOp::Minus | BinaryOp::Concat => (24, 25),
            BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod => (26, 27),
            _ => (0, 0),
        }
    }

    fn parse_param(&mut self) -> Param<'ast> {
        let start = self.current_token.span.start;
        
        // Type hint?
        let ty = if self.current_token.kind == TokenKind::Identifier {
             let t = self.arena.alloc(self.current_token);
             self.bump();
             Some(t as &'ast Token)
        } else {
            None
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
                name: param_name,
                ty,
                default,
                span: Span::new(start, end),
            }
        } else {
            // Error
            let span = Span::new(start, self.current_token.span.end);
            self.bump();
            Param {
                name: self.arena.alloc(Token { kind: TokenKind::Error, span }),
                ty: None,
                default: None,
                span,
            }
        }
    }

    fn parse_function(&mut self) -> StmtId<'ast> {
        let start = self.current_token.span.start;
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

        // Body
        let body_stmt = self.parse_stmt(); // Should be a block
        let body: &'ast [StmtId<'ast>] = match body_stmt {
            Stmt::Block { statements, .. } => statements,
            _ => self.arena.alloc_slice_copy(&[body_stmt]),
        };

        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::Function {
            name,
            params: self.arena.alloc_slice_copy(&params),
            body,
            span: Span::new(start, end),
        })
    }
}

