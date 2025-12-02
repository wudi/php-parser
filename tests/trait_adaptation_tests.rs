use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::parser::Parser;

#[test]
fn test_basic_trait_use() {
    let source = b"<?php
class MyClass {
    use MyTrait;
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_multiple_trait_use() {
    let source = b"<?php
class MyClass {
    use TraitA, TraitB, TraitC;
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_trait_alias_with_new_name() {
    let source = b"<?php
class MyClass {
    use MyTrait {
        foo as bar;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_trait_alias_with_visibility_only() {
    let source = b"<?php
class MyClass {
    use MyTrait {
        foo as private;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_trait_alias_with_visibility_and_name() {
    let source = b"<?php
class MyClass {
    use MyTrait {
        foo as protected bar;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_trait_precedence_insteadof() {
    let source = b"<?php
class MyClass {
    use TraitA, TraitB {
        TraitA::foo insteadof TraitB;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_trait_complex_adaptations() {
    let source = b"<?php
class MyClass {
    use TraitA, TraitB, TraitC {
        TraitA::foo insteadof TraitB, TraitC;
        TraitB::bar as baz;
        TraitC::qux as protected;
        TraitA::foo as public newFoo;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_trait_alias_semi_reserved_keyword_as_name() {
    // Testing using semi-reserved keywords as alias names (allowed as method names)
    let source = b"<?php
class MyClass {
    use MyTrait {
        foo as string;
        bar as int;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_trait_with_namespace() {
    let source = b"<?php
class MyClass {
    use \\Vendor\\Package\\MyTrait;
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_trait_multiple_adaptations_same_method() {
    let source = b"<?php
class MyClass {
    use MyTrait {
        foo as protected;
        foo as bar;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_trait_insteadof_multiple_traits() {
    let source = b"<?php
class MyClass {
    use TraitA, TraitB, TraitC, TraitD {
        TraitA::foo insteadof TraitB, TraitC, TraitD;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_trait_visibility_change_to_public() {
    let source = b"<?php
class MyClass {
    use MyTrait {
        privateMethod as public;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_trait_empty_adaptations_block() {
    let source = b"<?php
class MyClass {
    use MyTrait {
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}

#[test]
fn test_trait_multiple_namespaced() {
    let source = b"<?php
namespace Vendor\\Package {
    class MyClass {
        use Traits\\TraitA, Traits\\TraitB, Traits\\TraitC;
    }
}";

    let bump = Bump::new();
    let lexer = Lexer::new(source);
    let mut parser = Parser::new(lexer, &bump);
    let program = parser.parse_program();
    insta::assert_debug_snapshot!(program);
}
