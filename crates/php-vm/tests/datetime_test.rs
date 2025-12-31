use php_vm::core::value::Val;
use php_vm::runtime::context::EngineBuilder;
use php_vm::vm::engine::VM;

fn setup_vm() -> VM {
    let engine = EngineBuilder::new()
        .with_core_extensions()
        .build()
        .expect("Failed to build engine");
    VM::new(engine)
}

fn get_int_value(vm: &VM, handle: php_vm::core::value::Handle) -> i64 {
    let val = vm.arena.get(handle);
    match &val.value {
        Val::Int(i) => *i,
        _ => panic!("Expected integer value"),
    }
}

fn get_string_value(vm: &VM, handle: php_vm::core::value::Handle) -> String {
    let val = vm.arena.get(handle);
    match &val.value {
        Val::String(s) => String::from_utf8_lossy(s).to_string(),
        _ => panic!("Expected string value"),
    }
}

fn get_bool_value(vm: &VM, handle: php_vm::core::value::Handle) -> bool {
    let val = vm.arena.get(handle);
    match &val.value {
        Val::Bool(b) => *b,
        _ => panic!("Expected bool value"),
    }
}

fn get_float_value(vm: &VM, handle: php_vm::core::value::Handle) -> f64 {
    let val = vm.arena.get(handle);
    match &val.value {
        Val::Float(f) => *f,
        _ => panic!("Expected float value"),
    }
}

#[test]
fn test_checkdate_valid() {
    let mut vm = setup_vm();

    // Valid date: 2024-12-16
    let month = vm.arena.alloc(Val::Int(12));
    let day = vm.arena.alloc(Val::Int(16));
    let year = vm.arena.alloc(Val::Int(2024));

    let result = php_vm::builtins::datetime::php_checkdate(&mut vm, &[month, day, year]).unwrap();
    assert!(get_bool_value(&vm, result));
}

#[test]
fn test_checkdate_invalid() {
    let mut vm = setup_vm();

    // Invalid date: 2024-02-30
    let month = vm.arena.alloc(Val::Int(2));
    let day = vm.arena.alloc(Val::Int(30));
    let year = vm.arena.alloc(Val::Int(2024));

    let result = php_vm::builtins::datetime::php_checkdate(&mut vm, &[month, day, year]).unwrap();
    assert!(!get_bool_value(&vm, result));
}

#[test]
fn test_checkdate_leap_year() {
    let mut vm = setup_vm();

    // Valid leap year date: 2024-02-29
    let month = vm.arena.alloc(Val::Int(2));
    let day = vm.arena.alloc(Val::Int(29));
    let year = vm.arena.alloc(Val::Int(2024));

    let result = php_vm::builtins::datetime::php_checkdate(&mut vm, &[month, day, year]).unwrap();
    assert!(get_bool_value(&vm, result));

    // Invalid non-leap year: 2023-02-29
    let year = vm.arena.alloc(Val::Int(2023));
    let result = php_vm::builtins::datetime::php_checkdate(&mut vm, &[month, day, year]).unwrap();
    assert!(!get_bool_value(&vm, result));
}

#[test]
fn test_time() {
    let mut vm = setup_vm();

    let result = php_vm::builtins::datetime::php_time(&mut vm, &[]).unwrap();
    let timestamp = get_int_value(&vm, result);

    // Should be a reasonable timestamp (after 2020-01-01)
    assert!(timestamp > 1577836800);
}

#[test]
fn test_microtime_string() {
    let mut vm = setup_vm();

    let result = php_vm::builtins::datetime::php_microtime(&mut vm, &[]).unwrap();
    let output = get_string_value(&vm, result);

    // Should have format "0.XXXXXX YYYYYY"
    assert!(output.contains(' '));
    let parts: Vec<&str> = output.split(' ').collect();
    assert_eq!(parts.len(), 2);
    assert!(parts[0].starts_with("0."));
}

#[test]
fn test_microtime_float() {
    let mut vm = setup_vm();

    let as_float = vm.arena.alloc(Val::Bool(true));
    let result = php_vm::builtins::datetime::php_microtime(&mut vm, &[as_float]).unwrap();
    let timestamp = get_float_value(&vm, result);

    // Should be a reasonable timestamp
    assert!(timestamp > 1577836800.0);
}

#[test]
fn test_date_basic() {
    let mut vm = setup_vm();

    // Test basic date formatting
    let format = vm.arena.alloc(Val::String(b"Y-m-d".to_vec().into()));
    let timestamp = vm.arena.alloc(Val::Int(1609459200)); // 2021-01-01 00:00:00 UTC

    let result = php_vm::builtins::datetime::php_date(&mut vm, &[format, timestamp]).unwrap();
    let date_str = get_string_value(&vm, result);

    // Note: Result depends on timezone, so we just check it's a valid format
    assert!(date_str.len() >= 10); // YYYY-MM-DD
}

#[test]
fn test_date_format_specifiers() {
    let mut vm = setup_vm();

    let timestamp = vm.arena.alloc(Val::Int(1609459200)); // 2021-01-01 00:00:00 UTC

    // Test Y (4-digit year)
    let format = vm.arena.alloc(Val::String(b"Y".to_vec().into()));
    let result = php_vm::builtins::datetime::php_date(&mut vm, &[format, timestamp]).unwrap();
    let year = get_string_value(&vm, result);
    assert_eq!(year.len(), 4);

    // Test m (2-digit month)
    let format = vm.arena.alloc(Val::String(b"m".to_vec().into()));
    let result = php_vm::builtins::datetime::php_date(&mut vm, &[format, timestamp]).unwrap();
    let month = get_string_value(&vm, result);
    assert_eq!(month.len(), 2);

    // Test d (2-digit day)
    let format = vm.arena.alloc(Val::String(b"d".to_vec().into()));
    let result = php_vm::builtins::datetime::php_date(&mut vm, &[format, timestamp]).unwrap();
    let day = get_string_value(&vm, result);
    assert_eq!(day.len(), 2);
}

#[test]
fn test_gmdate() {
    let mut vm = setup_vm();

    let format = vm.arena.alloc(Val::String(b"Y-m-d H:i:s".to_vec().into()));
    let timestamp = vm.arena.alloc(Val::Int(1609459200)); // 2021-01-01 00:00:00 UTC

    let result = php_vm::builtins::datetime::php_gmdate(&mut vm, &[format, timestamp]).unwrap();
    let date_str = get_string_value(&vm, result);

    // Should be in UTC
    assert!(date_str.contains("2021-01-01"));
}

#[test]
fn test_mktime() {
    let mut vm = setup_vm();

    // mktime(12, 0, 0, 1, 1, 2021) = January 1, 2021, 12:00:00
    let hour = vm.arena.alloc(Val::Int(12));
    let minute = vm.arena.alloc(Val::Int(0));
    let second = vm.arena.alloc(Val::Int(0));
    let month = vm.arena.alloc(Val::Int(1));
    let day = vm.arena.alloc(Val::Int(1));
    let year = vm.arena.alloc(Val::Int(2021));

    let result =
        php_vm::builtins::datetime::php_mktime(&mut vm, &[hour, minute, second, month, day, year])
            .unwrap();
    let timestamp = get_int_value(&vm, result);

    // Should be a valid timestamp
    assert!(timestamp > 0);
}

#[test]
fn test_mktime_invalid() {
    let mut vm = setup_vm();

    // Invalid date
    let hour = vm.arena.alloc(Val::Int(0));
    let minute = vm.arena.alloc(Val::Int(0));
    let second = vm.arena.alloc(Val::Int(0));
    let month = vm.arena.alloc(Val::Int(13)); // Invalid month
    let day = vm.arena.alloc(Val::Int(1));
    let year = vm.arena.alloc(Val::Int(2021));

    let result =
        php_vm::builtins::datetime::php_mktime(&mut vm, &[hour, minute, second, month, day, year])
            .unwrap();
    let is_false = get_bool_value(&vm, result);
    assert!(!is_false);
}

#[test]
fn test_strtotime_now() {
    let mut vm = setup_vm();

    let datetime = vm.arena.alloc(Val::String(b"now".to_vec().into()));
    let result = php_vm::builtins::datetime::php_strtotime(&mut vm, &[datetime]).unwrap();
    let timestamp = get_int_value(&vm, result);

    // Should be a recent timestamp
    assert!(timestamp > 1577836800); // After 2020-01-01
}

#[test]
fn test_strtotime_iso_format() {
    let mut vm = setup_vm();

    let datetime = vm
        .arena
        .alloc(Val::String(b"2021-01-01T00:00:00Z".to_vec().into()));
    let result = php_vm::builtins::datetime::php_strtotime(&mut vm, &[datetime]).unwrap();
    let timestamp = get_int_value(&vm, result);

    assert_eq!(timestamp, 1609459200);
}

#[test]
fn test_strtotime_date_format() {
    let mut vm = setup_vm();

    let datetime = vm.arena.alloc(Val::String(b"2021-01-01".to_vec().into()));
    let result = php_vm::builtins::datetime::php_strtotime(&mut vm, &[datetime]).unwrap();
    let timestamp = get_int_value(&vm, result);

    assert_eq!(timestamp, 1609459200);
}

#[test]
fn test_strtotime_invalid() {
    let mut vm = setup_vm();

    let datetime = vm.arena.alloc(Val::String(b"not a date".to_vec().into()));
    let result = php_vm::builtins::datetime::php_strtotime(&mut vm, &[datetime]).unwrap();
    let is_false = get_bool_value(&vm, result);
    assert!(!is_false);
}

#[test]
fn test_getdate() {
    let mut vm = setup_vm();

    let timestamp = vm.arena.alloc(Val::Int(1609459200)); // 2021-01-01 00:00:00 UTC
    let result = php_vm::builtins::datetime::php_getdate(&mut vm, &[timestamp]).unwrap();

    // Should return an array
    let val = vm.arena.get(result);
    assert!(matches!(&val.value, Val::Array(_)));
}

#[test]
fn test_idate_year() {
    let mut vm = setup_vm();

    let format = vm.arena.alloc(Val::String(b"Y".to_vec().into()));
    let timestamp = vm.arena.alloc(Val::Int(1609459200)); // 2021-01-01 00:00:00 UTC

    let result = php_vm::builtins::datetime::php_idate(&mut vm, &[format, timestamp]).unwrap();
    let year = get_int_value(&vm, result);

    // Should be 2021 in UTC timezone
    assert!(year >= 2020 && year <= 2022); // Allow for timezone variations
}

#[test]
fn test_idate_month() {
    let mut vm = setup_vm();

    let format = vm.arena.alloc(Val::String(b"m".to_vec().into()));
    let timestamp = vm.arena.alloc(Val::Int(1609459200)); // 2021-01-01 00:00:00 UTC

    let result = php_vm::builtins::datetime::php_idate(&mut vm, &[format, timestamp]).unwrap();
    let month = get_int_value(&vm, result);

    assert!(month >= 1 && month <= 12);
}

#[test]
fn test_gettimeofday_array() {
    let mut vm = setup_vm();

    let result = php_vm::builtins::datetime::php_gettimeofday(&mut vm, &[]).unwrap();

    // Should return an array
    let val = vm.arena.get(result);
    assert!(matches!(&val.value, Val::Array(_)));
}

#[test]
fn test_gettimeofday_float() {
    let mut vm = setup_vm();

    let as_float = vm.arena.alloc(Val::Bool(true));
    let result = php_vm::builtins::datetime::php_gettimeofday(&mut vm, &[as_float]).unwrap();
    let timestamp = get_float_value(&vm, result);

    assert!(timestamp > 1577836800.0);
}

#[test]
fn test_localtime_indexed() {
    let mut vm = setup_vm();

    let timestamp = vm.arena.alloc(Val::Int(1609459200));
    let result = php_vm::builtins::datetime::php_localtime(&mut vm, &[timestamp]).unwrap();

    // Should return an array
    let val = vm.arena.get(result);
    assert!(matches!(&val.value, Val::Array(_)));
}

#[test]
fn test_localtime_associative() {
    let mut vm = setup_vm();

    let timestamp = vm.arena.alloc(Val::Int(1609459200));
    let associative = vm.arena.alloc(Val::Bool(true));
    let result =
        php_vm::builtins::datetime::php_localtime(&mut vm, &[timestamp, associative]).unwrap();

    // Should return an associative array
    let val = vm.arena.get(result);
    assert!(matches!(&val.value, Val::Array(_)));
}

#[test]
fn test_date_default_timezone_get() {
    let mut vm = setup_vm();

    let result = php_vm::builtins::datetime::php_date_default_timezone_get(&mut vm, &[]).unwrap();
    let timezone = get_string_value(&vm, result);

    // Default should be UTC
    assert_eq!(timezone, "UTC");
}

#[test]
fn test_date_default_timezone_set_valid() {
    let mut vm = setup_vm();

    let tz = vm
        .arena
        .alloc(Val::String(b"America/New_York".to_vec().into()));
    let result = php_vm::builtins::datetime::php_date_default_timezone_set(&mut vm, &[tz]).unwrap();
    let success = get_bool_value(&vm, result);

    assert!(success);
}

#[test]
fn test_date_default_timezone_set_invalid() {
    let mut vm = setup_vm();

    let tz = vm
        .arena
        .alloc(Val::String(b"Invalid/Timezone".to_vec().into()));
    let result = php_vm::builtins::datetime::php_date_default_timezone_set(&mut vm, &[tz]).unwrap();
    let success = get_bool_value(&vm, result);

    assert!(!success);
}

#[test]
fn test_date_sunrise() {
    let mut vm = setup_vm();

    let timestamp = vm.arena.alloc(Val::Int(1609459200));
    let format = vm.arena.alloc(Val::Int(1)); // SUNFUNCS_RET_STRING

    let result =
        php_vm::builtins::datetime::php_date_sunrise(&mut vm, &[timestamp, format]).unwrap();
    let sunrise = get_string_value(&vm, result);

    // Should return a time string
    assert!(!sunrise.is_empty());
}

#[test]
fn test_date_sunset() {
    let mut vm = setup_vm();

    let timestamp = vm.arena.alloc(Val::Int(1609459200));
    let format = vm.arena.alloc(Val::Int(1)); // SUNFUNCS_RET_STRING

    let result =
        php_vm::builtins::datetime::php_date_sunset(&mut vm, &[timestamp, format]).unwrap();
    let sunset = get_string_value(&vm, result);

    // Should return a time string
    assert!(!sunset.is_empty());
}

#[test]
fn test_date_sun_info() {
    let mut vm = setup_vm();

    let timestamp = vm.arena.alloc(Val::Int(1609459200));
    let latitude = vm.arena.alloc(Val::Float(40.7128)); // New York
    let longitude = vm.arena.alloc(Val::Float(-74.0060));

    let result =
        php_vm::builtins::datetime::php_date_sun_info(&mut vm, &[timestamp, latitude, longitude])
            .unwrap();

    // Should return an array
    let val = vm.arena.get(result);
    assert!(matches!(&val.value, Val::Array(_)));
}

#[test]
fn test_date_parse() {
    let mut vm = setup_vm();

    let datetime = vm
        .arena
        .alloc(Val::String(b"2021-01-01 12:00:00".to_vec().into()));
    let result = php_vm::builtins::datetime::php_date_parse(&mut vm, &[datetime]).unwrap();

    // Should return an array
    let val = vm.arena.get(result);
    assert!(matches!(&val.value, Val::Array(_)));
}

#[test]
fn test_date_parse_from_format() {
    let mut vm = setup_vm();

    let format = vm.arena.alloc(Val::String(b"Y-m-d".to_vec().into()));
    let datetime = vm.arena.alloc(Val::String(b"2021-01-01".to_vec().into()));
    let result =
        php_vm::builtins::datetime::php_date_parse_from_format(&mut vm, &[format, datetime])
            .unwrap();

    // Should return an array
    let val = vm.arena.get(result);
    assert!(matches!(&val.value, Val::Array(_)));
}

#[test]
fn test_date_constant_formats() {
    let mut vm = setup_vm();

    let timestamp = vm.arena.alloc(Val::Int(1609459200));

    // Test DATE_ATOM format
    let format = vm
        .arena
        .alloc(Val::String(b"Y-m-d\\TH:i:sP".to_vec().into()));
    let result = php_vm::builtins::datetime::php_gmdate(&mut vm, &[format, timestamp]).unwrap();
    let date_str = get_string_value(&vm, result);

    // Should contain date and time with timezone
    assert!(date_str.contains("2021-01-01"));
    assert!(date_str.contains("T"));
}

#[test]
fn test_leap_year_february() {
    let mut vm = setup_vm();

    // 2024 is a leap year
    let month = vm.arena.alloc(Val::Int(2));
    let day = vm.arena.alloc(Val::Int(29));
    let year = vm.arena.alloc(Val::Int(2024));

    let result = php_vm::builtins::datetime::php_checkdate(&mut vm, &[month, day, year]).unwrap();
    assert!(get_bool_value(&vm, result));

    // 2023 is not a leap year
    let year = vm.arena.alloc(Val::Int(2023));
    let result = php_vm::builtins::datetime::php_checkdate(&mut vm, &[month, day, year]).unwrap();
    assert!(!get_bool_value(&vm, result));

    // 1900 is not a leap year (divisible by 100 but not 400)
    let year = vm.arena.alloc(Val::Int(1900));
    let result = php_vm::builtins::datetime::php_checkdate(&mut vm, &[month, day, year]).unwrap();
    assert!(!get_bool_value(&vm, result));

    // 2000 is a leap year (divisible by 400)
    let year = vm.arena.alloc(Val::Int(2000));
    let result = php_vm::builtins::datetime::php_checkdate(&mut vm, &[month, day, year]).unwrap();
    assert!(get_bool_value(&vm, result));
}

#[test]
fn test_boundary_dates() {
    let mut vm = setup_vm();

    // Test month boundaries
    let test_cases = vec![
        (1, 31, 2024, true),  // January
        (2, 28, 2023, true),  // February non-leap
        (2, 29, 2024, true),  // February leap
        (3, 31, 2024, true),  // March
        (4, 30, 2024, true),  // April
        (4, 31, 2024, false), // April invalid
        (5, 31, 2024, true),  // May
        (6, 30, 2024, true),  // June
        (7, 31, 2024, true),  // July
        (8, 31, 2024, true),  // August
        (9, 30, 2024, true),  // September
        (10, 31, 2024, true), // October
        (11, 30, 2024, true), // November
        (12, 31, 2024, true), // December
    ];

    for (month, day, year, expected) in test_cases {
        let m = vm.arena.alloc(Val::Int(month));
        let d = vm.arena.alloc(Val::Int(day));
        let y = vm.arena.alloc(Val::Int(year));

        let result = php_vm::builtins::datetime::php_checkdate(&mut vm, &[m, d, y]).unwrap();
        assert_eq!(
            get_bool_value(&vm, result),
            expected,
            "Failed for date {}-{}-{}",
            year,
            month,
            day
        );
    }
}

#[test]
fn test_timestamp_edge_cases() {
    let mut vm = setup_vm();

    // Unix epoch
    let timestamp = vm.arena.alloc(Val::Int(0));
    let format = vm.arena.alloc(Val::String(b"Y-m-d H:i:s".to_vec().into()));
    let result = php_vm::builtins::datetime::php_gmdate(&mut vm, &[format, timestamp]).unwrap();
    let date_str = get_string_value(&vm, result);

    assert!(date_str.contains("1970-01-01"));
}
