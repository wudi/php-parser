use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;
use insta::assert_debug_snapshot;

#[test]
fn test_basic_parse() {
    let source = b"<?php echo 1 + 2;";
    let arena = Bump::new();
    
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_complex_expression() {
    let source = b"<?php echo 1 + 2 * 3 . 4;";
    let arena = Bump::new();
    
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_unary_and_strings() {
    let source = b"<?php echo -1 . 'hello' . !true;";
    let arena = Bump::new();
    
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_control_structures() {
    let source = b"<?php 
    if ($a > 0) {
        echo 'positive';
    } else {
        echo 'negative';
    }
    
    while ($i < 10) {
        $i = $i + 1;
    }
    
    return 0;
    ";
    let arena = Bump::new();
    
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_functions() {
    let source = b"<?php
    function add($a, $b) {
        return $a + $b;
    }
    
    echo add(1, 2);
    ";
    let arena = Bump::new();
    
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_arrays_and_objects() {
    let source = b"<?php
    $arr = [1, 2, 3];
    $map = array('a' => 1, 'b' => 2);
    echo $arr[0];
    
    $obj = new MyClass();
    echo $obj->prop;
    echo $obj->method(1);
    echo MyClass::CONST;
    echo MyClass::staticMethod();
    
    $x = $y = 1;
    ";
    let arena = Bump::new();
    
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_foreach() {
    let code = "<?php
    foreach ($arr as $value) {
        echo $value;
    }
    foreach ($arr as $key => $value) {
        echo $key;
        echo $value;
    }
    ";
    let arena = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    assert_debug_snapshot!("foreach", program);
}

#[test]
fn test_class() {
    let code = "<?php
    class User {
        public $name;
        private $age = 20;
        const TYPE = 1;
        
        public function getName() {
            return $this->name;
        }
    }
    ";
    let arena = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    assert_debug_snapshot!("class", program);
}

#[test]
fn test_switch() {
    let code = "<?php
    switch ($a) {
        case 1:
            echo 'one';
            break;
        case 2:
            echo 'two';
            break;
        default:
            echo 'default';
    }
    ";
    let arena = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    assert_debug_snapshot!("switch", program);
}

#[test]
fn test_try_catch() {
    let code = "<?php
    try {
        throw new Exception('error');
    } catch (Exception $e) {
        echo $e->getMessage();
    } finally {
        echo 'done';
    }
    ";
    let arena = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    assert_debug_snapshot!("try_catch", program);
}
