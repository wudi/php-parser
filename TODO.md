# Parser TODO (PHP Parity)

Reference sources: `$PHP_SRC_PATH/Zend/zend_language_scanner.l` (tokens/lexing), `$PHP_SRC_PATH/Zend/zend_language_parser.y` (grammar), and `$PHP_SRC_PATH/Zend/zend_ast.h` (AST kinds). Do **not** introduce non-PHP syntax or AST kinds; mirror Zend semantics.

Status legend: [ ] not started, [~] in progress, [x] implemented/verified.

## Current Parser Snapshot (for triage)
- Implemented: core statements (`if/else/elseif`, loops including alt syntax, `try/catch/finally`, `switch`, `match`, `declare`, `namespace`, `use`/group use, functions, closures/arrow functions, classes/interfaces/traits/enums), expressions (binary/unary ops, assigns/assign-op/ref assign, include kinds, match, casts, isset/empty, eval/die/exit, closures/arrow funcs, arrays/list destructuring with by-ref/unpack, calls with named args, property/static fetches including nullsafe, class const fetch, ternary, yield/yield from, print), attributes, types (unions/intersections/nullables), trait adaptations, class const groups, property hooks, enum backed types/implements, and snapshots for lexer/parser fixtures.
- Missing/not fully validated: pipe operator (not in target PHP), isset list semantics nuance, fine-grained lexer modes (heredoc/nowdoc completeness), modifier matrix edge cases, alt-syntax span accuracy, enum case semantics beyond basic validation, and PHP AST reference alignment.
- Span/error handling: semicolon recovery present; need Zend-like sync points and accurate spans for alt syntax/end tokens.

## Lexical & Token Alignment
- [ ] Cross-check token inventory and modes against `zend_language_scanner.l` (e.g., doc comments, heredoc/nowdoc, encapsed strings, nullsafe operator, pipe). Add missing tokens to lexer/token kinds without inventing new ones.
- [ ] Ensure lexer modes cover PHP contexts (inline HTML, script/embedding, heredoc interpolation, label vs identifier, property/var name modes).
- [ ] Normalize trivia handling to match PHP (comments skipped but preserved for spans where needed).

## Statement-Level Coverage (ZEND_AST_* with 0–4 children)
- [x] Labels/goto (`ZEND_AST_LABEL`, `ZEND_AST_GOTO`) and targeted continue/break levels matching grammar.
- [x] Alternate syntax variants for `if/while/for/foreach/switch` and `declare` (ensure `endif`, `endwhile`, etc. are parsed correctly).
- [~] `declare` directive edge cases (ticks/encoding), and `declare ... enddeclare;` form.
- [~] `static`/`global`/`unset`/`isset`/`empty` semantics with proper AST kinds (operators parsed; semantic parity still to be checked).
- [x] `try` with multiple catches and optional finally; catch types as names, optional variables; verify catch type union/intersection mapping.
- [x] Match/switch cases including `default`, `match` arms, and fallthrough semantics.

## Expression Coverage
- [x] Coalesce/nullish operators and assignments (`??`, `??=`, `ZEND_AST_ASSIGN_COALESCE`).
- [x] Yield/yield from (`ZEND_AST_YIELD`, `ZEND_AST_YIELD_FROM`) including precedence with assignments and commas.
- [x] Nullsafe property/method fetch (`ZEND_AST_NULLSAFE_PROP`, `ZEND_AST_NULLSAFE_METHOD_CALL`) basic support; property hooks remain.
- [x] Named arguments (`ZEND_AST_NAMED_ARG`), argument unpacking, and variadics basics; verify against PHP AST when available.
- [ ] Pipe operator (`ZEND_AST_PIPE`) – not available in target PHP (skip unless officially added).
- [x] Reference assignment (`ZEND_AST_ASSIGN_REF`) via `=&` handling.
- [~] Arrow functions vs closures (static, attribute support, use list rules, by-ref body) – parsed, needs cross-check with Zend rules.
- [x] Array syntax (short/long), spread in arrays, by-ref array items, list destructuring (`ZEND_AST_LIST` equivalent via `ZEND_AST_ARRAY` semantics).
- [x] Include/eval/print/shell exec AST kinds (`ZEND_AST_INCLUDE_OR_EVAL`, `ZEND_AST_SHELL_EXEC`, `ZEND_AST_PRINT`).
- [~] Casts and unary/binary op completeness per `zend_ast.h` (bitwise, logical, comparison, spaceship, instanceof, `**`, `**=`) – operators parsed, verify precedence/AST kinds.

## Class/Interface/Trait/Enum Coverage
- [~] Modifiers matrix (final/abstract/readonly/static/visibility) and validation rules per grammar (basic conflicts flagged; need full matrix and promotion rules; readonly class type requirements covered).
- [x] Property hooks (`ZEND_AST_PROPERTY_HOOK`, short body) parsing with get/set bodies.
- [x] Class constants (groups) with modifiers; traits use/adaptations parsing added; interfaces extends list validation pending.
- [x] Enum backed type + implements list captured; case value validation (backed requires value, pure disallows).
- [x] Enums reject declared properties; constructor promotions still allowed.
- [~] Attributes on declarations and parameters; ensure attr groups/arguments match `ZEND_AST_ATTRIBUTE_LIST`/`GROUP`.
- [x] Class constant validation: disallow abstract/static/readonly, enforce visibility rules (interface constants public).

## Types & Names
- [x] Union/intersection types (`ZEND_AST_TYPE_UNION`, `ZEND_AST_TYPE_INTERSECTION`), nullable shorthand, `callable`/`self`/`static`/`parent`.
- [x] Namespaces and grouped use statements (`ZEND_AST_GROUP_USE`, `ZEND_AST_USE_ELEM`, `UseKind` variants).
- [~] Fully-qualified/relative name parsing (leading `\`, namespace keyword) per scanner rules – spans should be rechecked.

## Error Recovery & Spans
- [~] Preserve Zend error recovery points (synchronize on `;`, `}`) and emit error nodes instead of panics.
- [~] Span accuracy for list nodes and implicit semicolons (close tag/EOF).

## Testing & Verification
- [~] For each implemented syntax, craft fixtures in `tests/` and expected outputs in `tests/snapshots/`.
- [ ] Cross-check against PHP reference: `php -d ast.dump_version=90 -r 'var_export(ast\\parse_code($code, $version));'` or `php -r 'print_r(token_get_all(<<<'PHP'\n...code...\nPHP));'` to confirm intended AST/tokens before updating snapshots.
- [x] Keep snapshots in sync via `cargo insta accept` only after confirming parity with PHP.***
