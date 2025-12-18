/// Tests for predefined PHP interfaces and classes
/// Reference: $PHP_SRC_PATH/Zend/zend_interfaces.c
///
/// This test suite ensures compatibility with native PHP behavior for:
/// - Traversable, Iterator, IteratorAggregate
/// - Throwable, Countable, ArrayAccess, Serializable
/// - Closure, stdClass, Generator, Fiber
/// - WeakReference, WeakMap, Stringable
/// - UnitEnum, BackedEnum
/// - SensitiveParameterValue, __PHP_Incomplete_Class
use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::{VmError, VM};
use std::rc::Rc;
use std::sync::Arc;

fn run_code(source: &str) -> Result<(), VmError> {
    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter = Emitter::new(source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk))?;

    Ok(())
}

//=============================================================================
// Interface Existence Tests
//=============================================================================

#[test]
fn test_traversable_interface_exists() {
    let source = r#"
        <?php
        if (!interface_exists('Traversable')) {
            throw new Exception('Traversable interface not found');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_iterator_interface_exists() {
    let source = r#"
        <?php
        if (!interface_exists('Iterator')) {
            throw new Exception('Iterator interface not found');
        }
        if (!is_subclass_of('Iterator', 'Traversable')) {
            throw new Exception('Iterator must extend Traversable');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_iterator_aggregate_interface_exists() {
    let source = r#"
        <?php
        if (!interface_exists('IteratorAggregate')) {
            throw new Exception('IteratorAggregate interface not found');
        }
        if (!is_subclass_of('IteratorAggregate', 'Traversable')) {
            throw new Exception('IteratorAggregate must extend Traversable');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_throwable_interface_exists() {
    let source = r#"
        <?php
        if (!interface_exists('Throwable')) {
            throw new Exception('Throwable interface not found');
        }
        // In PHP 8+, Throwable extends Stringable
        if (!is_subclass_of('Throwable', 'Stringable')) {
            throw new Exception('Throwable must extend Stringable');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_countable_interface_exists() {
    let source = r#"
        <?php
        if (!interface_exists('Countable')) {
            throw new Exception('Countable interface not found');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_array_access_interface_exists() {
    let source = r#"
        <?php
        if (!interface_exists('ArrayAccess')) {
            throw new Exception('ArrayAccess interface not found');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_serializable_interface_exists() {
    let source = r#"
        <?php
        if (!interface_exists('Serializable')) {
            throw new Exception('Serializable interface not found');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_stringable_interface_exists() {
    let source = r#"
        <?php
        if (!interface_exists('Stringable')) {
            throw new Exception('Stringable interface not found');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

//=============================================================================
// Class Existence Tests
//=============================================================================

#[test]
fn test_closure_class_exists() {
    let source = r#"
        <?php
        if (!class_exists('Closure')) {
            throw new Exception('Closure class not found');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_stdclass_exists() {
    let source = r#"
        <?php
        if (!class_exists('stdClass')) {
            throw new Exception('stdClass not found');
        }
        $obj = new stdClass();
        $obj->foo = 'bar';
        if ($obj->foo !== 'bar') {
            throw new Exception('Dynamic properties not working');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_generator_class_exists() {
    let source = r#"
        <?php
        if (!class_exists('Generator')) {
            throw new Exception('Generator class not found');
        }
        if (!is_subclass_of('Generator', 'Iterator')) {
            throw new Exception('Generator must implement Iterator');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_fiber_class_exists() {
    let source = r#"
        <?php
        if (!class_exists('Fiber')) {
            throw new Exception('Fiber class not found');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_weak_reference_class_exists() {
    let source = r#"
        <?php
        if (!class_exists('WeakReference')) {
            throw new Exception('WeakReference class not found');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_weak_map_class_exists() {
    let source = r#"
        <?php
        if (!class_exists('WeakMap')) {
            throw new Exception('WeakMap class not found');
        }
        if (!is_subclass_of('WeakMap', 'ArrayAccess')) {
            throw new Exception('WeakMap must implement ArrayAccess');
        }
        if (!is_subclass_of('WeakMap', 'Countable')) {
            throw new Exception('WeakMap must implement Countable');
        }
        if (!is_subclass_of('WeakMap', 'IteratorAggregate')) {
            throw new Exception('WeakMap must implement IteratorAggregate');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_sensitive_parameter_value_class_exists() {
    let source = r#"
        <?php
        if (!class_exists('SensitiveParameterValue')) {
            throw new Exception('SensitiveParameterValue class not found');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_incomplete_class_exists() {
    let source = r#"
        <?php
        if (!class_exists('__PHP_Incomplete_Class')) {
            throw new Exception('__PHP_Incomplete_Class not found');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

//=============================================================================
// Enum Interface Tests
//=============================================================================

#[test]
fn test_unit_enum_interface_exists() {
    let source = r#"
        <?php
        if (!interface_exists('UnitEnum')) {
            throw new Exception('UnitEnum interface not found');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_backed_enum_interface_exists() {
    let source = r#"
        <?php
        if (!interface_exists('BackedEnum')) {
            throw new Exception('BackedEnum interface not found');
        }
        if (!is_subclass_of('BackedEnum', 'UnitEnum')) {
            throw new Exception('BackedEnum must extend UnitEnum');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

//=============================================================================
// Interface Implementation Tests
//=============================================================================

#[test]
fn test_iterator_implementation() {
    let source = r#"
        <?php
        class MyIterator implements Iterator {
            private $position = 0;
            private $array = ['a', 'b', 'c'];
            
            public function rewind(): void {
                $this->position = 0;
            }
            
            public function current(): mixed {
                return $this->array[$this->position];
            }
            
            public function key(): mixed {
                return $this->position;
            }
            
            public function next(): void {
                ++$this->position;
            }
            
            public function valid(): bool {
                return isset($this->array[$this->position]);
            }
        }
        
        $it = new MyIterator();
        if (!($it instanceof Iterator)) {
            throw new Exception('MyIterator must be instanceof Iterator');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_exception_implements_throwable() {
    let source = r#"
        <?php
        if (!is_subclass_of('Exception', 'Throwable')) {
            throw new Exception('Exception must implement Throwable');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_error_implements_throwable() {
    let source = r#"
        <?php
        if (!is_subclass_of('Error', 'Throwable')) {
            throw new Exception('Error must implement Throwable');
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}

#[test]
fn test_all_predefined_interfaces_and_classes_exist() {
    let source = r#"
        <?php
        // Test all interfaces
        $interfaces = [
            'Traversable',
            'Iterator',
            'IteratorAggregate',
            'Throwable',
            'Countable',
            'ArrayAccess',
            'Serializable',
            'Stringable',
            'UnitEnum',
            'BackedEnum'
        ];
        
        foreach ($interfaces as $iface) {
            if (!interface_exists($iface)) {
                throw new Exception("Interface $iface not found");
            }
        }
        
        // Test all classes
        $classes = [
            'Closure',
            'stdClass',
            'Generator',
            'Fiber',
            'WeakReference',
            'WeakMap',
            'SensitiveParameterValue',
            '__PHP_Incomplete_Class',
            'Exception',
            'Error'
        ];
        
        foreach ($classes as $class) {
            if (!class_exists($class)) {
                throw new Exception("Class $class not found");
            }
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}
