use crate::ast::{Attribute, AttributeGroup};
use crate::lexer::token::TokenKind;
use crate::parser::Parser;
use crate::span::Span;

impl<'src, 'ast> Parser<'src, 'ast> {
    pub(super) fn parse_attributes(&mut self) -> &'ast [AttributeGroup<'ast>] {
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
}
