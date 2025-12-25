use std::rc::Rc;
use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::{ArrayKey, Val};
use php_vm::runtime::context::{EngineBuilder, RequestContext};
use php_vm::vm::engine::{ErrorHandler, ErrorLevel, VM};
use std::cell::RefCell;

// Custom error handler to capture warnings
struct TestErrorHandler {
    warnings: Rc<RefCell<Vec<(ErrorLevel, String)>>>,
}

impl TestErrorHandler {
    fn new(warnings_rc: Rc<RefCell<Vec<(ErrorLevel, String)>>>) -> Self {
        Self {
            warnings: warnings_rc,
        }
    }
}

impl ErrorHandler for TestErrorHandler {
    fn report(&mut self, level: ErrorLevel, message: &str) {
        self.warnings
            .borrow_mut()
            .push((level, message.to_string()));
    }
}

fn run_code(src: &str) -> (Val, Vec<(ErrorLevel, String)>, VM) {
    let engine_context = EngineBuilder::new().with_core_extensions().build().expect("Failed to build engine");
    let mut request_context = RequestContext::new(engine_context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(src.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter = Emitter::new(src.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(&program.statements);

    let shared_warnings = Rc::new(RefCell::new(Vec::new()));
    let handler_instance = TestErrorHandler::new(Rc::clone(&shared_warnings));
    let vm_error_handler = Box::new(handler_instance) as Box<dyn ErrorHandler>;

    let mut vm = VM::new_with_context(request_context);
    vm.set_error_handler(vm_error_handler);
    vm.run(Rc::new(chunk)).expect("Execution failed");

    let handle = vm.last_return_value.expect("No return value");
    let result_val = vm.arena.get(handle).value.clone();
    let cloned_warnings = shared_warnings.borrow().clone();
    (result_val, cloned_warnings, vm)
}

#[test]
fn test_strlen_string() {
    let src = "<?php return strlen('hello');";
    let (result, warnings, _) = run_code(src);
    assert_eq!(warnings.len(), 0);
    assert_eq!(result, Val::Int(5));

    let src = "<?php return strlen('');";
    let (result, warnings, _) = run_code(src);
    assert_eq!(warnings.len(), 0);
    assert_eq!(result, Val::Int(0));

    let src = "<?php return strlen('你好');"; // UTF-8 string
    let (result, warnings, _) = run_code(src);
    assert_eq!(warnings.len(), 0);
    // PHP strlen counts bytes, not characters for multi-byte strings
    assert_eq!(result, Val::Int(6));
}

#[test]
fn test_strlen_int() {
    let src = "<?php return strlen(12345);";
    let (result, warnings, _) = run_code(src);
    assert_eq!(warnings.len(), 0);
    assert_eq!(result, Val::Int(5));

    let src = "<?php return strlen(0);";
    let (result, warnings, _) = run_code(src);
    assert_eq!(warnings.len(), 0);
    assert_eq!(result, Val::Int(1));
}

#[test]
fn test_strlen_float() {
    let src = "<?php return strlen(123.45);";
    let (result, warnings, _) = run_code(src);
    assert_eq!(warnings.len(), 0);
    assert_eq!(result, Val::Int(6));

    let src = "<?php return strlen(0.0);";
    let (result, warnings, _) = run_code(src);
    assert_eq!(warnings.len(), 0);
    assert_eq!(result, Val::Int(1));

    let src = "<?php return strlen(-1.0);";
    let (result, warnings, _) = run_code(src);
    assert_eq!(warnings.len(), 0);
    assert_eq!(result, Val::Int(2));
}

#[test]
fn test_strlen_bool() {
    let src = "<?php return strlen(true);";
    let (result, warnings, _) = run_code(src);
    assert_eq!(warnings.len(), 0);
    assert_eq!(result, Val::Int(1));

    let src = "<?php return strlen(false);";
    let (result, warnings, _) = run_code(src);
    assert_eq!(warnings.len(), 0);
    assert_eq!(result, Val::Int(0));
}

#[test]
fn test_strlen_null() {
    let src = "<?php return strlen(null);";
    let (result, warnings, _) = run_code(src);
    assert_eq!(warnings.len(), 0);
    assert_eq!(result, Val::Int(0));
}

#[test]
fn test_strlen_array() {
    let src = "<?php return strlen([]);";
    let (result, warnings, _) = run_code(src);
    assert_eq!(result, Val::Null);
    assert_eq!(warnings.len(), 1);
}

#[test]
fn test_strlen_object() {
    let src = "<?php class MyClass {} return strlen(new MyClass());";
    let (result, warnings, _) = run_code(src);
    assert_eq!(result, Val::Null);
    assert_eq!(warnings.len(), 1);
}

#[test]
fn test_str_contains_basic() {
    let src = "<?php return str_contains('abc', 'a');";
    let (result, warnings, _) = run_code(src);
    assert_eq!(result, Val::Bool(true));

    let src = "<?php return str_contains('abc', 'd');";
    let (result, warnings, _) = run_code(src);
    assert_eq!(result, Val::Bool(false));
}

#[test]
fn test_str_contains_type_coercion() {
    let src = "<?php return str_contains(123, '2');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Bool(true));

    let src = "<?php return str_contains('true', true);";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Bool(false)); // 'true' does not contain '1'
}

#[test]
fn test_str_starts_with_basic() {
    let src = "<?php return str_starts_with('abcde', 'abc');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Bool(true));
}

#[test]
fn test_str_ends_with_basic() {
    let src = "<?php return str_ends_with('abcde', 'cde');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Bool(true));
}

#[test]
fn test_trim_basic() {
    let src = "<?php return trim('  hello  ');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"hello".to_vec().into()));
}

#[test]
fn test_trim_custom_mask() {
    let src = "<?php return trim('xxhelloxx', 'x');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"hello".to_vec().into()));
}

#[test]
fn test_str_replace_basic() {
    let src = "<?php return str_replace('l', 'x', 'hello');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"hexxo".to_vec().into()));
}

#[test]
fn test_str_replace_array() {
    let src = "<?php return str_replace(['a', 'b'], ['x', 'y'], 'abc');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"xyc".to_vec().into()));
}

#[test]
fn test_str_replace_subject_array() {
    let src = "<?php return str_replace('a', 'x', ['abc', 'def', 'aaa']);";
    let (result, _, vm) = run_code(src);
    match result {
        Val::Array(arr) => {
            assert_eq!(arr.map.len(), 3);
            assert_eq!(
                vm.arena.get(*arr.map.get(&ArrayKey::Int(0)).unwrap()).value,
                Val::String(b"xbc".to_vec().into())
            );
            assert_eq!(
                vm.arena.get(*arr.map.get(&ArrayKey::Int(1)).unwrap()).value,
                Val::String(b"def".to_vec().into())
            );
            assert_eq!(
                vm.arena.get(*arr.map.get(&ArrayKey::Int(2)).unwrap()).value,
                Val::String(b"xxx".to_vec().into())
            );
        }
        _ => panic!("Expected array, got {:?}", result),
    }
}

#[test]
fn test_str_replace_count() {
    let src = "<?php
        $count = 0;
        $res = str_replace('a', 'x', 'banana', $count);
        return [$res, $count];
    ";
    let (result, _, vm) = run_code(src);
    match result {
        Val::Array(arr) => {
            assert_eq!(
                vm.arena.get(*arr.map.get(&ArrayKey::Int(0)).unwrap()).value,
                Val::String(b"bxnxnx".to_vec().into())
            );
            assert_eq!(
                vm.arena.get(*arr.map.get(&ArrayKey::Int(1)).unwrap()).value,
                Val::Int(3)
            );
        }
        _ => panic!("Expected array, got {:?}", result),
    }
}

#[test]
fn test_str_ireplace_basic() {
    let src = "<?php return str_ireplace('L', 'x', 'hello');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"hexxo".to_vec().into()));
}

#[test]
fn test_substr_replace_basic() {
    let src = "<?php return substr_replace('hello', 'world', 0);";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"world".to_vec().into()));

    let src = "<?php return substr_replace('hello', 'world', 1, 2);";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"hworldlo".to_vec().into()));
}

#[test]
fn test_strtr_basic() {
    let src = "<?php return strtr('hello', 'eo', 'oa');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"holla".to_vec().into()));

    let src = "<?php return strtr('baab', ['ab' => '01']);";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"ba01".to_vec().into()));
}

#[test]
fn test_chr_basic() {
    let src = "<?php return chr(65);";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"A".to_vec().into()));

    let src = "<?php return chr(321);"; // 321 % 256 = 65
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"A".to_vec().into()));
}

#[test]
fn test_ord_basic() {
    let src = "<?php return ord('A');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Int(65));

    let src = "<?php return ord('');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Int(0));
}

#[test]
fn test_bin2hex_basic() {
    let src = "<?php return bin2hex('hello');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"68656c6c6f".to_vec().into()));
}

#[test]
fn test_hex2bin_basic() {
    let src = "<?php return hex2bin('68656c6c6f');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"hello".to_vec().into()));

    let src = "<?php return hex2bin('invalid');";
    let (result, warnings, _) = run_code(src);
    assert_eq!(result, Val::Bool(false));
    assert_eq!(warnings.len(), 1);
}

#[test]
fn test_addslashes_basic() {
    let src = "<?php return addslashes(\"O'Reilly\");";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"O\\'Reilly".to_vec().into()));
}

#[test]
fn test_stripslashes_basic() {
    let src = "<?php return stripslashes(\"O\\'Reilly\");";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"O'Reilly".to_vec().into()));
}

#[test]
fn test_addcslashes_basic() {
    let src = "<?php return addcslashes('hello', 'e');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"h\\ello".to_vec().into()));

    let src = "<?php return addcslashes('abcde', 'a..c');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"\\a\\b\\cde".to_vec().into()));
}

#[test]
fn test_stripcslashes_basic() {
    let src = "<?php return stripcslashes('h\\\\ello');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"hello".to_vec().into()));
}

#[test]
fn test_str_pad_basic() {
    let src = "<?php return str_pad('alien', 10);";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"alien     ".to_vec().into()));

    let src = "<?php return str_pad('alien', 10, '-=', STR_PAD_LEFT);";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"-=-=-alien".to_vec().into()));

    let src = "<?php return str_pad('alien', 10, '_', STR_PAD_BOTH);";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"__alien___".to_vec().into()));
}

#[test]
fn test_str_rot13_basic() {
    let src = "<?php return str_rot13('PHP 8.0');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"CUC 8.0".to_vec().into()));
}

#[test]
fn test_str_shuffle_basic() {
    let src = "<?php return strlen(str_shuffle('hello'));";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Int(5));
}

#[test]
fn test_str_split_basic() {
    let src = "<?php return str_split('hello', 2);";
    let (result, _, vm) = run_code(src);
    match result {
        Val::Array(arr) => {
            assert_eq!(arr.map.len(), 3);
            assert_eq!(
                vm.arena.get(*arr.map.get(&ArrayKey::Int(0)).unwrap()).value,
                Val::String(b"he".to_vec().into())
            );
            assert_eq!(
                vm.arena.get(*arr.map.get(&ArrayKey::Int(1)).unwrap()).value,
                Val::String(b"ll".to_vec().into())
            );
            assert_eq!(
                vm.arena.get(*arr.map.get(&ArrayKey::Int(2)).unwrap()).value,
                Val::String(b"o".to_vec().into())
            );
        }
        _ => panic!("Expected array, got {:?}", result),
    }
}

#[test]
fn test_strrev_basic() {
    let src = "<?php return strrev('hello');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"olleh".to_vec().into()));
}

#[test]
fn test_strcmp_basic() {
    let src = "<?php return strcmp('abc', 'abc');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Int(0));

    let src = "<?php return strcmp('abc', 'abd');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Int(-1));

    let src = "<?php return strcmp('abd', 'abc');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Int(1));
}

#[test]
fn test_strcasecmp_basic() {
    let src = "<?php return strcasecmp('abc', 'ABC');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Int(0));
}

#[test]
fn test_strncmp_basic() {
    let src = "<?php return strncmp('abcde', 'abcfg', 2);";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Int(0));

    let src = "<?php return strncmp('abcde', 'abcfg', 3);";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Int(0));

    let src = "<?php return strncmp('abcde', 'abcfg', 4);";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Int(-1));
}

#[test]
fn test_strncasecmp_basic() {
    let src = "<?php return strncasecmp('abcde', 'ABCFG', 2);";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Int(0));
}

#[test]
fn test_strstr_basic() {
    let src = "<?php return strstr('name@example.com', '@');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"@example.com".to_vec().into()));

    let src = "<?php return strstr('name@example.com', '@', true);";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"name".to_vec().into()));
}

#[test]
fn test_stristr_basic() {
    let src = "<?php return stristr('USER@EXAMPLE.COM', '@example');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"@EXAMPLE.COM".to_vec().into()));
}

#[test]
fn test_substr_count_basic() {
    let src = "<?php return substr_count('This is a test', 'is');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Int(2));

    let src = "<?php return substr_count('This is a test', 'is', 3);";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::Int(1));
}

#[test]
fn test_ucfirst_basic() {
    let src = "<?php return ucfirst('hello');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"Hello".to_vec().into()));
}

#[test]
fn test_lcfirst_basic() {
    let src = "<?php return lcfirst('Hello');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"hello".to_vec().into()));
}

#[test]
fn test_ucwords_basic() {
    let src = "<?php return ucwords('hello world');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"Hello World".to_vec().into()));

    let src = "<?php return ucwords('hello-world', '-');";
    let (result, _, _) = run_code(src);
    assert_eq!(result, Val::String(b"Hello-World".to_vec().into()));
}

#[test]
fn test_wordwrap_basic() {
    let src =
        "<?php return wordwrap('The quick brown fox jumped over the lazy dog.', 20, \"<br />\\n\");";
    let (result, _, _) = run_code(src);
    assert_eq!(
        result,
        Val::String(
            b"The quick brown fox<br />\njumped over the lazy<br />\ndog."
                .to_vec()
                .into()
        )
    );
}

#[test]
fn test_strtok_basic() {
    let src = "<?php
        $tok = strtok('This is a test', ' ');
        $res = [];
        while ($tok !== false) {
            $res[] = $tok;
            $tok = strtok(' ');
        }
        return $res;
    ";
    let (result, _, vm) = run_code(src);
    match result {
        Val::Array(arr) => {
            assert_eq!(arr.map.len(), 4);
            assert_eq!(
                vm.arena.get(*arr.map.get(&ArrayKey::Int(0)).unwrap()).value,
                Val::String(b"This".to_vec().into())
            );
            assert_eq!(
                vm.arena.get(*arr.map.get(&ArrayKey::Int(1)).unwrap()).value,
                Val::String(b"is".to_vec().into())
            );
            assert_eq!(
                vm.arena.get(*arr.map.get(&ArrayKey::Int(2)).unwrap()).value,
                Val::String(b"a".to_vec().into())
            );
            assert_eq!(
                vm.arena.get(*arr.map.get(&ArrayKey::Int(3)).unwrap()).value,
                Val::String(b"test".to_vec().into())
            );
        }
        _ => panic!("Expected array, got {:?}", result),
    }
}

#[test]
fn test_strlen_multiple_args() {
    let src = "<?php return strlen('a', 'b');";
    let (result, warnings, _) = run_code(src);
    assert_eq!(result, Val::Null);
    assert_eq!(warnings.len(), 1);
}
