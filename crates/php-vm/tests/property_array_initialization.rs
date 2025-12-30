mod common;
use common::run_code_capture_output;

#[test]
fn test_property_array_simple() {
    let code = r#"<?php
        class MyClass {
            private $data = ['count' => 5, 'name' => 'test'];
            
            public function get($key) {
                return $this->data[$key] ?? null;
            }
        }
        
        $obj = new MyClass();
        echo $obj->get('count');
        echo "\n";
        echo $obj->get('name');
    "#;

    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "5\ntest");
}

#[test]
fn test_property_array_numeric_keys() {
    let code = r#"<?php
        class MyClass {
            private $items = [10, 20, 30];
            
            public function getItem($index) {
                return $this->items[$index] ?? -1;
            }
        }
        
        $obj = new MyClass();
        echo $obj->getItem(0);
        echo "\n";
        echo $obj->getItem(1);
        echo "\n";
        echo $obj->getItem(2);
    "#;

    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "10\n20\n30");
}

#[test]
fn test_property_array_nested() {
    let code = r#"<?php
        class MyClass {
            private $config = [
                'db' => ['host' => 'localhost', 'port' => 3306],
                'cache' => ['enabled' => true]
            ];
            
            public function getDbHost() {
                return $this->config['db']['host'] ?? 'unknown';
            }
            
            public function getDbPort() {
                return $this->config['db']['port'] ?? 0;
            }
        }
        
        $obj = new MyClass();
        echo $obj->getDbHost();
        echo "\n";
        echo $obj->getDbPort();
    "#;

    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "localhost\n3306");
}

#[test]
fn test_property_empty_array() {
    let code = r#"<?php
        class MyClass {
            private $data = [];
            
            public function add($key, $value) {
                $this->data[$key] = $value;
                return $this->data[$key];
            }
        }
        
        $obj = new MyClass();
        echo $obj->add('test', 42);
    "#;

    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "42");
}

#[test]
fn test_static_property_array() {
    let code = r#"<?php
        class MyClass {
            public static $config = ['version' => 1, 'name' => 'app'];
            
            public static function getVersion() {
                return self::$config['version'];
            }
        }
        
        echo MyClass::getVersion();
    "#;

    let (_, output) = run_code_capture_output(code).unwrap();
    assert_eq!(output, "1");
}
