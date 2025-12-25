# Missing OOP Features Implementation Plan

## Executive Summary

**Issue Identified**: The Rust PHP VM lacks critical OOP validation:
1. No validation that interface methods are actually implemented in classes
2. No method signature compatibility checking when child classes override parent methods
3. No validation that only enums implement enum-specific interfaces (BackedEnum/UnitEnum)
4. Missing abstract class/method enforcement

**Test Case**: `/tmp/test.php`
```php
<?php
class Test implements BackedEnum {
   
}
$test = new Test();
var_dump($test);
```

**Standard PHP Output**:
```
PHP Fatal error:  Non-enum class Test cannot implement interface BackedEnum
```

**Rust VM Output**:
```
object(Test)#38 (0) { }
```

## Root Cause Analysis

### Current Implementation Gaps

1. **No Interface Implementation Validation** (CRITICAL)
   - Location: [`crates/php-vm/src/vm/engine.rs:5454`](crates/php-vm/src/vm/engine.rs#L5454-L5460)
   - The `OpCode::AddInterface` simply appends interfaces to the class without any validation
   - No check that interface methods are implemented

2. **No Method Signature Compatibility Checking** (CRITICAL)
   - When `class Child extends Parent`, overridden methods not validated for signature compatibility
   - Missing contravariance/covariance checking for parameters and return types
   - Visibility rules (can widen, not narrow) not enforced
   - Applies to: inheritance, interface implementation, and trait usage

3. **No Abstract Class/Method Enforcement** (HIGH)
   - Abstract methods may not be enforced in child classes
   - Abstract classes might be instantiable

4. **No Enum-Specific Restrictions** (MEDIUM)
   - `BackedEnum` and `UnitEnum` interfaces can be implemented by regular classes
   - No validation that only enum types implement enum interfaces

5. **Incomplete Trait Validation** (MEDIUM)
   - Trait conflict resolution may not be fully implemented
   - Trait precedence and aliasing might have gaps

## Detailed IMethod Signature Compatibility Validation (CRITICAL PRIORITY)

**Why First**: This affects all inheritance, not just interfaces. PHP enforces LSP (Liskov Substitution Principle).

#### Task 1.1: Add Method Signature Storage
**File**: `crates/php-vm/src/runtime/context.rs`

```rust
#[derive(Debug, Clone)]
pub struct MethodEntry {
    pub name: Symbol,
    pub func: Rc<UserFunc>,
    pub visibility: Visibility,
    pub is_static: bool,
    pub declaring_class: Symbol,
    pub is_abstract: bool,           // NEW
    pub signature: MethodSignature,  // NEW
}

#[derive(Debug, Clone)]
pub struct MethodSignature {
    pub parameters: Vec<ParameterInfo>,
    pub return_type: Option<TypeHint>,
    pub is_variadic: bool,
}

#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub name: Symbol,
    pub type_hint: Option<TypeHint>,
    pub is_reference: bool,
    pub is_variadic: bool,
    pub default_value: Option<Val>,
}

#[derive(Debug, Clone)]
pub enum TypeHint {
    Int,
    Float,
    String,
    Bool,
    Array,
    Object,
    Callable,
    Iterable,
    Mixed,
    Void,
    Never,
    Null,
    Class(Symbol),
    Union(Vec<TypeHint>),
    Intersection(Vec<TypeHint>),
}
```

#### Task 1.2: Validate Method Override Compatibility
**File**: `crates/php-vm/src/vm/engine.rs`

Add validation when methods are defined:

```rust
OpCode::DefMethod(class_name, method_name, func_idx, visibility, is_static) => {
    // ... existing code to get func and create entry ...
    
    // Check if method exists in parent class
    if let Some(class_def) = self.context.classes.get(&class_name) {
        if let Some(parent_sym) = class_def.parent {
            if let Some((parent_method, parent_vis, parent_static, _)) = 
                self.find_method(parent_sym, method_name) 
            {
                // Validate override compatibility
                self.validate_method_override(
                    class_name,
                    method_name,
                    &entry,
                    &parent_method,
                    parent_vis,
                    parent_static,
                )?;
            }
        }
    }
    
    // ... rest of method registration ...
}

fn validate_method_override(
    &self,
    child_class: Symbol,
    method_name: Symbol,
    child_method: &MethodEntry,
    parent_method: &UserFunc,
    parent_vis: Visibility,
    parent_static: bool,
) -> Result<(), VmError> {
    // 1. Static/non-static must match
    if child_method.is_static != parent_static {
        return Err(VmError::RuntimeError(format!(
            "Cannot make {}static method {}::{} {}static in class {}",
            if parent_static { "" } else { "non-" },
            self.format_symbol(method_name),
            self.format_symbol(method_name),
            if child_method.is_static { "" } else { "non-" },
            self.format_symbol(child_class),
        )));
    }
    
    // 2. 3: Abstract Class/Method Enforcement -> protected -> public)
    let valid_visibility = match parent_vis {
        Visibility::Private => true, // Can override with any visibility
        Visibility::Protected => matches!(
            child_method.visibility, 
            Visibility::Protected | Visibility::Public
        ),
        Visibility::Public => child_method.visibility == Visibility::Public,
    };
    
    if !valid_visibility {
        return Err(VmError::RuntimeError(format!(
            "Access level to {}::{} must be {} (as in class {}) or weaker",
            self.format_symbol(child_class),
            self.format_symbol(method_name),
            match parent_vis {
                Visibility::Public => "public",
                Visibility::Protected => "protected",
                Visibility::Private => "private",
            },
            // parent class name here
        )));
    }
    
    // 3. Parameter count (can have more with defaults, but not fewer)
    let parent_required = parent_method.param_count; // TODO: minus optional params
    let child_required = child_method.func.param_count;
    
    if child_required > parent_required {
        return Err(VmError::RuntimeError(format!(
            "Declaration of {}::{}() must be compatible with parent",
            self.format_symbol(child_class),
            self.format_symbol(method_name),
        )));
    }
    
    // 4. Validate parameter types (contravariance)
    self.validate_parameter_contravariance(
        child_class,
        method_name,
        &child_method.signature.parameters,
        &parent_method.signature.parameters,
    )?;
    
    // 5. Validate return type (covariance)
    self.validate_return_type_covariance(
        child_class,
        method_name,
        &child_method.signature.return_type,
        &parent_method.signature.return_type,
    )?;
    
    Ok(())
}
```

#### Task 1.3: Implement Type Compatibility Checking

Add comprehensive type validation methods to VM engine:

**File**: `crates/php-vm/src/vm/engine.rs`

```rust
/// Validate parameter type contravariance
/// Child parameters must be same or wider (contravariant)
fn validate_parameter_contravariance(
    &self,
    child_class: Symbol,
    method_name: Symbol,
    child_params: &[ParameterInfo],
    parent_params: &[ParameterInfo],
) -> Result<(), VmError> {
    // PHP allows child to have more parameters if they have defaults
    if child_params.len() < parent_params.len() {
        return Err(VmError::RuntimeError(format!(
            "Declaration of {}::{}() must be compatible with parent signature",
            self.format_symbol(child_class),
            self.format_symbol(method_name),
        )));
    }
    
    // Validate each parent parameter
    for (i, parent_param) in parent_params.iter().enumerate() {
        let child_param = &child_params[i];
        
        // If parent has no type hint, child can have any type hint or none
        let parent_type = match &parent_param.type_hint {
            None => continue,
            Some(t) => t,
        };
        
        // If parent has type hint, child must have compatible type hint
        let child_type = match &child_param.type_hint {
            None => {
                // Child removed type hint - not allowed
                return Err(VmError::RuntimeError(format!(
                    "Type of parameter ${} in {}::{}() must be compatible with parent",
                    self.format_symbol(child_param.name),
                    self.format_symbol(child_class),
                    self.format_symbol(method_name),
                )));
            }
            Some(t) => t,
        };
        
        // Validate contravariance: child type must be same or wider
        if !self.is_type_contravariant(child_type, parent_type)? {
            return Err(VmError::RuntimeError(format!(
                "Declaration of {}::{}(${}) must be compatible with parent (expected {}, got {})",
                self.format_symbol(child_class),
                self.format_symbol(method_name),
                self.format_symbol(child_param.name),
                self.format_type_hint(parent_type),
                self.format_type_hint(child_type),
            )));
        }
    }
    
    Ok(())
}

/// Validate return type covariance
/// Child return type must be same or narrower (covariant)
fn validate_return_type_covariance(
    &self,
    child_class: Symbol,
    method_name: Symbol,
    child_return: &Option<TypeHint>,
    parent_return: &Option<TypeHint>,
) -> Result<(), VmError> {
    match (parent_return, child_return) {
        // Parent has no return type - child can have any or none
        (None, _) => Ok(()),
        
        // Parent has return type, child has none - not allowed
        (Some(parent_type), None) => {
            Err(VmError::RuntimeError(format!(
                "Declaration of {}::{}() must be compatible with parent return type: {}",
                self.format_symbol(child_class),
                self.format_symbol(method_name),
                self.format_type_hint(parent_type),
            )))
        }
        
        // Both have return types - validate covariance
        (Some(parent_type), Some(child_type)) => {
            if !self.is_type_covariant(child_type, parent_type)? {
                Err(VmError::RuntimeError(format!(
                    "Declaration of {}::{}() must be compatible with parent return type (expected {}, got {})",
                    self.format_symbol(child_class),
                    self.format_symbol(method_name),
                    self.format_type_hint(parent_type),
                    self.format_type_hint(child_type),
                )))
            } else {
                Ok(())
            }
        }
    }
}

/// Check if child_type is contravariant with parent_type
/// Contravariance: child can accept wider types
/// Example: parent accepts Dog, child can accept Animal (wider)
fn is_type_contravariant(&self, child_type: &TypeHint, parent_type: &TypeHint) -> Result<bool, VmError> {
    // Exact match is always valid
    if self.types_equal(child_type, parent_type) {
        return Ok(true);
    }
    
    match (child_type, parent_type) {
        // Mixed accepts anything
        (TypeHint::Mixed, _) => Ok(true),
        
        // If parent is specific type, child cannot be mixed
        (_, TypeHint::Mixed) => Ok(false),
        
        // Union types: child union must be superset of parent union
        (TypeHint::Union(child_types), TypeHint::Union(parent_types)) => {
            // Every parent type must be in child union or subtype of something in child union
            for parent_t in parent_types {
                let matches = child_types.iter().any(|child_t| {
                    self.is_type_contravariant(child_t, parent_t).unwrap_or(false)
                });
                if !matches {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        
        // Child union can accept parent single type if union contains it
        (TypeHint::Union(child_types), parent_single) => {
            Ok(child_types.iter().any(|ct| 
                self.is_type_contravariant(ct, parent_single).unwrap_or(false)
            ))
        }
        
        // Parent union, child single - child must match all parent types (impossible)
        (child_single, TypeHint::Union(_)) => Ok(false),
        
        // Class inheritance: child can accept parent class (wider)
        (TypeHint::Class(child_class), TypeHint::Class(parent_class)) => {
            // Child class must be parent of parent type (or same)
            Ok(*child_class == *parent_class || self.is_subclass_of(*parent_class, *child_class))
        }
        
        // Object type compatibility
        (TypeHint::Object, TypeHint::Class(_)) => Ok(true),  // object is wider
        (TypeHint::Class(_), TypeHint::Object) => Ok(false), // class is narrower
        
        // Iterable compatibility
        (TypeHint::Iterable, TypeHint::Array) => Ok(true),
        
        // Scalar types are invariant (cannot widen)
        _ => Ok(false),
    }
}

/// Check if child_type is covariant with parent_type
/// Covariance: child can return narrower types
/// Example: parent returns Animal, child can return Dog (narrower)
fn is_type_covariant(&self, child_type: &TypeHint, parent_type: &TypeHint) -> Result<bool, VmError> {
    // Exact match is always valid
    if self.types_equal(child_type, parent_type) {
        return Ok(true);
    }
    
    match (child_type, parent_type) {
        // Mixed can be returned when parent expects anything
        (_, TypeHint::Mixed) => Ok(true),
        (TypeHint::Mixed, _) => Ok(false),
        
        // Never is covariant with everything (never returns)
        (TypeHint::Never, _) => Ok(true),
        
        // Void compatibility
        (TypeHint::Void, TypeHint::Void) => Ok(true),
        (TypeHint::Void, _) | (_, TypeHint::Void) => Ok(false),
        
        // Union types: child union must be subset of parent union
        (TypeHint::Union(child_types), TypeHint::Union(parent_types)) => {
            // Every child type must be in parent union or subtype of something in parent union
            for child_t in child_types {
                let matches = parent_types.iter().any(|parent_t| {
                    self.is_type_covariant(child_t, parent_t).unwrap_or(false)
                });
                if !matches {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        
        // Parent union, child single - child must be subtype of something in union
        (child_single, TypeHint::Union(parent_types)) => {
            Ok(parent_types.iter().any(|pt| 
                self.is_type_covariant(child_single, pt).unwrap_or(false)
            ))
        }
        
        // Child union, parent single - not covariant (child returns wider set)
        (TypeHint::Union(_), parent_single) => Ok(false),
        
        // Intersection types: child must implement all interfaces
        (TypeHint::Intersection(child_types), TypeHint::Intersection(parent_types)) => {
            // Child intersection must be superset of parent intersection
            for parent_t in parent_types {
                let matches = child_types.iter().any(|child_t| {
                    self.types_equal(child_t, parent_t)
                });
                if !matches {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        
        // Class inheritance: child can return subclass (narrower)
        (TypeHint::Class(child_class), TypeHint::Class(parent_class)) => {
            // Child class must be subclass of parent type (or same)
            Ok(*child_class == *parent_class || self.is_subclass_of(*child_class, *parent_class))
        }
        
        // Class is covariant with Object
        (TypeHint::Class(_), TypeHint::Object) => Ok(true),
        (TypeHint::Object, TypeHint::Class(_)) => Ok(false),
        
        // Array/Iterable compatibility
        (TypeHint::Array, TypeHint::Iterable) => Ok(true),
        (TypeHint::Iterable, TypeHint::Array) => Ok(false),
        
        // Null compatibility (for nullable types)
        (TypeHint::Null, TypeHint::Null) => Ok(true),
        
        // Scalar types are invariant (cannot narrow)
        _ => Ok(false),
    }
}

/// Check if two types are equal
fn types_equal(&self, a: &TypeHint, b: &TypeHint) -> bool {
    match (a, b) {
        (TypeHint::Int, TypeHint::Int) => true,
        (TypeHint::Float, TypeHint::Float) => true,
        (TypeHint::String, TypeHint::String) => true,
        (TypeHint::Bool, TypeHint::Bool) => true,
        (TypeHint::Array, TypeHint::Array) => true,
        (TypeHint::Object, TypeHint::Object) => true,
        (TypeHint::Callable, TypeHint::Callable) => true,
        (TypeHint::Iterable, TypeHint::Iterable) => true,
        (TypeHint::Mixed, TypeHint::Mixed) => true,
        (TypeHint::Void, TypeHint::Void) => true,
        (TypeHint::Never, TypeHint::Never) => true,
        (TypeHint::Null, TypeHint::Null) => true,
        (TypeHint::Class(a_sym), TypeHint::Class(b_sym)) => a_sym == b_sym,
        (TypeHint::Union(a_types), TypeHint::Union(b_types)) => {
            a_types.len() == b_types.len() &&
            a_types.iter().all(|at| b_types.iter().any(|bt| self.types_equal(at, bt)))
        }
        (TypeHint::Intersection(a_types), TypeHint::Intersection(b_types)) => {
            a_types.len() == b_types.len() &&
            a_types.iter().all(|at| b_types.iter().any(|bt| self.types_equal(at, bt)))
        }
        _ => false,
    }
}

/// Format type hint for error messages
fn format_type_hint(&self, hint: &TypeHint) -> String {
    match hint {
        TypeHint::Int => "int".to_string(),
        TypeHint::Float => "float".to_string(),
        TypeHint::String => "string".to_string(),
        TypeHint::Bool => "bool".to_string(),
        TypeHint::Array => "array".to_string(),
        TypeHint::Object => "object".to_string(),
        TypeHint::Callable => "callable".to_string(),
        TypeHint::Iterable => "iterable".to_string(),
        TypeHint::Mixed => "mixed".to_string(),
        TypeHint::Void => "void".to_string(),
        TypeHint::Never => "never".to_string(),
        TypeHint::Null => "null".to_string(),
        TypeHint::Class(sym) => {
            String::from_utf8_lossy(self.context.interner.lookup(*sym).unwrap_or(b"?")).to_string()
        }
        TypeHint::Union(types) => {
            types.iter()
                .map(|t| self.format_type_hint(t))
                .collect::<Vec<_>>()
                .join("|")
        }
        TypeHint::Intersection(types) => {
            types.iter()
                .map(|t| self.format_type_hint(t))
                .collect::<Vec<_>>()
                .join("&")
        }
    }
}
```

#### Task 1.4: Extract Type Information from AST

**File**: `crates/php-vm/src/compiler/emitter.rs`

When emitting methods, extract and store type information:

```rust
// Extract parameter types from function definition
fn extract_parameter_info(&mut self, param: &FunctionParam) -> ParameterInfo {
    let name = self.interner.intern(self.get_text(param.name.span));
    let type_hint = param.type_hint.as_ref().map(|th| self.extract_type_hint(th));
    
    ParameterInfo {
        name,
        type_hint,
        is_reference: param.is_reference,
        is_variadic: param.is_variadic,
        default_value: param.default.as_ref().map(|expr| self.evaluate_const_expr(expr)),
    }
}

// Evaluate constant expression at compile time
// Reference: $PHP_SRC_PATH/Zend/zend_compile.c - zend_try_ct_eval_const_expr
fn evaluate_const_expr(&mut self, expr: &Expr) -> Val {
    match expr {
        // Literals
        Expr::Int { value, .. } => Val::Int(*value),
        Expr::Float { value, .. } => Val::Float(*value),
        Expr::String { value, .. } => {
            Val::String(Rc::new(value.clone()))
        }
        Expr::Bool { value, .. } => Val::Bool(*value),
        Expr::Null { .. } => Val::Null,
        
        // Array literal
        Expr::Array { items, .. } => {
            let mut array = ArrayData::new();
            for item in *items {
                match item {
                    ArrayItem::KeyValue { key, value, .. } => {
                        let key_val = self.evaluate_const_expr(key);
                        let value_val = self.evaluate_const_expr(value);
                        let array_key = self.val_to_array_key(&key_val);
                        array.insert(array_key, value_val);
                    }
                    ArrayItem::Value { value, .. } => {
                        let value_val = self.evaluate_const_expr(value);
                        array.push(value_val);
                    }
                    ArrayItem::Spread { .. } => {
                        // Spread in default params not allowed at compile time
                        return Val::Null;
                    }
                }
            }
            Val::Array(Rc::new(RefCell::new(array)))
        }
        
        // Unary operations
        Expr::UnaryOp { op, operand, .. } => {
            let val = self.evaluate_const_expr(operand);
            match op {
                UnaryOp::Plus => val, // +$x
                UnaryOp::Minus => {   // -$x
                    match val {
                        Val::Int(i) => Val::Int(-i),
                        Val::Float(f) => Val::Float(-f),
                        _ => Val::Null,
                    }
                }
                UnaryOp::Not => {     // !$x
                    Val::Bool(!self.val_to_bool(&val))
                }
                UnaryOp::BitwiseNot => { // ~$x
                    match val {
                        Val::Int(i) => Val::Int(!i),
                        Val::String(s) => {
                            // Bitwise NOT on string flips each byte
                            let negated: Vec<u8> = s.iter().map(|&b| !b).collect();
                            Val::String(Rc::new(negated))
                        }
                        _ => Val::Null,
                    }
                }
                _ => Val::Null,
            }
        }
        
        // Binary operations
        Expr::BinaryOp { left, op, right, .. } => {
            let left_val = self.evaluate_const_expr(left);
            let right_val = self.evaluate_const_expr(right);
            
            match op {
                // Arithmetic
                BinaryOp::Add => self.const_add(&left_val, &right_val),
                BinaryOp::Sub => self.const_sub(&left_val, &right_val),
                BinaryOp::Mul => self.const_mul(&left_val, &right_val),
                BinaryOp::Div => self.const_div(&left_val, &right_val),
                BinaryOp::Mod => self.const_mod(&left_val, &right_val),
                BinaryOp::Pow => self.const_pow(&left_val, &right_val),
                
                // Bitwise
                BinaryOp::BitwiseAnd => self.const_bitwise_and(&left_val, &right_val),
                BinaryOp::BitwiseOr => self.const_bitwise_or(&left_val, &right_val),
                BinaryOp::BitwiseXor => self.const_bitwise_xor(&left_val, &right_val),
                BinaryOp::ShiftLeft => self.const_shift_left(&left_val, &right_val),
                BinaryOp::ShiftRight => self.const_shift_right(&left_val, &right_val),
                
                // String
                BinaryOp::Concat => self.const_concat(&left_val, &right_val),
                
                // Comparison (for ternary in defaults)
                BinaryOp::Identical => Val::Bool(self.const_identical(&left_val, &right_val)),
                BinaryOp::NotIdentical => Val::Bool(!self.const_identical(&left_val, &right_val)),
                BinaryOp::Equal => Val::Bool(self.const_equal(&left_val, &right_val)),
                BinaryOp::NotEqual => Val::Bool(!self.const_equal(&left_val, &right_val)),
                BinaryOp::LessThan => Val::Bool(self.const_less_than(&left_val, &right_val)),
                BinaryOp::LessThanOrEqual => Val::Bool(self.const_less_than_or_equal(&left_val, &right_val)),
                BinaryOp::GreaterThan => Val::Bool(self.const_greater_than(&left_val, &right_val)),
                BinaryOp::GreaterThanOrEqual => Val::Bool(self.const_greater_than_or_equal(&left_val, &right_val)),
                
                // Logical
                BinaryOp::And | BinaryOp::LogicalAnd => {
                    Val::Bool(self.val_to_bool(&left_val) && self.val_to_bool(&right_val))
                }
                BinaryOp::Or | BinaryOp::LogicalOr => {
                    Val::Bool(self.val_to_bool(&left_val) || self.val_to_bool(&right_val))
                }
                BinaryOp::Xor | BinaryOp::LogicalXor => {
                    Val::Bool(self.val_to_bool(&left_val) ^ self.val_to_bool(&right_val))
                }
                
                _ => Val::Null,
            }
        }
        
        // Ternary operator: condition ? true_val : false_val
        Expr::Ternary { condition, if_true, if_false, .. } => {
            let cond_val = self.evaluate_const_expr(condition);
            if self.val_to_bool(&cond_val) {
                if let Some(true_expr) = if_true {
                    self.evaluate_const_expr(true_expr)
                } else {
                    // ?: operator (elvis) - return condition if truthy
                    cond_val
                }
            } else {
                self.evaluate_const_expr(if_false)
            }
        }
        
        // Null coalesce: left ?? right
        Expr::Coalesce { left, right, .. } => {
            let left_val = self.evaluate_const_expr(left);
            match left_val {
                Val::Null => self.evaluate_const_expr(right),
                _ => left_val,
            }
        }
        
        // Class constant reference: MyClass::CONSTANT
        Expr::ClassConstFetch { class, constant, .. } => {
            // Try to resolve at compile time if class is known
            let class_name = self.extract_class_name_from_expr(class);
            let const_name = self.get_text(constant.span);
            
            if let Some(class_sym) = class_name {
                // Look up constant in known classes
                if let Some(val) = self.lookup_compile_time_class_const(class_sym, const_name) {
                    return val;
                }
            }
            
            // Cannot evaluate at compile time
            Val::Null
        }
        
        // Global constant reference
        Expr::ConstFetch { name, .. } => {
            let const_name = self.get_text(name.span);
            
            // Handle special constants
            match const_name {
                b"null" | b"NULL" => Val::Null,
                b"true" | b"TRUE" => Val::Bool(true),
                b"false" | b"FALSE" => Val::Bool(false),
                _ => {
                    // Try to look up in compile-time constants
                    if let Some(val) = self.lookup_compile_time_const(const_name) {
                        return val;
                    }
                    // Cannot evaluate at compile time
                    Val::Null
                }
            }
        }
        
        // Magic constants
        Expr::MagicConst { kind, .. } => {
            match kind {
                MagicConstKind::Line => Val::Int(self.current_line() as i64),
                MagicConstKind::File => {
                    Val::String(Rc::new(self.current_file().as_bytes().to_vec()))
                }
                MagicConstKind::Dir => {
                    Val::String(Rc::new(self.current_dir().as_bytes().to_vec()))
                }
                MagicConstKind::Function => {
                    Val::String(Rc::new(self.current_function().as_bytes().to_vec()))
                }
                MagicConstKind::Class => {
                    Val::String(Rc::new(self.current_class().as_bytes().to_vec()))
                }
                MagicConstKind::Method => {
                    Val::String(Rc::new(self.current_method().as_bytes().to_vec()))
                }
                MagicConstKind::Namespace => {
                    Val::String(Rc::new(self.current_namespace().as_bytes().to_vec()))
                }
                MagicConstKind::Trait => {
                    Val::String(Rc::new(self.current_trait().as_bytes().to_vec()))
                }
            }
        }
        
        // Anything else cannot be evaluated at compile time
        _ => Val::Null,
    }
}

// Helper: Convert Val to array key
fn val_to_array_key(&self, val: &Val) -> ArrayKey {
    match val {
        Val::Int(i) => ArrayKey::Int(*i),
        Val::String(s) => ArrayKey::String(s.clone()),
        Val::Bool(true) => ArrayKey::Int(1),
        Val::Bool(false) => ArrayKey::Int(0),
        Val::Float(f) => ArrayKey::Int(*f as i64),
        _ => ArrayKey::Int(0),
    }
}

// Helper: Convert Val to bool for logical operations
fn val_to_bool(&self, val: &Val) -> bool {
    match val {
        Val::Bool(b) => *b,
        Val::Null => false,
        Val::Int(i) => *i != 0,
        Val::Float(f) => *f != 0.0 && !f.is_nan(),
        Val::String(s) => !s.is_empty() && s.as_slice() != b"0",
        Val::Array(arr) => !arr.borrow().is_empty(),
        _ => true,
    }
}

// Constant expression arithmetic helpers
fn const_add(&self, left: &Val, right: &Val) -> Val {
    match (left, right) {
        (Val::Int(a), Val::Int(b)) => Val::Int(a.wrapping_add(*b)),
        (Val::Float(a), Val::Float(b)) => Val::Float(a + b),
        (Val::Int(a), Val::Float(b)) => Val::Float(*a as f64 + b),
        (Val::Float(a), Val::Int(b)) => Val::Float(a + *b as f64),
        (Val::Array(a), Val::Array(b)) => {
            // Array union
            let mut result = a.borrow().clone();
            for (k, v) in b.borrow().iter() {
                result.entry(k.clone()).or_insert(v.clone());
            }
            Val::Array(Rc::new(RefCell::new(result)))
        }
        _ => Val::Null,
    }
}

fn const_sub(&self, left: &Val, right: &Val) -> Val {
    match (left, right) {
        (Val::Int(a), Val::Int(b)) => Val::Int(a.wrapping_sub(*b)),
        (Val::Float(a), Val::Float(b)) => Val::Float(a - b),
        (Val::Int(a), Val::Float(b)) => Val::Float(*a as f64 - b),
        (Val::Float(a), Val::Int(b)) => Val::Float(a - *b as f64),
        _ => Val::Null,
    }
}

fn const_mul(&self, left: &Val, right: &Val) -> Val {
    match (left, right) {
        (Val::Int(a), Val::Int(b)) => Val::Int(a.wrapping_mul(*b)),
        (Val::Float(a), Val::Float(b)) => Val::Float(a * b),
        (Val::Int(a), Val::Float(b)) => Val::Float(*a as f64 * b),
        (Val::Float(a), Val::Int(b)) => Val::Float(a * *b as f64),
        _ => Val::Null,
    }
}

fn const_div(&self, left: &Val, right: &Val) -> Val {
    match (left, right) {
        (Val::Int(a), Val::Int(b)) if *b != 0 => {
            if a % b == 0 {
                Val::Int(a / b)
            } else {
                Val::Float(*a as f64 / *b as f64)
            }
        }
        (Val::Float(a), Val::Float(b)) => Val::Float(a / b),
        (Val::Int(a), Val::Float(b)) => Val::Float(*a as f64 / b),
        (Val::Float(a), Val::Int(b)) => Val::Float(a / *b as f64),
        _ => Val::Null,
    }
}

fn const_concat(&self, left: &Val, right: &Val) -> Val {
    let left_str = self.val_to_string(left);
    let right_str = self.val_to_string(right);
    let mut result = left_str.to_vec();
    result.extend_from_slice(&right_str);
    Val::String(Rc::new(result))
}

// Helper: Get string representation of value
fn val_to_string(&self, val: &Val) -> Vec<u8> {
    match val {
        Val::String(s) => s.as_ref().clone(),
        Val::Int(i) => i.to_string().into_bytes(),
        Val::Float(f) => f.to_string().into_bytes(),
        Val::Bool(true) => b"1".to_vec(),
        Val::Bool(false) => b"".to_vec(),
        Val::Null => b"".to_vec(),
        _ => b"".to_vec(),
    }
}

// Extract type hint from AST node
fn extract_type_hint(&mut self, hint: &TypeHintNode) -> TypeHint {
    match hint {
        TypeHintNode::Named(ident) => {
            let name = self.get_text(ident.span);
            match name {
                b"int" => TypeHint::Int,
                b"float" => TypeHint::Float,
                b"string" => TypeHint::String,
                b"bool" => TypeHint::Bool,
                b"array" => TypeHint::Array,
                b"object" => TypeHint::Object,
                b"callable" => TypeHint::Callable,
                b"iterable" => TypeHint::Iterable,
                b"mixed" => TypeHint::Mixed,
                b"void" => TypeHint::Void,
                b"never" => TypeHint::Never,
                b"null" => TypeHint::Null,
                _ => {
                    // Class name
                    let sym = self.interner.intern(name);
                    TypeHint::Class(sym)
                }
            }
        }
        TypeHintNode::Nullable(inner) => {
            // ?Type becomes Type|null union
            let inner_type = self.extract_type_hint(inner);
            TypeHint::Union(vec![inner_type, TypeHint::Null])
        }
        TypeHintNode::Union(types) => {
            let extracted = types.iter()
                .map(|t| self.extract_type_hint(t))
                .collect();
            TypeHint::Union(extracted)
        }
        TypeHintNode::Intersection(types) => {
            let extracted = types.iter()
                .map(|t| self.extract_type_hint(t))
                .collect();
            TypeHint::Intersection(extracted)
        }
    }
}
```

#### Task 1.6: Constant Expression Evaluation Strategy

**Implementation Considerations:**

1. **Two-Phase Compilation:**
   - First pass: Parse class structure, collect method signatures with placeholder defaults
   - Second pass: Evaluate const expressions after all class constants are registered
   - This handles cases like `function foo($x = MyClass::CONST) {}`

2. **Error Handling:**
   - Invalid const expressions (e.g., `1 + "string"`) should produce compile-time warnings but allow fallback to `Val::Null`
   - Division by zero in const expr should warn and return `Val::Null`
   - Undefined constants should produce warning and return `Val::Null`

3. **Circular Dependency Detection:**
   - Track evaluation stack to detect cycles like:
     ```php
     class A { const X = B::Y; }
     class B { const Y = A::X; }
     ```
   - Break cycle with `Val::Null` and emit warning

4. **Scope Context:**
   - Magic constants need compile-time context:
     - `__FILE__`, `__DIR__` - known from source file
     - `__CLASS__`, `__TRAIT__` - known from current class context
     - `__METHOD__`, `__FUNCTION__` - known from current method/function
   - Pass context struct to `evaluate_const_expr()`

**Usage in `extract_parameter_info`:**
```rust
fn extract_parameter_info(&mut self, params: &[Param]) -> Vec<ParameterInfo> {
    params.iter().map(|param| {
        let default_value = param.default.as_ref().map(|expr| {
            self.evaluate_const_expr(expr)
        });
        
        ParameterInfo {
            name: self.interner.intern(self.get_text(param.name.span)),
            type_hint: param.type_hint.as_ref().map(|h| self.extract_type_hint(h)),
            is_variadic: param.is_variadic,
            is_by_ref: param.is_by_ref,
            default_value,
        }
    }).collect()
}
```

#### Task 1.5: PHP Type System Rules Reference

**Contravariance (Parameters) - Child can accept WIDER types:**
```php
// Valid: Animal is wider than Dog
class Parent { public function foo(Dog $d) {} }
class Child extends Parent { 
    public function foo(Animal $a) {} // ✓ OK
}

// Invalid: Puppy is narrower than Dog
class Child2 extends Parent {
    public function foo(Puppy $p) {} // ✗ Error
}
```

**Covariance (Return Types) - Child can return NARROWER types:**
```php
// Valid: Dog is narrower than Animal
class Parent { public function foo(): Animal {} }
class Child extends Parent { 
    public function foo(): Dog {} // ✓ OK
}

// Invalid: Mammal is wider than Animal
class Child2 extends Parent {
    public function foo(): Mammal {} // ✗ Error (if Mammal is parent of Animal)
}
```

**Special Cases:**
```php
// Mixed is universal supertype
class Parent { public function foo(Dog $d): Animal {} }
class Child extends Parent {
    public function foo(mixed $m): never {} // ✓ OK: mixed is wider, never is narrower
}

// Union type subset/superset
class Parent { public function foo(Dog|Cat $x) {} }
class Child extends Parent {
    public function foo(Animal $a) {} // ✓ OK: Animal includes Dog|Cat
}

// Intersection types
class Parent { public function foo(): Serializable&Countable {} }
class Child extends Parent {
    public function foo(): MyClass {} // ✓ OK if MyClass implements both
}
```

**Reference**: `$PHP_SRC_PATH/Zend/zend_inheritance.c` - `do_inheritance_check_on_method`

### Phase 4: Enum-Specific Interface Restrictions

### Phase 1: Interface Implementation Validation (HIGH PRIORITY)

#### Task 2.1: Add Interface Method Collection
**File**: `crates/php-vm/src/vm/engine.rs`

Add helper method to collect all required methods from an interface:
```rust
/// Collect all method signatures required by an interface (including parent interfaces)
fn collect_interface_methods(&self, interface_sym: Symbol) -> HashMap<Symbol, InterfaceMethodSig> {
    let mut methods = HashMap::new();
    
    if let Some(interface_def) = self.context.classes.get(&interface_sym) {
        if !interface_def.is_interface {
            return methods;
        }
        
        // Collect methods from this interface
        for (method_name, entry) in &interface_def.methods {
            methods.insert(*method_name, InterfaceMethodSig {
                name: entry.name,
                visibility: entry.visibility,
                is_static: entry.is_static,
                param_count: entry.func.param_count,
                // TODO: Add return type, parameter types
            });
        }
        
        // Recursively collect from parent interfaces
        for &parent_interface in &interface_def.interfaces {
            methods.extend(self.collect_interface_methods(parent_interface));
        }
    }
    
    methods
}
```

#### Task 2.2: Validate Interface Implementation
**File**: `crates/php-vm/src/vm/engine.rs` (in `OpCode::AddInterface` handler)

```rust
OpCode::AddInterface(class_name, interface_name) => {
    // 1. Check that interface exists and is actually an interface
    let interface_def = self.context.classes.get(&interface_name)
        .ok_or(VmError::RuntimeError(format!("Interface {} not found", 
            String::from_utf8_lossy(self.context.interner.lookup(interface_name).unwrap()))))?;
    
    if !interface_def.is_interface {
        return Err(VmError::RuntimeError(format!(
            "{} cannot implement {} - not an interface",
            self.format_symbol(class_name),
            self.format_symbol(interface_name)
        )));
    }
    
    // 2. Collect required interface methods
    let required_methods = self.collect_interface_methods(interface_name);
    
    // 3. Get implementing class
    let class_def = self.context.classes.get(&class_name)
        .ok_or(VmError::RuntimeError("Class not found".into()))?;
    
    // 4. Validate each required method is implemented
    for (method_name, interface_method) in required_methods {
        match class_def.methods.get(&method_name) {
            None => {
                return Err(VmError::RuntimeError(format!(
                    "Class {} must implement method {}::{}()",
                    self.format_symbol(class_name),
                    self.format_symbol(interface_name),
                    self.format_symbol(method_name)
                )));
            }
            Some(impl_method) => {
                // Validate visibility (interface methods are implicitly public)
                if impl_method.visibility != Visibility::Public {
                    return Err(VmError::RuntimeError(format!(
                        "Implementation of {}::{} must be public",
                        self.format_symbol(interface_name),
                        self.format_symbol(method_name)
                    )));
                }
                
                // Validate static/non-static matches
                if impl_method.is_static != interface_method.is_static {
                    return Err(VmError::RuntimeError(format!(
                        "Method {}::{} must {}be static",
                        self.format_symbol(class_name),
                        self.format_symbol(method_name),
                        if interface_method.is_static { "" } else { "not " }
                    )));
                }
                
                // TODO: Validate parameter count, types, and return type
            }
        }
    }
    
    // 5. Add interface to class
    if let Some(class_def) = self.context.classes.get_mut(&class_name) {
        class_def.interfaces.push(interface_name);
    }
}
```

### Phase 2: Enum-Specific Interface Restrictions (HIGH PRIORITY)

#### Task 4.1: Add Enum Tracking to ClassDef
**File**: `crates/php-vm/src/runtime/context.rs`

```rust
#[derive(Debug, Clone)]
pub struct ClassDef {
    pub name: Symbol,
    pub parent: Option<Symbol>,
    pub is_interface: bool,
    pub is_trait: bool,
    pub is_enum: bool,        // NEW: Track if this is an enum
    pub enum_backed_type: Option<EnumBackedType>, // NEW: For BackedEnum
    pub interfaces: Vec<Symbol>,
    // ... rest of fields
}

#[derive(Debug, Clone, Copy)]
pub enum EnumBackedType {
    Int,
    String,
}
```

#### Task 4.2: Add OpCode for Enum Definition
**File**: `crates/php-vm/src/vm/opcode.rs`

```rust
DefEnum(Symbol, Option<EnumBackedType>), // (name, backing_type)
DefEnumCase(Symbol, Symbol, Option<u16>), // (enum_name, case_name, value_idx)
```

#### Task 4.3: Validate Enum-Only Interfaces
**File**: `crates/php-vm/src/vm/engine.rs` (in `OpCode::AddInterface`)

```rust
// Special handling for enum interfaces
let enum_interface_names = [b"UnitEnum", b"BackedEnum"];
let is_enum_interface = enum_interface_names.iter()
    .any(|&name| self.context.interner.intern(name) == interface_name);

if is_enum_interface {
    if !class_def.is_enum {
        return Err(VmError::RuntimeError(format!(
            "Non-enum class {} cannot implement interface {}",
            self.format_symbol(class_name),
            self.format_symbol(interface_name)
        )));
    }
    
    // For BackedEnum, validate backing type
    if self.context.interner.intern(b"BackedEnum") == interface_name {
        if class_def.enum_backed_type.is_none() {
            return Err(VmError::RuntimeError(format!(
                "Enum {} must be a backed enum to implement BackedEnum",
                self.format_symbol(class_name)
            )));
        }
    }
}
```

### Phase 3: Abstract Class/Method Enforcement (MEDIUM PRIORITY)

#### Task 5.1: Track Abstract Methods
**File**: `crates/php-vm/src/runtime/context.rs`

```rust
#[derive(Debug, Clone)]
pub struct ClassDef {
    // ... existing fields
    pub is_abstract: bool,
    pub abstract_methods: HashSet<Symbol>, // Methods that must be implemented
}

#[derive(Debug, Clone)]
pub struct MethodEntry {
    // ... existing fields
    pub is_abstract: bool,
}
```

#### Task 5.2: Prevent Abstract Class Instantiation
**File**: `crates/php-vm/src/vm/engine.rs` (in `OpCode::New`)

```rust
OpCode::New(class_name, arg_count) => {
    let resolved_class = self.resolve_class_name(class_name)?;
    
    // Check if class is abstract
    if let Some(class_def) = self.context.classes.get(&resolved_class) {
        if class_def.is_abstract {
            return Err(VmError::RuntimeError(format!(
                "Cannot instantiate abstract class {}",
                self.format_symbol(resolved_class)
            )));
        }
    }
    // ... rest of instantiation logic
}
```

#### Task 5.3: Validate Abstract Methods Implemented
**File**: `crates/php-vm/src/vm/engine.rs` (in `OpCode::DefClass`)

```rust
// After class definition is complete, validate abstract methods
if let Some(class_def) = self.context.classes.get(&name) {
    if !class_def.is_abstract {
        // Collect all abstract methods from parent classes
        let mut unimplemented = Vec::new();
        let mut current = class_def.parent;
        
        while let Some(parent_sym) = current {
            if let Some(parent_def) = self.context.classes.get(&parent_sym) {
                for &abstract_method in &parent_def.abstract_methods {
                    // Check if implemented
                    if !class_def.methods.contains_key(&abstract_method) {
                        unimplemented.push((parent_sym, abstract_method));
                    }
                }
                current = parent_def.parent;
            } else {
                break;
            }
        }
        
        if !unimplemented.is_empty() {
            let method_names: Vec<String> = unimplemented.iter()
                .map(|(cls, method)| format!("{}::{}", 
                    self.format_symbol(*cls), 
                    self.format_symbol(*method)))
                .collect();
            
            return Err(VmError::RuntimeError(format!(
                "Class {} contains {} abstract method{} and must therefore be declared abstract or implement the remaining methods ({})",
                self.format_symbol(name),
                unimplemented.len(),
                if unimplemented.len() == 1 { "" } else { "s" },
                method_names.join(", ")
            )));
        }
    }
}
```

### Phase 5: Trait Conflict Resolution (MEDIUM PRIORITY)

#### Task 6.1: Implement Trait Precedence Operators
**File**: `crates/php-vm/src/vm/opcode.rs`

```rust
TraitAlias(Symbol, Symbol, Symbol), // (class, trait_method, alias_name)
TraitInsteadOf(Symbol, Symbol, Vec<Symbol>), // (class, chosen_method, excluded_traits)
```

#### Task 6.2: Detect and Report Trait Conflicts
When using traits, check for method name conflicts and require explicit resolution.

### Phase 6: Property Type Validation (LOWER PRIORITY)

#### Task 7.1: Validate Typed Property Assignments
Ensure assignments to typed properties match the declared type.

## Testing Strategy

### Test File Structure
```
crates/php-vm/tests/oop/
├── interface_validation.rs
├── enum_interfaces.rs
├── abstract_classes.rs
├── trait_conflicts.rs
├── method_signatures.rs
└── property_types.rs
```

### Key Test Cases

1. **Interface Validation**:
   ```php
   interface Iface { public function foo(); }
   class Test implements Iface { } // Should fail: missing foo()
   ```

2. **Enum Interface Restriction**:
   ```php
   class Test implements BackedEnum { } // Should fail
   enum Suit implements BackedEnum { case Hearts; } // Should fail: not backed
   enum Priority: int implements BackedEnum { case High = 1; } // Should succeed
   ```

3. **Abstract Class**:
   ```php
   amethod_override_compatibility.rs   (CRITICAL - test inheritance)
├── interface_validation.rs            (HIGH - test interface impl)
├── abstract_classes.rs                (HIGH - test abstract enforcement)
├── enum_interfaces.rs                 (MEDIUM - test enum restrictions)
├── trait_conflicts.rs                 (MEDIUM - test trait resolution)
└── property_types.rs                  (LOW - test typed properties)*:
   ```php
   trait A { public function foo() {} }
   trait B { public function foo() {} }
   class Test { use A, B; } // Should fail: conflict without resolution
   ``Method Override Signature Compatibility**:
   ```php
   class Parent { public function foo(int $x): string {} }
   class Child extends Parent { 
       protected function foo(int $x): string {} // Should fail: visibility narrowed
   }
   ```

2. **Interface Validation**:
   ```php
   interface Iface { public function foo(); }
   class Test implements Iface { } // Should fail: missing foo()
   ```
4
9. **Abstract Class**:
   ```php
   abstract class Base { abstract public function foo(); }
   new Base(); // Should fail: cannot instantiate abstract
   class Child extends Base { } // Should fail: missing foo()
   ```

10. **Trait Conflicts**:
- [`crates/php-vm/src/vm/opcode.rs`](crates/php-vm/src/vm/opcode.rs) - New opcodes for enums
- [`crates/php-vm/src/runtime/context.rs`](crates/php-vm/src/runtime/context.rs) - ClassDef enhancements

### Compiler Files
- [`crates/php-vm/src/compiler/emitter.rs`](crates/php-vm/src/compiler/emitter.rs) - Emit new opcodes for enums
- [`crates/php-parser/src/ast/mod.rs`](crates/php-parser/src/ast/mod.rs) - AST nodes for enums (if needed)

### Test Files
- New test files in `crates/php-vm/tests/oop/`
- Update existing tests in `crates/php-vm/tests/classes.rs`

## Success Criteria

1. ✅ All test cases produce the same errors as standard PHP - **COMPLETE**
2. ✅ Type contravariance/covariance properly validated - **COMPLETE**
3. ✅ All existing tests continue to pass - **COMPLETE**
4. ✅ New comprehensive OOP test suite passes (including type variance tests) - **COMPLETE**
5. ✅ Zero panics on invalid OOP constructs (graceful errors instead) - **COMPLETE**

## Implementation Status

### Completed Features (December 2025)

**Phase 1: Infrastructure** ✅
- Added TypeHint enum with 15 variants (Int, Float, String, Bool, Array, Object, Callable, Iterable, Mixed, Void, Never, Null, Class, Union, Intersection)
- Added ParameterInfo struct for storing parameter metadata
- Added MethodSignature struct for method signature validation
- Updated MethodEntry with is_abstract and signature fields
- Updated ClassDef with is_abstract, is_enum, enum_backed_type, abstract_methods fields

**Phase 2: Method Override Validation** ✅
- Static/non-static consistency checking
- Visibility widening enforcement (private → protected → public)
- Parameter count validation
- Full parameter type contravariance checking
- Full return type covariance checking
- Helper methods: types_equal(), is_type_contravariant(), is_type_covariant()

**Phase 3: Interface Implementation Validation** ✅
- Added FinalizeClass opcode for post-method validation
- Validates all interface methods are implemented
- Supports interface inheritance chains
- Checks method visibility (interface methods must be public)
- Validates static/non-static consistency

**Phase 4: Enum Interface Restrictions** ✅
- Validates only enums can implement BackedEnum/UnitEnum
- Checks backing type for BackedEnum implementation

**Phase 5: Abstract Class Enforcement** ✅
- Prevents instantiation of abstract classes
- Added MarkAbstract opcode
- Compiler detects abstract modifier and emits opcode
- **NEW**: Validates all inherited abstract methods are implemented in concrete classes
- Tracks abstract methods in ClassDef.abstract_methods HashSet
- Abstract methods inherited from parent classes
- DefMethod removes methods from abstract_methods when implemented
- FinalizeClass validates no unimplemented abstract methods remain

**Phase 6: Type System** ✅
- AST type hint extraction (extract_type_hint method in emitter)
- ReturnType to TypeHint conversion (return_type_to_type_hint in engine)
- Type parameter extraction during method emission
- Full contravariance/covariance type checking

**Phase 7: Trait Conflict Detection** ✅ **(NEW - December 2025)**
- Detects method name conflicts when using multiple traits
- Prevents silent method overwrites from conflicting traits
- Allows class methods to override trait methods
- Error messages match PHP 8.x format
- Validates trait usage before method insertion

### Test Results

All test cases passing with identical behavior to standard PHP 8.x:
- ✅ Interface validation (missing methods detected)
- ✅ Method visibility narrowing (blocked)
- ✅ Static/non-static mismatch (detected)
- ✅ Valid visibility widening (allowed)
- ✅ Parameter contravariance (child accepts wider types)
- ✅ Return type covariance (child returns narrower types)
- ✅ Invalid parameter type narrowing (blocked)
- ✅ Invalid return type widening (blocked)
- ✅ Abstract class instantiation (prevented)
- ✅ Abstract method implementation enforcement (missing implementations detected)
- ✅ Multi-level abstract method inheritance (validated)
- ✅ Enum interface restrictions (enforced)
- ✅ **NEW**: Trait method conflicts detected (multiple traits with same method name)
- ✅ **NEW**: Class methods can override trait methods (allowed)
- ✅ **NEW**: Multiple traits with different methods work correctly

### Remaining Work

**Lower Priority Features:**
- ~~Abstract method enforcement in concrete classes~~ ✅ **COMPLETED** (December 2025)
- ~~Trait conflict detection~~ ✅ **COMPLETED** (December 2025)
- Trait conflict resolution (insteadof/as operators for explicit resolution)
- Property type validation (typed property assignments)
- Constant expression evaluation for default parameters (currently returns Val::Null for complex expressions)

## References

- PHP Source: `/Users/eagle/Sourcecode/php-src/Zend/zend_inheritance.c`
- PHP Source: `/Users/eagle/Sourcecode/php-src/Zend/zend_compile.c`
- PHP RFC: https://wiki.php.net/rfc/enumerations
- PHP Manual: https://www.php.net/manual/en/language.oop5.php

## Notes

- All validation should happen at class definition time (compile/load time), not at instantiation
- Error messages should match PHP's format as closely as possible
- Consider performance impact - validation is one-time cost at class load
- Maintain zero-panic guarantee even with invalid OOP constructs
