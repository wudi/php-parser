use php_vm::vm::engine::VM;
use php_vm::runtime::context::{EngineContext, RequestContext};
use php_vm::compiler::emitter::Emitter;
use std::sync::Arc;
use std::rc::Rc;
use std::fs;
use std::path::PathBuf;

fn compile_and_run(vm: &mut VM, code: &str) -> Result<(), php_vm::vm::engine::VmError> {
    let arena = bumpalo::Bump::new();
    let lexer = php_parser::lexer::Lexer::new(code.as_bytes());
    let mut parser = php_parser::parser::Parser::new(lexer, &arena);
    let program = parser.parse_program();
    
    if !program.errors.is_empty() {
        panic!("Parse errors: {:?}", program.errors);
    }
    
    let emitter = Emitter::new(code.as_bytes(), &mut vm.context.interner);
    let (chunk, _) = emitter.compile(program.statements);
    
    vm.run(Rc::new(chunk))
}

fn create_test_vm() -> VM {
    let engine = Arc::new(EngineContext::new());
    let request_context = RequestContext::new(engine);
    VM::new_with_context(request_context)
}

fn get_temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("php_vm_test_{}", name));
    path
}

fn cleanup_temp(path: &PathBuf) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_dir_all(path);
}

#[test]
fn test_file_get_contents() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("file_get_contents.txt");
    
    // Create test file
    fs::write(&temp_path, b"Hello, World!").unwrap();
    
    let code = format!(
        r#"<?php
        $content = file_get_contents("{}");
        echo $content;
        "#,
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_file_put_contents() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("file_put_contents.txt");
    
    let code = format!(
        r#"<?php
        $bytes = file_put_contents("{}", "Test data");
        echo $bytes;
        "#,
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    let contents = fs::read(&temp_path).unwrap();
    assert_eq!(contents, b"Test data");
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_file_put_contents_append() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("file_put_contents_append.txt");
    
    fs::write(&temp_path, b"First\n").unwrap();
    
    let code = format!(
        r#"<?php
        file_put_contents("{}", "Second
", FILE_APPEND);
        "#,
        temp_path.display()
    );
    
    // Define FILE_APPEND constant
    let define_code = "<?php define('FILE_APPEND', 8);";
    compile_and_run(&mut vm, define_code).unwrap();
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    let contents = fs::read(&temp_path).unwrap();
    assert_eq!(contents, b"First\nSecond\n");
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_file_exists() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("file_exists.txt");
    
    fs::write(&temp_path, b"test").unwrap();
    
    let code = format!(
        r#"<?php
        $exists = file_exists("{}");
        $not_exists = file_exists("/nonexistent/path/file.txt");
        if ($exists && !$not_exists) {{
            echo "OK";
        }}
        "#,
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_is_file_is_dir() {
    let mut vm = create_test_vm();
    let file_path = get_temp_path("test_file.txt");
    let dir_path = get_temp_path("test_dir");
    
    fs::write(&file_path, b"test").unwrap();
    fs::create_dir(&dir_path).unwrap();
    
    let code = format!(
        r#"<?php
        $file_is_file = is_file("{}");
        $file_is_dir = is_dir("{}");
        $dir_is_file = is_file("{}");
        $dir_is_dir = is_dir("{}");
        
        if ($file_is_file && !$file_is_dir && !$dir_is_file && $dir_is_dir) {{
            echo "OK";
        }}
        "#,
        file_path.display(),
        file_path.display(),
        dir_path.display(),
        dir_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    cleanup_temp(&file_path);
    cleanup_temp(&dir_path);
}

#[test]
fn test_filesize() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("filesize.txt");
    
    fs::write(&temp_path, b"12345").unwrap();
    
    let code = format!(
        r#"<?php
        $size = filesize("{}");
        echo $size;
        "#,
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_unlink() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("unlink.txt");
    
    fs::write(&temp_path, b"delete me").unwrap();
    
    let code = format!(
        r#"<?php
        $result = unlink("{}");
        $exists = file_exists("{}");
        if ($result && !$exists) {{
            echo "OK";
        }}
        "#,
        temp_path.display(),
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    assert!(!temp_path.exists());
}

#[test]
fn test_rename() {
    let mut vm = create_test_vm();
    let old_path = get_temp_path("rename_old.txt");
    let new_path = get_temp_path("rename_new.txt");
    
    fs::write(&old_path, b"test").unwrap();
    
    let code = format!(
        r#"<?php
        rename("{}", "{}");
        $old_exists = file_exists("{}");
        $new_exists = file_exists("{}");
        if (!$old_exists && $new_exists) {{
            echo "OK";
        }}
        "#,
        old_path.display(),
        new_path.display(),
        old_path.display(),
        new_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    cleanup_temp(&old_path);
    cleanup_temp(&new_path);
}

#[test]
fn test_mkdir_rmdir() {
    let mut vm = create_test_vm();
    let dir_path = get_temp_path("test_mkdir");
    
    let code = format!(
        r#"<?php
        mkdir("{}");
        $exists = is_dir("{}");
        rmdir("{}");
        $removed = !is_dir("{}");
        if ($exists && $removed) {{
            echo "OK";
        }}
        "#,
        dir_path.display(),
        dir_path.display(),
        dir_path.display(),
        dir_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    cleanup_temp(&dir_path);
}

#[test]
fn test_mkdir_recursive() {
    let mut vm = create_test_vm();
    let base = get_temp_path("mkdir_recursive");
    let nested = base.join("a").join("b").join("c");
    
    let code = format!(
        r#"<?php
        mkdir("{}", 0777, true);
        $exists = is_dir("{}");
        if ($exists) {{
            echo "OK";
        }}
        "#,
        nested.display(),
        nested.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    cleanup_temp(&base);
}

#[test]
fn test_scandir() {
    let mut vm = create_test_vm();
    let dir_path = get_temp_path("scandir_test");
    
    fs::create_dir(&dir_path).unwrap();
    fs::write(dir_path.join("file1.txt"), b"a").unwrap();
    fs::write(dir_path.join("file2.txt"), b"b").unwrap();
    fs::write(dir_path.join("file3.txt"), b"c").unwrap();
    
    let code = format!(
        r#"<?php
        $files = scandir("{}");
        echo count($files);
        "#,
        dir_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    cleanup_temp(&dir_path);
}

#[test]
fn test_basename_dirname() {
    let mut vm = create_test_vm();
    
    let code = r#"<?php
        $path = "/path/to/file.txt";
        $base = basename($path);
        $dir = dirname($path);
        echo $base . "," . $dir;
        "#;
    
    compile_and_run(&mut vm, code).unwrap();
    
}

#[test]
fn test_basename_with_suffix() {
    let mut vm = create_test_vm();
    
    let code = r#"<?php
        $path = "/path/to/file.php";
        $base = basename($path, ".php");
        echo $base;
        "#;
    
    compile_and_run(&mut vm, code).unwrap();
    
}

#[test]
fn test_copy() {
    let mut vm = create_test_vm();
    let src_path = get_temp_path("copy_src.txt");
    let dst_path = get_temp_path("copy_dst.txt");
    
    fs::write(&src_path, b"copy me").unwrap();
    
    let code = format!(
        r#"<?php
        copy("{}", "{}");
        $content = file_get_contents("{}");
        echo $content;
        "#,
        src_path.display(),
        dst_path.display(),
        dst_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    cleanup_temp(&src_path);
    cleanup_temp(&dst_path);
}

#[test]
fn test_file_reads_lines() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("file_lines.txt");
    
    fs::write(&temp_path, b"Line 1\nLine 2\nLine 3\n").unwrap();
    
    let code = format!(
        r#"<?php
        $lines = file("{}");
        echo count($lines);
        "#,
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_is_readable_writable() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("rw_test.txt");
    
    fs::write(&temp_path, b"test").unwrap();
    
    let code = format!(
        r#"<?php
        $readable = is_readable("{}");
        $writable = is_writable("{}");
        if ($readable && $writable) {{
            echo "OK";
        }}
        "#,
        temp_path.display(),
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_touch_creates_file() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("touch_test.txt");
    
    let code = format!(
        r#"<?php
        touch("{}");
        $exists = file_exists("{}");
        if ($exists) {{
            echo "OK";
        }}
        "#,
        temp_path.display(),
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_getcwd() {
    let mut vm = create_test_vm();
    
    let code = r#"<?php
        $cwd = getcwd();
        if (is_string($cwd) && strlen($cwd) > 0) {
            echo "OK";
        }
        "#;
    
    compile_and_run(&mut vm, code).unwrap();
    
}

#[test]
fn test_realpath() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("realpath_test.txt");
    
    fs::write(&temp_path, b"test").unwrap();
    
    let code = format!(
        r#"<?php
        $real = realpath("{}");
        if (is_string($real) && strlen($real) > 0) {{
            echo "OK";
        }}
        "#,
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_file_get_contents_missing_file() {
    let mut vm = create_test_vm();
    
    let code = r#"<?php
        $result = file_get_contents("/nonexistent/file.txt");
        "#;
    
    let result = compile_and_run(&mut vm, code);
    
    // Should fail with error
    assert!(result.is_err());
}

#[test]
fn test_filesize_missing_file() {
    let mut vm = create_test_vm();
    
    let code = r#"<?php
        $size = filesize("/nonexistent/file.txt");
        "#;
    
    let result = compile_and_run(&mut vm, code);
    
    // Should fail with error
    assert!(result.is_err());
}

#[test]
fn test_fread_fwrite() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("fread_fwrite_test.txt");
    
    let code = format!(
        r#"<?php
        $fp = fopen("{}", "w");
        fwrite($fp, "Hello from fwrite!");
        fclose($fp);
        
        $fp2 = fopen("{}", "r");
        $content = fread($fp2, 100);
        fclose($fp2);
        
        echo $content;
        "#,
        temp_path.display(),
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    // Verify file contents
    let contents = fs::read(&temp_path).unwrap();
    assert_eq!(contents, b"Hello from fwrite!");
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_fseek_ftell_rewind() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("fseek_test.txt");
    
    fs::write(&temp_path, b"0123456789").unwrap();
    
    let code = format!(
        r#"<?php
        $fp = fopen("{}", "r");
        
        // Seek to position 5
        fseek($fp, 5);
        $pos = ftell($fp);
        if ($pos != 5) {{
            echo "ERROR: ftell after fseek";
        }}
        
        // Read one byte
        $char = fgetc($fp);
        
        // Rewind to start
        rewind($fp);
        $pos = ftell($fp);
        if ($pos != 0) {{
            echo "ERROR: ftell after rewind";
        }}
        
        fclose($fp);
        "#,
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_fgets() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("fgets_test.txt");
    
    fs::write(&temp_path, b"Line 1\nLine 2\nLine 3\n").unwrap();
    
    let code = format!(
        r#"<?php
        $fp = fopen("{}", "r");
        
        $line1 = fgets($fp);
        $line2 = fgets($fp);
        
        echo $line1;
        echo $line2;
        
        fclose($fp);
        "#,
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_feof() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("feof_test.txt");
    
    fs::write(&temp_path, b"ab").unwrap();
    
    let code = format!(
        r#"<?php
        $fp = fopen("{}", "r");
        
        fgetc($fp);  // Read 'a'
        $eof1 = feof($fp);
        if ($eof1) {{
            echo "ERROR: EOF after 1 char";
        }}
        
        fgetc($fp);  // Read 'b'
        $eof2 = feof($fp);
        if ($eof2) {{
            echo "ERROR: EOF after 2 chars";
        }}
        
        fgetc($fp);  // Try to read past end
        $eof3 = feof($fp);
        if (!$eof3) {{
            echo "ERROR: Not EOF after reading past end";
        }}
        
        fclose($fp);
        "#,
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_stat() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("stat_test.txt");
    
    fs::write(&temp_path, b"test content").unwrap();
    
    let code = format!(
        r#"<?php
        $stats = stat("{}");
        
        if (is_array($stats)) {{
            // Check both numeric and string keys
            $size1 = $stats[7];
            $size2 = $stats['size'];
            
            if ($size1 != 12 || $size2 != 12) {{
                echo "ERROR: Size mismatch";
            }}
        }} else {{
            echo "ERROR: stat did not return array";
        }}
        "#,
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_filemtime() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("mtime_test.txt");
    
    fs::write(&temp_path, b"test").unwrap();
    
    let code = format!(
        r#"<?php
        $mtime = filemtime("{}");
        
        if (!is_int($mtime) || $mtime <= 0) {{
            echo "ERROR: Invalid mtime";
        }}
        "#,
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_fileperms() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("perms_test.txt");
    
    fs::write(&temp_path, b"test").unwrap();
    
    let code = format!(
        r#"<?php
        $perms = fileperms("{}");
        
        if (!is_int($perms)) {{
            echo "ERROR: fileperms did not return integer";
        }}
        "#,
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    cleanup_temp(&temp_path);
}

#[test]
fn test_tempnam() {
    let mut vm = create_test_vm();
    
    // Use a direct path instead of sys_get_temp_dir()
    let temp_dir = std::env::temp_dir();
    
    let code = format!(
        r#"<?php
        $temp = tempnam("{}", "php_vm_test_");
        
        if (!is_string($temp) || strlen($temp) == 0) {{
            echo "ERROR: tempnam failed";
        }} else {{
            unlink($temp);
        }}
        "#,
        temp_dir.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
}

#[test]
fn test_fputs_alias() {
    let mut vm = create_test_vm();
    let temp_path = get_temp_path("fputs_test.txt");
    
    let code = format!(
        r#"<?php
        $fp = fopen("{}", "w");
        fputs($fp, "Test fputs");
        fclose($fp);
        "#,
        temp_path.display()
    );
    
    compile_and_run(&mut vm, &code).unwrap();
    
    // Verify file contents
    let contents = fs::read(&temp_path).unwrap();
    assert_eq!(contents, b"Test fputs");
    
    cleanup_temp(&temp_path);
}

