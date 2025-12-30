# MBString Extension Design (PHP 8.5)

## Goal
Implement the mbstring core extension for the VM with PHP 8.5 parity, excluding mbregex (`mb_ereg*`) for a later phase. Provide full functional behavior for encoding conversion, case folding, length/position, width, detection, and related helpers.

## Scope
- Implement `crates/php-vm/src/runtime/mb_extension.rs` and helpers under `crates/php-vm/src/runtime/mb/`.
- Register constants and functions in `module_init`.
- Maintain per-request mbstring settings (internal encoding, detect order, substitute character, language).
- Use `icu4x` + `encoding_rs` for conversions and Unicode-aware transforms.

## Non-Goals (Deferred)
- mbregex functions (`mb_ereg*`, `mb_regex_*`) and Oniguruma parity.
- External SAPI configuration file parsing beyond runtime defaults.

## Approach Choice
Use `icu4x` plus `encoding_rs` for wide encoding coverage and Unicode transforms. This balances correctness and Rust-native implementation while keeping room for PHP parity shims.

## Architecture
- `mb_extension.rs` implements the `Extension` trait to register functions/constants.
- `mb/state.rs` defines per-request state stored in `RequestContext` extension data.
- `mb/encoding.rs` owns encoding alias resolution, detection order, and conversion pipeline.
- `mb/case.rs` handles case folding/upper/lower/title conversions.
- `mb/width.rs` provides width computations (`mb_strwidth`, `mb_strimwidth`).
- `mb/detect.rs` provides detection heuristics and ordering.

## Data Flow
- `mb_convert_encoding($str, $to, $from)` resolves encoding aliases, decodes bytes to Unicode, then encodes to the target. Return `false` and emit warnings on unknown encodings.
- `mb_strlen`, `mb_substr`, `mb_strpos`, `mb_strrpos` decode using the requested or internal encoding, operate in Unicode scalar indices, and return PHP-style indices or substrings.
- `mb_convert_case` delegates to ICU casefolding with proper mode flags and PHP behavior for simple vs full folds.
- `mb_detect_encoding` checks candidate encodings in order and returns the first match or `false`.

## Error Handling & PHP Parity
- Match PHP 8.5 return values and warnings per function.
- Respect the current substitute character for invalid byte sequences.
- Preserve alias names and ordering for `mb_list_encodings` and `mb_encoding_aliases`.
- Add targeted compatibility shims where ICU/encoding_rs differ from PHP semantics.

## Testing
- Add integration tests under `tests/` for each major function group.
- Snapshot tests for string conversion edge cases, invalid sequences, and alias handling.
- Cross-check with `php -r` on a curated set of inputs to verify parity.

## Open Questions
- None. mbregex deferred to a later phase.
