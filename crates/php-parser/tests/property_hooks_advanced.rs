use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn test_property_hooks_in_constructor_promotion() {
    let source = b"<?php
class User {
    public function __construct(
        public string $name {
            get => strtoupper($this->name);
        },
        private int $age = 0 {
            set(int $value) {
                if ($value < 0) throw new ValueError();
                $this->age = $value;
            }
        }
    ) {}
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_property_hooks_with_asymmetric_visibility() {
    let source = b"<?php
class Product {
    public private(set) string $name {
        get => $this->name;
        set => strtolower($value);
    }
    
    protected private(set) int $price {
        get => $this->price;
        set {
            if ($value < 0) throw new ValueError();
            $this->price = $value;
        }
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_multiple_hooks_on_same_property() {
    let source = b"<?php
class Example {
    public string $value {
        get => $this->value ?? 'default';
        set(string $val) => $this->value = trim($val);
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_property_hook_with_final_modifier() {
    let source = b"<?php
class Base {
    public string $name {
        final get => $this->name;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_property_hook_by_reference() {
    let source = b"<?php
class Container {
    private array $data = [];
    
    public mixed $value {
        &get => $this->data['value'];
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_property_hook_with_attributes() {
    let source = b"<?php
class Model {
    public string $email {
        #[Cached]
        get => strtolower($this->email);
        
        #[Validated]
        set(string $value) {
            if (!filter_var($value, FILTER_VALIDATE_EMAIL)) {
                throw new ValueError('Invalid email');
            }
            $this->email = $value;
        }
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_property_hook_with_visibility_modifiers() {
    let source = b"<?php
class Account {
    public string $name {
        public get => $this->name;
        protected set => $this->name = $value;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_abstract_property_hooks() {
    let source = b"<?php
abstract class AbstractModel {
    abstract public string $name {
        get;
        set;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_property_hook_with_default_value() {
    let source = b"<?php
class Config {
    public string $value = 'default' {
        get => $this->value;
        set => $this->value = strtoupper($value);
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_property_hook_empty_parameter_list() {
    let source = b"<?php
class Example {
    public string $value {
        get() => $this->value;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_property_hook_complex_body() {
    let source = b"<?php
class Calculator {
    private float $result = 0.0;
    
    public float $value {
        get {
            $this->log('Getting value');
            return $this->result;
        }
        set(float $val) {
            $this->log('Setting value');
            if ($val > 1000) {
                throw new RangeException();
            }
            $this->result = $val;
        }
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_property_hook_magic_constants() {
    let source = b"<?php
class Logger {
    public string $name {
        get => __PROPERTY__;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}
