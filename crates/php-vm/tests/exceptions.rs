use php_vm::vm::engine::{VM, VmError};
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::core::value::Val;
use php_vm::compiler::emitter::Emitter;
use std::sync::Arc;
use std::rc::Rc;

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

    let mut emitter = Emitter::new(source.as_bytes(), &mut request_context.interner);
    let chunk = emitter.compile(program.statements);
    
    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk))?;
    
    let val = if let Some(handle) = vm.last_return_value {
        vm.arena.get(handle).value.clone()
    } else {
        Val::Null
    };
    Ok((val, vm))
}

#[test]
fn test_basic_try_catch() {
    let src = r#"<?php
        class Exception {}
        class MyException extends Exception {}
        
        $res = "init";
        try {
            throw new MyException();
            $res = "not reached";
        } catch (MyException $e) {
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
fn test_catch_parent() {
    let src = r#"<?php
        class Exception {}
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
        class Exception {}
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
        class Exception {}
        
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
