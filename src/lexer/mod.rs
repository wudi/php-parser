pub mod token;

use token::{Token, TokenKind};
use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LexerMode {
    Standard,
    LookingForProperty,
    LookingForVarName,
}

#[derive(Debug, Clone, PartialEq)]
enum LexerState {
    Initial,
    Scripting,
    DoubleQuotes,
    Backquote,
    Heredoc(Vec<u8>),
    Nowdoc(Vec<u8>),
    HaltCompiler,
    RawData,
    VarOffset,
}

pub struct Lexer<'src> {
    input: &'src [u8],
    cursor: usize,
    state_stack: Vec<LexerState>,
    mode: LexerMode,
}

impl<'src> Lexer<'src> {
    pub fn new(input: &'src [u8]) -> Self {
        Self {
            input,
            cursor: 0,
            state_stack: vec![LexerState::Initial],
            mode: LexerMode::Standard,
        }
    }

    pub fn set_mode(&mut self, mode: LexerMode) {
        self.mode = mode;
    }

    fn peek(&self) -> Option<u8> {
        if self.cursor < self.input.len() {
            Some(self.input[self.cursor])
        } else {
            None
        }
    }

    fn advance(&mut self) {
        self.cursor += 1;
    }

    fn advance_n(&mut self, n: usize) {
        self.cursor += n;
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn read_identifier(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == b'_' {
                self.advance();
            } else if c >= 0x80 {
                 // PHP allows extended ASCII in identifiers
                 self.advance();
            } else {
                break;
            }
        }
    }

    fn read_number(&mut self) -> TokenKind {
        let mut is_float = false;
        
        // Check for hex/binary/octal
        if self.peek() == Some(b'0') {
            self.advance();
            if let Some(c) = self.peek() {
                if c == b'x' || c == b'X' {
                    self.advance();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_hexdigit() || c == b'_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    return TokenKind::LNumber;
                } else if c == b'b' || c == b'B' {
                    self.advance();
                    while let Some(c) = self.peek() {
                        if c == b'0' || c == b'1' || c == b'_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    return TokenKind::LNumber;
                } else if c == b'o' || c == b'O' {
                    self.advance();
                    while let Some(c) = self.peek() {
                        if (c >= b'0' && c <= b'7') || c == b'_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    return TokenKind::LNumber;
                }
            }
        }

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == b'_' {
                self.advance();
            } else if c == b'.' {
                if is_float {
                    break; // Already found a dot
                }
                is_float = true;
                self.advance();
            } else if c == b'e' || c == b'E' {
                is_float = true;
                self.advance();
                if let Some(next) = self.peek() {
                    if next == b'+' || next == b'-' {
                        self.advance();
                    }
                }
            } else {
                break;
            }
        }
        
        if is_float {
            TokenKind::DNumber
        } else {
            TokenKind::LNumber
        }
    }
    
    fn consume_single_line_comment(&mut self) -> TokenKind {
        while let Some(c) = self.peek() {
            if c == b'\n' || c == b'\r' {
                break;
            } else if c == b'?' && self.input.get(self.cursor + 1) == Some(&b'>') {
                // Don't consume closing tag
                break;
            }
            self.advance();
        }
        TokenKind::Comment
    }
    
    fn consume_multi_line_comment(&mut self) -> TokenKind {
        let is_doc = if self.peek() == Some(b'*') && self.input.get(self.cursor + 1) != Some(&b'/') {
            self.advance();
            true
        } else {
            false
        };
        
        while let Some(c) = self.peek() {
            if c == b'*' {
                self.advance();
                if self.peek() == Some(b'/') {
                    self.advance();
                    return if is_doc { TokenKind::DocComment } else { TokenKind::Comment };
                }
            } else {
                self.advance();
            }
        }
        
        TokenKind::Error // Unterminated comment
    }



    fn next_in_var_offset(&mut self) -> Option<Token> {
        let start = self.cursor;
        if self.cursor >= self.input.len() {
             return Some(Token { kind: TokenKind::Error, span: Span::new(start, start) });
        }
        
        let c = self.input[self.cursor];
        
        if c == b']' {
            self.advance();
            self.state_stack.pop();
            return Some(Token { kind: TokenKind::CloseBracket, span: Span::new(start, self.cursor) });
        }
        
        if c == b'$' {
            self.advance();
            if let Some(next) = self.peek() {
                if next.is_ascii_alphabetic() || next == b'_' {
                    let var_start = self.cursor - 1;
                    self.read_identifier();
                    return Some(Token { kind: TokenKind::Variable, span: Span::new(var_start, self.cursor) });
                }
            }
            // Fallback to identifier/etc if not variable?
            // PHP scanner: if $foo[bar], bar is T_STRING. if $foo[$bar], $bar is T_VARIABLE.
            // if $foo[1], 1 is T_NUM_STRING.
        }
        
        if c.is_ascii_digit() {
            self.read_number(); // This returns LNumber or DNumber, but we want NumString for LNumber?
            // Actually PHP returns T_NUM_STRING for integers in this context.
            // Let's check if it's an integer.
            // Re-read number logic but simpler since it's inside []
            // Wait, read_number handles hex/bin/oct/float.
            // T_NUM_STRING is only for decimal integers?
            // Let's check PHP: "$a[0x10]" -> 0x10 is T_NUM_STRING?
            // php -r 'print_r(token_get_all("<?php \"$a[0x10]\""));'
            // Array ( [0] => ... [3] => Array ( [0] => 266 [1] => $a ... ) [4] => [ [5] => 0 [6] => x10 [7] => ] ... )
            // It parses 0 then x10 as chars!
            // So only decimal digits are T_NUM_STRING?
            // php -r 'print_r(token_get_all("<?php \"$a[123]\""));' -> T_NUM_STRING 123.
            
            // So we should read digits.
            return Some(Token { kind: TokenKind::NumString, span: Span::new(start, self.cursor) });
        }
        
        if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 {
            self.read_identifier();
            return Some(Token { kind: TokenKind::Identifier, span: Span::new(start, self.cursor) });
        }
        
        // Any other char is just returned as is (e.g. - . etc)
        self.advance();
        
        // Map specific chars to tokens if needed, or just return Error/Char?
        // In this context, [ is not possible (nested?), ] is handled.
        // - is possible.
        // Let's return a generic token or map it.
        // But wait, if I return Error, my test maps it to UNKNOWN.
        // PHP returns CHAR for [ if it's not a variable offset start?
        // But we are IN variable offset state.
        // Wait, $foo[1]. [ is consumed before entering state?
        // No, I pushed state when I saw [.
        // But I did NOT consume [.
        // Ah!
        
        /*
                        // Check for array offset [
                        if self.peek() == Some(b'[') {
                            self.state_stack.push(LexerState::VarOffset);
                        }
        */
        
        // So the next char IS [.
        // So I need to handle [ in next_in_var_offset.
        
        if c == b'[' {
             return Some(Token { kind: TokenKind::OpenBracket, span: Span::new(start, self.cursor) });
        }

        Some(Token { kind: TokenKind::Error, span: Span::new(start, self.cursor) })
    }

    fn next_in_double_quotes(&mut self) -> Option<Token> {
        let start = self.cursor;
        if self.cursor >= self.input.len() {
             return Some(Token { kind: TokenKind::Error, span: Span::new(start, start) });
        }
        
        let char = self.input[self.cursor];
        
        match char {
            b'"' => {
                if let Some(LexerState::DoubleQuotes) = self.state_stack.last() {
                    self.advance();
                    self.state_stack.pop();
                    return Some(Token { kind: TokenKind::DoubleQuote, span: Span::new(start, self.cursor) });
                }
            },
            b'`' => {
                 if let Some(LexerState::Backquote) = self.state_stack.last() {
                    self.advance();
                    self.state_stack.pop();
                    return Some(Token { kind: TokenKind::Backtick, span: Span::new(start, self.cursor) });
                }
            },
            b'$' => {
                self.advance();
                if let Some(c) = self.peek() {
                    if c.is_ascii_alphabetic() || c == b'_' {
                        // Backtrack to $? No, we consumed it.
                        // But read_identifier expects to read identifier chars.
                        // It does not read $.
                        // So we are at the start of identifier.
                        let var_start = self.cursor - 1;
                        self.read_identifier();
                        
                        // Check for array offset [
                        if self.peek() == Some(b'[') {
                            self.state_stack.push(LexerState::VarOffset);
                        }
                        
                        return Some(Token { kind: TokenKind::Variable, span: Span::new(var_start, self.cursor) });
                    } else if c == b'{' {
                        self.advance(); // Eat {
                        return Some(Token { kind: TokenKind::DollarOpenCurlyBraces, span: Span::new(start, self.cursor) });
                    }
                }
                // Just a $ literal, continue as Encapsed
            },
            b'{' => {
                if self.input.get(self.cursor + 1) == Some(&b'$') {
                    self.advance();
                    // Do NOT consume $
                    self.state_stack.push(LexerState::Scripting);
                    return Some(Token { kind: TokenKind::CurlyOpen, span: Span::new(start, self.cursor) });
                }
            },
            _ => {}
        }
        
        // EncapsedAndWhitespace
        while let Some(c) = self.peek() {
            if c == b'"' && matches!(self.state_stack.last(), Some(LexerState::DoubleQuotes)) {
                break;
            }
            if c == b'`' && matches!(self.state_stack.last(), Some(LexerState::Backquote)) {
                break;
            }
            if c == b'$' {
                if let Some(next) = self.input.get(self.cursor + 1) {
                    if next.is_ascii_alphabetic() || *next == b'_' || *next == b'{' {
                        break;
                    }
                }
            }
            if c == b'{' {
                if self.input.get(self.cursor + 1) == Some(&b'$') {
                    break;
                }
            }
            
            if c == b'\\' {
                self.advance();
                if self.peek().is_some() {
                    self.advance();
                }
            } else {
                self.advance();
            }
        }
        
        if self.cursor > start {
            Some(Token { kind: TokenKind::EncapsedAndWhitespace, span: Span::new(start, self.cursor) })
        } else {
            // Should have matched something above or broke immediately
            // If we broke immediately (e.g. at "), we should have handled it in match char
            // But if we are at $ or { that is NOT a variable start, we should consume it.
            // Wait, if we are at $ and it fell through match char, it means it's NOT a variable.
            // So we should consume it.
            
            // My loop logic:
            // `while let Some(c) = self.peek()`
            // If `c` is `$`, check if variable. If NOT variable, consume.
            // But my loop breaks if `c == b'$'` and it IS a variable.
            // If it is NOT a variable, it continues?
            
            // Let's re-check loop:
            /*
            if c == b'$' {
                if let Some(next) ... {
                    if next.is_ascii... {
                        break;
                    }
                }
            }
            */
            // It doesn't advance if it's NOT a variable. It just falls through to `if c == b'\\' ... else self.advance()`.
            // So it advances. Correct.
            
            Some(Token { kind: TokenKind::EncapsedAndWhitespace, span: Span::new(start, self.cursor) })
        }
    }

    fn read_single_quoted(&mut self) -> TokenKind {
        while let Some(c) = self.peek() {
            if c == b'\'' {
                self.advance();
                return TokenKind::StringLiteral;
            } else if c == b'\\' {
                self.advance();
                if self.peek().is_some() {
                    self.advance(); // Skip escaped char
                }
            } else {
                self.advance();
            }
        }
        TokenKind::Error
    }

    fn read_double_quoted(&mut self, quote: u8, start_pos: usize) -> TokenKind {
        while let Some(c) = self.peek() {
            if c == quote {
                self.advance();
                return TokenKind::StringLiteral;
            } else if c == b'\\' {
                self.advance();
                if self.peek().is_some() {
                    self.advance();
                }
            } else if c == b'$' {
                if let Some(next) = self.input.get(self.cursor + 1) {
                    if next.is_ascii_alphabetic() || *next == b'_' || *next == b'{' {
                        self.cursor = start_pos + 1;
                        self.state_stack.push(if quote == b'"' { LexerState::DoubleQuotes } else { LexerState::Backquote });
                        return if quote == b'"' { TokenKind::DoubleQuote } else { TokenKind::Backtick };
                    }
                }
                self.advance();
            } else if c == b'{' {
                 if self.input.get(self.cursor + 1) == Some(&b'$') {
                     self.cursor = start_pos + 1;
                     self.state_stack.push(if quote == b'"' { LexerState::DoubleQuotes } else { LexerState::Backquote });
                     return if quote == b'"' { TokenKind::DoubleQuote } else { TokenKind::Backtick };
                 }
                 self.advance();
            } else {
                self.advance();
            }
        }
        TokenKind::Error
    }

    fn read_heredoc_start(&mut self, start: usize) -> Token {
        while let Some(c) = self.peek() {
            if c == b' ' || c == b'\t' {
                self.advance();
            } else {
                break;
            }
        }
        
        let quote = self.peek();
        let is_quoted = quote == Some(b'\'') || quote == Some(b'"');
        let is_nowdoc = quote == Some(b'\'');
        
        if is_quoted {
            self.advance();
        }
        
        let label_start = self.cursor;
        self.read_identifier();
        let label = self.input[label_start..self.cursor].to_vec();
        
        if is_quoted {
            if self.peek() == quote {
                self.advance();
            }
        }
        
        // Consume newline after label
        if let Some(c) = self.peek() {
            if c == b'\n' {
                self.advance();
            } else if c == b'\r' {
                self.advance();
                if self.peek() == Some(b'\n') {
                    self.advance();
                }
            }
        }
        
        if is_nowdoc {
            self.state_stack.push(LexerState::Nowdoc(label));
        } else {
            self.state_stack.push(LexerState::Heredoc(label));
        }
        
        Token {
            kind: TokenKind::StartHeredoc,
            span: Span::new(start, self.cursor),
        }
    }

    fn check_heredoc_end(&self, label: &[u8]) -> Option<usize> {
        let mut current = self.cursor;
        while current < self.input.len() {
            let c = self.input[current];
            if c == b' ' || c == b'\t' {
                current += 1;
            } else {
                break;
            }
        }
        
        if current + label.len() > self.input.len() {
            return None;
        }
        
        if &self.input[current..current + label.len()] == label {
            // Check what follows. Must not be a label character.
            let after = current + label.len();
            if after >= self.input.len() {
                return Some(after - self.cursor);
            }
            let c = self.input[after];
            if !c.is_ascii_alphanumeric() && c != b'_' && c < 0x80 {
                return Some(after - self.cursor);
            }
        }
        None
    }

    fn is_followed_by_var_or_vararg(&self) -> bool {
        let mut cursor = self.cursor;
        while cursor < self.input.len() {
            let c = self.input[cursor];
            if c.is_ascii_whitespace() {
                cursor += 1;
                continue;
            }
            
            // Comments
            if c == b'#' {
                // Single line comment
                while cursor < self.input.len() && self.input[cursor] != b'\n' {
                    cursor += 1;
                }
                continue;
            }
            if c == b'/' {
                if cursor + 1 < self.input.len() {
                    if self.input[cursor+1] == b'/' {
                         // Single line
                        while cursor < self.input.len() && self.input[cursor] != b'\n' {
                            cursor += 1;
                        }
                        continue;
                    } else if self.input[cursor+1] == b'*' {
                        // Multi line
                        cursor += 2;
                        while cursor < self.input.len() {
                            if self.input[cursor] == b'*' && cursor + 1 < self.input.len() && self.input[cursor+1] == b'/' {
                                cursor += 2;
                                break;
                            }
                            cursor += 1;
                        }
                        continue;
                    }
                }
            }
            
            // Check for Variable ($...)
            if c == b'$' {
                if cursor + 1 < self.input.len() {
                    let next = self.input[cursor+1];
                    if next.is_ascii_alphabetic() || next == b'_' || next >= 0x80 {
                        return true;
                    }
                }
            }
            
            // Check for Ellipsis (...)
            if c == b'.' {
                if cursor + 2 < self.input.len() && self.input[cursor+1] == b'.' && self.input[cursor+2] == b'.' {
                    return true;
                }
            }
            
            return false;
        }
        false
    }

    fn next_in_nowdoc(&mut self) -> Option<Token> {
        let label = if let Some(LexerState::Nowdoc(label)) = self.state_stack.last() {
            label.clone()
        } else {
            return None;
        };

        if self.cursor >= self.input.len() {
             return Some(Token { kind: TokenKind::Error, span: Span::new(self.cursor, self.cursor) });
        }

        let start = self.cursor;
        
        // Check if we are at the end label immediately
        if let Some(len) = self.check_heredoc_end(&label) {
            self.advance_n(len);
            self.state_stack.pop();
            
            return Some(Token { kind: TokenKind::EndHeredoc, span: Span::new(start, self.cursor) });
        }
        
        // Consume content until newline (inclusive)
        while let Some(c) = self.peek() {
            self.advance();
            if c == b'\n' {
                // Check if next line is the label
                if self.check_heredoc_end(&label).is_some() {
                    break;
                }
            }
        }
        
        Some(Token { kind: TokenKind::EncapsedAndWhitespace, span: Span::new(start, self.cursor) })
    }

    fn next_in_heredoc(&mut self) -> Option<Token> {
        let label = if let Some(LexerState::Heredoc(label)) = self.state_stack.last() {
            label.clone()
        } else {
            return None;
        };

        if self.cursor >= self.input.len() {
             return Some(Token { kind: TokenKind::Error, span: Span::new(self.cursor, self.cursor) });
        }

        let start = self.cursor;
        
        // Check end label
        if let Some(len) = self.check_heredoc_end(&label) {
            self.advance_n(len);
            self.state_stack.pop();
            
            return Some(Token { kind: TokenKind::EndHeredoc, span: Span::new(start, self.cursor) });
        }
        
        // Handle interpolation
        if let Some(c) = self.peek() {
            if c == b'$' {
                self.advance();
                if let Some(next) = self.peek() {
                    if next.is_ascii_alphabetic() || next == b'_' {
                        let var_start = self.cursor - 1;
                        self.read_identifier();
                        return Some(Token { kind: TokenKind::Variable, span: Span::new(var_start, self.cursor) });
                    } else if next == b'{' {
                        self.advance();
                        return Some(Token { kind: TokenKind::DollarOpenCurlyBraces, span: Span::new(start, self.cursor) });
                    }
                }
            } else if c == b'{' {
                if self.input.get(self.cursor + 1) == Some(&b'$') {
                    self.advance();
                    self.state_stack.push(LexerState::Scripting);
                    return Some(Token { kind: TokenKind::CurlyOpen, span: Span::new(start, self.cursor) });
                }
            }
        }
        
        // Consume content
        while let Some(c) = self.peek() {
            if c == b'$' {
                if let Some(next) = self.input.get(self.cursor + 1) {
                    if next.is_ascii_alphabetic() || *next == b'_' || *next == b'{' {
                        break;
                    }
                }
            }
            if c == b'{' {
                if self.input.get(self.cursor + 1) == Some(&b'$') {
                    break;
                }
            }
            
            self.advance();
            if c == b'\n' {
                if self.check_heredoc_end(&label).is_some() {
                    break;
                }
            }
            
            if c == b'\\' {
                 if self.peek().is_some() {
                     self.advance();
                 }
            }
        }
        
        if self.cursor > start {
            Some(Token { kind: TokenKind::EncapsedAndWhitespace, span: Span::new(start, self.cursor) })
        } else {
            // Should have matched something above
             Some(Token { kind: TokenKind::EncapsedAndWhitespace, span: Span::new(start, self.cursor) })
        }
    }

    fn next_in_halt_compiler(&mut self) -> Option<Token> {
        self.skip_whitespace();
        
        if self.cursor >= self.input.len() {
            return Some(Token {
                kind: TokenKind::Eof,
                span: Span::new(self.cursor, self.cursor),
            });
        }

        let start = self.cursor;
        let c = self.input[self.cursor];
        self.advance();

        let kind = match c {
            b'(' => TokenKind::OpenParen,
            b')' => TokenKind::CloseParen,
            b';' => {
                self.state_stack.pop();
                self.state_stack.push(LexerState::RawData);
                TokenKind::SemiColon
            },
            b'#' => self.consume_single_line_comment(),
            b'/' => {
                if self.peek() == Some(b'/') {
                    self.advance();
                    self.consume_single_line_comment()
                } else if self.peek() == Some(b'*') {
                    self.advance();
                    self.consume_multi_line_comment()
                } else {
                    TokenKind::Error
                }
            },
            _ => TokenKind::Error,
        };

        Some(Token {
            kind,
            span: Span::new(start, self.cursor),
        })
    }

    pub fn input_slice(&self, span: Span) -> &'src [u8] {
        &self.input[span.start..span.end]
    }
}

impl<'src> Iterator for Lexer<'src> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        // Handle initial state (looking for <?php)
        if let Some(LexerState::Initial) = self.state_stack.last() {
            let start = self.cursor;
            while self.cursor < self.input.len() {
                if self.input[self.cursor..].starts_with(b"<?php") {
                    if self.cursor > start {
                        return Some(Token {
                            kind: TokenKind::InlineHtml,
                            span: Span::new(start, self.cursor),
                        });
                    }
                    
                    let tag_start = self.cursor;
                    self.state_stack.pop();
                    self.state_stack.push(LexerState::Scripting);
                    self.advance_n(5);
                    
                    // Check for trailing newline/whitespace after <?php
                    if let Some(c) = self.peek() {
                        if c.is_ascii_whitespace() {
                            self.advance();
                        }
                    }
                    
                    return Some(Token {
                        kind: TokenKind::OpenTag,
                        span: Span::new(tag_start, self.cursor),
                    });
                } else if self.input[self.cursor..].starts_with(b"<?=") {
                     if self.cursor > start {
                        return Some(Token {
                            kind: TokenKind::InlineHtml,
                            span: Span::new(start, self.cursor),
                        });
                    }
                    let tag_start = self.cursor;
                    self.state_stack.pop();
                    self.state_stack.push(LexerState::Scripting);
                    self.advance_n(3);
                    return Some(Token {
                        kind: TokenKind::OpenTagEcho,
                        span: Span::new(tag_start, self.cursor),
                    });
                }
                self.advance();
            }
            
            if self.cursor > start {
                return Some(Token {
                    kind: TokenKind::InlineHtml,
                    span: Span::new(start, self.cursor),
                });
            }
            
            return Some(Token {
                kind: TokenKind::Eof,
                span: Span::new(self.cursor, self.cursor),
            });
        }

        // Handle DoubleQuotes/Backquote state
        if let Some(LexerState::DoubleQuotes) | Some(LexerState::Backquote) = self.state_stack.last() {
            return self.next_in_double_quotes();
        }
        
        if let Some(LexerState::Heredoc(_)) = self.state_stack.last() {
            return self.next_in_heredoc();
        }
        
        if let Some(LexerState::Nowdoc(_)) = self.state_stack.last() {
            return self.next_in_nowdoc();
        }

        if let Some(LexerState::HaltCompiler) = self.state_stack.last() {
            return self.next_in_halt_compiler();
        }

        if let Some(LexerState::VarOffset) = self.state_stack.last() {
            return self.next_in_var_offset();
        }

        if let Some(LexerState::RawData) = self.state_stack.last() {
             if self.cursor >= self.input.len() {
                 return Some(Token {
                    kind: TokenKind::Eof,
                    span: Span::new(self.cursor, self.cursor),
                });
             }
             let start = self.cursor;
             self.cursor = self.input.len(); // Consume all
             return Some(Token {
                 kind: TokenKind::InlineHtml,
                 span: Span::new(start, self.cursor),
             });
        }

        self.skip_whitespace();

        if self.cursor >= self.input.len() {
            return Some(Token {
                kind: TokenKind::Eof,
                span: Span::new(self.cursor, self.cursor),
            });
        }

        let start = self.cursor;
        let char = self.input[self.cursor];
        self.advance();

        let kind = match char {
            b'$' => {
                if let Some(c) = self.peek() {
                    if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 {
                        self.read_identifier();
                        TokenKind::Variable
                    } else {
                        TokenKind::Dollar
                    }
                } else {
                    TokenKind::Dollar
                }
            },
            b'\\' => TokenKind::NsSeparator,
            b'\'' => self.read_single_quoted(),
            b'"' => self.read_double_quoted(b'"', start),
            b'`' => self.read_double_quoted(b'`', start),
            b'#' => {
                if self.peek() == Some(b'[') {
                    self.advance();
                    TokenKind::Attribute
                } else {
                    self.consume_single_line_comment()
                }
            },
            b';' => TokenKind::SemiColon,
            b':' => {
                if self.peek() == Some(b':') {
                    self.advance();
                    TokenKind::DoubleColon
                } else {
                    TokenKind::Colon
                }
            },
            b',' => TokenKind::Comma,
            b'{' => {
                self.state_stack.push(LexerState::Scripting);
                TokenKind::OpenBrace
            },
            b'}' => {
                if self.state_stack.len() > 1 {
                    self.state_stack.pop();
                }
                TokenKind::CloseBrace
            },
            b'(' => {
                // Check for cast
                let saved_cursor = self.cursor;
                self.skip_whitespace();
                
                let start_ident = self.cursor;
                self.read_identifier();
                let ident_len = self.cursor - start_ident;
                
                if ident_len > 0 {
                    let ident = &self.input[start_ident..self.cursor];
                    self.skip_whitespace();
                    if self.peek() == Some(b')') {
                        let cast_kind = match ident.to_ascii_lowercase().as_slice() {
                            b"int" | b"integer" => Some(TokenKind::IntCast),
                            b"bool" | b"boolean" => Some(TokenKind::BoolCast),
                            b"float" | b"double" | b"real" => Some(TokenKind::FloatCast),
                            b"string" | b"binary" => Some(TokenKind::StringCast),
                            b"array" => Some(TokenKind::ArrayCast),
                            b"object" => Some(TokenKind::ObjectCast),
                            b"unset" => Some(TokenKind::UnsetCast),
                            _ => None,
                        };
                        
                        if let Some(k) = cast_kind {
                            self.advance(); // Eat ')'
                            k
                        } else {
                            self.cursor = saved_cursor;
                            TokenKind::OpenParen
                        }
                    } else {
                        self.cursor = saved_cursor;
                        TokenKind::OpenParen
                    }
                } else {
                    self.cursor = saved_cursor;
                    TokenKind::OpenParen
                }
            },
            b')' => TokenKind::CloseParen,
            b'[' => TokenKind::OpenBracket,
            b']' => TokenKind::CloseBracket,
            b'+' => {
                if self.peek() == Some(b'+') {
                    self.advance();
                    TokenKind::Inc
                } else if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::PlusEq
                } else {
                    TokenKind::Plus
                }
            },
            b'-' => {
                if self.peek() == Some(b'>') {
                    self.advance();
                    TokenKind::Arrow
                } else if self.peek() == Some(b'-') {
                    self.advance();
                    TokenKind::Dec
                } else if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::MinusEq
                } else {
                    TokenKind::Minus
                }
            },
            b'*' => {
                if self.peek() == Some(b'*') {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        TokenKind::PowEq
                    } else {
                        TokenKind::Pow
                    }
                } else if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::MulEq
                } else {
                    TokenKind::Asterisk
                }
            },
            b'/' => {
                if self.peek() == Some(b'/') {
                    self.advance();
                    self.consume_single_line_comment()
                } else if self.peek() == Some(b'*') {
                    self.advance();
                    self.consume_multi_line_comment()
                } else if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::DivEq
                } else {
                    TokenKind::Slash
                }
            },
            b'%' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::ModEq
                } else {
                    TokenKind::Percent
                }
            },
            b'.' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::ConcatEq
                } else if self.peek() == Some(b'.') {
                    self.advance();
                    if self.peek() == Some(b'.') {
                        self.advance();
                        TokenKind::Ellipsis
                    } else {
                        TokenKind::Dot
                    }
                } else if let Some(c) = self.peek() && c.is_ascii_digit() {
                    self.cursor -= 1;
                    self.read_number()
                } else {
                    TokenKind::Dot
                }
            },
            b'=' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        TokenKind::EqEqEq
                    } else {
                        TokenKind::EqEq
                    }
                } else if self.peek() == Some(b'>') {
                    self.advance();
                    TokenKind::DoubleArrow
                } else {
                    TokenKind::Eq
                }
            },
            b'!' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        TokenKind::BangEqEq
                    } else {
                        TokenKind::BangEq
                    }
                } else {
                    TokenKind::Bang
                }
            },
            b'<' => {
                if self.peek() == Some(b'<') && self.input.get(self.cursor + 1) == Some(&b'<') {
                    self.advance(); // Eat second <
                    self.advance(); // Eat third <
                    return Some(self.read_heredoc_start(start));
                } else if self.peek() == Some(b'=') {
                    self.advance();
                    if self.peek() == Some(b'>') {
                        self.advance();
                        TokenKind::Spaceship
                    } else {
                        TokenKind::LtEq
                    }
                } else if self.peek() == Some(b'<') {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        TokenKind::SlEq
                    } else {
                        TokenKind::Sl
                    }
                } else if self.peek() == Some(b'>') {
                    self.advance();
                    TokenKind::BangEq
                } else {
                    TokenKind::Lt
                }
            },
            b'>' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::GtEq
                } else if self.peek() == Some(b'>') {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        TokenKind::SrEq
                    } else {
                        TokenKind::Sr
                    }
                } else {
                    TokenKind::Gt
                }
            },
            b'&' => {
                if self.peek() == Some(b'&') {
                    self.advance();
                    TokenKind::AmpersandAmpersand
                } else if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::AndEq
                } else {
                    if self.is_followed_by_var_or_vararg() {
                        TokenKind::AmpersandFollowedByVarOrVararg
                    } else {
                        TokenKind::AmpersandNotFollowedByVarOrVararg
                    }
                }
            },
            b'|' => {
                if self.peek() == Some(b'|') {
                    self.advance();
                    TokenKind::PipePipe
                } else if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::OrEq
                } else {
                    TokenKind::Pipe
                }
            },
            b'^' => {
                if self.peek() == Some(b'=') {
                    self.advance();
                    TokenKind::XorEq
                } else {
                    TokenKind::Caret
                }
            },
            b'~' => TokenKind::BitNot,
            b'@' => TokenKind::At,
            b'?' => {
                if self.peek() == Some(b'>') {
                    self.advance();
                    self.state_stack.pop();
                    self.state_stack.push(LexerState::Initial);
                    TokenKind::CloseTag
                } else if self.peek() == Some(b'?') {
                    self.advance();
                    if self.peek() == Some(b'=') {
                        self.advance();
                        TokenKind::CoalesceEq
                    } else {
                        TokenKind::Coalesce
                    }
                } else if self.peek() == Some(b'-') && self.input.get(self.cursor + 1) == Some(&b'>') {
                    self.advance();
                    self.advance();
                    TokenKind::NullSafeArrow
                } else {
                    TokenKind::Question
                }
            },
            c if c.is_ascii_digit() => {
                self.cursor -= 1;
                self.read_number()
            },
            c if c.is_ascii_alphabetic() || c == b'_' || c >= 0x80 => {
                // Check for binary string prefix
                if c == b'b' || c == b'B' {
                    if let Some(next) = self.peek() {
                        if next == b'\'' {
                            self.advance(); // Eat '
                            return Some(Token {
                                kind: self.read_single_quoted(),
                                span: Span::new(start, self.cursor),
                            });
                        } else if next == b'"' {
                            let quote_pos = self.cursor;
                            self.advance(); // Eat "
                            return Some(Token {
                                kind: self.read_double_quoted(b'"', quote_pos),
                                span: Span::new(start, self.cursor),
                            });
                        }
                    }
                }

                self.read_identifier();
                let text = &self.input[start..self.cursor];
                
                if self.mode == LexerMode::LookingForProperty {
                    self.mode = LexerMode::Standard;
                    TokenKind::Identifier
                } else {
                    match text.to_ascii_lowercase().as_slice() {
                    b"or" => TokenKind::LogicalOr,
                    b"and" => TokenKind::LogicalAnd,
                    b"xor" => TokenKind::LogicalXor,
                    b"bool" => TokenKind::TypeBool,
                    b"int" => TokenKind::TypeInt,
                    b"float" => TokenKind::TypeFloat,
                    b"string" => TokenKind::TypeString,
                    b"mixed" => TokenKind::TypeMixed,
                    b"never" => TokenKind::TypeNever,
                    b"null" => TokenKind::TypeNull,
                    b"false" => TokenKind::TypeFalse,
                    b"true" => TokenKind::TypeTrue,
                        b"exit" => TokenKind::Exit,
                    b"die" => TokenKind::Exit,
                    b"function" => TokenKind::Function,
                    b"const" => TokenKind::Const,
                    b"return" => TokenKind::Return,
                    b"yield" => {
                        if self.input[self.cursor..].starts_with(b" from") {
                            TokenKind::Yield
                        } else {
                            TokenKind::Yield
                        }
                    },
                    b"try" => TokenKind::Try,
                    b"catch" => TokenKind::Catch,
                    b"finally" => TokenKind::Finally,
                    b"throw" => TokenKind::Throw,
                    b"if" => TokenKind::If,
                    b"elseif" => TokenKind::ElseIf,
                    b"endif" => TokenKind::If,
                    b"else" => TokenKind::Else,
                    b"while" => TokenKind::While,
                    b"endwhile" => TokenKind::While,
                    b"do" => TokenKind::Do,
                    b"for" => TokenKind::For,
                    b"endfor" => TokenKind::For,
                    b"foreach" => TokenKind::Foreach,
                    b"endforeach" => TokenKind::Foreach,
                    b"declare" => TokenKind::Declare,
                    b"enddeclare" => TokenKind::Declare,
                    b"instanceof" => TokenKind::InstanceOf,
                    b"as" => TokenKind::As,
                    b"switch" => TokenKind::Switch,
                    b"endswitch" => TokenKind::Switch,
                    b"case" => TokenKind::Case,
                    b"default" => TokenKind::Default,
                    b"break" => TokenKind::Break,
                    b"continue" => TokenKind::Continue,
                    b"goto" => TokenKind::Goto,
                    b"echo" => TokenKind::Echo,
                    b"print" => TokenKind::Print,
                    b"enum" => TokenKind::Enum,
                    b"class" => TokenKind::Class,
                    b"interface" => TokenKind::Interface,
                    b"trait" => TokenKind::Trait,
                    b"extends" => TokenKind::Extends,
                    b"implements" => TokenKind::Implements,
                    b"new" => TokenKind::New,
                    b"clone" => TokenKind::Clone,
                    b"var" => TokenKind::Public,
                    b"public" => TokenKind::Public,
                    b"protected" => TokenKind::Protected,
                    b"private" => TokenKind::Private,
                    b"final" => TokenKind::Final,
                    b"abstract" => TokenKind::Abstract,
                    b"static" => TokenKind::Static,
                    b"readonly" => TokenKind::Readonly,
                    b"namespace" => TokenKind::Namespace,
                    b"use" => TokenKind::Use,
                    b"global" => TokenKind::Global,
                    b"isset" => TokenKind::Isset,
                    b"empty" => TokenKind::Empty,
                    b"__halt_compiler" => {
                        self.state_stack.pop();
                        self.state_stack.push(LexerState::HaltCompiler);
                        TokenKind::HaltCompiler
                    },
                    b"__class__" => TokenKind::ClassC,
                    b"__trait__" => TokenKind::TraitC,
                    b"__function__" => TokenKind::FuncC,
                    b"__method__" => TokenKind::MethodC,
                    b"__line__" => TokenKind::Line,
                    b"__file__" => TokenKind::File,
                    b"__dir__" => TokenKind::Dir,
                    b"__namespace__" => TokenKind::NsC,
                    b"array" => TokenKind::Array,
                    b"callable" => TokenKind::TypeCallable,
                    b"iterable" => TokenKind::TypeIterable,
                    b"void" => TokenKind::TypeVoid,
                    b"object" => TokenKind::TypeObject,
                    b"match" => TokenKind::Match,
                    b"list" => TokenKind::List,
                    b"include" => TokenKind::Include,
                    b"include_once" => TokenKind::IncludeOnce,
                    b"require" => TokenKind::Require,
                    b"require_once" => TokenKind::RequireOnce,
                    b"eval" => TokenKind::Eval,
                    b"unset" => TokenKind::Unset,
                    _ => TokenKind::Identifier,
                }
                }
            },
            _ => TokenKind::Error,
        };

        Some(Token {
            kind,
            span: Span::new(start, self.cursor),
        })
    }
}
