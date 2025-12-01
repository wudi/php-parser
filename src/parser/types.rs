use crate::ast::Type;
use crate::lexer::token::TokenKind;
use crate::parser::Parser;

impl<'src, 'ast> Parser<'src, 'ast> {
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
        } else if matches!(
            self.current_token.kind,
            TokenKind::Array
                | TokenKind::Static
                | TokenKind::TypeInt
                | TokenKind::TypeString
                | TokenKind::TypeBool
                | TokenKind::TypeFloat
                | TokenKind::TypeVoid
                | TokenKind::TypeObject
                | TokenKind::TypeMixed
                | TokenKind::TypeNever
                | TokenKind::TypeNull
                | TokenKind::TypeFalse
                | TokenKind::TypeTrue
                | TokenKind::TypeIterable
                | TokenKind::TypeCallable
                | TokenKind::LogicalOr
                | TokenKind::Insteadof
                | TokenKind::LogicalAnd
                | TokenKind::LogicalXor
        ) {
            let t = self.arena.alloc(self.current_token);
            self.bump();
            Some(Type::Simple(t))
        } else if matches!(
            self.current_token.kind,
            TokenKind::Namespace | TokenKind::NsSeparator | TokenKind::Identifier
        ) || self.current_token.kind.is_semi_reserved()
        {
            let name = self.parse_name();
            Some(Type::Name(name))
        } else {
            None
        }
    }

    fn parse_type_intersection(&mut self) -> Option<Type<'ast>> {
        let mut left = self.parse_type_atomic()?;

        if matches!(
            self.current_token.kind,
            TokenKind::Ampersand | TokenKind::AmpersandNotFollowedByVarOrVararg
        ) {
            // Check lookahead to distinguish from by-ref param
            if !(self.next_token.kind == TokenKind::Identifier
                || self.next_token.kind == TokenKind::Question
                || self.next_token.kind == TokenKind::OpenParen
                || self.next_token.kind == TokenKind::NsSeparator
                || self.next_token.kind.is_semi_reserved())
            {
                return Some(left);
            }

            let mut types = std::vec::Vec::new();
            types.push(left);
            while matches!(
                self.current_token.kind,
                TokenKind::Ampersand | TokenKind::AmpersandNotFollowedByVarOrVararg
            ) {
                if !(self.next_token.kind == TokenKind::Identifier
                    || self.next_token.kind == TokenKind::Question
                    || self.next_token.kind == TokenKind::OpenParen
                    || self.next_token.kind == TokenKind::NsSeparator
                    || self.next_token.kind.is_semi_reserved())
                {
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

    pub(super) fn parse_type(&mut self) -> Option<Type<'ast>> {
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
}
