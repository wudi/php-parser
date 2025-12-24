mod common;

use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_default_args() {
    let src = "<?php
        function greet($name = 'World') {
            return 'Hello ' . $name;
        }
        
        $a = greet();
        $b = greet('PHP');
        
        return $a . ' ' . $b;
    ";

    let result = run_code(src);

    match result {
        Val::String(s) => assert_eq!(String::from_utf8_lossy(&s), "Hello World Hello PHP"),
        _ => panic!("Expected String, got {:?}", result),
    }
}

#[test]
fn test_multiple_default_args() {
    let src = "<?php
        function make_point($x = 0, $y = 0, $z = 0) {
            return $x . ',' . $y . ',' . $z;
        }
        
        $p1 = make_point();
        $p2 = make_point(10);
        $p3 = make_point(10, 20);
        $p4 = make_point(10, 20, 30);
        
        return $p1 . '|' . $p2 . '|' . $p3 . '|' . $p4;
    ";

    let result = run_code(src);

    match result {
        Val::String(s) => assert_eq!(String::from_utf8_lossy(&s), "0,0,0|10,0,0|10,20,0|10,20,30"),
        _ => panic!("Expected String, got {:?}", result),
    }
}

#[test]
fn test_pass_by_value_isolation() {
    let src = "<?php
        function modify($val) {
            $val = 100;
            return $val;
        }
        
        $a = 10;
        $b = modify($a);
        
        return $a . ',' . $b;
    ";

    let result = run_code(src);

    match result {
        Val::String(s) => assert_eq!(String::from_utf8_lossy(&s), "10,100"),
        _ => panic!("Expected String, got {:?}", result),
    }
}

#[test]
fn test_pass_by_ref() {
    let src = "<?php
        function modify(&$val) {
            $val = 100;
        }
        
        $a = 10;
        modify($a);
        
        return $a;
    ";

    let result = run_code(src);

    match result {
        Val::Int(i) => assert_eq!(i, 100),
        _ => panic!("Expected Int(100), got {:?}", result),
    }
}

#[test]
fn test_pass_by_ref_default() {
    // PHP allows default values for reference parameters, but they must be constant.
    // If no argument is passed, the local variable is initialized with the default value (as a value, not ref to anything external).
    let src = "<?php
        function modify(&$val = 10) {
            $val = 100;
            return $val;
        }
        
        $res = modify();
        return $res;
    ";

    let result = run_code(src);

    match result {
        Val::Int(i) => assert_eq!(i, 100),
        _ => panic!("Expected Int(100), got {:?}", result),
    }
}

#[test]
fn test_mixed_args() {
    let src = "<?php
        function test($a, $b = 20, &$c) {
            $c = $a + $b;
        }
        
        $res = 0;
        test(10, 30, $res);
        $res1 = $res; // 40
        
        $res = 0;
        test(5, 20, $res); // explicit default
        $res2 = $res; // 25
        
        // Note: In PHP, you can't skip arguments in the middle easily without named args (PHP 8).
        // But we can test passing fewer args if the last ones are optional? 
        // Wait, $c is mandatory (no default), so we must pass 3 args.
        // If $b has default, but $c is mandatory, we MUST pass $b to get to $c.
        // So `test(10, $res)` would be invalid because $c is missing.
        
        return $res1 . ',' . $res2;
    ";

    let result = run_code(src);

    match result {
        Val::String(s) => assert_eq!(String::from_utf8_lossy(&s), "40,25"),
        _ => panic!("Expected String, got {:?}", result),
    }
}
