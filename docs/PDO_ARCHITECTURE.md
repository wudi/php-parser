# PDO Extension - Architecture Summary

## Overview

PDO (PHP Data Objects) provides a unified database abstraction layer. This document summarizes the architectural decisions for the php-vm implementation.

## Key Design Principles

### 1. Simplification Over Feature Parity

**PHP PDO Approach** (Complex):
- Dynamic driver loading via `dl()` or shared libraries
- Runtime driver registration
- Complex C function pointer tables
- Manual memory management

**Our Approach** (Simplified):
- Static compilation of all drivers
- Compile-time driver registration
- Rust trait-based polymorphism
- Automatic memory management (RAII)

**Benefits**:
- ✅ No runtime loading complexity
- ✅ Better performance (no dynamic dispatch overhead)
- ✅ Type safety at compile time
- ✅ Easier to test and debug

### 2. Trait-Based Abstraction

```rust
// Three core traits define the contract
pub trait PdoDriver      // Driver registration & connection creation
pub trait PdoConnection  // Connection management & query execution
pub trait PdoStatement   // Statement execution & result fetching
```

**Why Traits?**
- Compile-time polymorphism (vs C's runtime function pointers)
- Type-safe interfaces
- Enables mocking for tests
- Better IDE support (autocomplete, type hints)

### 3. Code Encapsulation Strategy

#### Module Hierarchy
```
pdo/
├── mod.rs           → Public API (register_pdo_extension)
├── driver.rs        → Trait definitions (interface)
├── types.rs         → Shared types (ErrorMode, FetchMode, etc.)
├── error.rs         → Error mapping utilities
├── core.rs          → PDO & PDOStatement object implementations
└── drivers/
    ├── mod.rs       → Driver registry
    ├── sqlite.rs    → SQLite-specific implementation
    └── mysql.rs     → MySQL wrapper (reuses mysqli)
```

#### Separation of Concerns
- **driver.rs**: Defines "what" (interface contracts)
- **drivers/*.rs**: Implements "how" (driver-specific logic)
- **core.rs**: Bridges PHP API to Rust traits
- **types.rs**: Shared data structures
- **error.rs**: Centralized error handling

### 4. Code Reuse Opportunities

#### A. MySQL Driver Wraps mysqli Extension
```rust
// Instead of reimplementing MySQL protocol:
struct MysqlConnection {
    conn: MysqliConnection, // ← Reuse existing!
}

impl PdoConnection for MysqlConnection {
    fn prepare(&mut self, sql: &str) -> Result<...> {
        self.conn.prepare(sql)  // ← Delegate to mysqli
            .map(|stmt| Box::new(MysqlStatement { stmt }))
            .map_err(map_mysql_error)
    }
}
```

**Benefits**:
- ✅ Don't Repeat Yourself (DRY)
- ✅ Proven code (mysqli already tested)
- ✅ Faster implementation
- ✅ Shared bug fixes

#### B. Shared Type Conversion Layer
```rust
// types.rs - Used by ALL drivers
pub mod conversion {
    pub fn sql_null_to_php(vm: &mut VM) -> Handle { /* ... */ }
    pub fn sql_int_to_php(vm: &mut VM, value: i64) -> Handle { /* ... */ }
    pub fn sql_text_to_php(vm: &mut VM, value: &[u8]) -> Handle { /* ... */ }
}

// sqlite.rs
let handle = conversion::sql_text_to_php(vm, row_data);

// mysql.rs
let handle = conversion::sql_text_to_php(vm, row_data); // Same code!
```

#### C. Error Mapping Utilities
```rust
// error.rs
pub fn map_sqlite_error(err: rusqlite::Error) -> PdoError { /* ... */ }
pub fn map_mysql_error(err: mysql::Error) -> PdoError { /* ... */ }

// Centralizes error translation logic
```

### 5. Testing Strategy (Multi-Layer)

```
┌─────────────────────────────────────┐
│   Integration Tests (PHP scripts)   │  ← End-to-end workflows
├─────────────────────────────────────┤
│   Component Tests (Rust)            │  ← PDO class methods
├─────────────────────────────────────┤
│   Driver Tests (Rust)               │  ← Driver-specific logic
├─────────────────────────────────────┤
│   Unit Tests (Rust)                 │  ← Traits, types, conversions
└─────────────────────────────────────┘
```

#### Test Coverage Goals
- **Unit**: 100% of traits, types, error handling
- **Driver**: 100% of each driver's implementation
- **Component**: All PDO class methods
- **Integration**: Real-world PHP scripts
- **Compatibility**: Compare output with PHP 8.x

#### Example Test Structure
```rust
// Unit test
#[test]
fn test_fetch_mode_enum() {
    assert_eq!(FetchMode::Assoc as i64, 2);
}

// Driver test
#[test]
fn test_sqlite_prepare() {
    let driver = SqliteDriver;
    let mut conn = driver.connect("sqlite::memory:").unwrap();
    let stmt = conn.prepare("SELECT 1").unwrap();
    assert!(stmt.execute(None).is_ok());
}

// Integration test (PHP)
<?php
$pdo = new PDO('sqlite::memory:');
assert($pdo->exec('CREATE TABLE t(id INT)') === 0);
```

### 6. Performance Considerations

#### Memory Efficiency
- **Arena Allocation**: All PHP values use `vm.arena.alloc()` (zero-copy)
- **Smart Pointers**: `Rc<>` for shared ownership, `Box<>` for unique ownership
- **Lazy Evaluation**: Don't fetch all rows unless requested

#### Query Performance
```rust
// Bad: Clone entire result set
fn fetch_all(&mut self) -> Vec<Row> {
    self.rows.clone()  // ❌ Expensive!
}

// Good: Return handles (references to arena)
fn fetch_all(&mut self, vm: &mut VM) -> Vec<Handle> {
    self.rows.iter()
        .map(|row| convert_row_to_handle(vm, row))  // ✅ Efficient
        .collect()
}
```

#### Caching Strategy
```rust
pub struct PdoStatementObject {
    statement: Box<dyn PdoStatement>,
    
    // Cache bound parameters to avoid re-allocation
    bound_params: HashMap<ParamIdentifier, (Handle, ParamType)>,
    
    // Cache column metadata
    column_meta_cache: Option<Vec<ColumnMeta>>,
}
```

### 7. Error Handling Philosophy

#### No Panics Policy
```rust
// Bad: Can panic
fn get_column(&self, idx: usize) -> &Column {
    &self.columns[idx]  // ❌ Panics if out of bounds
}

// Good: Returns Result
fn get_column(&self, idx: usize) -> Result<&Column, PdoError> {
    self.columns.get(idx)
        .ok_or(PdoError::InvalidParameter(
            format!("Column {} does not exist", idx)
        ))
}
```

#### Error Mode Handling
```rust
impl PdoObject {
    fn handle_error(&mut self, vm: &mut VM, error: PdoError) -> Result<Handle, String> {
        self.last_error = Some(error_info);
        
        match self.error_mode {
            ErrorMode::Silent => {
                // Store error, return false
                Ok(vm.arena.alloc(Val::Bool(false)))
            }
            ErrorMode::Warning => {
                // Emit warning, return false
                vm.trigger_error(ErrorLevel::Warning, &msg);
                Ok(vm.arena.alloc(Val::Bool(false)))
            }
            ErrorMode::Exception => {
                // Throw PDOException
                Err(format!("PDOException: {}", error))
            }
        }
    }
}
```

### 8. Security Considerations

#### SQL Injection Prevention
```rust
impl PdoConnection for SqliteConnection {
    fn quote(&self, value: &str, param_type: ParamType) -> String {
        match param_type {
            ParamType::Str => {
                // Proper escaping for SQLite
                format!("'{}'", value.replace("'", "''"))
            }
            ParamType::Int => {
                // Validate integer
                value.parse::<i64>()
                    .map(|n| n.to_string())
                    .unwrap_or_else(|_| "0".to_string())
            }
            // ... other types
        }
    }
}
```

#### Prepared Statements (Preferred)
```rust
// Encourage safe practices by making prepare() easy
let stmt = $pdo->prepare('SELECT * FROM users WHERE id = ?');
$stmt->execute([42]);  // ✅ Safe - driver handles escaping
```

## Comparison with PHP Implementation

| Aspect | PHP PDO | Our Implementation |
|--------|---------|-------------------|
| Driver Loading | Dynamic (shared libs) | Static (compiled in) |
| Polymorphism | Function pointers | Rust traits |
| Memory Management | Manual (efree/emalloc) | Automatic (RAII) |
| Type Safety | Runtime checks | Compile-time checks |
| Error Handling | Mixed (returns + exceptions) | Consistent Result<T, E> |
| Code Reuse | Separate extensions | Trait-based sharing |
| Performance | Good | Better (static dispatch) |
| Complexity | High | Lower |

## Implementation Complexity Estimate

### Lines of Code (Estimated)
```
Core infrastructure:       ~500 LOC
SQLite driver:             ~800 LOC
MySQL driver (wrapper):    ~400 LOC  ← Code reuse wins!
PDO class:                 ~600 LOC
PDOStatement class:        ~700 LOC
Tests:                     ~2000 LOC
─────────────────────────────────────
Total:                     ~5000 LOC
```

Compare to PHP's PDO:
- PHP ext/pdo: ~6000+ lines of C
- Our implementation: ~5000 lines of Rust (safer, more maintainable)

### Time Estimate
- **With this plan**: 6 weeks for complete implementation
- **Without plan**: 12+ weeks (trial and error)

## Risk Mitigation

### Risk 1: Lifetime Issues with Traits
**Mitigation**: Use `Box<dyn Trait>` for dynamic dispatch, avoid complex lifetimes

### Risk 2: Driver-Specific Edge Cases
**Mitigation**: Comprehensive test suite per driver, fuzzing with random SQL

### Risk 3: Performance Regression
**Mitigation**: Benchmark against PHP, optimize hot paths, use `cargo flamegraph`

### Risk 4: Breaking Changes in Dependencies
**Mitigation**: Pin dependency versions, monitor changelogs, have fallback plans

## Success Metrics

- ✅ All PDO class methods implemented
- ✅ SQLite & MySQL drivers working
- ✅ 90%+ test coverage
- ✅ 95%+ compatibility with PHP PDO behavior
- ✅ Performance within 10% of PHP
- ✅ Zero panics in production
- ✅ No memory leaks after stress testing

## Conclusion

This architecture balances:
- **Simplicity**: Static linking, trait-based design
- **Encapsulation**: Clean module boundaries, SOLID principles
- **Code Reuse**: Shared utilities, mysqli wrapping
- **Testability**: Multi-layer testing strategy
- **Safety**: No panics, type safety, RAII

**The design is production-ready and can be implemented incrementally over 6 weeks.**

## Next Steps

1. Review this plan with the team
2. Set up project tracking (GitHub issues/milestones)
3. Start with Phase 1 (Core Infrastructure)
4. Iterate with weekly reviews

## References

- **Detailed Plan**: [PDO_EXTENSION_PLAN.md](./PDO_EXTENSION_PLAN.md)
- **Quick Start**: [PDO_QUICK_START.md](./PDO_QUICK_START.md)
- **PHP Source**: `/Users/eagle/Sourcecode/php-src/ext/pdo/`
- **mysqli Reference**: `crates/php-vm/src/builtins/mysqli/`
