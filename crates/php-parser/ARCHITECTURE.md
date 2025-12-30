# PHP Parser Architecture & Design Specification (v2.0)

## 1. Overview

**Objective:** specific Build a production-grade, fault-tolerant, zero-copy PHP parser in Rust.
**Target Compliance:** PHP 8.x Grammar.
**Key Architectural Principles:**

1. **Strict Lifetime Separation:** distinct lifetimes for Source Code (`'src`) and AST/Arena (`'ast`).
2. **Pure Arena Allocation:** The AST contains *no* heap allocations (`Vec`, `String`, `Box`). All data lives in the `Bump` arena.
3. **Resilience:** The parser never panics or aborts. It produces Error Nodes and synchronizes to recover context.
4. **Byte-Oriented:** Input is processed as `&[u8]` to handle mixed encodings safely, with Spans representing byte offsets.

---

## 2. Core Data Structures

### 2.1. Spans (Source Mapping)

Spans represent byte offsets. We do not assume UTF-8 validity at the `Span` level, allowing the parser to handle binary strings or legacy encodings if needed.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self { Self { start, end } }
    
    pub fn len(&self) -> usize { self.end - self.start }
    
    /// Safely slice the source. Returns None if indices are out of bounds.
    pub fn as_str<'src>(&self, source: &'src [u8]) -> &'src [u8] {
        &source[self.start..self.end]
    }
}
```

### 2.2. Tokens (The Lexeme)

Tokens are lightweight. Complex data (identifiers, literals) are not stored in the token; only their `Span` is.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    // Keywords (Hard)
    Function, Class, If, Return,
    // Keywords (Soft/Contextual - e.g. 'readonly', 'match')
    Identifier, 
    // Literals
    LNumber, // Integer
    DNumber, // Float
    StringLiteral, // '...' or "..."
    Variable,      // $var
    // Symbols
    Arrow, // ->
    Plus,
    OpenTag, // <?php
    Eof,
}
```

---

## 3. The AST (Abstract Syntax Tree)

### 3.1. Memory Layout Strategy

To ensure high cache locality and zero heap fragmentation:

1. **No `Box<T>`:** Use references `&'ast T`.
2. **No `Vec<T>`:** Use slice references `&'ast [T]`.
3. **Handle Types:** Use type aliases to make signatures readable.

```rust
use bumpalo::Bump;

/// Lifetime 'ast: The duration the Arena exists.
pub type ExprId<'ast> = &'ast Expr<'ast>;
pub type StmtId<'ast> = &'ast Stmt<'ast>;
```

### 3.2. AST Definitions

All AST nodes include a `span` covering the entire construct.

```rust
#[derive(Debug)]
pub struct Program<'ast> {
    pub statements: &'ast [StmtId<'ast>],
    pub span: Span,
}

#[derive(Debug)]
pub enum Stmt<'ast> {
    Echo {
        exprs: &'ast [ExprId<'ast>], // Arena-backed slice
        span: Span,
    },
    Function {
        name: &'ast Token, // Reference to the identifier token
        params: &'ast [Param<'ast>],
        body: &'ast [StmtId<'ast>],
        span: Span,
    },
    /// Represents a parsing failure at the Statement level
    Error {
        span: Span,
    },
    // ...
}

#[derive(Debug)]
pub enum Expr<'ast> {
    Binary {
        left: ExprId<'ast>,
        op: BinaryOp,
        right: ExprId<'ast>,
        span: Span,
    },
    Variable {
        name: Span,
        span: Span,
    },
    /// Represents a parsing failure at the Expression level
    Error {
        span: Span,
    },
    // ...
}
```

---

## 4. The Lexer (Context & State)

The Lexer is a **state machine** that operates on `&[u8]`. It accepts hints from the Parser to handle "Soft Keywords" (e.g., treating `match` as an identifier when following `->`).

### 4.1. Lexer Modes

The parser controls the lexer's sensitivity to keywords.

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LexerMode {
    Standard,           // Normal PHP parsing
    LookingForProperty, // After '->' or '::'. Keywords become identifiers.
    LookingForVarName,  // After '$'. 
}
```

### 4.2. Lexer Implementation

```rust
pub struct Lexer<'src> {
    input: &'src [u8],
    cursor: usize,
    /// Stack for interpolation (Scripting, DoubleQuote, Heredoc)
    state_stack: Vec<LexerState>, 
    /// Current internal state (e.g. InScripting)
    internal_state: LexerState,
    /// Mode hint from Parser
    mode: LexerMode,
}

impl<'src> Lexer<'src> {
    pub fn new(input: &'src [u8]) -> Self { /* ... */ }

    /// Called by the Parser/TokenSource to change context
    pub fn set_mode(&mut self, mode: LexerMode) {
        self.mode = mode;
    }
}

impl<'src> Iterator for Lexer<'src> {
    type Item = Token;
    fn next(&mut self) -> Option<Self::Item> {
        // Logic combining internal_state + self.mode
    }
}
```

---

## 5. The Parser (Recursive Descent + Pratt)

The parser orchestrates the Lexer, the Arena, and Error Recovery.

### 5.1. Token Source Abstraction

Decouples the parser from the raw lexer, enabling lookahead (LL(k)).

```rust
pub trait TokenSource<'src> {
    fn current(&self) -> &Token;
    fn lookahead(&self, n: usize) -> &Token;
    fn bump(&mut self);
    fn set_mode(&mut self, mode: LexerMode);
}
```

### 5.2. Parser Struct

Separates input lifetime (`'src`) from output lifetime (`'ast`).

```rust
pub struct Parser<'src, 'ast, T: TokenSource<'src>> {
    tokens: T,
    arena: &'ast Bump,
    errors: Vec<ParseError>,
    /// Marker to use 'src
    _marker: std::marker::PhantomData<&'src ()>, 
}
```

### 5.3. Error Recovery Strategy

The parser uses **Error Nodes** and **Synchronization**.

1. **Expected Token Missing:** Record error, insert synthetic node/token if trivial, or return `Expr::Error`.
2. **Unexpected Token:** Record error, advance tokens until a "Synchronization Point" (`;`, `}`, `)`).

```rust
impl<'src, 'ast, T: TokenSource<'src>> Parser<'src, 'ast, T> {
    
    /// Main entry point for expressions
    fn parse_expr(&mut self, min_bp: u8) -> ExprId<'ast> {
        // Check binding power, recurse...
        // If syntax is invalid, do NOT panic.
        // self.errors.push(...);
        // return self.arena.alloc(Expr::Error { span })
    }

    /// Synchronization helper
    fn sync_to_stmt_boundary(&mut self) {
        while self.tokens.current().kind != TokenKind::Eof {
            match self.tokens.current().kind {
                TokenKind::SemiColon | TokenKind::CloseBrace => {
                    self.tokens.bump();
                    return;
                }
                _ => self.tokens.bump(),
            }
        }
    }
}
```

---

## 6. Public API

This defines the library boundary.

```rust
pub struct ParseResult<'ast> {
    pub program: Program<'ast>,
    pub errors: Vec<ParseError>,
}

/// The main entry point.
/// 
/// - `source`: Raw bytes of the PHP file.
/// - `arena`: The Bump arena where AST nodes will be allocated.
pub fn parse<'src, 'ast>(
    source: &'src [u8], 
    arena: &'ast Bump
) -> ParseResult<'ast> {
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, arena);
    parser.parse_program()
}
```

---

## 7. Development Phases

### Phase 1: Infrastructure & Basics

1. **Lexer MVP:** Implement `TokenSource`, `Lexer` with basic states (`Initial`, `Scripting`).
2. **Arena Setup:** Integrate `bumpalo`.
3. **AST Skeleton:** Define basic `Stmt` and `Expr` structs.
4. **Test Harness:** Setup `insta` for snapshot testing.

### Phase 2: Expression Engine (Pratt)

1. **Precedence Table:** Map PHP precedence to Binding Powers.
2. **Operators:** Implement Binary, Unary, Ternary, and `instanceof`.
3. **Error Nodes:** Ensure malformed math (e.g., `1 + * 2`) produces `Expr::Error`.

### Phase 3: Statements & Control Flow

1. **Block Parsing:** Handle `{ ... }` and scopes.
2. **Control Structures:** `if`, `while`, `return`.
3. **Synchronization:** Implement `sync_to_stmt_boundary` to recover from missing semicolons.

### Phase 4: Advanced Lexing

1. **Interpolation Stack:** `DoubleQuotes`, `Heredoc`, `Backticks`.
2. **Complex Identifiers:** Support `LexerMode::LookingForProperty` for `$obj->class`.

---

## 8. Testing Strategy

1. **Unit Tests:** For individual Lexer state transitions.
2. **Snapshot Tests (Insta):**
    * Input: `test.php`
    * Output: Textual representation of the AST (Debug fmt).
    * Purpose: Catch regressions in tree structure.
3. **Recovery Tests:**
    * Input: `<?php echo 1 + ; echo "done";`
    * Assert: `program.statements[0]` is `Echo(Expr::Error)`.
    * Assert: `program.statements[1]` is `Echo("done")`.
    * The parser must *not* stop at the first semicolon error.
4. **Corpus Tests:** Parse large open-source PHP projects (WordPress, Laravel) to ensure no panics on valid code.
