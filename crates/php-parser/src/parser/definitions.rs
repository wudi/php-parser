use crate::ast::{
    Arg, AttributeGroup, ClassConst, ClassMember, Expr, ExprId, Name, Param, PropertyHook,
    PropertyHookBody, Stmt, StmtId, TraitAdaptation, TraitMethodRef, Type,
};
use crate::lexer::token::{Token, TokenKind};
use crate::parser::{ParseError, Parser};
use crate::span::Span;

#[derive(Debug, Clone, Copy)]
pub(super) enum ModifierContext {
    Method,
    Property,
    Other,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum ClassMemberCtx {
    Class {
        is_abstract: bool,
        is_readonly: bool,
    },
    Interface,
    Trait,
    Enum {
        backed: bool,
    },
}

impl<'src, 'ast> Parser<'src, 'ast> {
    pub(super) fn parse_class(
        &mut self,
        attributes: &'ast [AttributeGroup<'ast>],
        modifiers: &'ast [Token],
        doc_comment: Option<Span>,
    ) -> StmtId<'ast> {
        let start = if let Some(doc) = doc_comment {
            doc.start
        } else if let Some(first) = attributes.first() {
            first.span.start
        } else if let Some(first) = modifiers.first() {
            first.span.start
        } else {
            self.current_token.span.start
        };
        self.bump(); // Eat class

        let name = if matches!(
            self.current_token.kind,
            TokenKind::Identifier | TokenKind::Enum | TokenKind::Match
        ) {
            let token = self.arena.alloc(self.current_token);
            self.bump();
            token
        } else {
            // Error recovery
            self.arena.alloc(Token {
                kind: TokenKind::Error,
                span: Span::default(),
            })
        };

        let mut extends = None;
        if self.current_token.kind == TokenKind::Extends {
            self.bump();
            let parent = self.parse_name();
            /*
            if self.name_eq_token(&parent, name) {
                self.errors.push(ParseError {
                    span: parent.span,
                    message: "class cannot extend itself",
                });
            }
            */
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
                    self.errors.push(ParseError {
                        span: n.span,
                        message: "class cannot implement itself",
                    });
                }
                for prev in implements.iter().take(i) {
                    if self.name_eq(prev, n) {
                        self.errors.push(ParseError {
                            span: n.span,
                            message: "duplicate interface in implements list",
                        });
                        break;
                    }
                }
            }
        }

        if self.current_token.kind == TokenKind::OpenBrace {
            self.bump();
        } else {
            self.errors.push(ParseError {
                span: self.current_token.span,
                message: "Expected '{'",
            });
            return self.arena.alloc(Stmt::Class {
                attributes,
                modifiers,
                name,
                extends,
                implements: self.arena.alloc_slice_copy(&implements),
                members: &[],
                doc_comment,
                span: Span::new(start, self.current_token.span.end),
            });
        }

        let class_is_abstract = modifiers.iter().any(|m| m.kind == TokenKind::Abstract);
        let class_is_readonly = modifiers.iter().any(|m| m.kind == TokenKind::Readonly);
        self.validate_class_modifiers(modifiers);

        let mut members = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseBrace
            && self.current_token.kind != TokenKind::Eof
            && self.current_token.kind != TokenKind::CloseTag
        {
            members.push(self.parse_class_member(ClassMemberCtx::Class {
                is_abstract: class_is_abstract,
                is_readonly: class_is_readonly,
            }));
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

        self.arena.alloc(Stmt::Class {
            attributes,
            modifiers,
            name,
            extends,
            implements: self.arena.alloc_slice_copy(&implements),
            members: self.arena.alloc_slice_copy(&members),
            doc_comment,
            span: Span::new(start, end),
        })
    }

    pub(super) fn parse_anonymous_class(
        &mut self,
        attributes: &'ast [AttributeGroup<'ast>],
        modifiers: &'ast [Token],
    ) -> (ExprId<'ast>, &'ast [Arg<'ast>]) {
        let start = if let Some(attr) = attributes.first() {
            attr.span.start
        } else if let Some(m) = modifiers.first() {
            m.span.start
        } else {
            self.current_token.span.start
        };
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
                        self.errors.push(ParseError {
                            span: implements[i].span,
                            message: "duplicate interface in implements list",
                        });
                        break;
                    }
                }
            }
        }

        if self.current_token.kind == TokenKind::OpenBrace {
            self.bump();
        } else {
            self.errors.push(ParseError {
                span: self.current_token.span,
                message: "Expected '{'",
            });
            let span = Span::new(start, self.current_token.span.end);
            return (
                self.arena.alloc(Expr::AnonymousClass {
                    attributes,
                    modifiers,
                    args: ctor_args,
                    extends,
                    implements: self.arena.alloc_slice_copy(&implements),
                    members: &[],
                    span,
                }),
                ctor_args,
            );
        }

        let mut members = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseBrace
            && self.current_token.kind != TokenKind::Eof
            && self.current_token.kind != TokenKind::CloseTag
        {
            members.push(self.parse_class_member(ClassMemberCtx::Class {
                is_abstract: false,
                is_readonly: false,
            }));
        }

        if self.current_token.kind == TokenKind::CloseBrace {
            self.bump();
        } else {
            self.errors.push(ParseError {
                span: self.current_token.span,
                message: "Missing '}'",
            });
        }

        let end = self.current_token.span.end.max(ctor_end);

        (
            self.arena.alloc(Expr::AnonymousClass {
                attributes,
                modifiers,
                args: ctor_args,
                extends,
                implements: self.arena.alloc_slice_copy(&implements),
                members: self.arena.alloc_slice_copy(&members),
                span: Span::new(start, end),
            }),
            ctor_args,
        )
    }

    pub(super) fn parse_interface(
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
        self.bump(); // Eat interface

        let name = if matches!(
            self.current_token.kind,
            TokenKind::Identifier | TokenKind::Match
        ) {
            let token = self.arena.alloc(self.current_token);
            self.bump();
            token
        } else {
            self.arena.alloc(Token {
                kind: TokenKind::Error,
                span: Span::default(),
            })
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
                    self.errors.push(ParseError {
                        span: n.span,
                        message: "interface cannot extend itself",
                    });
                }
                for prev in extends.iter().take(i) {
                    if self.name_eq(prev, n) {
                        self.errors.push(ParseError {
                            span: n.span,
                            message: "duplicate interface in extends list",
                        });
                        break;
                    }
                }
            }
        }

        if self.current_token.kind == TokenKind::OpenBrace {
            self.bump();
        } else {
            self.errors.push(ParseError {
                span: self.current_token.span,
                message: "Expected '{'",
            });
            return self.arena.alloc(Stmt::Interface {
                attributes,
                name,
                extends: self.arena.alloc_slice_copy(&extends),
                members: &[],
                doc_comment,
                span: Span::new(start, self.current_token.span.end),
            });
        }

        let mut members = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseBrace
            && self.current_token.kind != TokenKind::Eof
            && self.current_token.kind != TokenKind::CloseTag
        {
            members.push(self.parse_class_member(ClassMemberCtx::Interface));
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

        self.arena.alloc(Stmt::Interface {
            attributes,
            name,
            extends: self.arena.alloc_slice_copy(&extends),
            members: self.arena.alloc_slice_copy(&members),
            doc_comment,
            span: Span::new(start, end),
        })
    }

    pub(super) fn parse_trait(
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
        self.bump(); // Eat trait

        let name = if matches!(
            self.current_token.kind,
            TokenKind::Identifier | TokenKind::Match
        ) {
            let token = self.arena.alloc(self.current_token);
            self.bump();
            token
        } else {
            self.arena.alloc(Token {
                kind: TokenKind::Error,
                span: Span::default(),
            })
        };

        if self.current_token.kind == TokenKind::OpenBrace {
            self.bump();
        } else {
            self.errors.push(ParseError {
                span: self.current_token.span,
                message: "Expected '{'",
            });
            return self.arena.alloc(Stmt::Trait {
                attributes,
                name,
                members: &[],
                doc_comment,
                span: Span::new(start, self.current_token.span.end),
            });
        }

        let mut members = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseBrace
            && self.current_token.kind != TokenKind::Eof
            && self.current_token.kind != TokenKind::CloseTag
        {
            members.push(self.parse_class_member(ClassMemberCtx::Trait));
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

        self.arena.alloc(Stmt::Trait {
            attributes,
            name,
            members: self.arena.alloc_slice_copy(&members),
            doc_comment,
            span: Span::new(start, end),
        })
    }

    pub(super) fn parse_enum(
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
        self.bump(); // Eat enum

        let name = if self.current_token.kind == TokenKind::Identifier {
            let token = self.arena.alloc(self.current_token);
            self.bump();
            token
        } else {
            self.arena.alloc(Token {
                kind: TokenKind::Error,
                span: Span::default(),
            })
        };

        let backed_type = if self.current_token.kind == TokenKind::Colon {
            self.bump();
            self.parse_type()
                .map(|t| self.arena.alloc(t) as &'ast Type<'ast>)
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
                    self.errors.push(ParseError {
                        span: n.span,
                        message: "enum cannot implement itself",
                    });
                }
                for prev in implements.iter().take(i) {
                    if self.name_eq(prev, n) {
                        self.errors.push(ParseError {
                            span: n.span,
                            message: "duplicate interface in implements list",
                        });
                        break;
                    }
                }
            }
        }

        if self.current_token.kind == TokenKind::OpenBrace {
            self.bump();
        } else {
            self.errors.push(ParseError {
                span: self.current_token.span,
                message: "Expected '{'",
            });
            return self.arena.alloc(Stmt::Enum {
                attributes,
                name,
                backed_type,
                implements: self.arena.alloc_slice_copy(&implements),
                members: &[],
                doc_comment,
                span: Span::new(start, self.current_token.span.end),
            });
        }

        let mut members = std::vec::Vec::new();
        while self.current_token.kind != TokenKind::CloseBrace
            && self.current_token.kind != TokenKind::Eof
            && self.current_token.kind != TokenKind::CloseTag
        {
            members.push(self.parse_class_member(ClassMemberCtx::Enum {
                backed: backed_type.is_some(),
            }));
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

        self.arena.alloc(Stmt::Enum {
            attributes,
            name,
            backed_type,
            implements: self.arena.alloc_slice_copy(&implements),
            members: self.arena.alloc_slice_copy(&members),
            doc_comment,
            span: Span::new(start, end),
        })
    }

    fn parse_class_member(&mut self, ctx: ClassMemberCtx) -> ClassMember<'ast> {
        let doc_comment = self.current_doc_comment;
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

        while matches!(
            self.current_token.kind,
            TokenKind::Public
                | TokenKind::Protected
                | TokenKind::Private
                | TokenKind::PublicSet
                | TokenKind::ProtectedSet
                | TokenKind::PrivateSet
                | TokenKind::Static
                | TokenKind::Abstract
                | TokenKind::Final
                | TokenKind::Readonly
        ) {
            let token = self.current_token;
            self.bump();
            modifiers.push(token);
        }
        self.validate_modifiers(&modifiers, ModifierContext::Other);
        if self.current_token.kind == TokenKind::Case {
            self.bump();
            let name = if self.current_token.kind == TokenKind::Identifier
                || self.current_token.kind.is_semi_reserved()
            {
                let token = self.arena.alloc(self.current_token);
                self.bump();
                token
            } else {
                self.arena.alloc(Token {
                    kind: TokenKind::Error,
                    span: Span::default(),
                })
            };

            let value = if self.current_token.kind == TokenKind::Eq {
                self.bump();
                Some(self.parse_expr(0))
            } else {
                None
            };

            if !matches!(ctx, ClassMemberCtx::Enum { .. }) {
                self.errors.push(ParseError {
                    span: name.span,
                    message: "case not allowed here",
                });
            } else if matches!(ctx, ClassMemberCtx::Enum { backed: true }) && value.is_none() {
                self.errors.push(ParseError {
                    span: name.span,
                    message: "backed enum cases require a value",
                });
            } else if matches!(ctx, ClassMemberCtx::Enum { backed: false }) && value.is_some() {
                self.errors.push(ParseError {
                    span: name.span,
                    message: "pure enum cases cannot have values",
                });
            }

            self.expect_semicolon();

            let end = self.current_token.span.end;
            return ClassMember::Case {
                attributes,
                name,
                value,
                doc_comment,
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
                while self.current_token.kind != TokenKind::CloseBrace
                    && self.current_token.kind != TokenKind::Eof
                {
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
                        adaptations.push(TraitAdaptation::Precedence {
                            method,
                            insteadof: self.arena.alloc_slice_copy(&insteads),
                            span: Span::new(adapt_span_start, self.current_token.span.end),
                        });
                    } else if self.current_token.kind == TokenKind::As {
                        self.bump();
                        let visibility = if matches!(
                            self.current_token.kind,
                            TokenKind::Public | TokenKind::Protected | TokenKind::Private
                        ) {
                            let v = self.arena.alloc(self.current_token);
                            self.bump();
                            Some(v)
                        } else {
                            None
                        };

                        let alias = if self.current_token.kind == TokenKind::Identifier
                            || self.current_token.kind.is_semi_reserved()
                        {
                            let a = self.arena.alloc(self.current_token);
                            self.bump();
                            Some(a)
                        } else {
                            None
                        };

                        adaptations.push(TraitAdaptation::Alias {
                            method,
                            alias: alias.map(|t| &*t),
                            visibility: visibility.map(|t| &*t),
                            span: Span::new(adapt_span_start, self.current_token.span.end),
                        });
                    } else {
                        self.errors.push(ParseError {
                            span: self.current_token.span,
                            message: "Expected insteadof or as in trait adaptation",
                        });
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
                doc_comment,
                span: Span::new(start, end),
            };
        }

        if self.current_token.kind == TokenKind::Function {
            self.bump();
            let name = if self.current_token.kind == TokenKind::Identifier
                || self.current_token.kind.is_semi_reserved()
            {
                let token = self.arena.alloc(self.current_token);
                self.bump();
                token
            } else {
                self.arena.alloc(Token {
                    kind: TokenKind::Error,
                    span: Span::default(),
                })
            };

            let params = self.parse_parameter_list();
            let return_type = self.parse_return_type();

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
                self.errors.push(ParseError {
                    span: Span::new(start, start),
                    message: "abstract method cannot have a body",
                });
            }
            if matches!(ctx, ClassMemberCtx::Interface) {
                if has_body {
                    self.errors.push(ParseError {
                        span: Span::new(start, start),
                        message: "interface methods cannot have a body",
                    });
                }
                if modifiers.iter().any(|m| {
                    matches!(
                        m.kind,
                        TokenKind::Protected | TokenKind::Private | TokenKind::Final
                    )
                }) {
                    self.errors.push(ParseError {
                        span: Span::new(start, start),
                        message: "invalid modifier in interface method",
                    });
                }
            }
            if let ClassMemberCtx::Class { is_abstract, .. } = ctx {
                if method_is_abstract && !is_abstract {
                    self.errors.push(ParseError {
                        span: Span::new(start, start),
                        message: "abstract method in non-abstract class",
                    });
                }
                if !method_is_abstract && !has_body {
                    self.errors.push(ParseError {
                        span: Span::new(start, start),
                        message: "non-abstract method must have a body",
                    });
                }
            }
            if matches!(ctx, ClassMemberCtx::Enum { .. }) && method_is_abstract {
                self.errors.push(ParseError {
                    span: Span::new(start, start),
                    message: "abstract methods not allowed in enums",
                });
            }

            if self.token_eq_ident(name, b"__construct") {
                for param in params.iter() {
                    if param.modifiers.is_empty() {
                        continue;
                    }
                    let _has_visibility = param.modifiers.iter().any(|m| {
                        matches!(
                            m.kind,
                            TokenKind::Public | TokenKind::Protected | TokenKind::Private
                        )
                    });
                    let vis_count = param
                        .modifiers
                        .iter()
                        .filter(|m| {
                            matches!(
                                m.kind,
                                TokenKind::Public | TokenKind::Protected | TokenKind::Private
                            )
                        })
                        .count();
                    let has_readonly = param
                        .modifiers
                        .iter()
                        .any(|m| m.kind == TokenKind::Readonly);
                    let readonly_count = param
                        .modifiers
                        .iter()
                        .filter(|m| m.kind == TokenKind::Readonly)
                        .count();
                    let _by_ref = param.by_ref;

                    if matches!(ctx, ClassMemberCtx::Interface) {
                        self.errors.push(ParseError {
                            span: param.span,
                            message: "property promotion not allowed in interfaces/traits",
                        });
                        continue;
                    }

                    if vis_count > 1 {
                        self.errors.push(ParseError {
                            span: param.span,
                            message: "multiple visibilities in promoted parameter",
                        });
                    }
                    // if !has_visibility {
                    //     self.errors.push(ParseError { span: param.span, message: "promoted parameter requires visibility" });
                    // }
                    // if has_readonly && !has_visibility {
                    //     self.errors.push(ParseError { span: param.span, message: "readonly promotion requires visibility" });
                    // }
                    if has_readonly && param.ty.is_none() {
                        self.errors.push(ParseError {
                            span: param.span,
                            message: "readonly promoted property requires a type",
                        });
                    }
                    if param.ty.is_none()
                        && matches!(
                            ctx,
                            ClassMemberCtx::Class {
                                is_readonly: true,
                                ..
                            }
                        )
                    {
                        self.errors.push(ParseError {
                            span: param.span,
                            message: "readonly property requires a type",
                        });
                    }
                    if readonly_count > 1 {
                        self.errors.push(ParseError {
                            span: param.span,
                            message: "Duplicate readonly modifier",
                        });
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
                params,
                return_type,
                body,
                doc_comment,
                span: Span::new(start, end),
            }
        } else if self.current_token.kind == TokenKind::Const {
            self.bump();

            let ty = self.parse_type();
            let mut const_type = None;
            let mut first_name = None;

            if let Some(t) = ty {
                if self.current_token.kind == TokenKind::Identifier
                    || self.current_token.kind.is_semi_reserved()
                {
                    const_type = Some(self.arena.alloc(t) as &'ast Type<'ast>);
                } else {
                    match t {
                        Type::Simple(token) => {
                            first_name = Some(token);
                        }
                        Type::Name(name) => {
                            if name.parts.len() == 1 {
                                first_name = Some(&name.parts[0]);
                            } else {
                                self.errors.push(ParseError {
                                    span: name.span,
                                    message: "Class constant must be an identifier",
                                });
                                first_name = Some(&name.parts[0]);
                            }
                        }
                        _ => {
                            self.errors.push(ParseError {
                                span: self.current_token.span,
                                message: "Expected identifier",
                            });
                            first_name = Some(self.arena.alloc(Token {
                                kind: TokenKind::Error,
                                span: Span::default(),
                            }));
                        }
                    }
                }
            }

            let mut consts = std::vec::Vec::new();
            let mut first = true;

            loop {
                let name = if let (true, Some(name)) = (first, first_name) {
                    name
                } else if self.current_token.kind == TokenKind::Identifier
                    || self.current_token.kind.is_semi_reserved()
                {
                    let token = self.arena.alloc(self.current_token);
                    self.bump();
                    token
                } else {
                    self.arena.alloc(Token {
                        kind: TokenKind::Error,
                        span: Span::default(),
                    })
                };
                first = false;

                if self.current_token.kind == TokenKind::Eq {
                    self.bump();
                }

                let value = self.parse_expr(0);
                consts.push(ClassConst {
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
                doc_comment,
                span: Span::new(start, end),
            }
        } else {
            // Property
            self.validate_modifiers(&modifiers, ModifierContext::Property);
            if matches!(ctx, ClassMemberCtx::Enum { .. }) {
                self.errors.push(ParseError {
                    span: Span::new(start, start),
                    message: "enums cannot declare properties",
                });
            }
            let class_is_readonly = matches!(
                ctx,
                ClassMemberCtx::Class {
                    is_readonly: true,
                    ..
                }
            );
            let mut ty = None;
            if self.current_token.kind != TokenKind::Variable
                && let Some(t) = self.parse_type()
            {
                ty = Some(self.arena.alloc(t) as &'ast Type<'ast>);
            }

            let name = if self.current_token.kind == TokenKind::Variable {
                let token = self.arena.alloc(self.current_token);
                self.bump();
                token
            } else {
                self.errors.push(ParseError {
                    span: self.current_token.span,
                    message: "Expected variable",
                });

                let is_terminator = matches!(
                    self.current_token.kind,
                    TokenKind::SemiColon
                        | TokenKind::CloseBrace
                        | TokenKind::CloseTag
                        | TokenKind::Eof
                );

                if !is_terminator {
                    self.bump();
                }

                self.arena.alloc(Token {
                    kind: TokenKind::Error,
                    span: Span::default(),
                })
            };

            let default = if self.current_token.kind == TokenKind::Eq {
                self.bump();
                Some(self.parse_expr(0))
            } else {
                None
            };

            if modifiers.iter().any(|m| m.kind == TokenKind::Readonly) && ty.is_none() {
                self.errors.push(ParseError {
                    span: Span::new(start, start),
                    message: "readonly property requires a type",
                });
            }
            if class_is_readonly && ty.is_none() {
                self.errors.push(ParseError {
                    span: Span::new(start, start),
                    message: "readonly property requires a type",
                });
            }

            // Property hooks
            if self.current_token.kind == TokenKind::OpenBrace {
                let hooks = self.parse_property_hooks();
                // self.expect_semicolon(); // Hooks do not require semicolon
                let end = self.current_token.span.end;
                ClassMember::PropertyHook {
                    attributes,
                    modifiers: self.arena.alloc_slice_copy(&modifiers),
                    ty,
                    name,
                    default,
                    hooks: self.arena.alloc_slice_copy(&hooks),
                    doc_comment,
                    span: Span::new(start, end),
                }
            } else {
                if matches!(ctx, ClassMemberCtx::Interface) {
                    self.errors.push(ParseError {
                        span: Span::new(start, start),
                        message: "interfaces cannot declare properties",
                    });
                }

                if modifiers.iter().any(|m| m.kind == TokenKind::Abstract) {
                    self.errors.push(ParseError {
                        span: modifiers.first().map(|t| t.span).unwrap_or_default(),
                        message: "Properties cannot be declared abstract",
                    });
                }

                let mut entries = std::vec::Vec::new();
                entries.push(crate::ast::PropertyEntry {
                    name,
                    default,
                    span: Span::new(
                        name.span.start,
                        default.map(|e| e.span().end).unwrap_or(name.span.end),
                    ),
                });

                while self.current_token.kind == TokenKind::Comma {
                    self.bump();
                    let name = if self.current_token.kind == TokenKind::Variable {
                        let token = self.arena.alloc(self.current_token);
                        self.bump();
                        token
                    } else {
                        self.bump();
                        self.arena.alloc(Token {
                            kind: TokenKind::Error,
                            span: Span::default(),
                        })
                    };

                    let default = if self.current_token.kind == TokenKind::Eq {
                        self.bump();
                        Some(self.parse_expr(0))
                    } else {
                        None
                    };

                    entries.push(crate::ast::PropertyEntry {
                        name,
                        default,
                        span: Span::new(
                            name.span.start,
                            default.map(|e| e.span().end).unwrap_or(name.span.end),
                        ),
                    });
                }

                self.expect_semicolon();

                let end = self.current_token.span.end;

                ClassMember::Property {
                    attributes,
                    modifiers: self.arena.alloc_slice_copy(&modifiers),
                    ty,
                    entries: self.arena.alloc_slice_copy(&entries),
                    doc_comment,
                    span: Span::new(start, end),
                }
            }
        }
    }

    fn parse_trait_method_ref(&mut self) -> TraitMethodRef<'ast> {
        let start = self.current_token.span.start;

        let name = self.parse_name();

        if self.current_token.kind == TokenKind::DoubleColon {
            self.bump(); // Eat ::

            let method = if self.current_token.kind == TokenKind::Identifier {
                let t = self.arena.alloc(self.current_token);
                self.bump();
                &*t
            } else {
                self.errors.push(ParseError {
                    span: self.current_token.span,
                    message: "Expected method name",
                });
                let t = self.arena.alloc(Token {
                    kind: TokenKind::Error,
                    span: self.current_token.span,
                });
                self.bump();
                &*t
            };

            return TraitMethodRef {
                trait_name: Some(name),
                method,
                span: Span::new(start, method.span.end),
            };
        }

        if name.parts.len() > 1 {
            self.errors.push(ParseError {
                span: name.span,
                message: "Method name cannot be qualified",
            });
        }

        let method = if let Some(first) = name.parts.first() {
            first
        } else {
            self.arena.alloc(Token {
                kind: TokenKind::Error,
                span: name.span,
            })
        };

        TraitMethodRef {
            trait_name: None,
            method,
            span: Span::new(start, method.span.end),
        }
    }

    fn parse_property_hooks(&mut self) -> Vec<PropertyHook<'ast>> {
        let mut hooks = std::vec::Vec::new();
        self.bump(); // eat {
        while self.current_token.kind != TokenKind::CloseBrace
            && self.current_token.kind != TokenKind::Eof
        {
            let mut attributes = &[] as &'ast [AttributeGroup<'ast>];
            if self.current_token.kind == TokenKind::Attribute {
                attributes = self.parse_attributes();
            }

            let mut modifiers = std::vec::Vec::new();
            while matches!(
                self.current_token.kind,
                TokenKind::Public
                    | TokenKind::Protected
                    | TokenKind::Private
                    | TokenKind::Static
                    | TokenKind::Abstract
                    | TokenKind::Final
                    | TokenKind::Readonly
            ) {
                modifiers.push(self.current_token);
                self.bump();
            }
            self.validate_modifiers(&modifiers, ModifierContext::Method);

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

            let start = self.current_token.span.start;
            let name = if self.current_token.kind == TokenKind::Identifier {
                let t = self.arena.alloc(self.current_token);
                self.bump();
                t
            } else {
                self.errors.push(ParseError {
                    span: self.current_token.span,
                    message: "Expected hook name",
                });
                let t = self.arena.alloc(Token {
                    kind: TokenKind::Error,
                    span: self.current_token.span,
                });
                self.bump();
                t
            };

            let params = if matches!(self.current_token.kind, TokenKind::OpenParen) {
                self.parse_parameter_list()
            } else {
                &[] as &'ast [Param<'ast>]
            };

            let body = match self.current_token.kind {
                TokenKind::SemiColon => {
                    self.bump();
                    PropertyHookBody::None
                }
                TokenKind::OpenBrace => {
                    let stmt = self.parse_block();
                    match stmt {
                        Stmt::Block { statements, .. } => PropertyHookBody::Statements(statements),
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
                    self.errors.push(ParseError {
                        span: self.current_token.span,
                        message: "Invalid property hook body",
                    });
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
                params,
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

    fn validate_modifiers(&mut self, modifiers: &[Token], ctx: ModifierContext) {
        let mut has_public = false;
        let mut has_protected = false;
        let mut has_private = false;
        let mut has_abstract = false;
        let mut has_final = false;
        let mut has_static = false;
        let mut has_readonly = false;
        let mut has_set_visibility = false;

        for m in modifiers {
            match m.kind {
                TokenKind::Public => {
                    if has_public || has_protected || has_private {
                        self.errors.push(ParseError {
                            span: m.span,
                            message: "Multiple visibility modifiers",
                        });
                    }
                    has_public = true;
                }
                TokenKind::Protected => {
                    if has_public || has_protected || has_private {
                        self.errors.push(ParseError {
                            span: m.span,
                            message: "Multiple visibility modifiers",
                        });
                    }
                    has_protected = true;
                }
                TokenKind::Private => {
                    if has_public || has_protected || has_private {
                        self.errors.push(ParseError {
                            span: m.span,
                            message: "Multiple visibility modifiers",
                        });
                    }
                    has_private = true;
                }
                TokenKind::PublicSet | TokenKind::ProtectedSet | TokenKind::PrivateSet => {
                    if has_set_visibility {
                        self.errors.push(ParseError {
                            span: m.span,
                            message: "Multiple set visibility modifiers",
                        });
                    }
                    has_set_visibility = true;
                }
                TokenKind::Abstract => {
                    if has_abstract {
                        self.errors.push(ParseError {
                            span: m.span,
                            message: "Duplicate abstract modifier",
                        });
                    }
                    has_abstract = true;
                }
                TokenKind::Final => {
                    if has_final {
                        self.errors.push(ParseError {
                            span: m.span,
                            message: "Duplicate final modifier",
                        });
                    }
                    has_final = true;
                }
                TokenKind::Static => {
                    if has_static {
                        self.errors.push(ParseError {
                            span: m.span,
                            message: "Duplicate static modifier",
                        });
                    }
                    has_static = true;
                }
                TokenKind::Readonly => {
                    if has_readonly {
                        self.errors.push(ParseError {
                            span: m.span,
                            message: "Duplicate readonly modifier",
                        });
                    }
                    has_readonly = true;
                }
                _ => {}
            }
        }

        if has_abstract && has_final {
            self.errors.push(ParseError {
                span: modifiers.first().map(|t| t.span).unwrap_or_default(),
                message: "abstract and final cannot be combined",
            });
        }

        // readonly is only valid on properties; flag when used on methods
        if matches!(ctx, ModifierContext::Method)
            && modifiers.iter().any(|m| m.kind == TokenKind::Readonly)
        {
            self.errors.push(ParseError {
                span: modifiers.first().map(|t| t.span).unwrap_or_default(),
                message: "readonly not allowed on methods",
            });
        }

        if matches!(ctx, ModifierContext::Method)
            && modifiers.iter().any(|m| {
                matches!(
                    m.kind,
                    TokenKind::PublicSet | TokenKind::ProtectedSet | TokenKind::PrivateSet
                )
            })
        {
            self.errors.push(ParseError {
                span: modifiers.first().map(|t| t.span).unwrap_or_default(),
                message: "asymmetric visibility not allowed on methods",
            });
        }

        if matches!(ctx, ModifierContext::Property) {
            /*
            if modifiers
                .iter()
                .any(|m| matches!(m.kind, TokenKind::Abstract | TokenKind::Final))
            {
                self.errors.push(ParseError {
                    span: modifiers.first().map(|t| t.span).unwrap_or_default(),
                    message: "abstract/final not allowed on properties",
                });
            }
            */
            let has_static = modifiers.iter().any(|m| m.kind == TokenKind::Static);
            if has_static && modifiers.iter().any(|m| m.kind == TokenKind::Readonly) {
                self.errors.push(ParseError {
                    span: modifiers.first().map(|t| t.span).unwrap_or_default(),
                    message: "readonly properties cannot be static",
                });
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
                        self.errors.push(ParseError {
                            span: m.span,
                            message: "Duplicate abstract modifier",
                        });
                    }
                    seen_abstract = true;
                }
                TokenKind::Final => {
                    if seen_final {
                        self.errors.push(ParseError {
                            span: m.span,
                            message: "Duplicate final modifier",
                        });
                    }
                    seen_final = true;
                }
                TokenKind::Readonly => {
                    if seen_readonly {
                        self.errors.push(ParseError {
                            span: m.span,
                            message: "Duplicate readonly modifier",
                        });
                    }
                    seen_readonly = true;
                }
                _ => {}
            }
        }

        if seen_abstract && seen_final {
            self.errors.push(ParseError {
                span: modifiers.first().map(|t| t.span).unwrap_or_default(),
                message: "abstract and final cannot be combined",
            });
        }
    }

    fn validate_const_modifiers(&mut self, modifiers: &[Token], ctx: ClassMemberCtx) {
        let mut seen_visibility: Option<TokenKind> = None;
        let mut seen_final = false;

        for m in modifiers {
            match m.kind {
                TokenKind::Public | TokenKind::Protected | TokenKind::Private => {
                    if seen_visibility.is_some() {
                        self.errors.push(ParseError {
                            span: m.span,
                            message: "Multiple visibility modifiers",
                        });
                    }
                    if matches!(ctx, ClassMemberCtx::Interface) && m.kind != TokenKind::Public {
                        self.errors.push(ParseError {
                            span: m.span,
                            message: "Interface constants must be public",
                        });
                    }
                    seen_visibility = Some(m.kind);
                }
                TokenKind::Final => {
                    if seen_final {
                        self.errors.push(ParseError {
                            span: m.span,
                            message: "Duplicate final modifier",
                        });
                    }
                    seen_final = true;
                }
                TokenKind::Abstract => {
                    self.errors.push(ParseError {
                        span: m.span,
                        message: "abstract not allowed on class constants",
                    });
                }
                TokenKind::Static => {
                    self.errors.push(ParseError {
                        span: m.span,
                        message: "static not allowed on class constants",
                    });
                }
                TokenKind::Readonly => {
                    self.errors.push(ParseError {
                        span: m.span,
                        message: "readonly not allowed on class constants",
                    });
                }
                _ => {}
            }
        }
    }

    pub(super) fn token_eq_ident(&self, token: &Token, ident: &[u8]) -> bool {
        let slice = self.lexer.slice(token.span);
        slice.eq_ignore_ascii_case(ident)
    }

    fn name_eq(&self, a: &Name<'ast>, b: &Name<'ast>) -> bool {
        if a.parts.len() != b.parts.len() {
            return false;
        }
        a.parts.iter().zip(b.parts.iter()).all(|(x, y)| {
            self.lexer
                .slice(x.span)
                .eq_ignore_ascii_case(self.lexer.slice(y.span))
        })
    }

    fn name_eq_token(&self, name: &Name<'ast>, tok: &Token) -> bool {
        if name.parts.len() != 1 {
            return false;
        }
        self.lexer
            .slice(name.parts[0].span)
            .eq_ignore_ascii_case(self.lexer.slice(tok.span))
    }

    pub(super) fn parse_function(
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
        self.bump(); // Eat function

        // By-reference return (returns_ref)
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

        // Name (function_name: T_STRING | T_READONLY)
        let name = if self.current_token.kind == TokenKind::Identifier
            || self.current_token.kind == TokenKind::Readonly
        {
            let token = self.arena.alloc(self.current_token);
            self.bump();
            token
        } else {
            // Error: expected identifier
            self.arena.alloc(self.current_token)
        };

        // Params
        let params = self.parse_parameter_list();

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
            Stmt::Block { statements, .. } => statements,
            _ => self.arena.alloc_slice_copy(&[body_stmt]) as &'ast [StmtId<'ast>],
        };

        let end = self.current_token.span.end;

        self.arena.alloc(Stmt::Function {
            attributes,
            name,
            by_ref,
            params,
            return_type,
            body,
            doc_comment,
            span: Span::new(start, end),
        })
    }

    pub(super) fn parse_param(&mut self) -> Param<'ast> {
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
        while matches!(
            self.current_token.kind,
            TokenKind::Public | TokenKind::Protected | TokenKind::Private | TokenKind::Readonly
        ) {
            modifiers.push(self.current_token);
            self.bump();
        }

        // Type hint?
        let ty = if let Some(t) = self.parse_type() {
            Some(self.arena.alloc(t) as &'ast Type<'ast>)
        } else {
            None
        };

        let by_ref = if matches!(
            self.current_token.kind,
            TokenKind::Ampersand | TokenKind::AmpersandFollowedByVarOrVararg
        ) {
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

            let hooks = if !modifiers.is_empty() && self.current_token.kind == TokenKind::OpenBrace
            {
                Some(self.arena.alloc_slice_copy(&self.parse_property_hooks())
                    as &'ast [PropertyHook<'ast>])
            } else {
                None
            };

            let end = if let Some(hooks) = hooks {
                if let Some(last) = hooks.last() {
                    last.span.end
                } else {
                    self.current_token.span.start
                }
            } else if let Some(expr) = default {
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
                hooks,
                span: Span::new(start, end),
            }
        } else {
            // Error
            let span = Span::new(start, self.current_token.span.end);
            self.bump();
            Param {
                attributes,
                modifiers: self.arena.alloc_slice_copy(&modifiers),
                name: self.arena.alloc(Token {
                    kind: TokenKind::Error,
                    span,
                }),
                ty: None,
                default: None,
                by_ref,
                variadic,
                hooks: None,
                span,
            }
        }
    }
}
