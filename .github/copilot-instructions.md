# PHP Parser (Rust) - AI Coding Instructions

## Project Context
This is a **production-grade, fault-tolerant, zero-copy PHP parser** written in Rust.
- **Target:** PHP 8.x Grammar compliance.
- **Key Constraint:** Zero heap allocations in the AST (pure `bumpalo` arena allocation).
- **Resilience:** The parser must NEVER panic. It must produce Error nodes and recover.

## Architecture & Core Components

### 1. Memory Layout & Lifetimes (CRITICAL)
- **Strict Lifetime Separation:**
  - `'src`: Lifetime of the input source code (`&[u8]`).
  - `'ast`: Lifetime of the `Bump` arena.
- **Zero-Copy / Zero-Heap:**
  - **NEVER** use `Box<T>`, `Vec<T>`, or `String` in AST nodes.
  - **ALWAYS** use `&'ast T` (via `ExprId<'ast>`, `StmtId<'ast>`) and `&'ast [T]`.
  - **Strings:** Store as `&'src [u8]` (references to original source) or `&'ast [u8]` (arena-allocated if modified).
- **Type Aliases:**
  ```rust
  pub type ExprId<'ast> = &'ast Expr<'ast>;
  pub type StmtId<'ast> = &'ast Stmt<'ast>;
  ```

### 2. The Lexer (`src/lexer`)
- **Input:** Operates on `&[u8]` to handle mixed encodings safely.
- **State Machine:** Uses a stack for interpolation (Scripting, DoubleQuote, Heredoc).
- **Modes:** Controlled by the Parser via `LexerMode` (e.g., `LookingForProperty` changes keyword sensitivity).
- **Token:** Lightweight, contains only `TokenKind` and `Span`. No data payload.

### 3. The Parser (`src/parser`)
- **Algorithm:** Recursive Descent combined with a Pratt parser for expressions.
- **Token Source:** Decoupled via `TokenSource` trait to allow lookahead.
- **Synchronization:** On error, record the error and advance to a boundary (`;`, `}`, `)`) instead of aborting.

## Coding Conventions

### Error Handling
- **No Panics:** Use `Result` only for fatal internal errors (rare).
- **Error Nodes:** If parsing fails, return `Expr::Error { span }` or `Stmt::Error { span }`.
- **Recovery:** Implement `sync_to_stmt_boundary` to skip tokens until a safe point.

### Data Structures
- **Spans:** Use `Span { start: usize, end: usize }` (byte offsets).
- **Byte-Oriented:** Do not assume valid UTF-8 everywhere. Use `&[u8]`.

## Testing Strategy
- **Snapshot Tests:** Use `insta` to verify AST structure (`Debug` fmt).
- **Recovery Tests:** specific tests that feed invalid code and assert that:
  1. An `Error` node is produced.
  2. The parser recovers and parses the *next* statement correctly.
- **Corpus Tests:** Parse large real-world PHP projects (WordPress, Laravel) to ensure no panics.

## Example: AST Node Definition
```rust
// Correct: Arena-allocated slice, no Vec
#[derive(Debug)]
pub struct Program<'ast> {
    pub statements: &'ast [StmtId<'ast>], 
    pub span: Span,
}

// Correct: Reference to arena-allocated Expr
#[derive(Debug)]
pub enum Expr<'ast> {
    Binary {
        left: ExprId<'ast>,
        op: BinaryOp,
        right: ExprId<'ast>,
        span: Span,
    },
    // ...
}
```

## References

 - PHP Scanner at `/Users/eagle/Sourcecode/php-src/Zend/zend_language_scanner.l`
 - PHP Parser at `/Users/eagle/Sourcecode/php-src/Zend/zend_language_parser.y`