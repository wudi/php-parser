use php_vm::compiler::emitter::Emitter;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::vm::engine::{OutputWriter, VmError, VM};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

struct StringOutputWriter {
    buffer: Vec<u8>,
}

impl StringOutputWriter {
    fn new() -> Self {
        Self { buffer: Vec::new() }
    }
}

impl OutputWriter for StringOutputWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        self.buffer.extend_from_slice(bytes);
        Ok(())
    }
}

struct RefCellOutputWriter {
    writer: Rc<RefCell<StringOutputWriter>>,
}

impl OutputWriter for RefCellOutputWriter {
    fn write(&mut self, bytes: &[u8]) -> Result<(), VmError> {
        self.writer.borrow_mut().write(bytes)
    }
}

fn main() {
    let code = r#"<?php
try {
    echo "before";
    throw new Exception();
} finally {
    echo " finally";
}
"#;

    let context = Arc::new(EngineContext::new());
    let mut request_context = RequestContext::new(context);

    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(code.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();

    let emitter = Emitter::new(code.as_bytes(), &mut request_context.interner);
    let (chunk, _) = emitter.compile(program.statements);

    let output_writer = Rc::new(RefCell::new(StringOutputWriter::new()));
    let output_writer_clone = output_writer.clone();

    let mut vm = VM::new_with_context(request_context);
    vm.output_writer = Box::new(RefCellOutputWriter {
        writer: output_writer,
    });

    match vm.run(Rc::new(chunk)) {
        Ok(_) => {
            let output = output_writer_clone.borrow().buffer.clone();
            eprintln!("Success. Output: {}", String::from_utf8_lossy(&output));
        }
        Err(e) => {
            let output = output_writer_clone.borrow().buffer.clone();
            eprintln!("Error: {:?}", e);
            eprintln!("Output before error: {}", String::from_utf8_lossy(&output));
        }
    }
}
