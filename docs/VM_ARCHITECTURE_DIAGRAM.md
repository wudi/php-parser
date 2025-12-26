# PHP VM Architecture Diagram

## Component Hierarchy

```
┌─────────────────────────────────────────────────────────────────┐
│                         EngineContext                           │
│  (Process-scoped, Arc-shared across requests)                   │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────────────────────────────────────────────┐    │
│  │            ExtensionRegistry                          │    │
│  │  ┌─────────────────────────────────────────────────┐ │    │
│  │  │ Functions: HashMap<Vec<u8>, NativeHandler>     │ │    │
│  │  │ Classes:   HashMap<Vec<u8>, NativeClassDef>   │ │    │
│  │  │ Constants: HashMap<Vec<u8>, Val>               │ │    │
│  │  │ Extensions: Vec<Box<dyn Extension>>            │ │    │
│  │  │ By-Ref Args: HashMap<Vec<u8>, Vec<usize>>      │ │    │
│  │  └─────────────────────────────────────────────────┘ │    │
│  └───────────────────────────────────────────────────────┘    │
│                                                                 │
│  [Deprecated - backward compat only]                           │
│  • functions: HashMap<Vec<u8>, NativeHandler>                  │
│  • constants: HashMap<Symbol, Val>                             │
│  • pdo_driver_registry: Arc<DriverRegistry>                    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                             │ Arc::clone()
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                       RequestContext                            │
│  (Per-request, mutable state for single PHP script execution)   │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  • engine: Arc<EngineContext>  (immutable reference)           │
│  • interner: Interner          (Symbol ↔ &[u8] mapping)        │
│                                                                 │
│  ┌───────────────────────────────────────────────────────┐    │
│  │ Symbol Tables                                         │    │
│  │  • globals: HashMap<Symbol, Handle>                  │    │
│  │  • constants: HashMap<Symbol, Val>                   │    │
│  │  • user_functions: HashMap<Symbol, Rc<UserFunc>>     │    │
│  │  • classes: HashMap<Symbol, ClassDef>                │    │
│  │  • native_methods: HashMap<(Symbol,Symbol), ...>     │    │
│  └───────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌───────────────────────────────────────────────────────┐    │
│  │ Resource Management                                   │    │
│  │  • resource_manager: ResourceManager                 │    │
│  │  • mysqli_connections: HashMap<u64, Rc<RefCell<...>>>│    │
│  │  • pdo_connections: HashMap<u64, ...>                │    │
│  │  • zip_archives: HashMap<u64, ...>                   │    │
│  └───────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌───────────────────────────────────────────────────────┐    │
│  │ Extension State                                       │    │
│  │  • extension_data: HashMap<TypeId, Box<dyn Any>>     │    │
│  │  • hash_states: HashMap<u64, Box<dyn HashState>>     │    │
│  │  • json_last_error: JsonError                        │    │
│  └───────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌───────────────────────────────────────────────────────┐    │
│  │ Request Metadata                                      │    │
│  │  • headers: Vec<HeaderEntry>                         │    │
│  │  • http_status: Option<i64>                          │    │
│  │  • error_reporting: u32                              │    │
│  │  • last_error: Option<ErrorInfo>                     │    │
│  │  • included_files: HashSet<String>                   │    │
│  │  • autoloaders: Vec<Handle>                          │    │
│  └───────────────────────────────────────────────────────┘    │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                             │ owned by
                             ▼
┌─────────────────────────────────────────────────────────────────┐
│                              VM                                 │
│  (Execution engine for single request)                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌───────────────────────────────────────────────────────┐    │
│  │ Memory & Allocation                                   │    │
│  │  • arena: Arena                (bump allocator)      │    │
│  │  • memory_limit: usize                               │    │
│  └───────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌───────────────────────────────────────────────────────┐    │
│  │ Execution State                                       │    │
│  │  • operand_stack: Stack                              │    │
│  │  • frames: Vec<CallFrame>                            │    │
│  │  • last_return_value: Option<Handle>                 │    │
│  │  • silence_stack: Vec<u32>                           │    │
│  │  • pending_calls: Vec<PendingCall>                   │    │
│  └───────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌───────────────────────────────────────────────────────┐    │
│  │ I/O & Error Handling                                  │    │
│  │  • output_writer: Box<dyn OutputWriter>              │    │
│  │  • error_handler: Box<dyn ErrorHandler>              │    │
│  │  • output_buffers: Vec<OutputBuffer>                 │    │
│  └───────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌───────────────────────────────────────────────────────┐    │
│  │ Runtime Tracking                                      │    │
│  │  • superglobal_map: HashMap<Symbol, SuperglobalKind> │    │
│  │  • var_handle_map: HashMap<Handle, Symbol>           │    │
│  │  • pending_undefined: HashMap<Handle, Symbol>        │    │
│  └───────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌───────────────────────────────────────────────────────┐    │
│  │ Sandboxing & Limits                                   │    │
│  │  • execution_start_time: SystemTime                  │    │
│  │  • allow_file_io: bool                               │    │
│  │  • allow_network: bool                               │    │
│  │  • disable_functions: HashSet<String>                │    │
│  │  • disable_classes: HashSet<String>                  │    │
│  └───────────────────────────────────────────────────────┘    │
│                                                                 │
│  • context: RequestContext                                     │
│  • opcodes_executed: u64                                       │
│  • function_calls: u64                                         │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

## Data Flow Architecture

### 1. Value Allocation & Lifetime

```
┌────────────┐
│ PHP Source │
└─────┬──────┘
      │ compile
      ▼
┌────────────┐
│  Bytecode  │
└─────┬──────┘
      │ VM::run()
      ▼
┌─────────────────────────────────────────────────────────────┐
│                       VM Execution                          │
│                                                             │
│  Arena (Bump Allocator)                                     │
│  ┌───────────────────────────────────────────────────┐    │
│  │  [Val | Val | Val | ... | Val]                   │    │
│  │   ▲                                               │    │
│  │   │ Handle (lifetime: 'arena)                     │    │
│  │   │ = &'arena Val                                 │    │
│  └───┼───────────────────────────────────────────────┘    │
│      │                                                     │
│      │ Zero-copy, zero-heap guarantees                    │
│      │                                                     │
│  Operand Stack                                             │
│  ┌───────────────────────────────────────────────────┐    │
│  │  [Handle | Handle | Handle | ...]                │    │
│  │   (cheap to push/pop, just pointer-sized)         │    │
│  └───────────────────────────────────────────────────┘    │
│                                                             │
│  Call Frames                                               │
│  ┌───────────────────────────────────────────────────┐    │
│  │ Frame N: { locals: HashMap<Symbol, Handle>,      │    │
│  │            ip: usize, func: Rc<UserFunc>, ... }   │    │
│  ├───────────────────────────────────────────────────┤    │
│  │ Frame N-1: { ... }                                │    │
│  ├───────────────────────────────────────────────────┤    │
│  │ ...                                               │    │
│  └───────────────────────────────────────────────────┘    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Key Characteristics:**
- **Zero-Heap**: AST nodes use `&'ast T` (arena references), never `Box<T>` or `Vec<T>`
- **Zero-Copy**: Values stored in arena, Handles are just `&'arena Val` references
- **Cheap Operations**: Pushing/popping Handles is pointer-sized (8 bytes on 64-bit)
- **Lifetime Safety**: All Handles tied to VM arena lifetime, enforced by Rust borrow checker

### 2. Symbol Resolution Pipeline

```
┌─────────────────────────────────────────────────────────────┐
│                  Symbol Lookup Process                      │
└─────────────────────────────────────────────────────────────┘

┌──────────────┐
│ PHP Code:    │
│ $x = foo()   │
│ $x = Foo::$y │
│ $x = FOO     │
└──────┬───────┘
       │
       ▼
┌──────────────────────────────────────────────────────────┐
│ 1. Bytecode: Contains raw byte slices (&[u8])           │
│    Example: CallFunc(b"foo")                            │
│              GetClassConst(b"Foo", b"BAR")              │
└──────┬───────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────┐
│ 2. Interner: Convert &[u8] → Symbol (deduplicated u32)  │
│    RequestContext::interner.intern(b"foo") → Symbol(42) │
│    - Symbol = u32 (4 bytes, cheap to copy/hash)         │
│    - Deduplicated: same byte slice → same Symbol        │
└──────┬───────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────┐
│ 3. Lookup in Symbol Tables (HashMap<Symbol, ...>)       │
│                                                          │
│    Function Call: foo()                                 │
│    ┌────────────────────────────────────────────┐      │
│    │ A. RequestContext.user_functions           │      │
│    │    → HashMap<Symbol, Rc<UserFunc>>         │      │
│    │    ✓ User-defined PHP functions            │      │
│    ├────────────────────────────────────────────┤      │
│    │ B. EngineContext.registry.get_function()   │      │
│    │    → HashMap<Vec<u8>, NativeHandler>       │      │
│    │    ✓ Native Rust functions (strlen, etc)   │      │
│    │    ✓ Case-insensitive fallback             │      │
│    ├────────────────────────────────────────────┤      │
│    │ C. Trigger autoload if not found           │      │
│    │    → Call registered __autoload handlers   │      │
│    └────────────────────────────────────────────┘      │
│                                                          │
│    Class Access: Foo::$bar / new Foo()                  │
│    ┌────────────────────────────────────────────┐      │
│    │ A. RequestContext.classes                  │      │
│    │    → HashMap<Symbol, ClassDef>             │      │
│    │    ✓ User-defined PHP classes              │      │
│    ├────────────────────────────────────────────┤      │
│    │ B. EngineContext.registry.get_class()      │      │
│    │    → HashMap<Vec<u8>, NativeClassDef>      │      │
│    │    ✓ Native classes (DateTime, PDO, etc)   │      │
│    ├────────────────────────────────────────────┤      │
│    │ C. Inheritance chain walking               │      │
│    │    → VM::walk_inheritance_chain()          │      │
│    │    → Check parent, interfaces, traits      │      │
│    └────────────────────────────────────────────┘      │
│                                                          │
│    Constant Access: FOO / Foo::BAR                      │
│    ┌────────────────────────────────────────────┐      │
│    │ A. RequestContext.constants                │      │
│    │    → HashMap<Symbol, Val>                  │      │
│    │    ✓ User-defined constants                │      │
│    ├────────────────────────────────────────────┤      │
│    │ B. EngineContext.registry.get_constant()   │      │
│    │    → HashMap<Vec<u8>, Val>                 │      │
│    │    ✓ Engine constants (PHP_VERSION, etc)   │      │
│    ├────────────────────────────────────────────┤      │
│    │ C. Class constant via inheritance          │      │
│    │    → VM::find_class_constant()             │      │
│    │    → Visibility check                      │      │
│    └────────────────────────────────────────────┘      │
│                                                          │
│    Variable Access: $x                                  │
│    ┌────────────────────────────────────────────┐      │
│    │ A. Current Frame Locals                    │      │
│    │    → frames.last().locals[Symbol]          │      │
│    │    ✓ Function/method local variables       │      │
│    ├────────────────────────────────────────────┤      │
│    │ B. RequestContext.globals                  │      │
│    │    → HashMap<Symbol, Handle>               │      │
│    │    ✓ Global scope variables                │      │
│    ├────────────────────────────────────────────┤      │
│    │ C. Superglobal Detection                   │      │
│    │    → VM.superglobal_map[Symbol]            │      │
│    │    → Lazy-create $_SERVER, $_GET, etc      │      │
│    └────────────────────────────────────────────┘      │
│                                                          │
└──────────────────────────────────────────────────────────┘
```

### 3. Extension Loading Lifecycle

```
┌─────────────────────────────────────────────────────────────┐
│                    Process Startup                          │
└─────────────────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────┐
│ 1. EngineBuilder::new()                                  │
│    .with_extension(CoreExtension)                        │
│    .with_extension(PdoExtension)                         │
│    .with_extension(MysqliExtension)                      │
│    .build() → Arc<EngineContext>                         │
└──────┬───────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────┐
│ 2. For Each Extension:                                   │
│    ┌──────────────────────────────────────────────┐    │
│    │ extension.module_init(&mut registry)         │    │
│    │  → MINIT Hook (Module Initialization)        │    │
│    │                                               │    │
│    │  registry.register_function(b"strlen", ...)  │    │
│    │  registry.register_class(NativeClassDef{...})│    │
│    │  registry.register_constant(b"PHP_VERSION",..)│   │
│    └──────────────────────────────────────────────┘    │
└──────┬───────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────┐
│ EngineContext Created (Process-Scoped, Immutable)       │
│  • ExtensionRegistry populated with all functions       │
│  • Native classes registered                            │
│  • Engine constants registered                          │
│  • Shared via Arc across all requests                   │
└──────┬───────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────┐
│                  Per-Request Execution                       │
└─────────────────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────┐
│ 3. RequestContext::new(engine.clone())                   │
│    ┌──────────────────────────────────────────────┐    │
│    │ copy_engine_constants()                      │    │
│    │  → Clone constants from registry to context  │    │
│    │  → Symbols pre-interned, cheap to copy       │    │
│    ├──────────────────────────────────────────────┤    │
│    │ materialize_extension_classes()              │    │
│    │  → Convert NativeClassDef to ClassDef        │    │
│    │  → On-demand, lazy materialization           │    │
│    └──────────────────────────────────────────────┘    │
└──────┬───────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────┐
│ 4. For Each Extension:                                   │
│    ┌──────────────────────────────────────────────┐    │
│    │ extension.request_init(&mut context)         │    │
│    │  → RINIT Hook (Request Initialization)       │    │
│    │                                               │    │
│    │  Initialize per-request state                │    │
│    │  context.set_extension_data(MyData{...})     │    │
│    │  Set up request-specific resources           │    │
│    └──────────────────────────────────────────────┘    │
└──────┬───────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────┐
│ 5. VM::new_with_context(context)                         │
│    ┌──────────────────────────────────────────────┐    │
│    │ Initialize superglobals ($_SERVER, etc)      │    │
│    │ Set up operand stack and call frames         │    │
│    │ Configure output/error handlers              │    │
│    └──────────────────────────────────────────────┘    │
└──────┬───────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────┐
│ 6. vm.run(chunk) → Execute Bytecode                      │
└──────┬───────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────┐
│ 7. Request Cleanup (Drop RequestContext)                 │
│    ┌──────────────────────────────────────────────┐    │
│    │ For Each Extension (reverse order):          │    │
│    │   extension.request_shutdown(&mut context)   │    │
│    │    → RSHUTDOWN Hook                          │    │
│    │    → Clean up per-request resources          │    │
│    └──────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────┘
       │
       ▼
┌─────────────────────────────────────────────────────────────┐
│                 Process Shutdown (Optional)                  │
└─────────────────────────────────────────────────────────────┘
       │
       ▼
┌──────────────────────────────────────────────────────────┐
│ 8. Drop EngineContext                                    │
│    ┌──────────────────────────────────────────────┐    │
│    │ For Each Extension (reverse order):          │    │
│    │   extension.module_shutdown()                │    │
│    │    → MSHUTDOWN Hook                          │    │
│    │    → Clean up process-level resources        │    │
│    └──────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────┘
```

## Key Lookup Sequences

### Function Call Resolution

```
PHP: foo($arg1, $arg2)
       │
       ▼
┌────────────────────────────────────────────────────────┐
│ OpCode::CallFunc { name: b"foo", arg_count: 2 }       │
└──────┬─────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────┐
│ 1. Intern symbol: Symbol = interner.intern(b"foo")    │
└──────┬─────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────┐
│ 2. Check user functions:                              │
│    if let Some(func) = context.user_functions[Symbol] │
│        → Prepare CallFrame                            │
│        → VM::push_function_frame(func, args)          │
│        → Continue execution in new frame              │
└──────┬─────────────────────────────────────────────────┘
       │ Not found
       ▼
┌────────────────────────────────────────────────────────┐
│ 3. Check native functions:                            │
│    if let Some(handler) = registry.get_function(b"foo")│
│        → Call handler(&mut vm, &args)                 │
│        → Push result to operand stack                 │
└──────┬─────────────────────────────────────────────────┘
       │ Not found
       ▼
┌────────────────────────────────────────────────────────┐
│ 4. Trigger autoload:                                  │
│    for autoloader in context.autoloaders {            │
│        call autoloader with class name               │
│        retry lookup                                   │
│    }                                                   │
└──────┬─────────────────────────────────────────────────┘
       │ Still not found
       ▼
┌────────────────────────────────────────────────────────┐
│ 5. Error: UndefinedFunction                           │
└────────────────────────────────────────────────────────┘
```

### Method Call Resolution (with Inheritance)

```
PHP: $obj->method($arg)
       │
       ▼
┌────────────────────────────────────────────────────────┐
│ OpCode::MethodCall { name: b"method", arg_count: 1 }  │
└──────┬─────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────┐
│ 1. Extract object class from handle                   │
│    obj_handle → ObjectData.class_name (Symbol)        │
└──────┬─────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────┐
│ 2. Intern method name: Symbol = interner.intern(...)  │
└──────┬─────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────┐
│ 3. Walk inheritance chain:                            │
│    VM::walk_inheritance_chain(class_name, |cls, sym| {│
│        if let Some(method) = cls.methods[method_sym]  │
│            ✓ Found in current class                   │
│            → Check visibility                         │
│            → Return method                            │
│        } else {                                        │
│            ↑ Check parent class                       │
│            ↑ Check interfaces                         │
│            ↑ Check traits                             │
│        }                                               │
│    })                                                  │
└──────┬─────────────────────────────────────────────────┘
       │ Found
       ▼
┌────────────────────────────────────────────────────────┐
│ 4. Visibility check (calling_scope vs declaring_class)│
│    → Public: always accessible                        │
│    → Protected: accessible if subclass                │
│    → Private: accessible only in declaring class      │
└──────┬─────────────────────────────────────────────────┘
       │ Allowed
       ▼
┌────────────────────────────────────────────────────────┐
│ 5. Create method frame:                               │
│    VM::push_method_frame(method, obj, args)           │
│    → Set $this binding                                │
│    → Set calling_scope and class_scope                │
└──────┬─────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────┐
│ 6. Execute method bytecode                            │
└────────────────────────────────────────────────────────┘
```

### Property Access with Magic Methods

```
PHP: $obj->prop
       │
       ▼
┌────────────────────────────────────────────────────────┐
│ OpCode::GetProp { name: b"prop" }                     │
└──────┬─────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────┐
│ 1. Extract ObjectData from handle                     │
│    obj_handle → &ObjectData                           │
└──────┬─────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────┐
│ 2. Check direct property access:                      │
│    if let Some(handle) = obj.properties[prop_sym]     │
│        ✓ Property exists                              │
│        → Check visibility                             │
│        → Return property handle                       │
└──────┬─────────────────────────────────────────────────┘
       │ Not found
       ▼
┌────────────────────────────────────────────────────────┐
│ 3. Check for __get() magic method:                    │
│    if class.methods.contains(__get)                   │
│        → VM::call_magic_method_sync()                 │
│        → Create temp frame for __get($name)           │
│        → Execute __get                                │
│        → Return result                                │
└──────┬─────────────────────────────────────────────────┘
       │ No __get
       ▼
┌────────────────────────────────────────────────────────┐
│ 4. Check if dynamic properties allowed:               │
│    if class.allows_dynamic_properties                 │
│        → Return NULL (undefined property)             │
│    else                                                │
│        → Error/Warning (dynamic prop not allowed)     │
└────────────────────────────────────────────────────────┘
```

## Performance Characteristics

### Memory Allocation Profile

```
┌──────────────────────┬──────────────────┬─────────────────┐
│ Operation            │ Heap Allocations │ Arena Allocs    │
├──────────────────────┼──────────────────┼─────────────────┤
│ Push constant        │ 0                │ 1 (if new Val)  │
│ Push variable        │ 0                │ 0 (ref existing)│
│ Call function        │ 1 (CallFrame)    │ 0               │
│ Create array         │ 0                │ 1 (ArrayData)   │
│ Array append         │ 0*               │ 0-1*            │
│ Create object        │ 0                │ 1 (ObjectData)  │
│ String concat        │ 0                │ 1 (new string)  │
└──────────────────────┴──────────────────┴─────────────────┘

* Array operations may require reallocation if capacity exceeded
  All array storage is arena-allocated (IndexMap inside Val::Array)
```

### Lookup Time Complexity

```
┌─────────────────────────┬──────────────┬────────────────┐
│ Lookup Type             │ Best Case    │ Worst Case     │
├─────────────────────────┼──────────────┼────────────────┤
│ User function           │ O(1)         │ O(1)           │
│ Native function         │ O(1)         │ O(n) fallback  │
│ Local variable          │ O(1)         │ O(1)           │
│ Global variable         │ O(1)         │ O(1)           │
│ Constant                │ O(1)         │ O(1)           │
│ Method (no inherit)     │ O(1)         │ O(1)           │
│ Method (with inherit)   │ O(1)         │ O(depth)       │
│ Property (direct)       │ O(1)         │ O(1)           │
│ Property (magic __get)  │ O(n)         │ O(n)           │
│ Class constant          │ O(1)         │ O(depth)       │
└─────────────────────────┴──────────────┴────────────────┘

Where:
  n = number of registered functions/classes
  depth = inheritance chain depth (parent → grandparent → ...)
```

## Thread Safety & Concurrency Model

```
┌─────────────────────────────────────────────────────────────┐
│                  Multi-Request Architecture                  │
│                (Typical PHP-FPM Worker Model)                │
└─────────────────────────────────────────────────────────────┘

           ┌─────────────────────────────────┐
           │    Arc<EngineContext>           │
           │  (Immutable, Arc-shared)        │
           │  • ExtensionRegistry            │
           │  • PDO Driver Registry          │
           └───────────┬─────────────────────┘
                       │
        ┌──────────────┼──────────────┐
        │              │              │
        ▼              ▼              ▼
   ┌─────────┐   ┌─────────┐   ┌─────────┐
   │Worker 1 │   │Worker 2 │   │Worker N │
   │         │   │         │   │         │
   │ Request │   │ Request │   │ Request │
   │ Context │   │ Context │   │ Context │
   │   │     │   │   │     │   │   │     │
   │   ▼     │   │   ▼     │   │   ▼     │
   │  VM 1   │   │  VM 2   │   │  VM N   │
   └─────────┘   └─────────┘   └─────────┘
   
   No shared mutable state between workers
   Each worker has isolated RequestContext + VM
```

**Key Properties:**
- **Immutable Shared State**: `EngineContext` is `Arc`-wrapped, never mutated after init
- **Isolated Requests**: Each request gets its own `RequestContext` and `VM`
- **No Locks Required**: No `Mutex`/`RwLock` needed for request execution
- **Fork-Safe**: Can safely fork after `EngineContext` creation
- **Extension Data Isolation**: Each request has separate `extension_data` HashMap

## Extension Data Flow

```
┌─────────────────────────────────────────────────────────────┐
│              Extension Request Lifecycle                     │
└─────────────────────────────────────────────────────────────┘

Request Start
    │
    ▼
┌────────────────────────────────────────────────────────┐
│ RINIT: extension.request_init(&mut context)            │
│  ┌──────────────────────────────────────────────┐    │
│  │ struct MyExtensionData { state: ... }        │    │
│  │                                               │    │
│  │ context.set_extension_data(MyExtensionData { │    │
│  │     state: initialize_state(),               │    │
│  │ });                                           │    │
│  └──────────────────────────────────────────────┘    │
└──────┬─────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────┐
│ Runtime: Native function calls                         │
│  ┌──────────────────────────────────────────────┐    │
│  │ fn my_function(vm: &mut VM, args: &[Handle]) │    │
│  │ {                                             │    │
│  │   let data = vm.context                       │    │
│  │       .get_extension_data_mut::<MyExtData>() │    │
│  │       .unwrap();                              │    │
│  │                                               │    │
│  │   data.state.update(...);                    │    │
│  │   // Use per-request state                   │    │
│  │ }                                             │    │
│  └──────────────────────────────────────────────┘    │
└──────┬─────────────────────────────────────────────────┘
       │
       ▼
┌────────────────────────────────────────────────────────┐
│ RSHUTDOWN: extension.request_shutdown(&mut context)    │
│  ┌──────────────────────────────────────────────┐    │
│  │ if let Some(data) = context                  │    │
│  │     .get_extension_data_mut::<MyExtData>() { │    │
│  │   data.cleanup();                            │    │
│  │ }                                             │    │
│  │ // Data automatically dropped                │    │
│  └──────────────────────────────────────────────┘    │
└────────────────────────────────────────────────────────┘
       │
       ▼
Request End (Drop RequestContext)
```

**Type Safety Guarantees:**
- Extension data keyed by `TypeId` (unique per type)
- No collisions possible between extensions using different types
- Compile-time type safety via generics
- Runtime check: `get_extension_data::<T>()` returns `Option<&T>`

## Summary of Design Principles

1. **Zero-Heap AST**: All AST nodes arena-allocated, no `Box`/`Vec`/`String`
2. **Zero-Copy Values**: Handles are references to arena memory
3. **Symbol Deduplication**: `&[u8]` → `Symbol` (u32) for fast hashing
4. **Lazy Materialization**: Classes/constants loaded on-demand
5. **Request Isolation**: No shared mutable state between requests
6. **Extension Modularity**: Clean MINIT/RINIT/RSHUTDOWN lifecycle
7. **Type-Safe Extensions**: `TypeId`-based extension data storage
8. **Fault Tolerance**: No panics, all errors via `VmError`
9. **Performance Profiling**: Opcode counting, execution timing
10. **Sandboxing Support**: Resource limits, function/class blacklists

---

**References:**
- Zend Engine Architecture: `$PHP_SRC_PATH/Zend/zend_vm_def.h`
- Extension API: `$PHP_SRC_PATH/Zend/zend_extensions.h`
- Symbol Tables: `$PHP_SRC_PATH/Zend/zend_hash.c`
- Inheritance: `$PHP_SRC_PATH/Zend/zend_inheritance.c`
