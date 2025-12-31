# Pls Bin Warning Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove the remaining Clippy warnings emitted by the `php-parser` `pls` binary by introducing clearer intermediate types and collapsing the nested conditionals that Clippy flags.

**Architecture:** We keep the existing LSP-style visitor structure but introduce a `SymbolMeta` helper that captures the vector-tuple fields so visitors can pass a single argument to `add`. We also simplify the `InlayHintVisitor`/type-hierarchy/`prepare_type_hierarchy` control flow by combining nested `if let` constructs into the `if let ... && let ...` form and by baking the inner `Expr::Variable` checks directly into the `AstNode` match arms.

**Tech Stack:** Rust 2024 (Cargo, Clippy, rustfmt), `tower-lsp`, and the existing php-parser AST helpers.

### Task 1: Capture the current warnings by rerunning Clippy to target the `pls` binary.

**Files:**
- Modify: None
- Test: `cargo clippy --package php-parser --bin pls`

**Step 1:** Run `cargo clippy --package php-parser --bin pls` to capture the exact warnings we’re removing. Expected output: the same set of collapsible `if`/`match` and type-complexity warnings we just saw followed by `warning: php-parser (bin "pls") generated ... warnings`.

**Step 2:** Save the trimmed output or note which spans are still pending so the later verification run can be compared against this baseline.

**Step 3:** No code changes yet.

**Step 4:** None.

**Step 5:** None.

### Task 2: Simplify `IndexingVisitor::entries` by introducing a reusable `SymbolMeta` type and aliasing the tuple.

**Files:**
- Modify: `crates/php-parser/src/bin/pls.rs:33-120`
- Test: run `cargo fmt` afterwards (`cargo fmt --package php-parser --bin pls`)

**Step 1:** Add `type SymbolEntry = (String, Range, SymbolType, Option<SymbolKind>, Option<Vec<String>>, Option<Vec<String>>, Option<Vec<String>>, Option<String>);` and a small `struct SymbolMeta { symbol_kind: Option<SymbolKind>, parameters: Option<Vec<String>>, extends: Option<Vec<String>>, implements: Option<Vec<String>>, type_info: Option<String> }` near the module imports.

**Step 2:** Update the `IndexingVisitor` definition so `entries: Vec<SymbolEntry>` and `add` takes `SymbolMeta` (bundled data) instead of 9 separate parameters while still computing `Range` from the span. Collapsing `add`’s arguments removes `too_many_arguments`.

**Step 3:** Update every call site in `IndexingVisitor` and the bootstrap indexing task (the `tokio::spawn` block) that destructures the tuple to use the alias/struct, keeping the push logic unchanged.

**Step 4:** Run `cargo fmt --package php-parser --bin pls` to keep formatting consistent after the new types.

**Step 5:** Verify the previous `type_complexity` and `too_many_arguments` warnings are gone by rerunning `cargo clippy --package php-parser --bin pls` (later task handles full check).

### Task 3: Collapse nested `if let` chains in the `InlayHintVisitor`, `type_hierarchy_subtypes`, `did_save`, and similar helpers flagged by Clippy.

**Files:**
- Modify: `crates/php-parser/src/bin/pls.rs:1010-1260`, `crates/php-parser/src/bin/pls.rs:1520-1590`
- Test: `cargo fmt --package php-parser --bin pls`

**Step 1:** In `InlayHintVisitor::visit_expr`, rewrite the nested `if let Some` chain into a single combined `if let cond && let cond && let cond` expression so the compiler and Clippy see only one `if let` per block. Similarly, change the `type_hierarchy_subtypes` checks for `extends`/`implements` to combine them with `&&` and use the combined form for the `initialize`/`did_save` logic.

**Step 2:** Ensure any additional `if .. { if let .. }` or `else { if ... }` patterns in the sections you touched now collapse cleanly (e.g., the `else` in `did_save`).

**Step 3:** Run `cargo fmt --package php-parser --bin pls` to keep formatting consistent.

**Step 4:** No tests this step besides formatting.

**Step 5:** The later full Clippy run will confirm these warnings are gone.

### Task 4: Flatten `match` arms that currently re-check `Expr::Variable` after a broader `AstNode` pattern.

**Files:**
- Modify: multiple `match node` arms around lines 1450–2660 in `crates/php-parser/src/bin/pls.rs`; focus on a representative set (document highlight/prepare type hierarchy/goto definition) flagged with `collapsible_match`.
- Test: `cargo fmt --package php-parser --bin pls`

**Step 1:** For each `AstNode::Expr(Expr::New { class, .. })` (and `Call`, `PropertyFetch`, `MethodCall`, etc.) clause that currently does `if let Expr::Variable ... = *class` (or similar on `func`, `property`, `method`, etc.), inline the deeper pattern directly (e.g., `AstNode::Expr(Expr::New { class: Expr::Variable { name, .. }, .. })`). This merges the two patterns and satisfies Clippy.

**Step 2:** Where those branches still need `cursor_offset` checks on the sub-patterns, carry those checks inside the collapsed arm but without re-introducing an `if let` block; use combined `if` guards as necessary.

**Step 3:** Run `cargo fmt --package php-parser --bin pls`.

**Step 4:** No tests beyond formatting for this step.

**Step 5:** After all adjustments rerun `cargo clippy --package php-parser --bin pls` (see Task 5) to verify the `collapsible_match` warnings are resolved.

### Task 5: Verify the cleaned file reports no targeted Clippy warnings.

**Files:**
- Modify: None
- Test: `cargo clippy --package php-parser --bin pls`

**Step 1:** Run `cargo clippy --package php-parser --bin pls` expecting zero warnings from the earlier list.

**Step 2:** If warnings remain, iterate on the relevant sections until Clippy passes.

**Step 3:** Run any additional targeted checks the user requested (e.g., `cargo test -q` if you modified shared logic). Expected result: the existing test suite still passes.

**Step 4:** Record the final clean output for reference.

**Step 5:** No Git operations yet; leave that for after implementation.

Plan complete and saved to `docs/plans/2025-12-30-pls-bin-warnings.md`. Two execution options:

1. Subagent-Driven (stay in this session, use superpowers:subagent-driven-development per task). 
2. Parallel Session (open new session and execute this plan using superpowers:executing-plans).

Which approach would you like to take? 
