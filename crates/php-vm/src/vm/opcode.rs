use crate::core::value::{Symbol, Visibility};

#[derive(Debug, Clone, Copy)]
pub enum OpCode {
    // Stack Ops
    Const(u16),      // Push constant from table
    Pop,
    
    // Arithmetic
    Add, Sub, Mul, Div, Concat,
    
    // Comparison
    IsEqual, IsNotEqual, IsIdentical, IsNotIdentical,
    IsGreater, IsLess, IsGreaterOrEqual, IsLessOrEqual,

    // Variables
    LoadVar(Symbol),  // Push local variable value
    StoreVar(Symbol), // Pop value, store in local
    AssignRef(Symbol), // Pop value (handle), mark as ref, store in local
    AssignDimRef,      // [Array, Index, ValueRef] -> Assigns ref to array index
    MakeVarRef(Symbol), // Convert local var to reference (COW if needed), push handle
    MakeRef,            // Convert top of stack to reference
    
    // Control Flow
    Jmp(u32),
    JmpIfFalse(u32),
    
    // Functions
    Call(u8),        // Call function with N args
    Return,
    DefFunc(Symbol, u32), // (name, func_idx) -> Define global function
    
    // System
    Include,         // Runtime compilation
    Echo,

    // Arrays
    InitArray,
    FetchDim,
    AssignDim,
    StoreDim, // AssignDim but with [val, key, array] stack order (popped as array, key, val)
    StoreNestedDim(u8), // Store into nested array. Arg is depth (number of keys). Stack: [val, key_n, ..., key_1, array]
    AppendArray,
    StoreAppend, // AppendArray but with [val, array] stack order (popped as array, val)

    // Iteration
    IterInit(u32),   // [Array] -> [Array, Index]. If empty, pop and jump.
    IterValid(u32),  // [Array, Index]. If invalid (end), pop both and jump.
    IterNext,        // [Array, Index] -> [Array, Index+1]
    IterGetVal(Symbol), // [Array, Index] -> Assigns val to local
    IterGetValRef(Symbol), // [Array, Index] -> Assigns ref to local
    IterGetKey(Symbol), // [Array, Index] -> Assigns key to local

    // Constants
    FetchGlobalConst(Symbol),
    DefGlobalConst(Symbol, u16), // (name, val_idx)

    // Objects
    DefClass(Symbol, Option<Symbol>),       // Define class (name, parent)
    DefMethod(Symbol, Symbol, u32, Visibility, bool), // (class_name, method_name, func_idx, visibility, is_static)
    DefProp(Symbol, Symbol, u16, Visibility), // (class_name, prop_name, default_val_idx, visibility)
    DefClassConst(Symbol, Symbol, u16, Visibility), // (class_name, const_name, val_idx, visibility)
    DefStaticProp(Symbol, Symbol, u16, Visibility), // (class_name, prop_name, default_val_idx, visibility)
    FetchClassConst(Symbol, Symbol), // (class_name, const_name) -> [Val]
    FetchStaticProp(Symbol, Symbol), // (class_name, prop_name) -> [Val]
    AssignStaticProp(Symbol, Symbol), // (class_name, prop_name) [Val] -> [Val]
    CallStaticMethod(Symbol, Symbol, u8), // (class_name, method_name, arg_count) -> [RetVal]
    New(Symbol, u8),        // Create instance, call constructor with N args
    FetchProp(Symbol),      // [Obj] -> [Val]
    AssignProp(Symbol),     // [Obj, Val] -> [Val]
    CallMethod(Symbol, u8), // [Obj, Arg1...ArgN] -> [RetVal]
    
    // Closures
    Closure(u32, u32), // (func_idx, num_captures) -> [Closure]

    // Exceptions
    Throw, // [Obj] -> !
}
