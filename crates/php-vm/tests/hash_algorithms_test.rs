use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::{VmError, VM};
use std::rc::Rc;
use std::sync::Arc;

fn run_code(source: &str) -> Result<String, VmError> {
    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(source.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }

    let emitter = Emitter::new(source.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let mut vm = VM::new_with_context(request_context);
    vm.run(Rc::new(chunk))?;

    Ok("Success".to_string())
}

#[test]
fn test_new_algorithms() {
    let source = r#"
        <?php
        $tests = [
            'adler32' => '062c0215',
            'fnv132' => 'b6fa7167',
            'fnv1a32' => '4f9f2cab',
            'fnv164' => '7b495389bdbdd4c7',
            'fnv1a64' => 'a430d84680aabd0b',
            'joaat' => 'c8fd181b',
            'xxh32' => 'fb0077f9',
            'xxh64' => '26c7827d889f6da3',
            'xxh3' => '9555e8555c62dcfd',
            'xxh128' => 'b5e9c1ad071b3e7fc779cfaa5e523818',
            'crc32' => '3610a686',
            'crc32b' => '3610a686',
            'tiger192,3' => '2cfd7f6f336288a7f2741b9bf874388a54026639cadb7bf2',
            'tiger160,3' => '2cfd7f6f336288a7f2741b9bf874388a54026639',
            'tiger128,3' => '2cfd7f6f336288a7f2741b9bf874388a',
        ];

        foreach ($tests as $algo => $expected) {
            $data = 'hello';
            echo "Data length: " . strlen($data) . "\n";
            $actual = hash($algo, $data);
            if ($actual !== $expected) {
                echo "Algo $algo failed: expected $expected, got $actual\n";
                $failed = true;
            }
        }
        if (isset($failed)) {
            throw new Exception("Some algorithms failed");
        }
    "#;

    let result = run_code(source);
    assert!(result.is_ok(), "Failed: {:?}", result.err());
}
