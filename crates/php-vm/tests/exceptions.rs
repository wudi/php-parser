use php_vm::compiler::emitter::Emitter;
use php_vm::core::value::Val;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::{VmError, VM};
use std::rc::Rc;
use std::sync::Arc;

fn run_code(source: &str) -> Result<(Val, VM), VmError> {
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

    let val = if let Some(handle) = vm.last_return_value {
        vm.arena.get(handle).value.clone()
    } else {
        Val::Null
    };
    Ok((val, vm))
}

// ============================================================================
// Basic Exception Handling Tests
// ============================================================================

#[test]
fn test_basic_try_catch() {
    let src = r#"<?php
        $res = "init";
        try {
            throw new Exception();
            $res = "not reached";
        } catch (Exception $e) {
            $res = "caught";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught");
    } else {
        panic!("Expected string 'caught', got {:?}", res);
    }
}

#[test]
fn test_catch_parent_class() {
    let src = r#"<?php
        class MyException extends Exception {}
        
        $res = "init";
        try {
            throw new MyException();
        } catch (Exception $e) {
            $res = "caught parent";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught parent");
    } else {
        panic!("Expected string 'caught parent', got {:?}", res);
    }
}

#[test]
fn test_uncaught_exception() {
    let src = r#"<?php
        throw new Exception();
    "#;

    let res = run_code(src);
    assert!(res.is_err());
    if let Err(VmError::Exception(_)) = res {
        // OK
    } else {
        match res {
            Ok(_) => panic!("Expected VmError::Exception, got Ok"),
            Err(e) => panic!("Expected VmError::Exception, got {:?}", e),
        }
    }
}

#[test]
fn test_nested_try_catch() {
    let src = r#"<?php
        $res = "";
        try {
            try {
                throw new Exception();
            } catch (Exception $e) {
                $res = "inner";
                throw $e; // Rethrow
            }
        } catch (Exception $e) {
            $res .= " outer";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "inner outer");
    } else {
        panic!("Expected string 'inner outer', got {:?}", res);
    }
}

// ============================================================================
// Multi-Catch Tests
// ============================================================================

#[test]
fn test_multi_catch_first_match() {
    let src = r#"<?php
        class ExceptionA extends Exception {}
        class ExceptionB extends Exception {}
        
        $res = "";
        try {
            throw new ExceptionA();
        } catch (ExceptionA $e) {
            $res = "A";
        } catch (ExceptionB $e) {
            $res = "B";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "A");
    } else {
        panic!("Expected 'A', got {:?}", res);
    }
}

#[test]
fn test_multi_catch_second_match() {
    let src = r#"<?php
        class ExceptionA extends Exception {}
        class ExceptionB extends Exception {}
        
        $res = "";
        try {
            throw new ExceptionB();
        } catch (ExceptionA $e) {
            $res = "A";
        } catch (ExceptionB $e) {
            $res = "B";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "B");
    } else {
        panic!("Expected 'B', got {:?}", res);
    }
}

#[test]
fn test_multi_catch_parent_fallback() {
    let src = r#"<?php
        class ExceptionA extends Exception {}
        class ExceptionB extends Exception {}
        class ExceptionC extends Exception {}
        
        $res = "";
        try {
            throw new ExceptionC();
        } catch (ExceptionA $e) {
            $res = "A";
        } catch (ExceptionB $e) {
            $res = "B";
        } catch (Exception $e) {
            $res = "parent";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "parent");
    } else {
        panic!("Expected 'parent', got {:?}", res);
    }
}

// ============================================================================
// Finally Block Tests
// ============================================================================

#[test]
fn test_finally_after_try_success() {
    let src = r#"<?php
        $res = "";
        try {
            $res = "try";
        } finally {
            $res .= " finally";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "try finally");
    } else {
        panic!("Expected 'try finally', got {:?}", res);
    }
}

#[test]
fn test_finally_after_catch() {
    let src = r#"<?php
        $res = "";
        try {
            throw new Exception();
        } catch (Exception $e) {
            $res = "catch";
        } finally {
            $res .= " finally";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "catch finally");
    } else {
        panic!("Expected 'catch finally', got {:?}", res);
    }
}

#[test]
fn test_finally_without_catch() {
    let src = r#"<?php
        $res = "";
        try {
            $res = "try";
        } finally {
            $res .= " finally";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "try finally");
    } else {
        panic!("Expected 'try finally', got {:?}", res);
    }
}

#[test]
fn test_nested_finally() {
    let src = r#"<?php
        $res = "";
        try {
            try {
                $res = "inner-try";
            } finally {
                $res .= " inner-finally";
            }
        } finally {
            $res .= " outer-finally";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "inner-try inner-finally outer-finally");
    } else {
        panic!("Expected correct finally execution order, got {:?}", res);
    }
}

// ============================================================================
// Error Hierarchy Tests (PHP 7+)
// ============================================================================

#[test]
fn test_catch_error_class() {
    let src = r#"<?php
        $res = "";
        try {
            throw new Error("test error");
        } catch (Error $e) {
            $res = "caught error";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught error");
    } else {
        panic!("Expected 'caught error', got {:?}", res);
    }
}

#[test]
fn test_catch_throwable() {
    let src = r#"<?php
        $res = "";
        try {
            throw new Exception();
        } catch (Throwable $t) {
            $res = "caught throwable";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught throwable");
    } else {
        panic!("Expected 'caught throwable', got {:?}", res);
    }
}

#[test]
fn test_error_caught_by_throwable() {
    let src = r#"<?php
        $res = "";
        try {
            throw new Error();
        } catch (Throwable $t) {
            $res = "caught";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught");
    } else {
        panic!("Expected 'caught', got {:?}", res);
    }
}

#[test]
fn test_type_error() {
    let src = r#"<?php
        $res = "";
        try {
            throw new TypeError("type mismatch");
        } catch (TypeError $e) {
            $res = "caught type error";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught type error");
    } else {
        panic!("Expected 'caught type error', got {:?}", res);
    }
}

#[test]
fn test_arithmetic_error() {
    let src = r#"<?php
        $res = "";
        try {
            throw new ArithmeticError("arithmetic problem");
        } catch (ArithmeticError $e) {
            $res = "caught arithmetic error";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught arithmetic error");
    } else {
        panic!("Expected 'caught arithmetic error', got {:?}", res);
    }
}

#[test]
fn test_division_by_zero_error() {
    let src = r#"<?php
        $res = "";
        try {
            throw new DivisionByZeroError("division by zero");
        } catch (DivisionByZeroError $e) {
            $res = "caught division by zero";
        } catch (ArithmeticError $e) {
            $res = "caught arithmetic";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught division by zero");
    } else {
        panic!("Expected 'caught division by zero', got {:?}", res);
    }
}

#[test]
fn test_division_by_zero_caught_by_parent() {
    let src = r#"<?php
        $res = "";
        try {
            throw new DivisionByZeroError("division by zero");
        } catch (ArithmeticError $e) {
            $res = "caught by parent";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught by parent");
    } else {
        panic!("Expected 'caught by parent', got {:?}", res);
    }
}

// ============================================================================
// Exception/Error SPL Hierarchy Tests
// ============================================================================

#[test]
fn test_runtime_exception() {
    let src = r#"<?php
        $res = "";
        try {
            throw new RuntimeException("runtime issue");
        } catch (RuntimeException $e) {
            $res = "caught runtime";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught runtime");
    } else {
        panic!("Expected 'caught runtime', got {:?}", res);
    }
}

#[test]
fn test_logic_exception() {
    let src = r#"<?php
        $res = "";
        try {
            throw new LogicException("logic issue");
        } catch (LogicException $e) {
            $res = "caught logic";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught logic");
    } else {
        panic!("Expected 'caught logic', got {:?}", res);
    }
}

#[test]
fn test_spl_exception_caught_by_exception() {
    let src = r#"<?php
        $res = "";
        try {
            throw new RuntimeException("runtime");
        } catch (Exception $e) {
            $res = "caught by base";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught by base");
    } else {
        panic!("Expected 'caught by base', got {:?}", res);
    }
}

// ============================================================================
// Exception Variable Tests
// ============================================================================

#[test]
fn test_catch_without_variable() {
    let src = r#"<?php
        $res = "";
        try {
            throw new Exception("test");
        } catch (Exception) {
            $res = "caught";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught");
    } else {
        panic!("Expected 'caught', got {:?}", res);
    }
}

// ============================================================================
// Rethrow Tests
// ============================================================================

#[test]
fn test_rethrow_in_catch() {
    let src = r#"<?php
        $res = "";
        try {
            try {
                throw new Exception("original");
            } catch (Exception $e) {
                $res = "inner ";
                throw $e;
            }
        } catch (Exception $e) {
            $res .= "outer";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "inner outer");
    } else {
        panic!("Expected 'inner outer', got {:?}", res);
    }
}

#[test]
fn test_throw_new_exception_in_catch() {
    let src = r#"<?php
        $res = "";
        try {
            try {
                throw new Exception("first");
            } catch (Exception $e) {
                $res = "caught first, ";
                throw new RuntimeException("second");
            }
        } catch (RuntimeException $e) {
            $res .= "caught second";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught first, caught second");
    } else {
        panic!("Expected correct catch sequence, got {:?}", res);
    }
}

// ============================================================================
// Non-Throwable Object Tests
// ============================================================================

#[test]
fn test_throw_non_throwable_object() {
    let src = r#"<?php
        class NotAnException {}
        throw new NotAnException();
    "#;

    let res = run_code(src);
    assert!(res.is_err());
    if let Err(VmError::RuntimeError(msg)) = res {
        assert!(msg.contains("Throwable") || msg.contains("throwable"));
    } else {
        panic!("Expected RuntimeError about Throwable");
    }
}

#[test]
fn test_throw_string() {
    let src = r#"<?php
        throw "not an object";
    "#;

    let res = run_code(src);
    assert!(res.is_err());
    if let Err(VmError::RuntimeError(msg)) = res {
        assert!(msg.contains("object"));
    } else {
        panic!("Expected RuntimeError about objects");
    }
}

// ============================================================================
// Control Flow Tests
// ============================================================================

#[test]
fn test_exception_skips_remaining_try_code() {
    let src = r#"<?php
        $res = "";
        try {
            $res = "before";
            throw new Exception();
            $res .= " after";
        } catch (Exception $e) {
            $res .= " caught";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "before caught");
    } else {
        panic!("Expected 'before caught', got {:?}", res);
    }
}

#[test]
fn test_no_exception_skips_catch() {
    let src = r#"<?php
        $res = "try";
        try {
            $res .= " success";
        } catch (Exception $e) {
            $res .= " caught";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "try success");
    } else {
        panic!("Expected 'try success', got {:?}", res);
    }
}

#[test]
fn test_exception_in_function() {
    let src = r#"<?php
        function throw_exception() {
            throw new Exception("from function");
        }
        
        $res = "";
        try {
            throw_exception();
            $res = "not reached";
        } catch (Exception $e) {
            $res = "caught";
        }
        return $res;
    "#;

    let (res, _) = run_code(src).unwrap();
    if let Val::String(s) = res {
        assert_eq!(std::str::from_utf8(&s).unwrap(), "caught");
    } else {
        panic!("Expected 'caught', got {:?}", res);
    }
}
