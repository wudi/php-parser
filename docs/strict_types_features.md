Implementing the declare(strict_types=1); feature in a PHP VM requires changes to both the compiler and the executor. This directive is unique because it is entirely compile-time and affects how opcodes are generated and executed based on the file scope. 

## 1. Parser & Compiler Implementation
 - Directive Positioning: Ensure the parser only accepts declare(strict_types=1); as the first statement in a file. Throw a compiler error if it appears later or inside a block.
 - Per-File Compilation Flag: During the compilation of a PHP file (Zend script), maintain a boolean flag (e.g., strict_types) in the file's compilation context.
 - Opcode Tagging: When compiling function calls (DO_FCALL, DO_ICALL) or return statements (RETURN), tag the resulting opcodes with the current file's strict_types status. 

## 2. Parameter Type Checking (The Caller's Rule)
 - Caller-Side Enforcement: In the VM's execution loop, check the strict_types flag of the calling file's opcode.
 - Disable Type Juggling: If the caller is in strict mode, disable automatic type coercion (e.g., converting "10" to 10) for scalar parameters (int, float, string, bool).
 - The "Int-to-Float" Exception: Implement the single allowed widening conversion: an int may be passed to a function expecting a float, even in strict mode.
 - TypeError Generation: If types do not match and are not the allowed exception, throw a TypeError instead of performing a cast. 

## 3. Return Type Checking (The Callee's Rule)
 - Callee-Side Enforcement: Unlike parameters, return type strictness is governed by the file where the function is defined.
 - Runtime Return Check: When a function returns a value, check the definition file's strict_types flag. If enabled, the return value must match the declared return type exactly (with the same int-to-float exception). 

## 4. Integration with Built-in Functions
 - Internal Function Dispatch: Ensure that calls to built-in PHP functions (internal extensions) also respect the caller's strict mode.
 - Error Level Shift: For internal functions, failures in strict mode should produce a TypeError (equivalent to E_RECOVERABLE_ERROR) rather than a standard E_WARNING. 

## 5. Scope Isolation Testing
 - To verify your implementation is comprehensive, test these scenarios:
 - Strict Caller -> Weak Callee: Parameters must be checked strictly.
 - Weak Caller -> Strict Callee: Parameters should be coerced (weakly checked), but the callee's return value must be checked strictly.
 - Include/Require: Ensure a strict file including a weak file does not force strictness onto the included file.
 - Eval(): Verify how eval() handles strictness (it typically defaults to the mode of the eval() call's location unless declared within the evaled string). 