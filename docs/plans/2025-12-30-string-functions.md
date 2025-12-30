# String Functions Parity Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement the missing core string functions from the provided list and register aliases to reach PHP parity.

**Architecture:** Add missing string builtins in `crates/php-vm/src/builtins/string.rs`, register them in `crates/php-vm/src/runtime/core_extension.rs`, and add parity tests in `crates/php-vm/tests/string_functions.rs` (and a new targeted test file when parsing/locale behavior warrants it). Focus on byte-oriented behavior (PHP strings are byte arrays) and match PHP warnings/return values.

**Tech Stack:** Rust 2024, `php-vm` builtins, `indexmap`, `hex`, existing VM helpers (`value_to_string`, `check_builtin_param_*`).

---

### Task 1: Alias registrations (chop, join, strchr)

**Files:**
- Modify: `crates/php-vm/src/runtime/core_extension.rs`
- Test: `crates/php-vm/tests/string_functions.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_chop_join_strchr_aliases() {
    let result = run_php(r#"<?php
        echo chop("hi\n");
        echo "|";
        echo join(",", ["a", "b"]);
        echo "|";
        echo strchr("hello", "l");
    "#);
    assert_eq!(result, "hi|a,b|llo");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p php-vm test_chop_join_strchr_aliases -v`
Expected: FAIL with “Call to undefined function chop/join/strchr”

**Step 3: Write minimal implementation**

```rust
// crates/php-vm/src/runtime/core_extension.rs
registry.register_function(b"chop", string::php_rtrim);
registry.register_function(b"join", string::php_implode);
registry.register_function(b"strchr", string::php_strstr);
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p php-vm test_chop_join_strchr_aliases -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/php-vm/src/runtime/core_extension.rs crates/php-vm/tests/string_functions.rs
git commit -m "feat: register string alias functions"
```

---

### Task 2: Case-insensitive and reverse position functions

**Files:**
- Modify: `crates/php-vm/src/builtins/string.rs`
- Modify: `crates/php-vm/src/runtime/core_extension.rs`
- Test: `crates/php-vm/tests/string_functions.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_stripos_strrpos_strripos_strrchr() {
    let result = run_php(r#"<?php
        echo stripos("Hello", "e");
        echo "|";
        var_export(strrpos("abcabc", "b"));
        echo "|";
        var_export(strripos("aBcAbC", "C"));
        echo "|";
        var_export(strrchr("abcabc", "b"));
    "#);
    assert_eq!(result, "1|4|5|bc");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p php-vm test_stripos_strrpos_strripos_strrchr -v`
Expected: FAIL with “Call to undefined function stripos/strrpos/strripos/strrchr”

**Step 3: Write minimal implementation**

```rust
pub fn php_stripos(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("stripos() expects 2 or 3 parameters".into());
    }
    let haystack = vm.value_to_string(args[0])?;
    let needle = vm.value_to_string(args[1])?;
    let offset = if args.len() == 3 { vm.arena.get(args[2]).value.to_int() } else { 0 };
    if offset < 0 || offset as usize > haystack.len() {
        return Ok(vm.arena.alloc(Val::Bool(false)));
    }
    let hay = haystack[offset as usize..]
        .iter().map(|b| b.to_ascii_lowercase()).collect::<Vec<u8>>();
    let nee = needle.iter().map(|b| b.to_ascii_lowercase()).collect::<Vec<u8>>();
    if nee.is_empty() { return Ok(vm.arena.alloc(Val::Bool(false))); }
    let pos = hay.windows(nee.len()).position(|w| w == nee.as_slice());
    Ok(match pos {
        Some(p) => vm.arena.alloc(Val::Int(offset + p as i64)),
        None => vm.arena.alloc(Val::Bool(false)),
    })
}

pub fn php_strrpos(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("strrpos() expects 2 or 3 parameters".into());
    }
    let haystack = vm.value_to_string(args[0])?;
    let needle = vm.value_to_string(args[1])?;
    let offset = if args.len() == 3 { vm.arena.get(args[2]).value.to_int() } else { 0 };
    if needle.is_empty() { return Ok(vm.arena.alloc(Val::Bool(false))); }
    let start = if offset >= 0 { offset as usize } else { haystack.len().saturating_sub((-offset) as usize) };
    if start > haystack.len() { return Ok(vm.arena.alloc(Val::Bool(false))); }
    let search = &haystack[start..];
    let pos = search.windows(needle.len()).rposition(|w| w == needle.as_slice());
    Ok(match pos {
        Some(p) => vm.arena.alloc(Val::Int((start + p) as i64)),
        None => vm.arena.alloc(Val::Bool(false)),
    })
}

pub fn php_strripos(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 3 {
        return Err("strripos() expects 2 or 3 parameters".into());
    }
    let haystack = vm.value_to_string(args[0])?;
    let needle = vm.value_to_string(args[1])?;
    let offset = if args.len() == 3 { vm.arena.get(args[2]).value.to_int() } else { 0 };
    if needle.is_empty() { return Ok(vm.arena.alloc(Val::Bool(false))); }
    let start = if offset >= 0 { offset as usize } else { haystack.len().saturating_sub((-offset) as usize) };
    if start > haystack.len() { return Ok(vm.arena.alloc(Val::Bool(false))); }
    let hay = haystack[start..]
        .iter().map(|b| b.to_ascii_lowercase()).collect::<Vec<u8>>();
    let nee = needle.iter().map(|b| b.to_ascii_lowercase()).collect::<Vec<u8>>();
    let pos = hay.windows(nee.len()).rposition(|w| w == nee.as_slice());
    Ok(match pos {
        Some(p) => vm.arena.alloc(Val::Int((start + p) as i64)),
        None => vm.arena.alloc(Val::Bool(false)),
    })
}

pub fn php_strrchr(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("strrchr() expects exactly 2 parameters".into());
    }
    let haystack = vm.value_to_string(args[0])?;
    let needle = vm.value_to_string(args[1])?;
    let ch = needle.first().copied().unwrap_or(0);
    if let Some(pos) = haystack.iter().rposition(|b| *b == ch) {
        return Ok(vm.arena.alloc(Val::String(haystack[pos..].to_vec().into())));
    }
    Ok(vm.arena.alloc(Val::Bool(false)))
}
```

Register in `core_extension.rs`:

```rust
registry.register_function(b"stripos", string::php_stripos);
registry.register_function(b"strrpos", string::php_strrpos);
registry.register_function(b"strripos", string::php_strripos);
registry.register_function(b"strrchr", string::php_strrchr);
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p php-vm test_stripos_strrpos_strripos_strrchr -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/php-vm/src/builtins/string.rs crates/php-vm/src/runtime/core_extension.rs crates/php-vm/tests/string_functions.rs
git commit -m "feat: add stripos and reverse string searches"
```

---

### Task 3: Character class scanning helpers (strpbrk/strspn/strcspn)

**Files:**
- Modify: `crates/php-vm/src/builtins/string.rs`
- Modify: `crates/php-vm/src/runtime/core_extension.rs`
- Test: `crates/php-vm/tests/string_functions.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_strpbrk_spn_cspn() {
    let result = run_php(r#"<?php
        echo strpbrk("abcdef", "xd");
        echo "|";
        echo strspn("abc123", "abc");
        echo "|";
        echo strcspn("abc123", "123");
    "#);
    assert_eq!(result, "def|3|3");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p php-vm test_strpbrk_spn_cspn -v`
Expected: FAIL with “Call to undefined function strpbrk/strspn/strcspn”

**Step 3: Write minimal implementation**

```rust
pub fn php_strpbrk(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 { return Err("strpbrk() expects exactly 2 parameters".into()); }
    let hay = vm.value_to_string(args[0])?;
    let mask = vm.value_to_string(args[1])?;
    for (idx, b) in hay.iter().enumerate() {
        if mask.contains(b) {
            return Ok(vm.arena.alloc(Val::String(hay[idx..].to_vec().into())));
        }
    }
    Ok(vm.arena.alloc(Val::Bool(false)))
}

pub fn php_strspn(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 4 { return Err("strspn() expects 2 to 4 parameters".into()); }
    let s = vm.value_to_string(args[0])?;
    let mask = vm.value_to_string(args[1])?;
    let start = if args.len() >= 3 { vm.arena.get(args[2]).value.to_int() } else { 0 };
    let length = if args.len() == 4 { Some(vm.arena.get(args[3]).value.to_int()) } else { None };
    let start = if start < 0 { 0 } else { start as usize };
    if start > s.len() { return Ok(vm.arena.alloc(Val::Int(0))); }
    let slice = &s[start..];
    let slice = if let Some(l) = length { &slice[..slice.len().min(l as usize)] } else { slice };
    let mut count = 0;
    for b in slice { if mask.contains(b) { count += 1; } else { break; } }
    Ok(vm.arena.alloc(Val::Int(count)))
}

pub fn php_strcspn(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 || args.len() > 4 { return Err("strcspn() expects 2 to 4 parameters".into()); }
    let s = vm.value_to_string(args[0])?;
    let mask = vm.value_to_string(args[1])?;
    let start = if args.len() >= 3 { vm.arena.get(args[2]).value.to_int() } else { 0 };
    let length = if args.len() == 4 { Some(vm.arena.get(args[3]).value.to_int()) } else { None };
    let start = if start < 0 { 0 } else { start as usize };
    if start > s.len() { return Ok(vm.arena.alloc(Val::Int(0))); }
    let slice = &s[start..];
    let slice = if let Some(l) = length { &slice[..slice.len().min(l as usize)] } else { slice };
    let mut count = 0;
    for b in slice { if !mask.contains(b) { count += 1; } else { break; } }
    Ok(vm.arena.alloc(Val::Int(count)))
}
```

Register in `core_extension.rs`:

```rust
registry.register_function(b"strpbrk", string::php_strpbrk);
registry.register_function(b"strspn", string::php_strspn);
registry.register_function(b"strcspn", string::php_strcspn);
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p php-vm test_strpbrk_spn_cspn -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/php-vm/src/builtins/string.rs crates/php-vm/src/runtime/core_extension.rs crates/php-vm/tests/string_functions.rs
git commit -m "feat: add strpbrk strspn strcspn"
```

---

### Task 4: Substring comparison (substr_compare)

**Files:**
- Modify: `crates/php-vm/src/builtins/string.rs`
- Modify: `crates/php-vm/src/runtime/core_extension.rs`
- Test: `crates/php-vm/tests/string_functions.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_substr_compare_basic() {
    let result = run_php(r#"<?php
        echo substr_compare("abcde", "bc", 1, 2);
        echo "|";
        echo substr_compare("abcde", "BC", 1, 2, true);
    "#);
    assert_eq!(result, "0|0");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p php-vm test_substr_compare_basic -v`
Expected: FAIL with “Call to undefined function substr_compare”

**Step 3: Write minimal implementation**

```rust
pub fn php_substr_compare(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 3 || args.len() > 5 {
        return Err("substr_compare() expects 3 to 5 parameters".into());
    }
    let main = vm.value_to_string(args[0])?;
    let str = vm.value_to_string(args[1])?;
    let offset = vm.arena.get(args[2]).value.to_int();
    let length = if args.len() >= 4 { Some(vm.arena.get(args[3]).value.to_int()) } else { None };
    let ci = if args.len() == 5 { vm.arena.get(args[4]).value.to_bool() } else { false };
    if offset < 0 || offset as usize > main.len() { return Ok(vm.arena.alloc(Val::Int(-1))); }
    let slice = &main[offset as usize..];
    let slice = if let Some(l) = length { &slice[..slice.len().min(l as usize)] } else { slice };
    let mut a = slice.to_vec();
    let mut b = str.clone();
    if ci { a = a.to_ascii_lowercase(); b = b.to_ascii_lowercase(); }
    let res = match a.cmp(&b) { Ordering::Less => -1, Ordering::Equal => 0, Ordering::Greater => 1 };
    Ok(vm.arena.alloc(Val::Int(res)))
}
```

Register in `core_extension.rs`:

```rust
registry.register_function(b"substr_compare", string::php_substr_compare);
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p php-vm test_substr_compare_basic -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/php-vm/src/builtins/string.rs crates/php-vm/src/runtime/core_extension.rs crates/php-vm/tests/string_functions.rs
git commit -m "feat: add substr_compare"
```

---

### Task 5: Word counting and CSV parsing (str_word_count, str_getcsv)

**Files:**
- Modify: `crates/php-vm/src/builtins/string.rs`
- Modify: `crates/php-vm/src/runtime/core_extension.rs`
- Test: `crates/php-vm/tests/string_functions.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_str_word_count_and_str_getcsv() {
    let result = run_php(r#"<?php
        echo str_word_count("Hello, world!");
        echo "|";
        $csv = str_getcsv("a,\"b,c\",d");
        echo implode("-", $csv);
    "#);
    assert_eq!(result, "2|a-b,c-d");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p php-vm test_str_word_count_and_str_getcsv -v`
Expected: FAIL with “Call to undefined function str_word_count/str_getcsv”

**Step 3: Write minimal implementation**

```rust
pub fn php_str_word_count(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 1 || args.len() > 3 { return Err("str_word_count() expects 1 to 3 parameters".into()); }
    let s = vm.value_to_string(args[0])?;
    let format = if args.len() >= 2 { vm.arena.get(args[1]).value.to_int() } else { 0 };
    let additional = if args.len() == 3 { vm.value_to_string(args[2])? } else { Vec::new() };
    let is_word = |b: u8| b.is_ascii_alphanumeric() || additional.contains(&b);
    let mut words = Vec::new();
    let mut i = 0;
    while i < s.len() {
        while i < s.len() && !is_word(s[i]) { i += 1; }
        let start = i;
        while i < s.len() && is_word(s[i]) { i += 1; }
        if start < i { words.push((start, &s[start..i])); }
    }
    match format {
        0 => Ok(vm.arena.alloc(Val::Int(words.len() as i64))),
        1 => {
            let mut map = indexmap::IndexMap::new();
            for (idx, (_, w)) in words.iter().enumerate() {
                map.insert(crate::core::value::ArrayKey::Int(idx as i64), vm.arena.alloc(Val::String(w.to_vec().into())));
            }
            Ok(vm.arena.alloc(Val::Array(crate::core::value::ArrayData::from(map).into())))
        }
        2 => {
            let mut map = indexmap::IndexMap::new();
            for (pos, w) in words { map.insert(crate::core::value::ArrayKey::Int(pos as i64), vm.arena.alloc(Val::String(w.to_vec().into()))); }
            Ok(vm.arena.alloc(Val::Array(crate::core::value::ArrayData::from(map).into())))
        }
        _ => Err("str_word_count(): Invalid format".into()),
    }
}

pub fn php_str_getcsv(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 1 || args.len() > 5 { return Err("str_getcsv() expects 1 to 5 parameters".into()); }
    let input = vm.value_to_string(args[0])?;
    let delimiter = if args.len() >= 2 { vm.value_to_string(args[1])? } else { b",".to_vec() };
    let enclosure = if args.len() >= 3 { vm.value_to_string(args[2])? } else { b"\"".to_vec() };
    let escape = if args.len() >= 4 { vm.value_to_string(args[3])? } else { b"\\".to_vec() };
    let delim = delimiter.first().copied().unwrap_or(b',');
    let enc = enclosure.first().copied().unwrap_or(b'"');
    let esc = escape.first().copied().unwrap_or(b'\\');

    let mut fields = Vec::new();
    let mut cur = Vec::new();
    let mut in_quotes = false;
    let mut i = 0;
    while i < input.len() {
        let b = input[i];
        if in_quotes {
            if b == esc && i + 1 < input.len() { i += 1; cur.push(input[i]); }
            else if b == enc { in_quotes = false; }
            else { cur.push(b); }
        } else {
            if b == enc { in_quotes = true; }
            else if b == delim {
                fields.push(cur); cur = Vec::new();
            } else { cur.push(b); }
        }
        i += 1;
    }
    fields.push(cur);

    let mut map = indexmap::IndexMap::new();
    for (idx, f) in fields.into_iter().enumerate() {
        map.insert(crate::core::value::ArrayKey::Int(idx as i64), vm.arena.alloc(Val::String(f.into())));
    }
    Ok(vm.arena.alloc(Val::Array(crate::core::value::ArrayData::from(map).into())))
}
```

Register in `core_extension.rs`:

```rust
registry.register_function(b"str_word_count", string::php_str_word_count);
registry.register_function(b"str_getcsv", string::php_str_getcsv);
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p php-vm test_str_word_count_and_str_getcsv -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/php-vm/src/builtins/string.rs crates/php-vm/src/runtime/core_extension.rs crates/php-vm/tests/string_functions.rs
git commit -m "feat: add str_word_count and str_getcsv"
```

---

### Task 6: Query parsing + formatting helpers (parse_str, number_format, nl2br, chunk_split, quotemeta)

**Files:**
- Modify: `crates/php-vm/src/builtins/string.rs`
- Modify: `crates/php-vm/src/runtime/core_extension.rs`
- Test: `crates/php-vm/tests/string_functions.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_parse_str_and_formatting_helpers() {
    let result = run_php(r#"<?php
        parse_str("a=1&b=two", $out);
        echo $out["a"] . "," . $out["b"];
        echo "|";
        echo number_format(1234.5, 1, ".", ",");
        echo "|";
        echo nl2br("a\n b");
        echo "|";
        echo chunk_split("abcd", 2, ":");
        echo "|";
        echo quotemeta(".$^*");
    "#);
    assert_eq!(result, "1,two|1,234.5|a<br />\n b|ab:cd:|\\.\\$\\^\\*");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p php-vm test_parse_str_and_formatting_helpers -v`
Expected: FAIL with “Call to undefined function parse_str/number_format/nl2br/chunk_split/quotemeta”

**Step 3: Write minimal implementation**

```rust
pub fn php_parse_str(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 1 || args.len() > 2 { return Err("parse_str() expects 1 or 2 parameters".into()); }
    let input = vm.value_to_string(args[0])?;
    let mut map = indexmap::IndexMap::new();
    for pair in input.split(|b| *b == b'&' || *b == b';') {
        if pair.is_empty() { continue; }
        let mut it = pair.splitn(2, |b| *b == b'=');
        let key = it.next().unwrap_or(&[]);
        let val = it.next().unwrap_or(&[]);
        let key = crate::builtins::url::url_decode_bytes(key);
        let val = crate::builtins::url::url_decode_bytes(val);
        map.insert(crate::core::value::ArrayKey::Str(key.into()), vm.arena.alloc(Val::String(val.into())));
    }
    let arr = vm.arena.alloc(Val::Array(crate::core::value::ArrayData::from(map).into()));
    if args.len() == 2 { vm.arena.get_mut(args[1]).value = vm.arena.get(arr).value.clone(); return Ok(vm.arena.alloc(Val::Null)); }
    Ok(arr)
}

pub fn php_number_format(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 4 { return Err("number_format() expects 1 to 4 parameters".into()); }
    let num = vm.arena.get(args[0]).value.to_float();
    let decimals = if args.len() >= 2 { vm.arena.get(args[1]).value.to_int() as usize } else { 0 };
    let dec_point = if args.len() >= 3 { vm.value_to_string(args[2])? } else { b".".to_vec() };
    let thousands = if args.len() == 4 { vm.value_to_string(args[3])? } else { b",".to_vec() };
    let mut s = format!("{:.*}", decimals, num.abs());
    if let Some(dot) = s.find('.') {
        let (int_part, frac_part) = s.split_at(dot);
        let mut grouped = String::new();
        for (i, ch) in int_part.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 { grouped.push_str(&String::from_utf8_lossy(&thousands)); }
            grouped.push(ch);
        }
        let int_rev: String = grouped.chars().rev().collect();
        s = format!("{}{}{}", int_rev, String::from_utf8_lossy(&dec_point), &frac_part[1..]);
    } else if !thousands.is_empty() {
        let mut grouped = String::new();
        for (i, ch) in s.chars().rev().enumerate() {
            if i > 0 && i % 3 == 0 { grouped.push_str(&String::from_utf8_lossy(&thousands)); }
            grouped.push(ch);
        }
        s = grouped.chars().rev().collect();
    }
    if num.is_sign_negative() { s.insert(0, '-'); }
    Ok(vm.arena.alloc(Val::String(s.into_bytes().into())))
}

pub fn php_nl2br(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 { return Err("nl2br() expects 1 or 2 parameters".into()); }
    let s = vm.value_to_string(args[0])?;
    let is_xhtml = if args.len() == 2 { vm.arena.get(args[1]).value.to_bool() } else { true };
    let br = if is_xhtml { b"<br />\n".to_vec() } else { b"<br>\n".to_vec() };
    let mut out = Vec::new();
    for &b in &s {
        if b == b'\n' { out.extend_from_slice(&br); }
        else { out.push(b); }
    }
    Ok(vm.arena.alloc(Val::String(out.into())))
}

pub fn php_chunk_split(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 3 { return Err("chunk_split() expects 1 to 3 parameters".into()); }
    let s = vm.value_to_string(args[0])?;
    let len = if args.len() >= 2 { vm.arena.get(args[1]).value.to_int() as usize } else { 76 };
    let end = if args.len() == 3 { vm.value_to_string(args[2])? } else { b"\r\n".to_vec() };
    if len == 0 { return Ok(vm.arena.alloc(Val::String(s.into()))); }
    let mut out = Vec::new();
    for chunk in s.chunks(len) { out.extend_from_slice(chunk); out.extend_from_slice(&end); }
    Ok(vm.arena.alloc(Val::String(out.into())))
}

pub fn php_quotemeta(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("quotemeta() expects exactly 1 parameter".into()); }
    let s = vm.value_to_string(args[0])?;
    let mut out = Vec::with_capacity(s.len());
    for &b in &s {
        if matches!(b, b'.'|b'\\'|b'+'|b'*'|b'?'|b'['|b'^'|b']'|b'$'|b'('|b')') {
            out.push(b'\\');
        }
        out.push(b);
    }
    Ok(vm.arena.alloc(Val::String(out.into())))
}
```

Register in `core_extension.rs`:

```rust
registry.register_function(b"parse_str", string::php_parse_str);
registry.register_function(b"number_format", string::php_number_format);
registry.register_function(b"nl2br", string::php_nl2br);
registry.register_function(b"chunk_split", string::php_chunk_split);
registry.register_function(b"quotemeta", string::php_quotemeta);
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p php-vm test_parse_str_and_formatting_helpers -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/php-vm/src/builtins/string.rs crates/php-vm/src/runtime/core_extension.rs crates/php-vm/tests/string_functions.rs
git commit -m "feat: add parse_str and formatting helpers"
```

---

### Task 7: count_chars + increment/decrement

**Files:**
- Modify: `crates/php-vm/src/builtins/string.rs`
- Modify: `crates/php-vm/src/runtime/core_extension.rs`
- Test: `crates/php-vm/tests/string_functions.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_count_chars_and_inc_dec() {
    let result = run_php(r#"<?php
        $arr = count_chars("abca", 1);
        echo $arr[97] . "," . $arr[98] . "," . $arr[99];
        echo "|";
        $s = "a9"; $s++; echo $s; echo "|"; $s--; echo $s;
    "#);
    assert_eq!(result, "2,1,1|b0|a9");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p php-vm test_count_chars_and_inc_dec -v`
Expected: FAIL with “Call to undefined function count_chars/str_increment/str_decrement”

**Step 3: Write minimal implementation**

```rust
pub fn php_count_chars(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 1 || args.len() > 2 { return Err("count_chars() expects 1 or 2 parameters".into()); }
    let s = vm.value_to_string(args[0])?;
    let mode = if args.len() == 2 { vm.arena.get(args[1]).value.to_int() } else { 0 };
    let mut counts = [0i64; 256];
    for b in s { counts[b as usize] += 1; }
    match mode {
        0 | 1 | 2 | 3 | 4 => {
            let mut map = indexmap::IndexMap::new();
            for (i, c) in counts.iter().enumerate() {
                let include = match mode { 0 | 1 => *c > 0, 2 => *c == 0, 3 => *c > 0, 4 => *c == 0, _ => false };
                if include {
                    let val = if mode == 3 || mode == 4 { Val::String(vec![i as u8].into()) } else { Val::Int(*c) };
                    map.insert(crate::core::value::ArrayKey::Int(i as i64), vm.arena.alloc(val));
                }
            }
            Ok(vm.arena.alloc(Val::Array(crate::core::value::ArrayData::from(map).into())))
        }
        _ => Err("count_chars(): Invalid mode".into()),
    }
}

pub fn php_str_increment(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("str_increment() expects exactly 1 parameter".into()); }
    let mut s = vm.value_to_string(args[0])?;
    if s.is_empty() { return Ok(vm.arena.alloc(Val::String(b"1".to_vec().into()))); }
    let mut i = s.len();
    while i > 0 {
        i -= 1;
        match s[i] {
            b'0'..=b'8' => { s[i] += 1; return Ok(vm.arena.alloc(Val::String(s.into()))); }
            b'9' => { s[i] = b'0'; if i == 0 { s.insert(0, b'1'); return Ok(vm.arena.alloc(Val::String(s.into()))); } }
            b'a'..=b'y' => { s[i] += 1; return Ok(vm.arena.alloc(Val::String(s.into()))); }
            b'z' => { s[i] = b'a'; if i == 0 { s.insert(0, b'a'); return Ok(vm.arena.alloc(Val::String(s.into()))); } }
            b'A'..=b'Y' => { s[i] += 1; return Ok(vm.arena.alloc(Val::String(s.into()))); }
            b'Z' => { s[i] = b'A'; if i == 0 { s.insert(0, b'A'); return Ok(vm.arena.alloc(Val::String(s.into()))); } }
            _ => return Ok(vm.arena.alloc(Val::String(s.into()))),
        }
    }
    Ok(vm.arena.alloc(Val::String(s.into())))
}

pub fn php_str_decrement(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("str_decrement() expects exactly 1 parameter".into()); }
    let mut s = vm.value_to_string(args[0])?;
    if s.is_empty() { return Ok(vm.arena.alloc(Val::String(Vec::new().into()))); }
    let mut i = s.len();
    while i > 0 {
        i -= 1;
        match s[i] {
            b'1'..=b'9' => { s[i] -= 1; return Ok(vm.arena.alloc(Val::String(s.into()))); }
            b'0' => { s[i] = b'9'; if i == 0 { if s.len() > 1 { s.remove(0); } return Ok(vm.arena.alloc(Val::String(s.into()))); } }
            b'b'..=b'z' => { s[i] -= 1; return Ok(vm.arena.alloc(Val::String(s.into()))); }
            b'a' => { s[i] = b'z'; if i == 0 { if s.len() > 1 { s.remove(0); } return Ok(vm.arena.alloc(Val::String(s.into()))); } }
            b'B'..=b'Z' => { s[i] -= 1; return Ok(vm.arena.alloc(Val::String(s.into()))); }
            b'A' => { s[i] = b'Z'; if i == 0 { if s.len() > 1 { s.remove(0); } return Ok(vm.arena.alloc(Val::String(s.into()))); } }
            _ => return Ok(vm.arena.alloc(Val::String(s.into()))),
        }
    }
    Ok(vm.arena.alloc(Val::String(s.into())))
}
```

Register in `core_extension.rs`:

```rust
registry.register_function(b"count_chars", string::php_count_chars);
registry.register_function(b"str_increment", string::php_str_increment);
registry.register_function(b"str_decrement", string::php_str_decrement);
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p php-vm test_count_chars_and_inc_dec -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/php-vm/src/builtins/string.rs crates/php-vm/src/runtime/core_extension.rs crates/php-vm/tests/string_functions.rs
git commit -m "feat: add count_chars and string inc/dec"
```

---

### Task 8: printf family (vsprintf, vprintf, vfprintf, fprintf, sscanf)

**Files:**
- Modify: `crates/php-vm/src/builtins/string.rs`
- Modify: `crates/php-vm/src/runtime/core_extension.rs`
- Test: `crates/php-vm/tests/string_functions.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_printf_family() {
    let result = run_php(r#"<?php
        $args = ["%s-%d", "hi", 7];
        echo vsprintf($args[0], array_slice($args, 1));
        echo "|";
        $out = sscanf("10,20", "%d,%d");
        echo $out[0] . "," . $out[1];
    "#);
    assert_eq!(result, "hi-7|10,20");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p php-vm test_printf_family -v`
Expected: FAIL with “Call to undefined function vsprintf/sscanf”

**Step 3: Write minimal implementation**

```rust
pub fn php_vsprintf(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 { return Err("vsprintf() expects exactly 2 parameters".into()); }
    let format = match &vm.arena.get(args[0]).value { Val::String(s) => s.clone(), _ => return Err("vsprintf(): Argument #1 must be string".into()) };
    let list = match &vm.arena.get(args[1]).value { Val::Array(a) => a, _ => return Err("vsprintf(): Argument #2 must be array".into()) };
    let mut argv = vec![vm.arena.alloc(Val::String(format))];
    for h in list.map.values() { argv.push(*h); }
    let bytes = format_sprintf_bytes(vm, &argv)?;
    Ok(vm.arena.alloc(Val::String(bytes.into())))
}

pub fn php_vprintf(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    let bytes = match php_vsprintf(vm, args) { Ok(h) => vm.arena.get(h).value.to_php_string_bytes(), Err(e) => return Err(e) };
    vm.print_bytes(&bytes)?;
    Ok(vm.arena.alloc(Val::Int(bytes.len() as i64)))
}

pub fn php_fprintf(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // For now, treat as sprintf and ignore stream (first arg), match PHP error if not resource later.
    if args.len() < 2 { return Err("fprintf() expects at least 2 parameters".into()); }
    let bytes = format_sprintf_bytes(vm, &args[1..])?;
    vm.print_bytes(&bytes)?;
    Ok(vm.arena.alloc(Val::Int(bytes.len() as i64)))
}

pub fn php_vfprintf(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 3 { return Err("vfprintf() expects exactly 3 parameters".into()); }
    let format = match &vm.arena.get(args[1]).value { Val::String(s) => s.clone(), _ => return Err("vfprintf(): Argument #2 must be string".into()) };
    let list = match &vm.arena.get(args[2]).value { Val::Array(a) => a, _ => return Err("vfprintf(): Argument #3 must be array".into()) };
    let mut argv = vec![vm.arena.alloc(Val::String(format))];
    for h in list.map.values() { argv.push(*h); }
    let bytes = format_sprintf_bytes(vm, &argv)?;
    vm.print_bytes(&bytes)?;
    Ok(vm.arena.alloc(Val::Int(bytes.len() as i64)))
}

pub fn php_sscanf(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 2 { return Err("sscanf() expects at least 2 parameters".into()); }
    let input = vm.value_to_string(args[0])?;
    let format = vm.value_to_string(args[1])?;
    let mut out = Vec::new();
    let mut i = 0;
    let mut f = 0;
    while f < format.len() {
        if format[f] == b'%' && f + 1 < format.len() {
            f += 1;
            match format[f] {
                b'd' => {
                    let start = i;
                    while i < input.len() && (input[i] as char).is_ascii_digit() { i += 1; }
                    let num = std::str::from_utf8(&input[start..i]).unwrap_or("0").parse::<i64>().unwrap_or(0);
                    out.push(vm.arena.alloc(Val::Int(num)));
                }
                b's' => {
                    let start = i;
                    while i < input.len() && !input[i].is_ascii_whitespace() { i += 1; }
                    out.push(vm.arena.alloc(Val::String(input[start..i].to_vec().into())));
                }
                _ => {}
            }
        } else {
            if i < input.len() && format[f] == input[i] { i += 1; }
        }
        f += 1;
    }
    let mut map = indexmap::IndexMap::new();
    for (idx, h) in out.into_iter().enumerate() {
        map.insert(crate::core::value::ArrayKey::Int(idx as i64), h);
    }
    Ok(vm.arena.alloc(Val::Array(crate::core::value::ArrayData::from(map).into())))
}
```

Register in `core_extension.rs`:

```rust
registry.register_function(b"vsprintf", string::php_vsprintf);
registry.register_function(b"vprintf", string::php_vprintf);
registry.register_function(b"fprintf", string::php_fprintf);
registry.register_function(b"vfprintf", string::php_vfprintf);
registry.register_function(b"sscanf", string::php_sscanf);
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p php-vm test_printf_family -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/php-vm/src/builtins/string.rs crates/php-vm/src/runtime/core_extension.rs crates/php-vm/tests/string_functions.rs
git commit -m "feat: add printf family helpers"
```

---

### Task 9: Hash wrappers (crc32, md5/sha1 + file variants)

**Files:**
- Modify: `crates/php-vm/src/builtins/string.rs`
- Modify: `crates/php-vm/src/runtime/core_extension.rs`
- Test: `crates/php-vm/tests/string_functions.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_hash_wrappers() {
    let result = run_php(r#"<?php
        echo crc32("abc");
        echo "|";
        echo md5("abc");
        echo "|";
        echo sha1("abc");
    "#);
    assert_eq!(result, "891568578|900150983cd24fb0d6963f7d28e17f72|a9993e364706816aba3e25717850c26c9cd0d89d");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p php-vm test_hash_wrappers -v`
Expected: FAIL with “Call to undefined function crc32/md5/sha1”

**Step 3: Write minimal implementation**

```rust
pub fn php_crc32(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("crc32() expects exactly 1 parameter".into()); }
    let s = vm.value_to_string(args[0])?;
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(&s);
    Ok(vm.arena.alloc(Val::Int(hasher.finalize() as i64)))
}

pub fn php_md5(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 1 || args.len() > 2 { return Err("md5() expects 1 or 2 parameters".into()); }
    let s = vm.value_to_string(args[0])?;
    let raw = if args.len() == 2 { vm.arena.get(args[1]).value.to_bool() } else { false };
    let digest = md5::compute(&s);
    let out = if raw { digest.0.to_vec() } else { format!("{:x}", digest).into_bytes() };
    Ok(vm.arena.alloc(Val::String(out.into())))
}

pub fn php_sha1(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 1 || args.len() > 2 { return Err("sha1() expects 1 or 2 parameters".into()); }
    let s = vm.value_to_string(args[0])?;
    let raw = if args.len() == 2 { vm.arena.get(args[1]).value.to_bool() } else { false };
    let digest = sha1::Sha1::from(&s).digest().bytes();
    let out = if raw { digest.to_vec() } else { hex::encode(digest).into_bytes() };
    Ok(vm.arena.alloc(Val::String(out.into())))
}

pub fn php_md5_file(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 1 || args.len() > 2 { return Err("md5_file() expects 1 or 2 parameters".into()); }
    let path = vm.value_to_string(args[0])?;
    let raw = if args.len() == 2 { vm.arena.get(args[1]).value.to_bool() } else { false };
    let data = std::fs::read(String::from_utf8_lossy(&path).as_ref()).map_err(|e| e.to_string())?;
    let digest = md5::compute(data);
    let out = if raw { digest.0.to_vec() } else { format!("{:x}", digest).into_bytes() };
    Ok(vm.arena.alloc(Val::String(out.into())))
}

pub fn php_sha1_file(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() < 1 || args.len() > 2 { return Err("sha1_file() expects 1 or 2 parameters".into()); }
    let path = vm.value_to_string(args[0])?;
    let raw = if args.len() == 2 { vm.arena.get(args[1]).value.to_bool() } else { false };
    let data = std::fs::read(String::from_utf8_lossy(&path).as_ref()).map_err(|e| e.to_string())?;
    let digest = sha1::Sha1::from(&data).digest().bytes();
    let out = if raw { digest.to_vec() } else { hex::encode(digest).into_bytes() };
    Ok(vm.arena.alloc(Val::String(out.into())))
}
```

Register in `core_extension.rs`:

```rust
registry.register_function(b"crc32", string::php_crc32);
registry.register_function(b"md5", string::php_md5);
registry.register_function(b"md5_file", string::php_md5_file);
registry.register_function(b"sha1", string::php_sha1);
registry.register_function(b"sha1_file", string::php_sha1_file);
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p php-vm test_hash_wrappers -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/php-vm/src/builtins/string.rs crates/php-vm/src/runtime/core_extension.rs crates/php-vm/tests/string_functions.rs
git commit -m "feat: add crc32 md5 sha1 wrappers"
```

---

### Task 10: HTML entity helpers (htmlspecialchars/htmlentities/decoders + strip_tags)

**Files:**
- Modify: `crates/php-vm/src/builtins/string.rs`
- Modify: `crates/php-vm/src/runtime/core_extension.rs`
- Test: `crates/php-vm/tests/string_functions.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_html_entities_and_strip_tags() {
    let result = run_php(r#"<?php
        echo htmlspecialchars("<a>&\"", ENT_QUOTES);
        echo "|";
        echo htmlspecialchars_decode("&lt;a&gt;", ENT_QUOTES);
        echo "|";
        echo strip_tags("<b>hi</b>");
    "#);
    assert_eq!(result, "&lt;a&gt;&amp;\"|<a>|hi");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p php-vm test_html_entities_and_strip_tags -v`
Expected: FAIL with “Call to undefined function htmlspecialchars/strip_tags”

**Step 3: Write minimal implementation**

```rust
fn html_escape_basic(input: &[u8], quote_style: i64) -> Vec<u8> {
    let mut out = Vec::new();
    for &b in input {
        match b {
            b'<' => out.extend_from_slice(b"&lt;"),
            b'>' => out.extend_from_slice(b"&gt;"),
            b'&' => out.extend_from_slice(b"&amp;"),
            b'\"' if quote_style & 3 != 0 => out.extend_from_slice(b"&quot;"),
            b'\'' if quote_style & 1 != 0 => out.extend_from_slice(b"&#039;"),
            _ => out.push(b),
        }
    }
    out
}

pub fn php_htmlspecialchars(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 4 { return Err("htmlspecialchars() expects 1 to 4 parameters".into()); }
    let s = vm.value_to_string(args[0])?;
    let flags = if args.len() >= 2 { vm.arena.get(args[1]).value.to_int() } else { 2 };
    let out = html_escape_basic(&s, flags);
    Ok(vm.arena.alloc(Val::String(out.into())))
}

pub fn php_htmlspecialchars_decode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 { return Err("htmlspecialchars_decode() expects 1 or 2 parameters".into()); }
    let s = vm.value_to_string(args[0])?;
    let mut out = String::from_utf8_lossy(&s).to_string();
    out = out.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&").replace("&quot;", "\"").replace("&#039;", "'");
    Ok(vm.arena.alloc(Val::String(out.into_bytes().into())))
}

pub fn php_strip_tags(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 { return Err("strip_tags() expects 1 or 2 parameters".into()); }
    let s = vm.value_to_string(args[0])?;
    let mut out = Vec::new();
    let mut in_tag = false;
    for &b in &s {
        if b == b'<' { in_tag = true; continue; }
        if b == b'>' { in_tag = false; continue; }
        if !in_tag { out.push(b); }
    }
    Ok(vm.arena.alloc(Val::String(out.into())))
}

pub fn php_htmlentities(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    php_htmlspecialchars(vm, args)
}

pub fn php_html_entity_decode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    php_htmlspecialchars_decode(vm, args)
}

pub fn php_get_html_translation_table(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    let mut map = indexmap::IndexMap::new();
    map.insert(crate::core::value::ArrayKey::Str(b"<".to_vec().into()), vm.arena.alloc(Val::String(b"&lt;".to_vec().into())));
    map.insert(crate::core::value::ArrayKey::Str(b">".to_vec().into()), vm.arena.alloc(Val::String(b"&gt;".to_vec().into())));
    map.insert(crate::core::value::ArrayKey::Str(b"&".to_vec().into()), vm.arena.alloc(Val::String(b"&amp;".to_vec().into())));
    Ok(vm.arena.alloc(Val::Array(crate::core::value::ArrayData::from(map).into())))
}
```

Register in `core_extension.rs`:

```rust
registry.register_function(b"htmlspecialchars", string::php_htmlspecialchars);
registry.register_function(b"htmlspecialchars_decode", string::php_htmlspecialchars_decode);
registry.register_function(b"htmlentities", string::php_htmlentities);
registry.register_function(b"html_entity_decode", string::php_html_entity_decode);
registry.register_function(b"get_html_translation_table", string::php_get_html_translation_table);
registry.register_function(b"strip_tags", string::php_strip_tags);
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p php-vm test_html_entities_and_strip_tags -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/php-vm/src/builtins/string.rs crates/php-vm/src/runtime/core_extension.rs crates/php-vm/tests/string_functions.rs
git commit -m "feat: add html entity helpers"
```

---

### Task 11: Remaining locale/encoding and legacy helpers

**Files:**
- Modify: `crates/php-vm/src/builtins/string.rs`
- Modify: `crates/php-vm/src/runtime/core_extension.rs`
- Test: `crates/php-vm/tests/string_functions.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn test_legacy_string_helpers() {
    let result = run_php(r#"<?php
        echo utf8_encode("test");
        echo "|";
        echo utf8_decode("test");
    "#);
    assert_eq!(result, "test|test");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p php-vm test_legacy_string_helpers -v`
Expected: FAIL with “Call to undefined function utf8_encode/utf8_decode”

**Step 3: Write minimal implementation**

```rust
pub fn php_utf8_encode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("utf8_encode() expects exactly 1 parameter".into()); }
    let s = vm.value_to_string(args[0])?;
    Ok(vm.arena.alloc(Val::String(s.into())))
}

pub fn php_utf8_decode(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 { return Err("utf8_decode() expects exactly 1 parameter".into()); }
    let s = vm.value_to_string(args[0])?;
    Ok(vm.arena.alloc(Val::String(s.into())))
}
```

Register in `core_extension.rs`:

```rust
registry.register_function(b"utf8_encode", string::php_utf8_encode);
registry.register_function(b"utf8_decode", string::php_utf8_decode);
```

**Step 4: Run test to verify it passes**

Run: `cargo test -p php-vm test_legacy_string_helpers -v`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/php-vm/src/builtins/string.rs crates/php-vm/src/runtime/core_extension.rs crates/php-vm/tests/string_functions.rs
git commit -m "feat: add utf8 encode/decode stubs"
```

---

### Task 12: Verification sweep

**Files:**
- Modify: `crates/php-vm/tests/string_functions.rs`

**Step 1: Run full string tests**

Run: `cargo test -p php-vm string_functions -v`
Expected: PASS

**Step 2: Run full suite**

Run: `cargo test`
Expected: PASS (existing warnings allowed)

**Step 3: Commit any final adjustments**

```bash
git add crates/php-vm/tests/string_functions.rs
git commit -m "test: extend string function coverage"
```
