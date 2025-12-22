# PDO Extension - Quick Implementation Guide

## Quick Start

This is a condensed guide for implementing the PDO extension. See [PDO_EXTENSION_PLAN.md](./PDO_EXTENSION_PLAN.md) for the comprehensive plan.

## File Structure

```
crates/php-vm/src/builtins/pdo/
├── mod.rs              # Public API
├── core.rs             # PDO & PDOStatement classes
├── driver.rs           # Traits (PdoDriver, PdoConnection, PdoStatement)
├── types.rs            # Enums, errors, conversions
├── error.rs            # Error mapping utilities
└── drivers/
    ├── mod.rs          # Registry
    ├── sqlite.rs       # SQLite implementation
    └── mysql.rs        # MySQL wrapper (reuses mysqli)
```

## Core Traits

```rust
// driver.rs
pub trait PdoDriver: Debug + Send + Sync {
    fn name(&self) -> &'static str;
    fn connect(&self, dsn: &str, ...) -> Result<Box<dyn PdoConnection>, PdoError>;
}

pub trait PdoConnection: Debug + Send {
    fn prepare(&mut self, sql: &str) -> Result<Box<dyn PdoStatement>, PdoError>;
    fn exec(&mut self, sql: &str) -> Result<i64, PdoError>;
    fn begin_transaction(&mut self) -> Result<(), PdoError>;
    fn commit(&mut self) -> Result<(), PdoError>;
    fn rollback(&mut self) -> Result<(), PdoError>;
    // ... etc
}

pub trait PdoStatement: Debug + Send {
    fn bind_param(&mut self, ...) -> Result<(), PdoError>;
    fn execute(&mut self, ...) -> Result<bool, PdoError>;
    fn fetch(&mut self, mode: FetchMode) -> Result<Option<FetchedRow>, PdoError>;
    fn row_count(&self) -> i64;
    // ... etc
}
```

## Implementation Phases (6 Weeks)

### Week 1: Core Infrastructure
```bash
# Create files
touch crates/php-vm/src/builtins/pdo/{mod,driver,types,error,core}.rs
mkdir -p crates/php-vm/src/builtins/pdo/drivers
touch crates/php-vm/src/builtins/pdo/drivers/{mod,sqlite,mysql}.rs

# Add to builtins/mod.rs:
pub mod pdo;

# Tests
touch tests/pdo_{driver_trait,types,error}.rs
```

### Week 2: SQLite Driver
```rust
// drivers/sqlite.rs
use rusqlite::{Connection, Statement};

pub struct SqliteDriver;
impl PdoDriver for SqliteDriver { /* ... */ }

struct SqliteConnection { conn: Connection }
impl PdoConnection for SqliteConnection { /* ... */ }

struct SqliteStatement<'conn> { stmt: Statement<'conn> }
impl PdoStatement for SqliteStatement<'_> { /* ... */ }
```

### Week 3-4: PDO & PDOStatement Classes
```rust
// core.rs
pub struct PdoObject {
    connection: Box<dyn PdoConnection>,
    error_mode: ErrorMode,
    // ...
}

pub struct PdoStatementObject {
    statement: Box<dyn PdoStatement>,
    bound_params: HashMap<ParamIdentifier, (Handle, ParamType)>,
    // ...
}

// Register with VM in mod.rs
pub fn register_pdo_extension(context: &mut EngineContext) {
    register_pdo_classes(context);
    register_pdo_constants(context);
}
```

### Week 5: MySQL Driver (Code Reuse!)
```rust
// drivers/mysql.rs
use crate::builtins::mysqli::MysqliConnection;

pub struct MysqlDriver;

struct MysqlConnection {
    conn: MysqliConnection, // Reuse existing!
}

impl PdoConnection for MysqlConnection {
    fn prepare(&mut self, sql: &str) -> Result<Box<dyn PdoStatement>, PdoError> {
        // Delegate to mysqli
        self.conn.prepare(sql)
            .map(|stmt| Box::new(MysqlStatement { stmt }) as _)
            .map_err(map_mysql_error)
    }
}
```

### Week 6: Testing & Polish
```php
// tests/pdo/basic_sqlite.php
<?php
$pdo = new PDO('sqlite::memory:');
$pdo->exec('CREATE TABLE test (id INT, name TEXT)');

$stmt = $pdo->prepare('INSERT INTO test VALUES (?, ?)');
$stmt->execute([1, 'Alice']);

$results = $pdo->query('SELECT * FROM test')->fetchAll(PDO::FETCH_ASSOC);
assert($results[0]['name'] === 'Alice');
```

## Key Design Decisions

### 1. Static vs Dynamic Drivers
**Decision**: Static linking (compiled in)
**Why**: Simplicity, type safety, no runtime loading complexity

### 2. Trait-Based vs Function Pointers
**Decision**: Rust traits
**Why**: Type safety, compile-time checks, better IDE support

### 3. Code Reuse Strategy
**Decision**: MySQL driver wraps mysqli
**Why**: DRY principle, proven code, faster implementation

### 4. Error Handling
**Decision**: Result<T, PdoError> everywhere
**Why**: No panics, explicit error propagation, fault tolerance

## Testing Checklist

- [ ] Driver registry loads all drivers
- [ ] SQLite: connect, prepare, execute, fetch
- [ ] SQLite: transactions (begin, commit, rollback)
- [ ] SQLite: parameter binding (positional & named)
- [ ] SQLite: all fetch modes (ASSOC, NUM, BOTH, OBJ)
- [ ] MySQL: same tests as SQLite
- [ ] Error modes: SILENT, WARNING, EXCEPTION
- [ ] Type conversions: NULL, INT, STRING, FLOAT, BLOB
- [ ] Memory safety: no leaks after 10k queries
- [ ] Compatibility: compare output with `php -r`

## Common Pitfalls to Avoid

1. **Lifetime Issues**: Use `'static` or `Box<dyn>` for trait objects
2. **Borrow Checker**: Don't hold mutable borrows across await points
3. **Arena Allocation**: All PHP values go through `vm.arena.alloc()`
4. **Error Propagation**: Use `?` operator, don't unwrap()
5. **Testing**: Write tests BEFORE implementation (TDD)

## Performance Targets

- Connection pooling: < 1ms to get pooled connection
- Query execution: Within 5% of native PHP PDO
- Memory overhead: < 10% more than PHP PDO
- No memory leaks after 1M queries

## References

- **Full Plan**: [PDO_EXTENSION_PLAN.md](./PDO_EXTENSION_PLAN.md)
- **PHP Source**: `/Users/eagle/Sourcecode/php-src/ext/pdo/`
- **mysqli Example**: `crates/php-vm/src/builtins/mysqli/`
- **hash Example**: `crates/php-vm/src/builtins/hash/`

## Getting Started Command

```bash
# 1. Add dependencies to Cargo.toml
cat >> crates/php-vm/Cargo.toml << 'EOF'

# PDO extension dependencies
rusqlite = { version = "0.31", features = ["bundled"] }
EOF

# 2. Create module structure
mkdir -p crates/php-vm/src/builtins/pdo/drivers
touch crates/php-vm/src/builtins/pdo/{mod,driver,types,error,core}.rs
touch crates/php-vm/src/builtins/pdo/drivers/{mod,sqlite,mysql}.rs

# 3. Create test structure
mkdir -p tests/pdo
touch tests/{pdo_driver_trait,pdo_sqlite_basic,pdo_mysql_basic}.rs

# 4. Start with driver.rs (define traits)
code crates/php-vm/src/builtins/pdo/driver.rs
```

## Example: Minimal Working PDO (SQLite)

```rust
// mod.rs - Entry point
pub mod driver;
pub mod types;
pub mod drivers;

use crate::runtime::context::EngineContext;

pub fn register_pdo_extension(context: &mut EngineContext) {
    // TODO: Register PDO class
    // TODO: Register PDOStatement class
    // TODO: Register constants
}
```

```rust
// driver.rs - Traits
pub trait PdoDriver: std::fmt::Debug + Send + Sync {
    fn name(&self) -> &'static str;
    fn connect(&self, dsn: &str) -> Result<Box<dyn PdoConnection>, String>;
}

pub trait PdoConnection: std::fmt::Debug + Send {
    fn exec(&mut self, sql: &str) -> Result<i64, String>;
}
```

```rust
// drivers/sqlite.rs - Implementation
use rusqlite::Connection;
use super::super::driver::*;

pub struct SqliteDriver;

impl PdoDriver for SqliteDriver {
    fn name(&self) -> &'static str { "sqlite" }
    
    fn connect(&self, dsn: &str) -> Result<Box<dyn PdoConnection>, String> {
        let path = dsn.strip_prefix("sqlite:").unwrap_or(dsn);
        let conn = Connection::open(path).map_err(|e| e.to_string())?;
        Ok(Box::new(SqliteConnection { conn }))
    }
}

#[derive(Debug)]
struct SqliteConnection { conn: Connection }

impl PdoConnection for SqliteConnection {
    fn exec(&mut self, sql: &str) -> Result<i64, String> {
        self.conn.execute(sql, [])
            .map(|n| n as i64)
            .map_err(|e| e.to_string())
    }
}
```

```rust
// Test it!
#[test]
fn test_minimal_pdo() {
    let driver = SqliteDriver;
    let mut conn = driver.connect("sqlite::memory:").unwrap();
    let affected = conn.exec("CREATE TABLE test (id INT)").unwrap();
    assert_eq!(affected, 0);
}
```

This minimal example compiles and runs! Build from here. 🚀
