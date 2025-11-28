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

#[test]
fn test_loops() {
    let source = b"<?php
    do {
        echo $i;
    } while ($i > 0);

    for ($i = 0; $i < 10; $i++) {
        echo $i;
    }
    ";
    let arena = Bump::new();
    
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_control_flow_statements() {
    let source = b"<?php
    break;
    break 2;
    continue;
    continue 2;
    global $a, $b;
    static $c = 1, $d;
    unset($a, $b);
    ";
    let arena = Bump::new();
    
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_ternary_and_coalesce() {
    let source = b"<?php
    $a = $b ? $c : $d;
    $a = $b ?: $d;
    $a = $b ?? $c ?? $d;
    $a = $b <=> $c;
    $a = $b ** $c;
    ";
    let arena = Bump::new();
    
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_match_expression() {
    let source = b"<?php
    $result = match ($status) {
        1 => 'pending',
        2, 3 => 'active',
        default => 'unknown',
    };
    ";
    let arena = Bump::new();
    
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_instanceof() {
    let source = b"<?php
    $a = $b instanceof A;
    $a = $b instanceof $c;
    $a = !$b instanceof A; // !($b instanceof A)
    $a = $b instanceof A && $c; // ($b instanceof A) && $c
    ";
    let arena = Bump::new();
    
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_casts() {
    let source = b"<?php
    $a = (int) $b;
    $a = (bool) $b;
    $a = (float) $b;
    $a = (string) $b;
    $a = (array) $b;
    $a = (object) $b;
    $a = (unset) $b;
    $a = (int) $b + 1; // ((int) $b) + 1
    ";
    let arena = Bump::new();
    
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_special_constructs() {
    let code = "
<?php
empty($a);
isset($a, $b);
eval('echo 1;');
die();
die('error');
exit;
exit(1);
";
    let mut lexer = Lexer::new(code.as_bytes());
    let bump = Bump::new();
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    assert_debug_snapshot!("special_constructs", program);
}

#[test]
fn test_closures_and_arrow_functions() {
    let code = "
<?php
$a = function($b) { return $b; };
$c = function($d) use ($e) { return $d + $e; };
$f = fn($x) => $x * 2;
$g = fn($y): int => $y + 1;
";
    let mut lexer = Lexer::new(code.as_bytes());
    let bump = Bump::new();
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    assert_debug_snapshot!("closures_and_arrow_functions", program);
}

#[test]
fn test_break_continue() {
    let source = b"<?php
    while (true) {
        break;
        break 2;
        continue;
        continue 2;
    }
    ";
    let arena = Bump::new();
    
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}

#[test]
fn test_global_static_unset() {
    let source = b"<?php
    function foo() {
        global $a, $b;
        static $c = 1, $d;
        unset($a, $b);
    }
    ";
    let arena = Bump::new();
    
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &arena);
    
    let program = parser.parse_program();
    assert_debug_snapshot!(program);
}
