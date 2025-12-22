use php_vm::core::value::Val;
use php_vm::runtime::context::EngineContext;
use php_vm::vm::engine::VM;
use std::rc::Rc;
use std::sync::Arc;

fn create_test_vm() -> VM {
    let engine = Arc::new(EngineContext::new());
    VM::new(engine)
}

#[test]
fn test_gzcompress_gzuncompress() {
    let mut vm = create_test_vm();
    let data = b"Hello world! Hello world! Hello world!";
    let data_handle = vm.arena.alloc(Val::String(Rc::new(data.to_vec())));

    let compressed_handle =
        php_vm::builtins::zlib::php_gzcompress(&mut vm, &[data_handle]).unwrap();
    let compressed = match &vm.arena.get(compressed_handle).value {
        Val::String(s) => s.clone(),
        _ => panic!("gzcompress did not return a string"),
    };

    assert!(compressed.len() < data.len());

    let decompressed_handle =
        php_vm::builtins::zlib::php_gzuncompress(&mut vm, &[compressed_handle]).unwrap();
    let decompressed = match &vm.arena.get(decompressed_handle).value {
        Val::String(s) => s.clone(),
        _ => panic!("gzuncompress did not return a string"),
    };

    assert_eq!(decompressed.as_ref(), data);
}

#[test]
fn test_gzdeflate_gzinflate() {
    let mut vm = create_test_vm();
    let data = b"Hello world! Hello world! Hello world!";
    let data_handle = vm.arena.alloc(Val::String(Rc::new(data.to_vec())));

    let compressed_handle = php_vm::builtins::zlib::php_gzdeflate(&mut vm, &[data_handle]).unwrap();
    let compressed = match &vm.arena.get(compressed_handle).value {
        Val::String(s) => s.clone(),
        _ => panic!("gzdeflate did not return a string"),
    };

    assert!(compressed.len() < data.len());

    let decompressed_handle =
        php_vm::builtins::zlib::php_gzinflate(&mut vm, &[compressed_handle]).unwrap();
    let decompressed = match &vm.arena.get(decompressed_handle).value {
        Val::String(s) => s.clone(),
        _ => panic!("gzinflate did not return a string"),
    };

    assert_eq!(decompressed.as_ref(), data);
}

#[test]
fn test_gzencode_gzdecode() {
    let mut vm = create_test_vm();
    let data = b"Hello world! Hello world! Hello world!";
    let data_handle = vm.arena.alloc(Val::String(Rc::new(data.to_vec())));

    let compressed_handle = php_vm::builtins::zlib::php_gzencode(&mut vm, &[data_handle]).unwrap();
    let compressed = match &vm.arena.get(compressed_handle).value {
        Val::String(s) => s.clone(),
        _ => panic!("gzencode did not return a string"),
    };

    assert!(compressed.len() < data.len());

    let decompressed_handle =
        php_vm::builtins::zlib::php_gzdecode(&mut vm, &[compressed_handle]).unwrap();
    let decompressed = match &vm.arena.get(decompressed_handle).value {
        Val::String(s) => s.clone(),
        _ => panic!("gzdecode did not return a string"),
    };

    assert_eq!(decompressed.as_ref(), data);
}

#[test]
fn test_zlib_encode_decode() {
    let mut vm = create_test_vm();
    let data = b"Hello world! Hello world! Hello world!";
    let data_handle = vm.arena.alloc(Val::String(Rc::new(data.to_vec())));
    let encoding_handle = vm.arena.alloc(Val::Int(15)); // ZLIB_ENCODING_DEFLATE

    let compressed_handle =
        php_vm::builtins::zlib::php_zlib_encode(&mut vm, &[data_handle, encoding_handle]).unwrap();
    assert!(matches!(
        &vm.arena.get(compressed_handle).value,
        Val::String(_)
    ));

    let decompressed_handle =
        php_vm::builtins::zlib::php_zlib_decode(&mut vm, &[compressed_handle]).unwrap();
    let decompressed = match &vm.arena.get(decompressed_handle).value {
        Val::String(s) => s.clone(),
        _ => panic!("zlib_decode did not return a string"),
    };

    assert_eq!(decompressed.as_ref(), data);
}

#[test]
fn test_incremental_deflate_inflate() {
    let mut vm = create_test_vm();
    let data1 = b"Hello ";
    let data2 = b"world!";

    let encoding_handle = vm.arena.alloc(Val::Int(15)); // ZLIB_ENCODING_DEFLATE
    let ctx_handle = php_vm::builtins::zlib::php_deflate_init(&mut vm, &[encoding_handle]).unwrap();

    let data1_handle = vm.arena.alloc(Val::String(Rc::new(data1.to_vec())));
    let flush_none = vm.arena.alloc(Val::Int(0));
    let part1_handle =
        php_vm::builtins::zlib::php_deflate_add(&mut vm, &[ctx_handle, data1_handle, flush_none])
            .unwrap();

    let data2_handle = vm.arena.alloc(Val::String(Rc::new(data2.to_vec())));
    let flush_finish = vm.arena.alloc(Val::Int(4));
    let part2_handle =
        php_vm::builtins::zlib::php_deflate_add(&mut vm, &[ctx_handle, data2_handle, flush_finish])
            .unwrap(); // ZLIB_FINISH

    let mut compressed = match &vm.arena.get(part1_handle).value {
        Val::String(s) => s.as_ref().clone(),
        _ => panic!("deflate_add part 1 failed"),
    };
    let part2 = match &vm.arena.get(part2_handle).value {
        Val::String(s) => s.as_ref().clone(),
        _ => panic!("deflate_add part 2 failed"),
    };
    compressed.extend_from_slice(&part2);

    // Inflate
    let ictx_handle =
        php_vm::builtins::zlib::php_inflate_init(&mut vm, &[encoding_handle]).unwrap();
    let compressed_handle = vm.arena.alloc(Val::String(Rc::new(compressed)));
    let flush_finish_inflate = vm.arena.alloc(Val::Int(4));
    let decompressed_handle = php_vm::builtins::zlib::php_inflate_add(
        &mut vm,
        &[ictx_handle, compressed_handle, flush_finish_inflate],
    )
    .unwrap();

    let decompressed = match &vm.arena.get(decompressed_handle).value {
        Val::String(s) => s.as_ref().clone(),
        _ => panic!("inflate_add failed"),
    };

    assert_eq!(decompressed, b"Hello world!");
}

#[test]
fn test_zlib_file_ops() {
    let mut vm = create_test_vm();
    let filename = "test.gz";
    let data = b"Hello, Zlib file operations!";

    // gzopen for writing
    let filename_handle = vm
        .arena
        .alloc(Val::String(Rc::new(filename.as_bytes().to_vec())));
    let mode_w_handle = vm.arena.alloc(Val::String(Rc::new(b"wb".to_vec())));
    let gz_w_handle =
        php_vm::builtins::zlib::php_gzopen(&mut vm, &[filename_handle, mode_w_handle]).unwrap();
    assert!(matches!(vm.arena.get(gz_w_handle).value, Val::Resource(_)));

    // gzwrite
    let data_handle = vm.arena.alloc(Val::String(Rc::new(data.to_vec())));
    let written_handle =
        php_vm::builtins::zlib::php_gzwrite(&mut vm, &[gz_w_handle, data_handle]).unwrap();
    if let Val::Int(n) = vm.arena.get(written_handle).value {
        assert_eq!(n as usize, data.len());
    } else {
        panic!("gzwrite() should return int");
    }

    // gzclose
    php_vm::builtins::zlib::php_gzclose(&mut vm, &[gz_w_handle]).unwrap();

    // gzopen for reading
    let mode_r_handle = vm.arena.alloc(Val::String(Rc::new(b"rb".to_vec())));
    let gz_r_handle =
        php_vm::builtins::zlib::php_gzopen(&mut vm, &[filename_handle, mode_r_handle]).unwrap();
    assert!(matches!(vm.arena.get(gz_r_handle).value, Val::Resource(_)));

    // gzread
    let len_handle = vm.arena.alloc(Val::Int(100));
    let read_handle =
        php_vm::builtins::zlib::php_gzread(&mut vm, &[gz_r_handle, len_handle]).unwrap();
    if let Val::String(s) = &vm.arena.get(read_handle).value {
        assert_eq!(s.as_ref(), data);
    } else {
        panic!("gzread() should return string");
    }

    // gzrewind
    let rewind_handle = php_vm::builtins::zlib::php_gzrewind(&mut vm, &[gz_r_handle]).unwrap();
    assert_eq!(vm.arena.get(rewind_handle).value, Val::Bool(true));

    // gztell
    let tell_handle = php_vm::builtins::zlib::php_gztell(&mut vm, &[gz_r_handle]).unwrap();
    assert_eq!(vm.arena.get(tell_handle).value, Val::Int(0));

    // gzread again
    let read_handle2 =
        php_vm::builtins::zlib::php_gzread(&mut vm, &[gz_r_handle, len_handle]).unwrap();
    if let Val::String(s) = &vm.arena.get(read_handle2).value {
        assert_eq!(s.as_ref(), data);
    } else {
        panic!("gzread() should return string");
    }

    // gzclose
    php_vm::builtins::zlib::php_gzclose(&mut vm, &[gz_r_handle]).unwrap();

    // Cleanup
    let _ = std::fs::remove_file(filename);
}

#[test]
fn test_zlib_max_length() {
    let mut vm = create_test_vm();
    let data = b"Hello world! Hello world! Hello world!";
    let data_handle = vm.arena.alloc(Val::String(Rc::new(data.to_vec())));

    // Compress
    let compressed_handle = php_vm::builtins::zlib::php_gzdeflate(&mut vm, &[data_handle]).unwrap();

    // Decompress with max_length
    let max_len = 11; // "Hello world"
    let max_len_handle = vm.arena.alloc(Val::Int(max_len as i64));
    let decompressed_handle =
        php_vm::builtins::zlib::php_gzinflate(&mut vm, &[compressed_handle, max_len_handle])
            .unwrap();

    if let Val::String(s) = &vm.arena.get(decompressed_handle).value {
        assert_eq!(s.len(), max_len);
        assert_eq!(s.as_ref(), b"Hello world");
    } else {
        panic!("gzinflate did not return a string");
    }
}

#[test]
fn test_gzgetc_gzpassthru() {
    let mut vm = create_test_vm();
    let filename = "test_getc.gz";
    let data = b"ABC";

    // Write data
    let filename_handle = vm
        .arena
        .alloc(Val::String(Rc::new(filename.as_bytes().to_vec())));
    let mode_w_handle = vm.arena.alloc(Val::String(Rc::new(b"wb".to_vec())));
    let gz_w_handle =
        php_vm::builtins::zlib::php_gzopen(&mut vm, &[filename_handle, mode_w_handle]).unwrap();
    let data_handle = vm.arena.alloc(Val::String(Rc::new(data.to_vec())));
    php_vm::builtins::zlib::php_gzwrite(&mut vm, &[gz_w_handle, data_handle]).unwrap();
    php_vm::builtins::zlib::php_gzclose(&mut vm, &[gz_w_handle]).unwrap();

    // Test gzgetc
    let mode_r_handle = vm.arena.alloc(Val::String(Rc::new(b"rb".to_vec())));
    let gz_r_handle =
        php_vm::builtins::zlib::php_gzopen(&mut vm, &[filename_handle, mode_r_handle]).unwrap();

    let c1_handle = php_vm::builtins::zlib::php_gzgetc(&mut vm, &[gz_r_handle]).unwrap();
    if let Val::String(s) = &vm.arena.get(c1_handle).value {
        assert_eq!(s.as_ref(), b"A");
    }

    // Test gzpassthru (remaining data: "BC")
    let passthru_handle = php_vm::builtins::zlib::php_gzpassthru(&mut vm, &[gz_r_handle]).unwrap();
    if let Val::Int(n) = vm.arena.get(passthru_handle).value {
        assert_eq!(n, 2);
    }

    php_vm::builtins::zlib::php_gzclose(&mut vm, &[gz_r_handle]).unwrap();

    // Cleanup
    let _ = std::fs::remove_file(filename);
}

#[test]
fn test_gzgets_gzfile() {
    let mut vm = create_test_vm();
    let filename = "test_lines.gz";
    let data = b"Line 1\nLine 2\nLine 3";

    // Write data
    let filename_handle = vm
        .arena
        .alloc(Val::String(Rc::new(filename.as_bytes().to_vec())));
    let mode_w_handle = vm.arena.alloc(Val::String(Rc::new(b"wb".to_vec())));
    let gz_w_handle =
        php_vm::builtins::zlib::php_gzopen(&mut vm, &[filename_handle, mode_w_handle]).unwrap();
    let data_handle = vm.arena.alloc(Val::String(Rc::new(data.to_vec())));
    php_vm::builtins::zlib::php_gzwrite(&mut vm, &[gz_w_handle, data_handle]).unwrap();
    php_vm::builtins::zlib::php_gzclose(&mut vm, &[gz_w_handle]).unwrap();

    // Test gzgets
    let mode_r_handle = vm.arena.alloc(Val::String(Rc::new(b"rb".to_vec())));
    let gz_r_handle =
        php_vm::builtins::zlib::php_gzopen(&mut vm, &[filename_handle, mode_r_handle]).unwrap();

    let line1_handle = php_vm::builtins::zlib::php_gzgets(&mut vm, &[gz_r_handle]).unwrap();
    if let Val::String(s) = &vm.arena.get(line1_handle).value {
        assert_eq!(s.as_ref(), b"Line 1\n");
    } else {
        panic!("gzgets() should return string");
    }

    php_vm::builtins::zlib::php_gzclose(&mut vm, &[gz_r_handle]).unwrap();

    // Test gzfile
    let lines_handle = php_vm::builtins::zlib::php_gzfile(&mut vm, &[filename_handle]).unwrap();
    if let Val::Array(arr) = &vm.arena.get(lines_handle).value {
        assert_eq!(arr.map.len(), 3);
        // Check first line
        let l1_handle = *arr.map.get(&php_vm::core::value::ArrayKey::Int(0)).unwrap();
        if let Val::String(s) = &vm.arena.get(l1_handle).value {
            assert_eq!(s.as_ref(), b"Line 1\n");
        }
    } else {
        panic!("gzfile() should return array");
    }

    // Cleanup
    let _ = std::fs::remove_file(filename);
}
