use crate::builtins::pdo;
use crate::builtins::pdo::drivers::DriverRegistry;
use crate::core::value::{ArrayData, ObjectData, Val};
use crate::runtime::context::EngineContext;
use crate::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

fn create_test_vm() -> VM {
    let engine = Arc::new(EngineContext::new());
    VM::new(engine)
}

#[test]
fn test_driver_registry_creation() {
    let registry = DriverRegistry::new();
    assert!(registry.get("sqlite").is_some());
}

#[test]
fn test_parse_dsn() {
    let (driver, conn_str) = DriverRegistry::parse_dsn("sqlite::memory:").unwrap();
    assert_eq!(driver, "sqlite");
    assert_eq!(conn_str, ":memory:");
}

fn setup_pdo_object(vm: &mut VM) -> crate::core::value::Handle {
    let pdo_class_sym = vm.context.interner.intern(b"PDO");
    let properties = vm.collect_properties(
        pdo_class_sym,
        crate::vm::engine::PropertyCollectionMode::All,
    );
    let obj_data = ObjectData {
        class: pdo_class_sym,
        properties,
        internal: None,
        dynamic_properties: std::collections::HashSet::new(),
    };
    let payload_handle = vm.arena.alloc(Val::ObjPayload(obj_data));
    let pdo_obj_handle = vm.arena.alloc(Val::Object(payload_handle));

    // Push a frame and set 'this'
    vm.frames.push(crate::vm::frame::CallFrame::new(Rc::new(
        crate::compiler::chunk::CodeChunk::default(),
    )));
    vm.frames.last_mut().unwrap().this = Some(pdo_obj_handle);

    pdo_obj_handle
}

#[test]
fn test_pdo_sqlite_connect() {
    let mut vm = create_test_vm();

    let pdo_obj = setup_pdo_object(&mut vm);

    // PDO::__construct("sqlite::memory:")
    let dsn = vm
        .arena
        .alloc(Val::String(Rc::new(b"sqlite::memory:".to_vec())));
    pdo::php_pdo_construct(&mut vm, &[dsn]).expect("Connection failed");

    // Verify it's an object and has an ID
    assert!(matches!(vm.arena.get(pdo_obj).value, Val::Object(_)));

    let id_sym = vm.context.interner.intern(b"__id");
    let obj_payload = match vm.arena.get(pdo_obj).value {
        Val::Object(h) => h,
        _ => panic!("Expected object"),
    };

    if let Val::ObjPayload(obj) = &vm.arena.get(obj_payload).value {
        assert!(obj.properties.contains_key(&id_sym));
    } else {
        panic!("Expected ObjPayload");
    }
}

#[test]
fn test_pdo_sqlite_exec_and_query() {
    let mut vm = create_test_vm();

    let _pdo_obj = setup_pdo_object(&mut vm);

    let dsn = vm
        .arena
        .alloc(Val::String(Rc::new(b"sqlite::memory:".to_vec())));
    pdo::php_pdo_construct(&mut vm, &[dsn]).unwrap();

    // PDO::exec("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)")
    let sql = vm.arena.alloc(Val::String(Rc::new(
        b"CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)".to_vec(),
    )));
    let affected = pdo::php_pdo_exec(&mut vm, &[sql]).expect("Exec failed");
    assert_eq!(vm.arena.get(affected).value, Val::Int(0));

    // PDO::exec("INSERT INTO test (name) VALUES ('Alice')")
    let insert = vm.arena.alloc(Val::String(Rc::new(
        b"INSERT INTO test (name) VALUES ('Alice')".to_vec(),
    )));
    let affected = pdo::php_pdo_exec(&mut vm, &[insert]).unwrap();
    assert_eq!(vm.arena.get(affected).value, Val::Int(1));

    // PDO::query("SELECT * FROM test")
    let query_sql = vm
        .arena
        .alloc(Val::String(Rc::new(b"SELECT * FROM test".to_vec())));
    let stmt = pdo::php_pdo_query(&mut vm, &[query_sql]).expect("Query failed");

    // Verify statement object
    assert!(matches!(vm.arena.get(stmt).value, Val::Object(_)));

    // stmt->fetch()
    vm.frames.last_mut().unwrap().this = Some(stmt);
    let row = pdo::php_pdo_stmt_fetch(&mut vm, &[]).expect("Fetch failed");
    assert!(matches!(vm.arena.get(row).value, Val::Array(_)));
}

#[test]
fn test_pdo_sqlite_prepared_statement() {
    let mut vm = create_test_vm();

    let pdo_obj = setup_pdo_object(&mut vm);

    let dsn = vm
        .arena
        .alloc(Val::String(Rc::new(b"sqlite::memory:".to_vec())));
    pdo::php_pdo_construct(&mut vm, &[dsn]).unwrap();

    let sql = vm.arena.alloc(Val::String(Rc::new(
        b"CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT)".to_vec(),
    )));
    pdo::php_pdo_exec(&mut vm, &[sql]).unwrap();

    // $stmt = $pdo->prepare("INSERT INTO users (email) VALUES (?)")
    let prepare_sql = vm.arena.alloc(Val::String(Rc::new(
        b"INSERT INTO users (email) VALUES (?)".to_vec(),
    )));
    let stmt = pdo::php_pdo_prepare(&mut vm, &[prepare_sql]).expect("Prepare failed");

    // $stmt->execute(['alice@example.com'])
    vm.frames.last_mut().unwrap().this = Some(stmt);
    let mut params_arr = ArrayData::new();
    let email_val = vm
        .arena
        .alloc(Val::String(Rc::new(b"alice@example.com".to_vec())));
    params_arr.push(email_val);
    let params_handle = vm.arena.alloc(Val::Array(Rc::new(params_arr)));

    pdo::php_pdo_stmt_execute(&mut vm, &[params_handle]).expect("Execute failed");

    // $pdo->query("SELECT email FROM users WHERE id = 1")
    vm.frames.last_mut().unwrap().this = Some(pdo_obj);
    let select_sql = vm.arena.alloc(Val::String(Rc::new(
        b"SELECT email FROM users WHERE id = 1".to_vec(),
    )));
    let query_stmt = pdo::php_pdo_query(&mut vm, &[select_sql]).unwrap();

    // $query_stmt->fetch()
    vm.frames.last_mut().unwrap().this = Some(query_stmt);
    let row = pdo::php_pdo_stmt_fetch(&mut vm, &[]).unwrap();

    if let Val::Array(arr) = &vm.arena.get(row).value {
        // Fetch both (numeric and associative)
        let email_val = arr
            .map
            .get(&crate::core::value::ArrayKey::Str(Rc::new(
                b"email".to_vec(),
            )))
            .expect("Column 'email' not found");
        if let Val::String(s) = &vm.arena.get(*email_val).value {
            assert_eq!(s.as_ref(), b"alice@example.com");
        } else {
            panic!("Expected string value");
        }
    } else {
        panic!("Expected array row");
    }
}
