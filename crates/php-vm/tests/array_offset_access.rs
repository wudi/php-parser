mod common;

use common::run_code_vm_only;
use php_vm::core::value::Val;
use php_vm::vm::engine::VM;

#[test]
fn test_array_offset_integer_key() {
    let vm = run_code_vm_only("<?php $arr = [10, 20, 30]; return [$arr[0], $arr[1], $arr[2]];");
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);

    if let Val::Array(arr) = &val.value {
        assert_eq!(arr.map.len(), 3);
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_string_offset_negative() {
    let vm = run_code_vm_only(r#"<?php $str = "Hello"; return [$str[-1], $str[-2], $str[-5]];"#);
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);

    if let Val::Array(arr) = &val.value {
        let first = vm.arena.get(*arr.map.values().nth(0).unwrap());
        assert!(matches!(first.value, Val::String(ref s) if s.as_ref() == b"o"));
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_string_offset_type_coercion() {
    let vm = run_code_vm_only(
        r#"<?php
$str = "Hello";
return [$str["1"], $str[1.9], $str[true], $str[false]];
"#,
    );
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);

    if let Val::Array(arr) = &val.value {
        assert_eq!(arr.map.len(), 4);
        for (_idx, handle) in arr.map.iter() {
            let elem = vm.arena.get(*handle);
            assert!(matches!(elem.value, Val::String(_)));
        }
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_offset_on_scalar_returns_null() {
    let vm = run_code_vm_only(
        r#"<?php
$bool = true;
$num = 123;
$float = 3.14;
return [$bool[0], $num[0], $float[0]];
"#,
    );
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);

    if let Val::Array(arr) = &val.value {
        for (_idx, handle) in arr.map.iter() {
            let elem = vm.arena.get(*handle);
            assert_eq!(elem.value, Val::Null);
        }
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_isset_on_string_offset() {
    let vm = run_code_vm_only(
        r#"<?php
$str = "Hello";
return [isset($str[0]), isset($str[10]), isset($str[-1]), isset($str[-10])];
"#,
    );
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);

    if let Val::Array(arr) = &val.value {
        let v0 = vm.arena.get(*arr.map.values().nth(0).unwrap());
        let v1 = vm.arena.get(*arr.map.values().nth(1).unwrap());
        let v2 = vm.arena.get(*arr.map.values().nth(2).unwrap());
        let v3 = vm.arena.get(*arr.map.values().nth(3).unwrap());

        assert_eq!(v0.value, Val::Bool(true));
        assert_eq!(v1.value, Val::Bool(false));
        assert_eq!(v2.value, Val::Bool(true));
        assert_eq!(v3.value, Val::Bool(false));
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_numeric_string_key_conversion() {
    let vm = run_code_vm_only(
        r#"<?php
$arr = [];
$arr["42"] = "value";
return [$arr[42], $arr["42"]];
"#,
    );
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);

    if let Val::Array(arr) = &val.value {
        for (_idx, handle) in arr.map.iter() {
            let elem = vm.arena.get(*handle);
            assert!(matches!(elem.value, Val::String(_)));
        }
    } else {
        panic!("Expected array");
    }
}

#[test]
fn test_array_key_coercion() {
    let vm = run_code_vm_only(
        "<?php
$arr = [];
$arr[true] = 'a';
$arr[false] = 'b';
$arr[3.14] = 'c';
$arr[null] = 'd';
return [$arr[1], $arr[0], $arr[3], $arr['']];
",
    );
    let ret = vm.last_return_value.expect("No return value");
    let val = vm.arena.get(ret);

    if let Val::Array(arr) = &val.value {
        assert_eq!(arr.map.len(), 4);
    } else {
        panic!("Expected array");
    }
}
