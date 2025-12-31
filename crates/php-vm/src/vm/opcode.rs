use crate::core::value::{Symbol, Visibility};

#[derive(Debug, Clone, Copy)]
pub enum OpCode {
    // Stack Ops
    Nop,
    Const(u16), // Push constant from table
    Pop,
    Dup,

    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Concat,
    FastConcat,

    // Bitwise
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    BitwiseNot,
    ShiftLeft,
    ShiftRight,

    // Comparison
    IsEqual,
    IsNotEqual,
    IsIdentical,
    IsNotIdentical,
    IsGreater,
    IsLess,
    IsGreaterOrEqual,
    IsLessOrEqual,
    Spaceship,

    // Logical
    BoolNot,
    BoolXor,

    // Variables
    LoadVar(Symbol),    // Push local variable value
    LoadVarDynamic,     // [Name] -> [Val]
    StoreVar(Symbol),   // Pop value, store in local
    StoreVarDynamic,    // [Val, Name] -> [Val] (Stores Val in Name, pushes Val)
    AssignRef(Symbol),  // Pop value (handle), mark as ref, store in local
    AssignDimRef,       // [Array, Index, ValueRef] -> Assigns ref to array index
    MakeVarRef(Symbol), // Convert local var to reference (COW if needed), push handle
    MakeRef,            // Convert top of stack to reference
    UnsetVar(Symbol),
    UnsetVarDynamic,
    BindGlobal(Symbol), // Bind local variable to global variable (by reference)
    BindStatic(Symbol, u16), // Bind local variable to static variable (name, default_val_idx)

    // Control Flow
    Jmp(u32),
    JmpIfFalse(u32),
    JmpIfTrue(u32),
    JmpZEx(u32),
    JmpNzEx(u32),
    Coalesce(u32),
    JmpFinally(u32), // Jump to target after executing finally blocks at current IP

    // Functions
    Call(u8), // Call function with N args
    Return,
    DefFunc(Symbol, u32), // (name, func_idx) -> Define global function
    Recv(u32),
    RecvInit(u32, u16), // Arg index, default val index
    SendVal,
    SendVar,
    SendRef,
    LoadRef(Symbol), // Load variable as reference (converting if necessary)

    // System
    Include, // Runtime compilation
    Echo,
    Exit,
    Silence(bool),
    Ticks(u32),

    // Arrays
    InitArray(u32),
    FetchDim,
    AssignDim,
    StoreDim, // AssignDim but with [val, key, array] stack order (popped as array, key, val)
    StoreNestedDim(u8), // Store into nested array. Arg is depth (number of keys). Stack: [val, key_n, ..., key_1, array]
    FetchNestedDim(u8), // Fetch from nested array. Arg is depth. Stack: [array, key_n, ..., key_1] -> [array, key_n, ..., key_1, val]
    AppendArray,
    StoreAppend, // AppendArray but with [val, array] stack order (popped as array, val)
    UnsetDim,
    UnsetNestedDim(u8), // Unset nested array element. Arg is depth (number of keys). Stack: [key_n, ..., key_1, array] -> [modified_array]
    InArray,
    ArrayKeyExists,

    // Iteration
    IterInit(u32),         // [Array] -> [Array, Index]. If empty, pop and jump.
    IterValid(u32),        // [Array, Index]. If invalid (end), pop both and jump.
    IterNext,              // [Array, Index] -> [Array, Index+1]
    IterGetVal(Symbol),    // [Array, Index] -> Assigns val to local
    IterGetValRef(Symbol), // [Array, Index] -> Assigns ref to local
    IterGetKey(Symbol),    // [Array, Index] -> Assigns key to local
    FeResetR(u32),
    FeFetchR(u32),
    FeResetRw(u32),
    FeFetchRw(u32),
    FeFree,

    // Constants
    FetchGlobalConst(Symbol),
    DefGlobalConst(Symbol, u16), // (name, val_idx)

    // Objects
    DefClass(Symbol, Option<Symbol>), // Define class (name, parent)
    DefInterface(Symbol),             // Define interface (name)
    DefTrait(Symbol),                 // Define trait (name)
    AddInterface(Symbol, Symbol),     // (class_name, interface_name)
    UseTrait(Symbol, Symbol),         // (class_name, trait_name)
    AllowDynamicProperties(Symbol), // Mark class as allowing dynamic properties (for #[AllowDynamicProperties])
    MarkAbstract(Symbol),           // Mark class as abstract
    FinalizeClass(Symbol), // Validate class after all methods are defined (interfaces, abstract methods)
    DefMethod(Symbol, Symbol, u32, Visibility, bool, bool), // (class_name, method_name, func_idx, visibility, is_static, is_abstract)
    DefProp(Symbol, Symbol, u16, Visibility, u32, bool), // (class_name, prop_name, default_val_idx, visibility, type_hint_idx, is_readonly)
    DefClassConst(Symbol, Symbol, u16, Visibility), // (class_name, const_name, val_idx, visibility)
    DefStaticProp(Symbol, Symbol, u16, Visibility, u32), // (class_name, prop_name, default_val_idx, visibility, type_hint_idx)
    FetchClassConst(Symbol, Symbol),                     // (class_name, const_name) -> [Val]
    FetchClassConstDynamic(Symbol),                      // [Class] -> [Val] (const_name is arg)
    FetchStaticProp(Symbol, Symbol),                     // (class_name, prop_name) -> [Val]
    AssignStaticProp(Symbol, Symbol),                    // (class_name, prop_name) [Val] -> [Val]
    CallStaticMethod(Symbol, Symbol, u8), // (class_name, method_name, arg_count) -> [RetVal]
    CallStaticMethodDynamic(u8),          // [ClassName, MethodName, Arg1...ArgN] -> [RetVal]
    New(Symbol, u8),                      // Create instance, call constructor with N args
    NewDynamic(u8),     // [ClassName] -> Create instance, call constructor with N args
    FetchProp(Symbol),  // [Obj] -> [Val]
    FetchPropDynamic,   // [Obj, Name] -> [Val]
    AssignProp(Symbol), // [Obj, Val] -> [Val]
    CallMethod(Symbol, u8), // [Obj, Arg1...ArgN] -> [RetVal]
    CallMethodDynamic(u8), // [Obj, MethodName, Arg1...ArgN] -> [RetVal]
    UnsetObj,
    UnsetStaticProp,
    InstanceOf,
    GetClass,
    GetCalledClass,
    GetType,
    Clone,
    Copy, // Copy value (for closure capture by value)

    // Closures
    Closure(u32, u32), // (func_idx, num_captures) -> [Closure]

    // Exceptions
    Throw, // [Obj] -> !
    Catch,

    // Generators
    Yield(bool), // bool: has_key
    YieldFrom,
    GetSentValue, // Push sent value from GeneratorData

    // Assignment Ops
    AssignOp(u8), // 0=Add, 1=Sub, 2=Mul, 3=Div, 4=Mod, 5=Sl, 6=Sr, 7=Concat, 8=BwOr, 9=BwAnd, 10=BwXor, 11=Pow
    PreInc,
    PreDec,
    PostInc,
    PostDec,

    // Casts
    Cast(u8), // 0=Int, 1=Bool, 2=Float, 3=String, 4=Array, 5=Object, 6=Unset

    // Type Check
    TypeCheck,

    // Isset/Empty
    IssetVar(Symbol),
    IssetVarDynamic,
    IssetDim,
    IssetProp(Symbol),
    IssetStaticProp(Symbol),

    // Match
    Match,
    MatchError,

    // Zend Opcodes (Added for completeness)
    AssignObj,
    AssignStaticPropOp(u8),
    AssignObjOp(u8),
    AssignDimOp(u8),
    AssignObjRef,
    AssignStaticPropRef,
    PreIncStaticProp,
    PreDecStaticProp,
    PostIncStaticProp,
    PostDecStaticProp,
    CheckVar(Symbol),
    SendVarNoRefEx,
    Bool,
    RopeInit,
    RopeAdd,
    RopeEnd,
    BeginSilence,
    EndSilence,
    InitFcallByName,
    DoFcall,
    InitFcall,
    SendVarEx,
    InitNsFcallByName,
    Free,
    AddArrayElement,
    IncludeOrEval,
    FetchR(Symbol),
    FetchW(Symbol),
    FetchRw(Symbol),
    FetchIs(Symbol),
    FetchUnset(Symbol),
    FetchDimR,
    FetchDimW,
    FetchDimRw,
    FetchDimIs,
    FetchDimUnset,
    FetchObjR,
    FetchObjW,
    FetchObjRw,
    FetchObjIs,
    FetchObjUnset,
    FetchFuncArg(Symbol),
    FetchDimFuncArg,
    FetchObjFuncArg,
    FetchListR,
    FetchConstant(Symbol),
    CheckFuncArg(Symbol),
    ExtStmt,
    ExtFcallBegin,
    ExtFcallEnd,
    ExtNop,
    SendVarNoRef,
    FetchClass,
    ReturnByRef,
    InitMethodCall,
    InitStaticMethodCall,
    IssetIsemptyVar,
    IssetIsemptyDimObj,
    SendValEx,
    InitUserCall,
    SendArray,
    SendUser,
    VerifyReturnType,
    InitDynamicCall,
    DoIcall,
    DoUcall,
    DoFcallByName,
    PreIncObj,
    PreDecObj,
    PostIncObj,
    PostDecObj,
    OpData,
    GeneratorCreate,
    DeclareFunction,
    DeclareLambdaFunction,
    DeclareConst,
    DeclareClass,
    DeclareClassDelayed,
    DeclareAnonClass,
    AddArrayUnpack,
    IssetIsemptyPropObj,
    HandleException,
    UserOpcode,
    AssertCheck,
    JmpSet,
    UnsetCv,
    IssetIsemptyCv,
    FetchListW,
    Separate,
    FetchClassName,
    CallTrampoline,
    DiscardException,
    GeneratorReturn,
    FastCall,
    FastRet,
    RecvVariadic(u32),
    SendUnpack,
    CopyTmp,
    FuncNumArgs,
    FuncGetArgs,
    FetchStaticPropR,
    FetchStaticPropW,
    FetchStaticPropRw,
    FetchStaticPropIs,
    FetchStaticPropFuncArg,
    FetchStaticPropUnset,
    IssetIsemptyStaticProp,
    BindLexical,
    FetchThis,
    SendFuncArg,
    IssetIsemptyThis,
    SwitchLong,
    SwitchString,
    CaseStrict,
    JmpNull,
    CheckUndefArgs,
    FetchGlobals,
    VerifyNeverType,
    CallableConvert,
    BindInitStaticOrJmp,
    FramelessIcall0,
    FramelessIcall1,
    FramelessIcall2,
    FramelessIcall3,
    JmpFrameless,
    InitParentPropertyHookCall,
    DeclareAttributedConst,
}
