# Parser TODO (PHP Parity)

Reference sources: `$PHP_SRC_PATH/Zend/zend_language_scanner.l` (tokens/lexing), `$PHP_SRC_PATH/Zend/zend_language_parser.y` (grammar), and `$PHP_SRC_PATH/Zend/zend_ast.h` (AST kinds). Do **not** introduce non-PHP syntax or AST kinds; mirror Zend semantics.

# VM OpCode Coverage
- Verify all Zend opcodes have parity behavior (dispatch coverage is now complete; many are still no-ops mirroring placeholders). Remaining work: `HandleException` unwinding + catch table walk, `AssertCheck`, `JmpSet`, frameless call family (`FramelessIcall*`, `JmpFrameless`), `CallTrampoline`, `FastCall/FastRet`, `DiscardException`, `BindLexical`, `CallableConvert`, `CheckUndefArgs`, `BindInitStaticOrJmp`, property hook opcodes (`InitParentPropertyHookCall`, `DeclareAttributedConst`).
- Fill real behavior for stubbed/no-op arms where Zend performs work: `Catch`, `Ticks`, `TypeCheck`, `Match` error handling, static property by-ref/func-arg/unset flows (respect visibility/public branch).
- Align `VerifyReturnType` and related type checks with Zend diagnostics; currently treated as a nop.
- Keep emitter in sync so PHP constructs exercise the implemented opcodes (e.g., match error path, static property arg/unset, frameless calls). Remove dead variants if PHP cannot emit them.
- Add integration tests against native `php` for each newly implemented opcode path (stdout/stderr/exit code as needed); existing coverage: `strlen`, send ops/ref mutation/dynamic calls, variadics/unpack, array unpack spreads, static property fetch modes.
