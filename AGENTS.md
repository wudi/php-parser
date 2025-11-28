# Repository Guidelines

## Project Structure & Module Organization
- Core parser pieces live in `src/lexer/`, `src/parser/`, `src/ast/`, and `src/span.rs`, wired together from `src/lib.rs`. The demo entry point is `src/main.rs`, and the corpus runner is `src/bin/corpus_test.rs`.
- Tests are integration-based under `tests/` with insta snapshots in `tests/snapshots/`. Consult `ARCHITECTURE.md` before touching arenas, spans, or lifetime plumbing.
- Keep PHP parity in mind: do not invent new syntax or AST kinds. Mirror upstream PHP tokens/AST from `zend_language_scanner.y` and `zend_ast.h`.

## Build, Test, and Development Commands
- `cargo build` — compile the library and binaries.
- `cargo test` — run the full suite, including snapshot tests and validation cases.
- `cargo test snapshot_tests` — focus on parser snapshot expectations when iterating quickly.
- `cargo insta review` / `cargo insta accept` — review or accept updated snapshots after intentional parser changes.
- `cargo fmt` and `cargo clippy --all-targets --all-features` — format and lint before pushing.
- `cargo run --release --bin corpus_test /path/to/php/project` — scan a PHP tree (e.g., `/tmp/wordpress-develop`) to measure real-world compatibility.

## Coding Style & Naming Conventions
- Rust 2024 edition, 4-space indent; follow `rustfmt` defaults and prefer small, explicit helpers.
- AST nodes are arena-backed; avoid owned `String`/`Vec` in node structs and keep spans cheap.
- Names: modules/files `snake_case`; types/enums `CamelCase`; locals/functions `snake_case`; constants `SCREAMING_SNAKE_CASE`.
- Preserve clear lifetimes (`'src` for input, `'ast` for arena) and document any safety or ownership contracts that are not obvious.

## Testing Guidelines
- Add focused fixtures per PHP feature (e.g., `enum_case_validation.rs`, `declare_alt.rs`) and include both success and recovery cases.
- When output shifts, run `cargo insta accept` and commit updated files in `tests/snapshots/`.
- For semantic checks (declare literals, break/continue levels, inheritance rules), prefer targeted rust tests over broad corpus expectations.
- Compare tricky behaviors against `php -l` or `php -r` to confirm alignment before encoding parser rules.

## Commit & Pull Request Guidelines
- Use imperative, concise subjects similar to existing history (`fix: validate break levels`, `chore: refresh snapshots`), ~72 characters.
- PRs should call out behavior changes, linked issues, and any skipped checks; include repro snippets for parser changes.
- Ensure `cargo fmt`, `cargo clippy`, and `cargo test` (or a scoped subset with justification) are clean before review; mention any corpus runs like WordPress to show compatibility progress.
