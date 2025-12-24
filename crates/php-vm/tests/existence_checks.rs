mod common;

use common::run_code;
use php_vm::core::value::Val;

#[test]
fn test_class_exists() {
    let code = r#"<?php
        class A {}
        return class_exists('A');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_class_exists_false() {
    let code = r#"<?php
        return class_exists('NonExistent');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, false);
    } else {
        panic!("Expected bool false, got {:?}", val);
    }
}

#[test]
fn test_interface_exists() {
    let code = r#"<?php
        interface I {}
        return interface_exists('I');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_interface_exists_false() {
    let code = r#"<?php
        class A {}
        return interface_exists('A');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, false);
    } else {
        panic!("Expected bool false, got {:?}", val);
    }
}

#[test]
fn test_trait_exists() {
    let code = r#"<?php
        trait T {}
        return trait_exists('T');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_trait_exists_false() {
    let code = r#"<?php
        class A {}
        return trait_exists('A');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, false);
    } else {
        panic!("Expected bool false, got {:?}", val);
    }
}

#[test]
fn test_is_a() {
    let code = r#"<?php
        class A {}
        $a = new A();
        return is_a($a, 'A');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_is_a_subclass() {
    let code = r#"<?php
        class A {}
        class B extends A {}
        $b = new B();
        return is_a($b, 'A');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_is_a_string() {
    let code = r#"<?php
        class A {}
        return is_a('A', 'A');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_is_a_false() {
    let code = r#"<?php
        class A {}
        class B {}
        $b = new B();
        return is_a($b, 'A');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, false);
    } else {
        panic!("Expected bool false, got {:?}", val);
    }
}

#[test]
fn test_method_exists() {
    let code = r#"<?php
        class A {
            function foo() {}
        }
        $a = new A();
        return method_exists($a, 'foo');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_method_exists_static() {
    let code = r#"<?php
        class A {
            static function foo() {}
        }
        return method_exists('A', 'foo');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_method_exists_inherited() {
    let code = r#"<?php
        class A {
            function foo() {}
        }
        class B extends A {}
        return method_exists('B', 'foo');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_method_exists_false() {
    let code = r#"<?php
        class A {}
        return method_exists('A', 'foo');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, false);
    } else {
        panic!("Expected bool false, got {:?}", val);
    }
}

#[test]
fn test_property_exists() {
    let code = r#"<?php
        class A {
            public $foo;
        }
        $a = new A();
        return property_exists($a, 'foo');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_property_exists_dynamic() {
    let code = r#"<?php
        class A {}
        $a = new A();
        $a->foo = 1;
        return property_exists($a, 'foo');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_property_exists_static_check() {
    let code = r#"<?php
        class A {
            public $foo;
        }
        return property_exists('A', 'foo');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_property_exists_inherited() {
    let code = r#"<?php
        class A {
            public $foo;
        }
        class B extends A {}
        return property_exists('B', 'foo');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, true);
    } else {
        panic!("Expected bool true, got {:?}", val);
    }
}

#[test]
fn test_get_class_methods() {
    let code = r#"<?php
        class A {
            function foo() {}
            function bar() {}
        }
        return get_class_methods('A');
    "#;

    let val = run_code(code);
    if let Val::Array(arr) = val {
        assert_eq!(arr.map.len(), 2);
    } else {
        panic!("Expected array, got {:?}", val);
    }
}

#[test]
fn test_get_class_methods_string() {
    let code = r#"<?php
        class A {
            function foo() {}
            function bar() {}
        }
        $methods = get_class_methods('A');
        // Sort to ensure deterministic output for test
        // sort($methods); // sort not implemented yet?
        // Let's just check count
        return count($methods);
    "#;

    let val = run_code(code);
    if let Val::Int(i) = val {
        assert_eq!(i, 2);
    } else {
        panic!("Expected int 2, got {:?}", val);
    }
}

#[test]
fn test_get_class_methods_visibility_global_scope() {
    let code = r#"<?php
        class A {
            private function hidden() {}
            protected function mid() {}
            public function open() {}
        }
        return implode(',', get_class_methods('A'));
    "#;

    let val = run_code(code);
    if let Val::String(s) = val {
        assert_eq!(s.as_ref(), b"open");
    } else {
        panic!("Expected string, got {:?}", val);
    }
}

#[test]
fn test_get_class_methods_visibility_internal_scope() {
    let code = r#"<?php
        class A {
            private function hidden() {}
            protected function mid() {}
            public function open() {}
            public static function expose() {
                return implode(',', get_class_methods('A'));
            }
        }
        return A::expose();
    "#;

    let val = run_code(code);
    if let Val::String(s) = val {
        assert_eq!(s.as_ref(), b"expose,hidden,mid,open");
    } else {
        panic!("Expected string, got {:?}", val);
    }
}

#[test]
fn test_get_class_vars() {
    let code = r#"<?php
        class A {
            public $foo = 1;
            public $bar = 2;
        }
        $vars = get_class_vars('A');
        return count($vars);
    "#;

    let val = run_code(code);
    if let Val::Int(i) = val {
        assert_eq!(i, 2);
    } else {
        panic!("Expected int 2, got {:?}", val);
    }
}

#[test]
fn test_get_class_vars_visibility_global_scope() {
    let code = r#"<?php
        class A {
            public $open = 1;
            protected $mid = 2;
            private $hidden = 3;
        }
        $vars = get_class_vars('A');
        if (isset($vars['open']) && !isset($vars['mid']) && !isset($vars['hidden'])) {
            return count($vars);
        }
        return -1;
    "#;

    let val = run_code(code);
    if let Val::Int(i) = val {
        assert_eq!(i, 1);
    } else {
        panic!("Expected int, got {:?}", val);
    }
}

#[test]
fn test_get_class_vars_visibility_internal_scope() {
    let code = r#"<?php
        class A {
            public $open = 1;
            protected $mid = 2;
            private $hidden = 3;
            public static function expose() {
                $vars = get_class_vars('A');
                if (isset($vars['open']) && isset($vars['mid']) && isset($vars['hidden'])) {
                    return count($vars);
                }
                return -1;
            }
        }
        return A::expose();
    "#;

    let val = run_code(code);
    if let Val::Int(i) = val {
        assert_eq!(i, 3);
    } else {
        panic!("Expected int, got {:?}", val);
    }
}

#[test]
fn test_property_exists_false() {
    let code = r#"<?php
        class A {}
        return property_exists('A', 'foo');
    "#;

    let val = run_code(code);
    if let Val::Bool(b) = val {
        assert_eq!(b, false);
    } else {
        panic!("Expected bool false, got {:?}", val);
    }
}
