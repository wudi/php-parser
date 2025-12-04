use bumpalo::Bump;
use php_parser::lexer::Lexer;
use php_parser::parser::Parser;

#[test]
fn test_anonymous_class_basic() {
    let source = b"<?php
$obj = new class {
    public $prop = 1;
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_anonymous_class_with_attribute() {
    let source = b"<?php
$obj = new #[Attr] class {
    public $prop = 1;
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_anonymous_class_with_multiple_attributes() {
    let source = b"<?php
$obj = new #[Attr1, Attr2] class {
    public $prop = 1;
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_anonymous_class_with_constructor() {
    let source = b"<?php
$obj = new #[Injectable] class($arg1, $arg2) {
    public function __construct($arg1, $arg2) {
        // constructor code
    }
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_anonymous_class_extends() {
    let source = b"<?php
$obj = new #[Proxy] class extends BaseClass {
    public function method() {}
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_anonymous_class_implements() {
    let source = b"<?php
$obj = new #[Service] class implements Interface1, Interface2 {
    public function method() {}
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_anonymous_class_extends_and_implements() {
    let source = b"<?php
$obj = new #[Component] class extends Base implements Interface1 {
    public function method() {}
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_anonymous_class_with_attribute_params() {
    let source = b"<?php
$obj = new #[Route('/api/users', methods: ['GET', 'POST'])] class {
    public function handle() {}
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_anonymous_class_in_function_return() {
    let source = b"<?php
function factory() {
    return new #[Singleton] class {
        public function create() {}
    };
}
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_anonymous_class_as_argument() {
    let source = b"<?php
doSomething(new #[Mock] class implements TestInterface {
    public function test() {}
});
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_anonymous_class_with_use_trait() {
    let source = b"<?php
$obj = new #[Entity] class {
    use MyTrait;
    
    public function method() {}
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_anonymous_class_nested_attributes() {
    let source = b"<?php
$obj = new #[
    Attr1,
    Attr2(param: 'value'),
    Attr3
] class {
    public function method() {}
};
";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}
