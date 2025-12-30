# eval() Strictness Inheritance - Implementation Complete

**Date**: December 26, 2025  
**Status**: ✅ **COMPLETE**

## Summary

Successfully implemented `eval()` strictness inheritance, completing the final missing feature from `strict_types_features.md`.

## Changes Made

### 1. Emitter Modifications (`crates/php-vm/src/compiler/emitter.rs`)

**Added Field**:
```rust
pub struct Emitter<'src> {
    // ... existing fields
    inherited_strict_types: Option<bool>,  // NEW: Inherited from parent scope
}
```

**Added Builder Method** (lines 127-131):
```rust
pub fn with_inherited_strict_types(mut self, strict: bool) -> Self {
    self.inherited_strict_types = Some(strict);
    self
}
```

**Modified `compile()` Method** (lines 150-165):
- Checks if any statement explicitly declares `strict_types`
- Only applies inherited strictness if NO explicit declare found
- Logic: Preserves explicit overrides while supporting inheritance

### 2. VM Engine Modifications (`crates/php-vm/src/vm/engine.rs`)

**OpCode::IncludeOrEval Handler** (lines 7120-7150):
- Extracts caller's strictness from current frame: `frame.chunk.strict_types`
- Passes to emitter via `.with_inherited_strict_types(caller_strict)`
- eval() code compiles with inherited strictness (unless overridden)

### 3. Test Suite (`crates/php-vm/tests/strict_types_eval.rs`)

**9 Comprehensive Tests** covering:
1. **Inheritance**: eval() in strict file without declare → inherits strict mode
2. **Inheritance**: eval() in weak file without declare → inherits weak mode
3. **Override**: eval() with explicit `declare(strict_types=1)` → strict (even in weak parent)
4. **Override**: eval() with explicit `declare(strict_types=0)` → weak (even in strict parent)
5. **Nested**: Nested eval() → strictness inherits through layers
6. **Return Types**: eval() return type checking uses inherited strictness
7. **Return Types**: eval() in weak mode allows coercion
8. **Complex**: Mixed inheritance and overrides in single script
9. **Functions**: Functions defined in eval() inherit strictness

**All 9 tests passing**

## Test Results

### Before Implementation
- **Total**: 334 tests (260 VM + 74 strict_types)
- **Missing**: eval() strictness (9 tests)

### After Implementation
- **Total**: 343 tests (260 VM + 83 strict_types) ✅
- **New**: 9 eval() strictness tests
- **Regressions**: 0 (all existing tests still pass)

## Behavior Examples

### Example 1: Inheritance (Strict → Strict)
```php
<?php
declare(strict_types=1);

function test(int $x) { return $x; }

// eval() inherits strict mode (no explicit declare)
eval('test("42");'); // ❌ TypeError: must be of type int, string given
```

### Example 2: Inheritance (Weak → Weak)
```php
<?php
// No declare - weak mode

function test(int $x) { return $x; }

// eval() inherits weak mode
eval('test("42");'); // ✅ Coerces "42" → 42
```

### Example 3: Override (Strict → Weak)
```php
<?php
declare(strict_types=1);

function test(int $x) { return $x; }

// eval() explicitly overrides to weak mode
eval('declare(strict_types=0); test("42");'); // ✅ Coerces "42" → 42
```

### Example 4: Override (Weak → Strict)
```php
<?php
// No declare - weak mode

function test(int $x) { return $x; }

// eval() explicitly overrides to strict mode
eval('declare(strict_types=1); test("42");'); // ❌ TypeError
```

## Architecture Decisions

### Why `inherited_strict_types: Option<bool>`?

- **None**: Default state - no inheritance, use file's declared value
- **Some(true)**: Inherit strict mode from parent
- **Some(false)**: Inherit weak mode from parent

This allows distinguishing between:
1. Top-level compilation (no parent) → None
2. eval() from strict context → Some(true)
3. eval() from weak context → Some(false)

### Why Check for Explicit Declare?

PHP semantics: explicit `declare(strict_types=...)` in eval() code ALWAYS overrides inherited value.

Implementation:
```rust
// Check if eval'd code has explicit declare
let has_explicit_declare = stmts.iter().any(|stmt| {
    matches!(stmt, Stmt::Declare { directives, .. } 
        if directives.iter().any(|(k, _)| k == b"strict_types"))
});

// Only apply inherited if no explicit declare
if !has_explicit_declare {
    if let Some(inherited) = self.inherited_strict_types {
        chunk.strict_types = inherited;
    }
}
```

### Why `frame.chunk.strict_types` Not `frame.func`?

- Top-level frames don't have `func` set (only callable frames do)
- `chunk` is ALWAYS set for all frames
- `chunk.strict_types` contains the file's strictness setting
- Works for both top-level and function contexts

## Testing Strategy

### Integration Tests (Not Unit Tests)
- Created full PHP scripts with `declare()`, functions, eval()
- Used `compile_and_run()` helper (same as include/require tests)
- Verified type errors vs successful coercion
- Tested both positive (should work) and negative (should fail) cases

### Test Scenarios Matrix

| Parent Mode | eval() Declare | Expected Mode | Test Status |
|-------------|----------------|---------------|-------------|
| Strict      | None           | Strict        | ✅ Pass     |
| Weak        | None           | Weak          | ✅ Pass     |
| Strict      | strict=1       | Strict        | ✅ Pass     |
| Strict      | strict=0       | Weak          | ✅ Pass     |
| Weak        | strict=1       | Strict        | ✅ Pass     |
| Weak        | strict=0       | Weak          | ✅ Pass     |

## Impact

### Files Modified
1. `crates/php-vm/src/compiler/emitter.rs` - 3 changes (field, builder, compile logic)
2. `crates/php-vm/src/vm/engine.rs` - 1 change (OpCode::IncludeOrEval handler)
3. `crates/php-vm/tests/strict_types_eval.rs` - NEW (9 tests, 289 lines)

### Documentation Updated
1. `STRICT_TYPES_STATUS.md` - Test count 334 → 343
2. `IMPLEMENTATION_REVIEW.md` - Marked eval() as complete
3. `MISSING_FEATURES_PLAN.md` - (now obsolete - all features complete)

## Completion Status

🎉 **ALL `strict_types` requirements from `strict_types_features.md` are now implemented and tested.**

**Total Implementation**: 100% complete
- ✅ Parser validation
- ✅ Per-file compilation
- ✅ Parameter checking
- ✅ Return type checking
- ✅ Weak-mode coercion
- ✅ Position enforcement
- ✅ Include/require isolation
- ✅ eval() inheritance (NEW)

**Production Ready**: Yes
