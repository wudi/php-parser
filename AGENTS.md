# Repository Guidelines

## Project Structure & Module Organization
- Core library entry points live in `src/lib.rs` and `src/main.rs`; the CLI helpers live in `src/bin/corpus_test.rs`.
- Parsing components are split into `src/lexer/` (tokenization), `src/parser/` (syntax + recovery), `src/ast/` (arena-backed node definitions), and `src/span.rs` (source mapping).
- Integration tests sit in `tests/` with snapshots in `tests/snapshots/`. See `ARCHITECTURE.md` for deeper design context before touching parser lifetimes or arena ownership.

## Build, Test, and Development Commands
- `cargo build` — compile the library and binaries.
- `cargo test` — run the full suite, including lexer/parser fixtures and insta snapshots.
- `cargo test snapshot_tests` — execute only snapshot-based parser expectations.
- `cargo insta review` / `cargo insta accept` — review or accept updated snapshots after parser changes.
- `cargo fmt` and `cargo clippy --all-targets --all-features` — format and lint; keep CI noise down by running both before pushing.

## Coding Style & Naming Conventions
- Rust 2024 edition, 4-space indent, follow `rustfmt` defaults; prefer small, explicit helper functions over clever macros.
- Keep arenas zero-copy: no `String`/`Vec` allocations inside AST nodes; store spans and borrow from the arena or source slices.
- Modules and files use `snake_case`; types and enums use `CamelCase`; functions and variables use `snake_case`; constants use `SCREAMING_SNAKE_CASE`.
- Maintain clear separation of lifetimes: source (`'src`) and AST/arena (`'ast`) must not mix. Add doc comments when lifetimes or safety contracts are non-obvious.

## Testing Guidelines
- Tests are integration-style (`*_tests.rs`) and rely on `insta` snapshots for parser output. Name new fixtures to describe the PHP feature under test (e.g., `lexer_heredoc.rs`).
- When parser output changes intentionally, run `cargo insta accept`, then commit the updated files in `tests/snapshots/`.
- Prefer adding a focused fixture over broad rewrites; include both “happy path” and recovery cases to keep resilience guarantees.

## Commit & Pull Request Guidelines
- Commit messages follow a conventional style seen in history (`feat:`, `fix:`, `chore:`). Use the imperative mood and keep the subject under ~72 chars.
- PRs should describe scope, rationale, and user-visible changes; link issues when applicable and call out parser or lexer behavior changes explicitly.
- Ensure `cargo fmt`, `cargo clippy`, and `cargo test` pass locally before requesting review; mention any skipped checks and why.
