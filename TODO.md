# WordPress on php-vm TODO List

- [x] 1. Reproduce the delta: Run native `php` and `php-vm` against WordPress `index.php` and capture the first fatal error.
- [x] 2. Fix CLI script context: Update `crates/php-vm/src/bin/php.rs` to `chdir` to the script directory and set correct `$argv`/`$argc`.
- [x] 3. Fix `$_SERVER` consistency: Update `crates/php-vm/src/superglobals.rs` (or relevant file) to populate `SCRIPT_FILENAME`, `SCRIPT_NAME`, `PHP_SELF`, `DOCUMENT_ROOT` based on the actual invoked script.
- [x] 4. Fix include/require relative path semantics: Update `resolve_include_path` in `crates/php-vm/src/include.rs` (or relevant file) to try the including file's directory.
- [x] 5. Add minimal missing runtime pieces:
    - [x] Implement `preg_match`, `preg_replace`, `preg_split`.
    - [x] Implement simple `ini_get`, `ini_set`.
    - [x] Trigger SPL autoloaders on "class not found".

