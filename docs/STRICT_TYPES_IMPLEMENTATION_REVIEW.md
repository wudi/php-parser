# Strict Types Implementation Review

Date: December 26, 2025

## Executive Summary

The `declare(strict_types=1)` implementation in php-parser-rs is **substantially complete** with excellent coverage of the core features. However, there is **one critical missing piece**: built-in (native) functions do not currently respect the caller's strict mode for parameter type checking.

## Implementation Status by Feature

### ✅ 1. Parser & Compiler Implementation (COMPLETE)

**Status:** Fully implemented and robust.

**Location:** 
- Parser: `crates/php-parser/src/parser/stmt.rs`
- Compiler: `crates/php-vm/src/compiler/emitter.rs` (lines 390-425)

**Details:**
- ✅ The parser validates that `declare(strict_types=1)` appears only as the first statement
- ✅ Per-file boolean flag is maintained in `Chunk.strict_types`
- ✅ The flag is propagated from file compilation context
- ✅ Methods inherit their containing file's `strict_types` setting (line 236)
- ✅ `eval()` correctly inherits strict_types from calling scope unless explicitly declared (lines 104-162)
- ✅ The compiler parses integer literals with `_` separators correctly

**Code Quality:** Excellent, with clear comments and proper reference to PHP semantics.

---

### ✅ 2. Parameter Type Checking - Caller's Rule (COMPLETE)

**Status:** Fully implemented with correct semantics.

**Location:** 
- `crates/php-vm/src/vm/engine.rs`:
  - `check_parameter_type()` (lines 11025-11076)
  - `coerce_parameter_value()` (lines 11078-11160)
  - OpCode handlers: `Recv` (line 4010), `RecvInit` (line 4065), `RecvVariadic` (line 4127)

**Details:**
- ✅ Caller-side enforcement: Uses `frame.callsite_strict_types` from the calling file
- ✅ Strict mode disables type juggling for scalar parameters
- ✅ **Int-to-Float Exception:** Correctly implemented - `int` can be passed to `float` parameters even in strict mode
- ✅ TypeError generation on mismatch in strict mode
- ✅ Weak mode attempts coercion with fallback to warning
- ✅ Union types and nullable types handled correctly
- ✅ By-reference parameters preserve reference semantics

**Code Quality:** Very good, with inline PHP source references and comprehensive type handling.

**Test Coverage:** Excellent
- `tests/strict_types_param_validation.rs` - 206 lines
- `tests/strict_types_edge_cases.rs` - 384 lines covering union types, nullable types, etc.

---

### ✅ 3. Return Type Checking - Callee's Rule (COMPLETE)

**Status:** Fully implemented with correct semantics.

**Location:** 
- `crates/php-vm/src/vm/engine.rs` (lines 2505-2560)

**Details:**
- ✅ Callee-side enforcement: Uses the **defining function's** `chunk.strict_types` (line 2522)
- ✅ Return type checked BEFORE popping the frame
- ✅ Same int-to-float exception applied
- ✅ Weak mode attempts coercion for return values
- ✅ Strict mode throws TypeError on mismatch
- ✅ Proper error messages with function name

**Code Quality:** Very good, with clear separation of caller vs. callee strict mode.

**Test Coverage:** Good
- `tests/strict_types_return_validation.rs` dedicated to return type scenarios

---

### ❌ 4. Integration with Built-in Functions (INCOMPLETE)

**Status:** **NOT IMPLEMENTED** - This is the critical missing piece.

**Current Behavior:**
- Built-in functions (e.g., `strlen`, `count`, `array_push`) are registered in the extension registry
- They are invoked via `handler(self, &args)` in `callable.rs` (line 151)
- **Problem:** The `callsite_strict_types` flag is captured (line 29) but is **never passed** to built-in function handlers
- Built-in functions currently implement their own ad-hoc type checking with `Warning` errors (see `string.rs` line 21-32)
- They do NOT respect the caller's strict mode - no TypeError is thrown in strict mode

**What's Missing:**

1. **Pass `callsite_strict_types` to Built-in Handlers:**
   - Built-in function signature needs to accept a strict flag: 
     ```rust
     pub fn php_strlen(vm: &mut VM, args: &[Handle], strict: bool) -> Result<Handle, String>
     ```
   - Or store it in the VM temporarily before calling the handler

2. **Implement Strict Type Checking in Built-ins:**
   - Each built-in function with typed parameters needs to:
     - Check if `strict == true`
     - If strict and type mismatches: throw `TypeError` (RuntimeError)
     - If weak mode: attempt coercion, emit Warning on failure

3. **Update ALL Built-in Functions:**
   - Affected modules: `string.rs`, `array.rs`, `math.rs`, `pcre.rs`, `json.rs`, etc.
   - Functions like `strlen()`, `count()`, `array_push()`, `json_encode()`, etc.

**Impact:** HIGH - This breaks PHP compatibility for any code that uses `declare(strict_types=1)` and calls built-in functions with type-juggled arguments.

**Test Coverage:** None - no tests for `strlen(42)` in strict mode vs weak mode.

---

### ✅ 5. Scope Isolation Testing (GOOD)

**Test Coverage:**
- `tests/strict_types_include_require.rs` - Tests include/require isolation
- `tests/strict_types_eval.rs` - Tests eval() inheritance
- Cross-file scenarios tested

**Scenarios Validated:**
- ✅ Strict Caller -> Weak Callee: Parameters checked strictly at call site
- ✅ Weak Caller -> Strict Callee: Returns checked strictly at callee
- ✅ Include/Require: Each file maintains its own strict mode
- ✅ Eval(): Inherits caller's mode unless declared inside eval string

---

## Missing Features & Implementation Plan

### Priority 1: Built-in Function Strict Mode Support (CRITICAL)

**Estimated Effort:** Medium (2-4 hours)

**Steps:**

1. **Modify Built-in Function Signature:**
   ```rust
   // Option A: Add strict parameter
   pub type NativeHandler = fn(&mut VM, args: &[Handle], strict: bool) -> Result<Handle, String>;
   
   // Option B: Store in VM state
   // (May be cleaner - less function signature changes)
   ```

2. **Update Call Sites:**
   - In `callable.rs`, pass `callsite_strict_types` when invoking built-in handlers
   - In `engine.rs`, ensure native methods also receive the strict flag

3. **Implement Type Validation Helper:**
   ```rust
   fn validate_builtin_param(
       vm: &mut VM,
       arg: Handle,
       expected_type: &str, // "string", "int", "array", etc.
       param_num: usize,
       func_name: &str,
       strict: bool
   ) -> Result<Handle, String> {
       // Check type, coerce in weak mode, throw TypeError in strict mode
   }
   ```

4. **Update Built-in Functions:** (Prioritize by usage frequency)
   - Phase 1: Core functions
     - `strlen`, `count`, `array_push`, `array_pop`
     - `json_encode`, `json_decode`
   - Phase 2: Type-sensitive functions
     - Math functions (`abs`, `sqrt`, etc.)
     - String functions (`str_repeat`, `substr`, etc.)
   - Phase 3: Remaining functions

5. **Add Tests:**
   - Create `tests/strict_types_builtin_functions.rs`
   - Test scenarios:
     ```php
     <?php
     declare(strict_types=1);
     strlen(42); // Should throw TypeError
     
     // vs weak mode
     strlen(42); // Should work (coerce to "42")
     ```

---

### Priority 2: Documentation & Validation

**Steps:**

1. **Document Completion:**
   - Update `STRICT_TYPES_STATUS.md` to reflect completion after built-in fix
   - Add section on built-in function behavior

2. **Corpus Testing:**
   - Run corpus tests on large projects to validate real-world behavior
   - Focus on: WordPress, Laravel, Symfony

3. **Benchmark:**
   - Measure performance impact of strict type checking
   - Ensure no regression in weak mode

---

## Code Quality Assessment

### Strengths
- ✅ Excellent separation of caller vs. callee strict mode
- ✅ Proper int-to-float exception handling
- ✅ Comprehensive test coverage for user functions
- ✅ Clear inline documentation with PHP source references
- ✅ Arena allocation preserved (no heap overhead)
- ✅ Error recovery and proper error messages

### Areas for Improvement
- ❌ Built-in functions don't respect strict mode (critical fix needed)
- ⚠️ Some built-in functions use ad-hoc type checking (should be unified)
- ⚠️ No centralized type validation helper for built-ins (would reduce duplication)

---

## Recommendations

### Immediate Actions (This Week)

1. **Fix Built-in Function Strict Mode** (Priority 1)
   - Implement Option B (store strict flag in VM) for cleaner code
   - Update top 20 most-used built-in functions
   - Add test coverage

2. **Create Central Type Validator**
   - Single function to handle type checking for built-ins
   - Reduces duplication and ensures consistency
   - Easier to maintain as PHP evolves

### Short-term (Next 2 Weeks)

3. **Complete Built-in Coverage**
   - Update all remaining built-in functions
   - Run full test suite

4. **Performance Validation**
   - Benchmark strict vs weak mode
   - Ensure no regression

### Long-term Considerations

5. **Type System Evolution**
   - PHP 8.4+ may add new type features
   - Current architecture is extensible enough to accommodate

6. **Advanced Type Features** (Future)
   - Generics (if PHP adds them)
   - Intersection types (PHP 8.1+)
   - DNF types (PHP 8.2+)

---

## Conclusion

The strict_types implementation is **95% complete** with excellent quality. The only critical missing piece is built-in function integration, which should be straightforward to implement following the existing patterns in the codebase. Once completed, the implementation will be production-ready and fully compliant with PHP's strict_types semantics.

**Next Step:** Implement built-in function strict mode support as outlined in Priority 1.
