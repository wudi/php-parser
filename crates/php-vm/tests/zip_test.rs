use std::rc::Rc;
use indexmap::IndexMap;
use php_vm::compiler::chunk::CodeChunk;
use php_vm::core::value::{ObjectData, Val};
use php_vm::runtime::context::EngineBuilder;
use php_vm::vm::engine::VM;
use php_vm::vm::frame::CallFrame;
use std::collections::HashSet;
use std::fs;

fn create_test_vm() -> VM {
    let engine = EngineBuilder::new().with_core_extensions().build().expect("Failed to build engine");
    VM::new(engine)
}

#[test]
fn test_zip_archive_basic() {
    let mut vm = create_test_vm();
    let temp_dir = tempfile::tempdir().unwrap();
    let zip_path = temp_dir.path().join("test.zip");
    let zip_path_str = zip_path.to_str().unwrap();

    // Create ZipArchive object
    let zip_class_name = vm.context.interner.intern(b"ZipArchive");
    let obj_data = ObjectData {
        class: zip_class_name,
        properties: IndexMap::new(),
        internal: None,
        dynamic_properties: HashSet::new(),
    };
    let obj_handle = vm.arena.alloc(Val::ObjPayload(obj_data));
    let zip_handle = vm.arena.alloc(Val::Object(obj_handle));

    // Setup call frame with 'this'
    let chunk = Rc::new(CodeChunk::default());
    let mut frame = CallFrame::new(chunk);
    frame.this = Some(zip_handle);
    vm.frames.push(frame);

    // $zip->open("test.zip", ZipArchive::CREATE)
    let path_val = vm
        .arena
        .alloc(Val::String(Rc::new(zip_path_str.as_bytes().to_vec())));
    let create_flag = vm.arena.alloc(Val::Int(1));
    let result =
        php_vm::builtins::zip::php_zip_archive_open(&mut vm, &[path_val, create_flag]).unwrap();
    assert_eq!(vm.arena.get(result).value, Val::Bool(true));

    // $zip->addFromString("test.txt", "hello zip")
    let name_val = vm.arena.alloc(Val::String(Rc::new(b"test.txt".to_vec())));
    let content_val = vm.arena.alloc(Val::String(Rc::new(b"hello zip".to_vec())));
    let result =
        php_vm::builtins::zip::php_zip_archive_add_from_string(&mut vm, &[name_val, content_val])
            .unwrap();
    assert_eq!(vm.arena.get(result).value, Val::Bool(true));

    // $zip->close()
    let result = php_vm::builtins::zip::php_zip_archive_close(&mut vm, &[]).unwrap();
    assert_eq!(vm.arena.get(result).value, Val::Bool(true));

    vm.frames.pop();

    // Verify file exists and has content
    assert!(zip_path.exists());
    let file = fs::File::open(&zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    assert_eq!(archive.len(), 1);
    let mut entry = archive.by_name("test.txt").unwrap();
    let mut content = String::new();
    use std::io::Read;
    entry.read_to_string(&mut content).unwrap();
    assert_eq!(content, "hello zip");
}

#[test]
fn test_zip_procedural_basic() {
    let mut vm = create_test_vm();
    let temp_dir = tempfile::tempdir().unwrap();
    let zip_path = temp_dir.path().join("test_proc.zip");
    let zip_path_str = zip_path.to_str().unwrap();

    // Create a zip file first (using ZipArchive for convenience)
    {
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("test.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        use std::io::Write;
        zip.write_all(b"procedural hello").unwrap();
        zip.finish().unwrap();
    }

    // $zip = zip_open("test_proc.zip")
    let path_val = vm
        .arena
        .alloc(Val::String(Rc::new(zip_path_str.as_bytes().to_vec())));
    let zip_res = php_vm::builtins::zip::php_zip_open(&mut vm, &[path_val]).unwrap();

    assert!(matches!(vm.arena.get(zip_res).value, Val::Resource(_)));

    // $entry = zip_read($zip)
    let entry_res = php_vm::builtins::zip::php_zip_read(&mut vm, &[zip_res]).unwrap();
    assert!(matches!(vm.arena.get(entry_res).value, Val::Resource(_)));

    // zip_entry_name($entry)
    let name = php_vm::builtins::zip::php_zip_entry_name(&mut vm, &[entry_res]).unwrap();
    match &vm.arena.get(name).value {
        Val::String(s) => assert_eq!(s.as_slice(), b"test.txt"),
        _ => panic!("Expected string, got {:?}", vm.arena.get(name).value),
    }

    // zip_entry_read($entry)
    let content = php_vm::builtins::zip::php_zip_entry_read(&mut vm, &[entry_res]).unwrap();
    match &vm.arena.get(content).value {
        Val::String(s) => assert_eq!(s.as_slice(), b"procedural hello"),
        _ => panic!("Expected string, got {:?}", vm.arena.get(content).value),
    }

    // zip_close($zip)
    php_vm::builtins::zip::php_zip_close(&mut vm, &[zip_res]).unwrap();
}

#[test]
fn test_zip_archive_add_file() {
    let mut vm = create_test_vm();
    let temp_dir = tempfile::tempdir().unwrap();
    let zip_path = temp_dir.path().join("test_add_file.zip");
    let zip_path_str = zip_path.to_str().unwrap();

    let to_add = temp_dir.path().join("to_add.txt");
    let to_add_str = to_add.to_str().unwrap();
    fs::write(&to_add, "file content").unwrap();

    let zip_class_name = vm.context.interner.intern(b"ZipArchive");
    let obj_data = ObjectData {
        class: zip_class_name,
        properties: IndexMap::new(),
        internal: None,
        dynamic_properties: HashSet::new(),
    };
    let obj_handle = vm.arena.alloc(Val::ObjPayload(obj_data));
    let zip_handle = vm.arena.alloc(Val::Object(obj_handle));

    let chunk = Rc::new(CodeChunk::default());
    let mut frame = CallFrame::new(chunk);
    frame.this = Some(zip_handle);
    vm.frames.push(frame);

    let path_val = vm
        .arena
        .alloc(Val::String(Rc::new(zip_path_str.as_bytes().to_vec())));
    let create_flag = vm.arena.alloc(Val::Int(1));
    php_vm::builtins::zip::php_zip_archive_open(&mut vm, &[path_val, create_flag]).unwrap();

    let file_val = vm
        .arena
        .alloc(Val::String(Rc::new(to_add_str.as_bytes().to_vec())));
    let local_val = vm
        .arena
        .alloc(Val::String(Rc::new(b"renamed.txt".to_vec())));
    let result =
        php_vm::builtins::zip::php_zip_archive_add_file(&mut vm, &[file_val, local_val]).unwrap();
    assert_eq!(vm.arena.get(result).value, Val::Bool(true));

    php_vm::builtins::zip::php_zip_archive_close(&mut vm, &[]).unwrap();
    vm.frames.pop();

    // Verify
    let file = fs::File::open(&zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    assert_eq!(archive.len(), 1);
    assert_eq!(archive.by_index(0).unwrap().name(), "renamed.txt");
}

#[test]
fn test_zip_archive_add_empty_dir() {
    let mut vm = create_test_vm();
    let temp_dir = tempfile::tempdir().unwrap();
    let zip_path = temp_dir.path().join("test_add_dir.zip");
    let zip_path_str = zip_path.to_str().unwrap();

    let zip_class_name = vm.context.interner.intern(b"ZipArchive");
    let obj_data = ObjectData {
        class: zip_class_name,
        properties: IndexMap::new(),
        internal: None,
        dynamic_properties: HashSet::new(),
    };
    let obj_handle = vm.arena.alloc(Val::ObjPayload(obj_data));
    let zip_handle = vm.arena.alloc(Val::Object(obj_handle));

    let chunk = Rc::new(CodeChunk::default());
    let mut frame = CallFrame::new(chunk);
    frame.this = Some(zip_handle);
    vm.frames.push(frame);

    let path_val = vm
        .arena
        .alloc(Val::String(Rc::new(zip_path_str.as_bytes().to_vec())));
    let create_flag = vm.arena.alloc(Val::Int(1));
    php_vm::builtins::zip::php_zip_archive_open(&mut vm, &[path_val, create_flag]).unwrap();

    let dir_val = vm.arena.alloc(Val::String(Rc::new(b"empty_dir".to_vec())));
    let result = php_vm::builtins::zip::php_zip_archive_add_empty_dir(&mut vm, &[dir_val]).unwrap();
    assert_eq!(vm.arena.get(result).value, Val::Bool(true));

    php_vm::builtins::zip::php_zip_archive_close(&mut vm, &[]).unwrap();
    vm.frames.pop();

    // Verify
    let file = fs::File::open(&zip_path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    assert_eq!(archive.len(), 1);
    assert!(archive.by_index(0).unwrap().is_dir());
    assert_eq!(archive.by_index(0).unwrap().name(), "empty_dir/");
}

#[test]
fn test_zip_archive_properties() {
    let mut vm = create_test_vm();
    let temp_dir = tempfile::tempdir().unwrap();
    let zip_path = temp_dir.path().join("test_props.zip");
    let zip_path_str = zip_path.to_str().unwrap();

    let zip_class_name = vm.context.interner.intern(b"ZipArchive");
    let obj_data = ObjectData {
        class: zip_class_name,
        properties: IndexMap::new(),
        internal: None,
        dynamic_properties: HashSet::new(),
    };
    let obj_handle = vm.arena.alloc(Val::ObjPayload(obj_data));
    let zip_handle = vm.arena.alloc(Val::Object(obj_handle));

    let chunk = Rc::new(CodeChunk::default());
    let mut frame = CallFrame::new(chunk);
    frame.this = Some(zip_handle);
    vm.frames.push(frame);

    // Open
    let path_val = vm
        .arena
        .alloc(Val::String(Rc::new(zip_path_str.as_bytes().to_vec())));
    let create_flag = vm.arena.alloc(Val::Int(1));
    php_vm::builtins::zip::php_zip_archive_open(&mut vm, &[path_val, create_flag]).unwrap();

    // Check initial properties
    {
        let obj_val = vm.arena.get(obj_handle);
        if let Val::ObjPayload(obj_data) = &obj_val.value {
            let num_files_sym = vm.context.interner.intern(b"numFiles");
            let num_files_handle = obj_data
                .properties
                .get(&num_files_sym)
                .expect("numFiles property missing");
            match &vm.arena.get(*num_files_handle).value {
                Val::Int(n) => assert_eq!(*n, 0),
                _ => panic!("numFiles should be int"),
            }
        }
    }

    // Add a file
    let name_handle = vm.arena.alloc(Val::String(Rc::new(b"test.txt".to_vec())));
    let content_handle = vm.arena.alloc(Val::String(Rc::new(b"hello".to_vec())));
    php_vm::builtins::zip::php_zip_archive_add_from_string(&mut vm, &[name_handle, content_handle])
        .unwrap();

    // Check properties after add
    {
        let obj_val = vm.arena.get(obj_handle);
        if let Val::ObjPayload(obj_data) = &obj_val.value {
            let num_files_sym = vm.context.interner.intern(b"numFiles");
            let num_files_handle = obj_data
                .properties
                .get(&num_files_sym)
                .expect("numFiles property missing");
            match &vm.arena.get(*num_files_handle).value {
                Val::Int(n) => assert_eq!(*n, 1),
                _ => panic!("numFiles should be int"),
            }
        }
    }

    // Delete
    let index_handle = vm.arena.alloc(Val::Int(0));
    php_vm::builtins::zip::php_zip_archive_delete_index(&mut vm, &[index_handle]).unwrap();

    // Check properties after delete
    {
        let obj_val = vm.arena.get(obj_handle);
        if let Val::ObjPayload(obj_data) = &obj_val.value {
            let num_files_sym = vm.context.interner.intern(b"numFiles");
            let num_files_handle = obj_data
                .properties
                .get(&num_files_sym)
                .expect("numFiles property missing");
            match &vm.arena.get(*num_files_handle).value {
                Val::Int(n) => assert_eq!(*n, 0),
                _ => panic!("numFiles should be int"),
            }
        }
    }

    php_vm::builtins::zip::php_zip_archive_close(&mut vm, &[]).unwrap();
    vm.frames.pop();
}

#[test]
fn test_zip_archive_extract_to() {
    let mut vm = create_test_vm();
    let temp_dir = tempfile::tempdir().unwrap();
    let zip_path = temp_dir.path().join("test_extract.zip");
    let zip_path_str = zip_path.to_str().unwrap();

    // Create a zip file with some content
    {
        let file = fs::File::create(&zip_path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("file1.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        use std::io::Write;
        zip.write_all(b"content1").unwrap();
        zip.start_file("dir/file2.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"content2").unwrap();
        zip.finish().unwrap();
    }

    let zip_class_name = vm.context.interner.intern(b"ZipArchive");
    let obj_data = ObjectData {
        class: zip_class_name,
        properties: IndexMap::new(),
        internal: None,
        dynamic_properties: HashSet::new(),
    };
    let obj_handle = vm.arena.alloc(Val::ObjPayload(obj_data));
    let zip_handle = vm.arena.alloc(Val::Object(obj_handle));

    let chunk = Rc::new(CodeChunk::default());
    let mut frame = CallFrame::new(chunk);
    frame.this = Some(zip_handle);
    vm.frames.push(frame);

    let path_val = vm
        .arena
        .alloc(Val::String(Rc::new(zip_path_str.as_bytes().to_vec())));
    php_vm::builtins::zip::php_zip_archive_open(&mut vm, &[path_val]).unwrap();

    // getNameIndex(0)
    let idx0 = vm.arena.alloc(Val::Int(0));
    let name0 = php_vm::builtins::zip::php_zip_archive_get_name_index(&mut vm, &[idx0]).unwrap();
    match &vm.arena.get(name0).value {
        Val::String(s) => assert_eq!(s.as_slice(), b"file1.txt"),
        _ => panic!("Expected string"),
    }

    // extractTo
    let extract_path = temp_dir.path().join("extracted");
    let extract_path_str = extract_path.to_str().unwrap();
    let dest_val = vm
        .arena
        .alloc(Val::String(Rc::new(extract_path_str.as_bytes().to_vec())));
    let result = php_vm::builtins::zip::php_zip_archive_extract_to(&mut vm, &[dest_val]).unwrap();
    assert_eq!(vm.arena.get(result).value, Val::Bool(true));

    // Verify extracted files
    assert_eq!(
        fs::read_to_string(extract_path.join("file1.txt")).unwrap(),
        "content1"
    );
    assert_eq!(
        fs::read_to_string(extract_path.join("dir/file2.txt")).unwrap(),
        "content2"
    );

    php_vm::builtins::zip::php_zip_archive_close(&mut vm, &[]).unwrap();
    vm.frames.pop();
}
