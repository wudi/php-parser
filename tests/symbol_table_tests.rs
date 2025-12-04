use php_parser::parser::Parser;
use php_parser::lexer::Lexer;
use php_parser::ast::symbol_table::{SymbolVisitor, SymbolKind};
use php_parser::ast::visitor::Visitor;
use bumpalo::Bump;

#[test]
fn test_function_symbol() {
    let code = "<?php function foo($a) { $b = 1; }";
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();
    
    let mut visitor = SymbolVisitor::new(code.as_bytes());
    visitor.visit_program(&result);
    
    // Root scope should have "foo"
    let foo = visitor.table.lookup("foo");
    assert!(foo.is_some());
    assert_eq!(foo.unwrap().kind, SymbolKind::Function);
    
    // "foo" scope should have "$a" and "$b"
    // We need to find the scope index for "foo".
    // The root scope is 0. It has children.
    let root = &visitor.table.scopes[0];
    assert!(!root.children.is_empty());
    
    let func_scope_idx = root.children[0];
    let func_scope = &visitor.table.scopes[func_scope_idx];
    
    assert!(func_scope.get("$a").is_some());
    assert_eq!(func_scope.get("$a").unwrap().kind, SymbolKind::Parameter);
    
    assert!(func_scope.get("$b").is_some());
    assert_eq!(func_scope.get("$b").unwrap().kind, SymbolKind::Variable);
}

#[test]
fn test_class_symbol() {
    let code = "<?php class MyClass { public $prop; function method($p) {} }";
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();
    
    let mut visitor = SymbolVisitor::new(code.as_bytes());
    visitor.visit_program(&result);
    
    // Root scope has MyClass
    let cls = visitor.table.lookup("MyClass");
    assert!(cls.is_some());
    assert_eq!(cls.unwrap().kind, SymbolKind::Class);
    
    // Class scope
    let root = &visitor.table.scopes[0];
    let class_scope_idx = root.children[0];
    let class_scope = &visitor.table.scopes[class_scope_idx];
    
    // Property $prop
    assert!(class_scope.get("$prop").is_some());
    
    // Method method
    assert!(class_scope.get("method").is_some());
    
    // Method scope
    let method_scope_idx = class_scope.children[0];
    let method_scope = &visitor.table.scopes[method_scope_idx];
    
    assert!(method_scope.get("$p").is_some());
}

#[test]
fn test_enum_symbol() {
    let code = "<?php enum MyEnum { case A; case B; }";
    let bump = Bump::new();
    let lexer = Lexer::new(code.as_bytes());
    let mut parser = Parser::new(lexer, &bump);
    let result = parser.parse_program();
    
    let mut visitor = SymbolVisitor::new(code.as_bytes());
    visitor.visit_program(&result);
    
    let enm = visitor.table.lookup("MyEnum");
    assert!(enm.is_some());
    assert_eq!(enm.unwrap().kind, SymbolKind::Enum);
    
    let root = &visitor.table.scopes[0];
    let enum_scope_idx = root.children[0];
    let enum_scope = &visitor.table.scopes[enum_scope_idx];
    
    assert!(enum_scope.get("A").is_some());
    assert_eq!(enum_scope.get("A").unwrap().kind, SymbolKind::EnumCase);
}
