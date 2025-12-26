# How Native PHP Implements Strict Types for Built-in Functions

**Date:** December 26, 2025  
**Reference:** PHP Source Code at `/Users/eagle/Sourcecode/php-src/`

## Summary

**YES, native PHP DOES implement strict_types for built-in functions.**

Built-in functions respect the **caller's** `strict_types` mode through the `ZEND_ARG_USES_STRICT_TYPES()` macro.

---

## Key PHP Implementation Details

### 1. The Strict Types Macro (`zend_compile.h:716-725`)

```c
// Check if the current call uses strict types
#define ZEND_CALL_USES_STRICT_TYPES(call) \
    (((call)->func->common.fn_flags & ZEND_ACC_STRICT_TYPES) != 0)

// For the current execution frame
#define EX_USES_STRICT_TYPES() \
    ZEND_CALL_USES_STRICT_TYPES(execute_data)

// For internal functions - checks the CALLER's (previous frame's) strict mode
#define ZEND_ARG_USES_STRICT_TYPES() \
    (EG(current_execute_data)->prev_execute_data && \
     EG(current_execute_data)->prev_execute_data->func && \
     ZEND_CALL_USES_STRICT_TYPES(EG(current_execute_data)->prev_execute_data))
```

**Critical Insight:**
- `ZEND_ARG_USES_STRICT_TYPES()` accesses the **previous frame** (`prev_execute_data`)
- This is the **caller's frame**, which holds the caller's `strict_types` setting
- Built-in functions execute in their own frame, so they check the caller's frame

### 2. Parameter Parsing with Type Coercion (`zend_API.c`)

PHP's parameter parsing functions check strict types before attempting coercion:

#### Example: `zend_parse_arg_bool_slow()` (line 549)
```c
ZEND_API bool ZEND_FASTCALL zend_parse_arg_bool_slow(const zval *arg, bool *dest, uint32_t arg_num)
{
    if (UNEXPECTED(ZEND_ARG_USES_STRICT_TYPES())) {
        return 0;  // Reject coercion in strict mode
    }
    return zend_parse_arg_bool_weak(arg, dest, arg_num);
}
```

#### Example: `zend_parse_arg_long_slow()` (line 636)
```c
ZEND_API bool ZEND_FASTCALL zend_parse_arg_long_slow(const zval *arg, zend_long *dest, uint32_t arg_num)
{
    if (UNEXPECTED(ZEND_ARG_USES_STRICT_TYPES())) {
        return 0;  // Reject coercion in strict mode
    }
    return zend_parse_arg_long_weak(arg, dest, arg_num);
}
```

### 3. How It Works

When a built-in function like `strlen()` is called:

1. **PHP pushes a new frame** for the built-in function
2. **Parameter parsing** (`zend_parse_parameters()`) calls type-specific parsers
3. **Type parsers check** `ZEND_ARG_USES_STRICT_TYPES()`
4. If the **caller** had `strict_types=1`:
   - Type coercion is **rejected** (returns 0)
   - PHP throws a **TypeError**
5. If the **caller** had weak mode:
   - Type coercion is **attempted** (`zend_parse_arg_*_weak()`)
   - On success, continues with coerced value
   - On failure, emits Warning

---

## Locations Found in PHP Source

### Type Checking Uses
- `zend_API.c:549` - `zend_parse_arg_bool_slow`
- `zend_API.c:636` - `zend_parse_arg_long_slow`
- `zend_API.c:688` - float parsing
- `zend_API.c:697` - double parsing
- `zend_API.c:731` - string parsing
- `zend_API.c:785` - class parsing
- `zend_API.c:802` - callable parsing
- `zend_API.c:4647` - typed reference assignment

### Property Assignment (Different Macro)
- `zend_execute.c:585, 605, 1089, 1096` - Uses `EX_USES_STRICT_TYPES()` for property writes
- `zend_object_handlers.c:998` - `property_uses_strict_types()`

---

## Application to php-parser-rs

### Current Gap in Our Implementation

Our built-in functions do NOT check `callsite_strict_types`. Example from `string.rs`:

```rust
pub fn php_strlen(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // ... parameter count check ...
    
    let val = vm.arena.get(args[0]);
    let len = match &val.value {
        Val::String(s) => s.len(),
        Val::Int(_) | Val::Float(_) | Val::Bool(_) | Val::Null => {
            // ❌ PROBLEM: This always coerces, even in strict mode!
            val.value.to_php_string_bytes().len()
        }
        Val::Array(_) => {
            // Emits Warning, not TypeError
            vm.report_error(ErrorLevel::Warning, "...");
            return Ok(vm.arena.alloc(Val::Null));
        }
        // ...
    };
    Ok(vm.arena.alloc(Val::Int(len as i64)))
}
```

### What We Need to Implement

Following PHP's pattern, we need:

1. **Access to caller's `strict_types`:**
   ```rust
   // Option A: Pass as parameter
   pub fn php_strlen(vm: &mut VM, args: &[Handle], strict: bool) -> Result<Handle, String>
   
   // Option B: Store in VM temporarily (cleaner)
   vm.current_call_strict_types = callsite_strict_types;
   ```

2. **Type checking helper:**
   ```rust
   fn check_builtin_param_string(
       vm: &mut VM,
       arg: Handle,
       param_name: &str,
       func_name: &str,
   ) -> Result<Vec<u8>, String> {
       let val = &vm.arena.get(arg).value;
       match val {
           Val::String(s) => Ok(s.to_vec()),
           Val::Int(_) | Val::Float(_) | Val::Bool(_) | Val::Null => {
               // Check if strict mode
               if vm.is_strict_call() {
                   // Throw TypeError
                   return Err(format!(
                       "{}(): Argument #{} (${}
) must be of type string, {} given",
                       func_name, 1, param_name, val.type_name()
                   ));
               }
               // Weak mode: coerce
               Ok(val.to_php_string_bytes())
           }
           Val::Array(_) | Val::Object(_) => {
               // Arrays/Objects cannot be coerced even in weak mode
               return Err(format!(
                   "{}(): Argument #{} must be of type string, {} given",
                   func_name, 1, val.type_name()
               ));
           }
           _ => Ok(vec![]),
       }
   }
   ```

3. **Update all built-in functions** to use the helper

---

## Testing Strategy

### PHP Behavior Reference

```php
<?php
declare(strict_types=1);
strlen(42);  // TypeError: strlen(): Argument #1 ($string) must be of type string, int given

// vs weak mode:
strlen(42);  // Works: returns 2 (coerces 42 to "42")
```

### Test Plan for php-parser-rs

```rust
#[test]
fn test_builtin_strlen_strict_rejects_int() {
    let src = r#"<?php
declare(strict_types=1);
return strlen(42);
"#;
    // Should throw TypeError, not coerce
    expect_type_error(src, "must be of type string");
}

#[test]
fn test_builtin_strlen_weak_coerces_int() {
    let src = r#"<?php
return strlen(42);
"#;
    let val = run_code(src);
    assert_eq!(val, Val::Int(2)); // Coerced "42" has length 2
}
```

---

## Implementation Recommendation

### Cleanest Approach: Store in VM State

```rust
// In VM struct
pub struct VM {
    // ... existing fields ...
    
    /// Strict types mode of the current builtin call's caller
    /// Set before invoking builtin, checked during parameter validation
    pub(crate) builtin_call_strict: bool,
}

// In callable.rs, before calling builtin handler:
vm.builtin_call_strict = callsite_strict_types;
let res = handler(vm, &args).map_err(VmError::RuntimeError)?;
vm.builtin_call_strict = false; // Reset

// In builtins, check:
if vm.builtin_call_strict && !matches!(val, Val::String(_)) {
    return Err(TypeError);
}
```

**Advantages:**
- No signature changes to 100+ builtin functions
- Matches PHP's architectural pattern (checking execution state)
- Easy to extend later (e.g., for error context)

**Alternative:** Pass as parameter (more explicit, but 100+ function edits)

---

## Conclusion

Native PHP **fully implements** strict_types for built-in functions using the `ZEND_ARG_USES_STRICT_TYPES()` macro, which checks the **caller's** frame for the strict_types flag.

Our implementation should follow the same pattern:
1. Capture `callsite_strict_types` when invoking builtins
2. Check it during parameter validation
3. Throw TypeError in strict mode, attempt coercion in weak mode

This is the **only missing piece** to achieve full PHP compatibility for `declare(strict_types=1)`.
