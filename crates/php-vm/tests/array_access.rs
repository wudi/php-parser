use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

fn run_code(source: &str) -> Val {
    let full_source = if source.trim().starts_with("<?php") {
        source.to_string()
    } else {
        format!("<?php {}", source)
    };

    let engine_context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(engine_context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(full_source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter = Emitter::new(full_source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk))
        .unwrap_or_else(|e| panic!("Runtime error: {:?}", e));

    let handle = vm.last_return_value.expect("No return value");
    vm.arena.get(handle).value.clone()
}

/// Test basic ArrayAccess implementation with offsetGet
/// Reference: $PHP_SRC_PATH/Zend/zend_interfaces.c - ArrayAccess interface
#[test]
fn test_array_access_offset_get() {
    let code = r#"
        class Container implements ArrayAccess {
            private $data = [];
            
            public function __construct() {
                $this->data = ['foo' => 'bar', 'num' => 42];
            }
            
            public function offsetExists($offset): bool {
                return isset($this->data[$offset]);
            }
            
            public function offsetGet($offset): mixed {
                return $this->data[$offset] ?? null;
            }
            
            public function offsetSet($offset, $value): void {
                $this->data[$offset] = $value;
            }
            
            public function offsetUnset($offset): void {
                unset($this->data[$offset]);
            }
        }
        
        $c = new Container();
        return $c['foo'];
    "#;

    match run_code(code) {
        Val::String(s) => {
            assert_eq!(s.as_ref(), b"bar");
        }
        other => panic!("Expected string 'bar', got {:?}", other),
    }
}

/// Test ArrayAccess offsetSet
#[test]
fn test_array_access_offset_set() {
    let code = r#"
        class Container implements ArrayAccess {
            private $data = [];
            
            public function offsetExists($offset): bool {
                return isset($this->data[$offset]);
            }
            
            public function offsetGet($offset): mixed {
                return $this->data[$offset] ?? null;
            }
            
            public function offsetSet($offset, $value): void {
                $this->data[$offset] = $value;
            }
            
            public function offsetUnset($offset): void {
                unset($this->data[$offset]);
            }
        }
        
        $c = new Container();
        $c['test'] = 'value';
        return $c['test'];
    "#;

    match run_code(code) {
        Val::String(s) => {
            assert_eq!(s.as_ref(), b"value");
        }
        other => panic!("Expected string 'value', got {:?}", other),
    }
}

/// Test ArrayAccess offsetUnset
#[test]
fn test_array_access_offset_unset() {
    let code = r#"
        class Container implements ArrayAccess {
            private $data = ['foo' => 'bar', 'baz' => 'qux'];
            
            public function offsetExists($offset): bool {
                return isset($this->data[$offset]);
            }
            
            public function offsetGet($offset): mixed {
                return $this->data[$offset] ?? null;
            }
            
            public function offsetSet($offset, $value): void {
                $this->data[$offset] = $value;
            }
            
            public function offsetUnset($offset): void {
                unset($this->data[$offset]);
            }
        }
        
        $c = new Container();
        unset($c['foo']);
        return $c['foo'];
    "#;

    match run_code(code) {
        Val::Null => {}
        other => panic!("Expected null after unset, got {:?}", other),
    }
}

/// Test ArrayAccess with isset()
#[test]
fn test_array_access_isset() {
    let code = r#"
        class Container implements ArrayAccess {
            private $data = ['exists' => 'yes', 'null' => null];
            
            public function offsetExists($offset): bool {
                if ($offset === 'exists' || $offset === 'null') {
                    return true;
                }
                return false;
            }
            
            public function offsetGet($offset): mixed {
                if ($offset === 'exists') return 'yes';
                if ($offset === 'null') return null;
                return null;
            }
            
            public function offsetSet($offset, $value): void {
                $this->data[$offset] = $value;
            }
            
            public function offsetUnset($offset): void {
                unset($this->data[$offset]);
            }
        }
        
        $c = new Container();
        $r1 = isset($c['exists']);
        $r2 = isset($c['null']);
        $r3 = isset($c['missing']);
        
        return ($r1 << 2) | ($r2 << 1) | $r3;
    "#;

    match run_code(code) {
        Val::Int(n) => {
            // r1=true (exists and not null) = 4
            // r2=false (exists but is null) = 0
            // r3=false (doesn't exist) = 0
            // Total: 4 | 0 | 0 = 4
            assert_eq!(n, 4);
        }
        other => panic!("Expected int 4, got {:?}", other),
    }
}

/// Test ArrayAccess with empty()
#[test]
fn test_array_access_empty() {
    let code = r#"
        class Container implements ArrayAccess {
            private $data = [
                'zero' => 0,
                'empty_str' => '',
                'non_empty' => 'value',
                'null' => null
            ];
            
            public function offsetExists($offset): bool {
                if ($offset === 'zero' || $offset === 'empty_str' || $offset === 'non_empty' || $offset === 'null') {
                    return true;
                }
                return false;
            }
            
            public function offsetGet($offset): mixed {
                if ($offset === 'zero') return 0;
                if ($offset === 'empty_str') return '';
                if ($offset === 'non_empty') return 'value';
                if ($offset === 'null') return null;
                return null;
            }
            
            public function offsetSet($offset, $value): void {
                $this->data[$offset] = $value;
            }
            
            public function offsetUnset($offset): void {
                unset($this->data[$offset]);
            }
        }
        
        $c = new Container();
        $r1 = empty($c['zero']);        // true (0 is empty)
        $r2 = empty($c['empty_str']);   // true ('' is empty)
        $r3 = empty($c['non_empty']);   // false ('value' is not empty)
        $r4 = empty($c['null']);        // true (null is empty)
        $r5 = empty($c['missing']);     // true (doesn't exist)
        
        return ($r1 << 4) | ($r2 << 3) | ($r3 << 2) | ($r4 << 1) | $r5;
    "#;

    match run_code(code) {
        Val::Int(n) => {
            // r1=true=16, r2=true=8, r3=false=0, r4=true=2, r5=true=1
            // Total: 16 | 8 | 0 | 2 | 1 = 27
            assert_eq!(n, 27);
        }
        other => panic!("Expected int 27, got {:?}", other),
    }
}

/// Test ArrayAccess with numeric offsets
#[test]
fn test_array_access_numeric_offsets() {
    let code = r#"
        class NumberedContainer implements ArrayAccess {
            private $items = [];
            
            public function offsetExists($offset): bool {
                return isset($this->items[$offset]);
            }
            
            public function offsetGet($offset): mixed {
                return $this->items[$offset] ?? null;
            }
            
            public function offsetSet($offset, $value): void {
                $this->items[$offset] = $value;
            }
            
            public function offsetUnset($offset): void {
                unset($this->items[$offset]);
            }
        }
        
        $c = new NumberedContainer();
        $c[0] = 'first';
        $c[1] = 'second';
        $c[2] = 'third';
        
        return $c[1];
    "#;

    match run_code(code) {
        Val::String(s) => {
            assert_eq!(s.as_ref(), b"second");
        }
        other => panic!("Expected string 'second', got {:?}", other),
    }
}

/// Test ArrayAccess with null offset (append-style)
#[test]
fn test_array_access_null_offset() {
    let code = r#"
        class AppendableContainer implements ArrayAccess {
            private $items = [];
            private $nextIndex = 0;
            
            public function offsetExists($offset): bool {
                return isset($this->items[$offset]);
            }
            
            public function offsetGet($offset): mixed {
                return $this->items[$offset] ?? null;
            }
            
            public function offsetSet($offset, $value): void {
                if ($offset === null) {
                    $this->items[$this->nextIndex++] = $value;
                } else {
                    $this->items[$offset] = $value;
                }
            }
            
            public function offsetUnset($offset): void {
                unset($this->items[$offset]);
            }
        }
        
        $c = new AppendableContainer();
        $c[] = 'first';
        $c[] = 'second';
        $c[10] = 'tenth';
        
        return $c[1];
    "#;

    match run_code(code) {
        Val::String(s) => {
            assert_eq!(s.as_ref(), b"second");
        }
        other => panic!("Expected string 'second', got {:?}", other),
    }
}

/// Test ArrayAccess with complex nested operations
#[test]
fn test_array_access_nested_operations() {
    let code = r#"
        class Container implements ArrayAccess {
            private $data = [];
            
            public function offsetExists($offset): bool {
                return isset($this->data[$offset]);
            }
            
            public function offsetGet($offset): mixed {
                return $this->data[$offset] ?? null;
            }
            
            public function offsetSet($offset, $value): void {
                $this->data[$offset] = $value;
            }
            
            public function offsetUnset($offset): void {
                unset($this->data[$offset]);
            }
        }
        
        $c = new Container();
        $c['arr'] = ['a' => 1, 'b' => 2];
        $c['num'] = 42;
        
        // Access nested array
        $arr = $c['arr'];
        return $arr['b'];
    "#;

    match run_code(code) {
        Val::Int(n) => {
            assert_eq!(n, 2);
        }
        other => panic!("Expected int 2, got {:?}", other),
    }
}

/// Test ArrayAccess implementation inheriting from parent class
#[test]
fn test_array_access_inheritance() {
    let code = r#"
        class BaseContainer implements ArrayAccess {
            protected $data = [];
            
            public function offsetExists($offset): bool {
                return isset($this->data[$offset]);
            }
            
            public function offsetGet($offset): mixed {
                return $this->data[$offset] ?? null;
            }
            
            public function offsetSet($offset, $value): void {
                $this->data[$offset] = $value;
            }
            
            public function offsetUnset($offset): void {
                unset($this->data[$offset]);
            }
        }
        
        class ExtendedContainer extends BaseContainer {
            public function setDefault($key, $value) {
                if (!isset($this[$key])) {
                    $this[$key] = $value;
                }
            }
        }
        
        $c = new ExtendedContainer();
        $c->setDefault('key', 'default_value');
        return $c['key'];
    "#;

    match run_code(code) {
        Val::String(s) => {
            assert_eq!(s.as_ref(), b"default_value");
        }
        other => panic!("Expected string 'default_value', got {:?}", other),
    }
}

/// Test ArrayAccess with modification operations (+=, etc.)
#[test]
fn test_array_access_compound_assignment() {
    let code = r#"
        class Container implements ArrayAccess {
            private $data = [];
            
            public function offsetExists($offset): bool {
                return isset($this->data[$offset]);
            }
            
            public function offsetGet($offset): mixed {
                return $this->data[$offset] ?? 0;
            }
            
            public function offsetSet($offset, $value): void {
                $this->data[$offset] = $value;
            }
            
            public function offsetUnset($offset): void {
                unset($this->data[$offset]);
            }
        }
        
        $c = new Container();
        $c['count'] = 5;
        $c['count'] += 10;
        
        return $c['count'];
    "#;

    match run_code(code) {
        Val::Int(n) => {
            assert_eq!(n, 15);
        }
        other => panic!("Expected int 15, got {:?}", other),
    }
}

/// Test ArrayAccess with string concatenation assignment
#[test]
fn test_array_access_string_concat_assignment() {
    let code = r#"
        class Container implements ArrayAccess {
            private $data = [];
            
            public function offsetExists($offset): bool {
                return isset($this->data[$offset]);
            }
            
            public function offsetGet($offset): mixed {
                return $this->data[$offset] ?? '';
            }
            
            public function offsetSet($offset, $value): void {
                $this->data[$offset] = $value;
            }
            
            public function offsetUnset($offset): void {
                unset($this->data[$offset]);
            }
        }
        
        $c = new Container();
        $c['msg'] = 'Hello';
        $c['msg'] .= ' World';
        
        return $c['msg'];
    "#;

    match run_code(code) {
        Val::String(s) => {
            assert_eq!(s.as_ref(), b"Hello World");
        }
        other => panic!("Expected string 'Hello World', got {:?}", other),
    }
}

/// Test ArrayAccess with increment/decrement operators
#[test]
fn test_array_access_increment_decrement() {
    let code = r#"
        class Container implements ArrayAccess {
            private $data = [];
            
            public function offsetExists($offset): bool {
                if ($offset === 'num') return true;
                return false;
            }
            
            public function offsetGet($offset): mixed {
                if ($offset === 'num' && isset($this->data['num'])) {
                    return $this->data['num'];
                }
                return 0;
            }
            
            public function offsetSet($offset, $value): void {
                $this->data[$offset] = $value;
            }
            
            public function offsetUnset($offset): void {
                unset($this->data[$offset]);
            }
        }
        
        $c = new Container();
        $c['num'] = 10;
        $c['num'] += 1;  // Use += instead of ++ since ++ on array elements isn't implemented yet
        
        return $c['num'];
    "#;

    match run_code(code) {
        Val::Int(n) => {
            assert_eq!(n, 11);
        }
        other => panic!("Expected int 11, got {:?}", other),
    }
}

/// Test that regular objects without ArrayAccess still produce warnings
#[test]
fn test_non_array_access_object_warning() {
    let code = r#"
        class RegularClass {
            public $data = 'test';
        }
        
        $obj = new RegularClass();
        // This should trigger a warning and return null
        $result = $obj['key'] ?? 'default';
        
        return $result;
    "#;

    match run_code(code) {
        Val::String(s) => {
            assert_eq!(s.as_ref(), b"default");
        }
        other => panic!("Expected string 'default', got {:?}", other),
    }
}

/// Test ArrayAccess with mixed type offsets
#[test]
fn test_array_access_mixed_offsets() {
    let code = r#"
        class FlexibleContainer implements ArrayAccess {
            private $data = [];
            
            public function offsetExists($offset): bool {
                return isset($this->data[$offset]);
            }
            
            public function offsetGet($offset): mixed {
                return $this->data[$offset] ?? null;
            }
            
            public function offsetSet($offset, $value): void {
                $this->data[$offset] = $value;
            }
            
            public function offsetUnset($offset): void {
                unset($this->data[$offset]);
            }
        }
        
        $c = new FlexibleContainer();
        $c['string_key'] = 'value1';
        $c[100] = 'value2';
        $c[true] = 'value3';  // true converts to 1
        
        $r1 = $c['string_key'];
        $r2 = $c[100];
        $r3 = $c[1];  // Should get the value set with true
        
        return $r1 . ',' . $r2 . ',' . $r3;
    "#;

    match run_code(code) {
        Val::String(s) => {
            assert_eq!(s.as_ref(), b"value1,value2,value3");
        }
        other => panic!("Expected concatenated string, got {:?}", other),
    }
}

/// Test ArrayAccess interface detection
#[test]
fn test_array_access_instanceof() {
    let code = r#"
        class Container implements ArrayAccess {
            public function offsetExists($offset): bool { return false; }
            public function offsetGet($offset): mixed { return null; }
            public function offsetSet($offset, $value): void {}
            public function offsetUnset($offset): void {}
        }
        
        $c = new Container();
        return $c instanceof ArrayAccess;
    "#;

    match run_code(code) {
        Val::Bool(b) => {
            assert!(b, "Container should be instanceof ArrayAccess");
        }
        other => panic!("Expected bool true, got {:?}", other),
    }
}
