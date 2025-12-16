use crate::core::value::{ArrayKey, Handle, Val};
use crate::vm::engine::VM;
use chrono::{
    DateTime as ChronoDateTime, Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Offset,
    TimeZone, Timelike, Utc, Weekday,
};
use chrono_tz::Tz;
use indexmap::IndexMap;
use std::rc::Rc;
use std::str::FromStr;

// ============================================================================
// Date/Time Constants
// ============================================================================

pub const DATE_ATOM: &str = "Y-m-d\\TH:i:sP";
pub const DATE_COOKIE: &str = "l, d-M-Y H:i:s T";
pub const DATE_ISO8601: &str = "Y-m-d\\TH:i:sO";
pub const DATE_ISO8601_EXPANDED: &str = "X-m-d\\TH:i:sP";
pub const DATE_RFC822: &str = "D, d M y H:i:s O";
pub const DATE_RFC850: &str = "l, d-M-y H:i:s T";
pub const DATE_RFC1036: &str = "D, d M y H:i:s O";
pub const DATE_RFC1123: &str = "D, d M Y H:i:s O";
pub const DATE_RFC7231: &str = "D, d M Y H:i:s \\G\\M\\T";
pub const DATE_RFC2822: &str = "D, d M Y H:i:s O";
pub const DATE_RFC3339: &str = "Y-m-d\\TH:i:sP";
pub const DATE_RFC3339_EXTENDED: &str = "Y-m-d\\TH:i:s.vP";
pub const DATE_RSS: &str = "D, d M Y H:i:s O";
pub const DATE_W3C: &str = "Y-m-d\\TH:i:sP";

// Deprecated constants for date_sunrise/date_sunset
pub const SUNFUNCS_RET_TIMESTAMP: i64 = 0;
pub const SUNFUNCS_RET_STRING: i64 = 1;
pub const SUNFUNCS_RET_DOUBLE: i64 = 2;

// ============================================================================
// Helper Functions
// ============================================================================

fn get_string_arg(vm: &VM, handle: Handle) -> Result<Vec<u8>, String> {
    let val = vm.arena.get(handle);
    match &val.value {
        Val::String(s) => Ok(s.to_vec()),
        _ => Err("Expected string argument".into()),
    }
}

fn get_int_arg(vm: &VM, handle: Handle) -> Result<i64, String> {
    let val = vm.arena.get(handle);
    match &val.value {
        Val::Int(i) => Ok(*i),
        _ => Err("Expected integer argument".into()),
    }
}

fn get_float_arg(vm: &VM, handle: Handle) -> Result<f64, String> {
    let val = vm.arena.get(handle);
    match &val.value {
        Val::Float(f) => Ok(*f),
        Val::Int(i) => Ok(*i as f64),
        _ => Err("Expected float argument".into()),
    }
}

fn parse_timezone(tz_str: &str) -> Result<Tz, String> {
    Tz::from_str(tz_str).map_err(|_| format!("Unknown or invalid timezone: {}", tz_str))
}

fn make_array_key(key: &str) -> ArrayKey {
    ArrayKey::Str(Rc::new(key.as_bytes().to_vec()))
}

fn format_php_date(dt: &ChronoDateTime<Tz>, format: &str) -> String {
    let mut result = String::new();
    let mut chars = format.chars().peekable();
    let mut escape_next = false;

    while let Some(ch) = chars.next() {
        if escape_next {
            result.push(ch);
            escape_next = false;
            continue;
        }

        if ch == '\\' {
            escape_next = true;
            continue;
        }

        match ch {
            // Day
            'd' => result.push_str(&format!("{:02}", dt.day())),
            'D' => {
                let day = match dt.weekday() {
                    Weekday::Mon => "Mon",
                    Weekday::Tue => "Tue",
                    Weekday::Wed => "Wed",
                    Weekday::Thu => "Thu",
                    Weekday::Fri => "Fri",
                    Weekday::Sat => "Sat",
                    Weekday::Sun => "Sun",
                };
                result.push_str(day);
            }
            'j' => result.push_str(&dt.day().to_string()),
            'l' => {
                let day = match dt.weekday() {
                    Weekday::Mon => "Monday",
                    Weekday::Tue => "Tuesday",
                    Weekday::Wed => "Wednesday",
                    Weekday::Thu => "Thursday",
                    Weekday::Fri => "Friday",
                    Weekday::Sat => "Saturday",
                    Weekday::Sun => "Sunday",
                };
                result.push_str(day);
            }
            'N' => result.push_str(&dt.weekday().num_days_from_monday().to_string()),
            'S' => {
                let day = dt.day();
                let suffix = match day {
                    1 | 21 | 31 => "st",
                    2 | 22 => "nd",
                    3 | 23 => "rd",
                    _ => "th",
                };
                result.push_str(suffix);
            }
            'w' => result.push_str(&dt.weekday().number_from_sunday().to_string()),
            'z' => result.push_str(&dt.ordinal0().to_string()),

            // Week
            'W' => result.push_str(&format!("{:02}", dt.iso_week().week())),

            // Month
            'F' => {
                let month = match dt.month() {
                    1 => "January",
                    2 => "February",
                    3 => "March",
                    4 => "April",
                    5 => "May",
                    6 => "June",
                    7 => "July",
                    8 => "August",
                    9 => "September",
                    10 => "October",
                    11 => "November",
                    12 => "December",
                    _ => "",
                };
                result.push_str(month);
            }
            'm' => result.push_str(&format!("{:02}", dt.month())),
            'M' => {
                let month = match dt.month() {
                    1 => "Jan",
                    2 => "Feb",
                    3 => "Mar",
                    4 => "Apr",
                    5 => "May",
                    6 => "Jun",
                    7 => "Jul",
                    8 => "Aug",
                    9 => "Sep",
                    10 => "Oct",
                    11 => "Nov",
                    12 => "Dec",
                    _ => "",
                };
                result.push_str(month);
            }
            'n' => result.push_str(&dt.month().to_string()),
            't' => {
                let days_in_month = NaiveDate::from_ymd_opt(
                    dt.year(),
                    dt.month() + 1,
                    1,
                )
                .unwrap_or(NaiveDate::from_ymd_opt(dt.year() + 1, 1, 1).unwrap())
                .signed_duration_since(NaiveDate::from_ymd_opt(dt.year(), dt.month(), 1).unwrap())
                .num_days();
                result.push_str(&days_in_month.to_string());
            }

            // Year
            'L' => {
                let is_leap = NaiveDate::from_ymd_opt(dt.year(), 2, 29).is_some();
                result.push(if is_leap { '1' } else { '0' });
            }
            'o' => result.push_str(&dt.iso_week().year().to_string()),
            'X' => result.push_str(&format!("{:+05}", dt.year())),
            'x' => result.push_str(&format!("{:+05}", dt.iso_week().year())),
            'Y' => result.push_str(&dt.year().to_string()),
            'y' => result.push_str(&format!("{:02}", dt.year() % 100)),

            // Time
            'a' => result.push_str(if dt.hour() < 12 { "am" } else { "pm" }),
            'A' => result.push_str(if dt.hour() < 12 { "AM" } else { "PM" }),
            'B' => {
                // Swatch Internet time
                let seconds = (dt.hour() * 3600 + dt.minute() * 60 + dt.second()) as f64;
                let beats = ((seconds + 3600.0) / 86.4).floor() as i32 % 1000;
                result.push_str(&format!("{:03}", beats));
            }
            'g' => {
                let hour = dt.hour();
                result.push_str(&(if hour == 0 || hour == 12 {
                    12
                } else {
                    hour % 12
                })
                .to_string());
            }
            'G' => result.push_str(&dt.hour().to_string()),
            'h' => {
                let hour = dt.hour();
                result.push_str(&format!(
                    "{:02}",
                    if hour == 0 || hour == 12 {
                        12
                    } else {
                        hour % 12
                    }
                ));
            }
            'H' => result.push_str(&format!("{:02}", dt.hour())),
            'i' => result.push_str(&format!("{:02}", dt.minute())),
            's' => result.push_str(&format!("{:02}", dt.second())),
            'u' => result.push_str(&format!("{:06}", dt.timestamp_subsec_micros())),
            'v' => result.push_str(&format!("{:03}", dt.timestamp_subsec_millis())),

            // Timezone
            'e' => result.push_str(&dt.timezone().name()),
            'I' => result.push('0'), // Daylight saving time (simplified)
            'O' => {
                let offset = dt.offset().fix().local_minus_utc();
                let sign = if offset >= 0 { '+' } else { '-' };
                let offset = offset.abs();
                let hours = offset / 3600;
                let minutes = (offset % 3600) / 60;
                result.push_str(&format!("{}{:02}{:02}", sign, hours, minutes));
            }
            'P' => {
                let offset = dt.offset().fix().local_minus_utc();
                let sign = if offset >= 0 { '+' } else { '-' };
                let offset = offset.abs();
                let hours = offset / 3600;
                let minutes = (offset % 3600) / 60;
                result.push_str(&format!("{}{}:{:02}", sign, hours, minutes));
            }
            'p' => {
                let offset = dt.offset().fix().local_minus_utc();
                if offset == 0 {
                    result.push('Z');
                } else {
                    let sign = if offset >= 0 { '+' } else { '-' };
                    let offset = offset.abs();
                    let hours = offset / 3600;
                    let minutes = (offset % 3600) / 60;
                    if minutes == 0 {
                        result.push_str(&format!("{}{:02}", sign, hours));
                    } else {
                        result.push_str(&format!("{}{}:{:02}", sign, hours, minutes));
                    }
                }
            }
            'T' => result.push_str(&dt.timezone().name()),
            'Z' => result.push_str(&dt.offset().fix().local_minus_utc().to_string()),

            // Full Date/Time
            'c' => result.push_str(&format_php_date(dt, DATE_ISO8601)),
            'r' => result.push_str(&format_php_date(dt, DATE_RFC2822)),
            'U' => result.push_str(&dt.timestamp().to_string()),

            _ => result.push(ch),
        }
    }

    result
}

// ============================================================================
// Date/Time Functions
// ============================================================================

/// checkdate(int $month, int $day, int $year): bool
pub fn php_checkdate(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 3 {
        return Err("checkdate() expects exactly 3 parameters".into());
    }

    let month = get_int_arg(vm, args[0])?;
    let day = get_int_arg(vm, args[1])?;
    let year = get_int_arg(vm, args[2])?;

    let is_valid = month >= 1
        && month <= 12
        && year >= 1
        && year <= 32767
        && NaiveDate::from_ymd_opt(year as i32, month as u32, day as u32).is_some();

    Ok(vm.arena.alloc(Val::Bool(is_valid)))
}

/// date(string $format, ?int $timestamp = null): string
pub fn php_date(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("date() expects 1 or 2 parameters".into());
    }

    let format = String::from_utf8_lossy(&get_string_arg(vm, args[0])?).to_string();

    let timestamp = if args.len() == 2 {
        get_int_arg(vm, args[1])?
    } else {
        Utc::now().timestamp()
    };

    // Use system default timezone (simplified - in real PHP this would use date.timezone ini setting)
    let dt = Local.timestamp_opt(timestamp, 0).unwrap();
    let tz_dt = dt.with_timezone(&Tz::UTC); // Simplified - should use configured timezone

    let formatted = format_php_date(&tz_dt, &format);
    Ok(vm.arena.alloc(Val::String(formatted.into_bytes().into())))
}

/// gmdate(string $format, ?int $timestamp = null): string
pub fn php_gmdate(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("gmdate() expects 1 or 2 parameters".into());
    }

    let format = String::from_utf8_lossy(&get_string_arg(vm, args[0])?).to_string();

    let timestamp = if args.len() == 2 {
        get_int_arg(vm, args[1])?
    } else {
        Utc::now().timestamp()
    };

    let dt = Utc.timestamp_opt(timestamp, 0).unwrap();
    let tz_dt = dt.with_timezone(&Tz::UTC);

    let formatted = format_php_date(&tz_dt, &format);
    Ok(vm.arena.alloc(Val::String(formatted.into_bytes().into())))
}

/// time(): int
pub fn php_time(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if !args.is_empty() {
        return Err("time() expects exactly 0 parameters".into());
    }

    let timestamp = Utc::now().timestamp();
    Ok(vm.arena.alloc(Val::Int(timestamp)))
}

/// microtime(bool $as_float = false): string|float
pub fn php_microtime(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() > 1 {
        return Err("microtime() expects at most 1 parameter".into());
    }

    let as_float = if args.len() == 1 {
        let val = vm.arena.get(args[0]);
        matches!(&val.value, Val::Bool(true) | Val::Int(1))
    } else {
        false
    };

    let now = Utc::now();
    let secs = now.timestamp();
    let usecs = now.timestamp_subsec_micros();

    if as_float {
        let float_time = secs as f64 + (usecs as f64 / 1_000_000.0);
        Ok(vm.arena.alloc(Val::Float(float_time)))
    } else {
        let result = format!("0.{:06} {}", usecs, secs);
        Ok(vm.arena.alloc(Val::String(result.into_bytes().into())))
    }
}

/// mktime(int $hour, ?int $minute = null, ?int $second = null, ?int $month = null, ?int $day = null, ?int $year = null): int|false
pub fn php_mktime(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 6 {
        return Err("mktime() expects 0 to 6 parameters".into());
    }

    let now = Local::now();

    let hour = if !args.is_empty() {
        get_int_arg(vm, args[0])? as u32
    } else {
        now.hour()
    };

    let minute = if args.len() > 1 {
        get_int_arg(vm, args[1])? as u32
    } else {
        now.minute()
    };

    let second = if args.len() > 2 {
        get_int_arg(vm, args[2])? as u32
    } else {
        now.second()
    };

    let month = if args.len() > 3 {
        get_int_arg(vm, args[3])? as u32
    } else {
        now.month()
    };

    let day = if args.len() > 4 {
        get_int_arg(vm, args[4])? as u32
    } else {
        now.day()
    };

    let year = if args.len() > 5 {
        get_int_arg(vm, args[5])? as i32
    } else {
        now.year()
    };

    match NaiveDate::from_ymd_opt(year, month, day) {
        Some(date) => match NaiveTime::from_hms_opt(hour, minute, second) {
            Some(time) => {
                let dt = NaiveDateTime::new(date, time);
                let timestamp = dt.and_utc().timestamp();
                Ok(vm.arena.alloc(Val::Int(timestamp)))
            }
            None => Ok(vm.arena.alloc(Val::Bool(false))),
        },
        None => Ok(vm.arena.alloc(Val::Bool(false))),
    }
}

/// gmmktime(int $hour, ?int $minute = null, ?int $second = null, ?int $month = null, ?int $day = null, ?int $year = null): int|false
pub fn php_gmmktime(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    // Same as mktime but always uses UTC
    php_mktime(vm, args)
}

/// strtotime(string $datetime, ?int $baseTimestamp = null): int|false
pub fn php_strtotime(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("strtotime() expects 1 or 2 parameters".into());
    }

    let datetime_str = String::from_utf8_lossy(&get_string_arg(vm, args[0])?).to_string();

    let _base_timestamp = if args.len() == 2 {
        get_int_arg(vm, args[1])?
    } else {
        Utc::now().timestamp()
    };

    // Simplified implementation - real PHP has very complex parsing
    // Handle common cases
    if datetime_str == "now" {
        return Ok(vm.arena.alloc(Val::Int(Utc::now().timestamp())));
    }

    // Try to parse as ISO format
    if let Ok(dt) = ChronoDateTime::parse_from_rfc3339(&datetime_str) {
        return Ok(vm.arena.alloc(Val::Int(dt.timestamp())));
    }

    // Try common formats
    if let Ok(dt) = NaiveDateTime::parse_from_str(&datetime_str, "%Y-%m-%d %H:%M:%S") {
        return Ok(vm.arena.alloc(Val::Int(dt.and_utc().timestamp())));
    }

    if let Ok(date) = NaiveDate::parse_from_str(&datetime_str, "%Y-%m-%d") {
        return Ok(vm
            .arena
            .alloc(Val::Int(date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp())));
    }

    // Return false for unparseable strings
    Ok(vm.arena.alloc(Val::Bool(false)))
}

/// getdate(?int $timestamp = null): array
pub fn php_getdate(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() > 1 {
        return Err("getdate() expects at most 1 parameter".into());
    }

    let timestamp = if args.len() == 1 {
        get_int_arg(vm, args[0])?
    } else {
        Utc::now().timestamp()
    };

    let dt = Local.timestamp_opt(timestamp, 0).unwrap();

    let mut map = IndexMap::new();
    map.insert(
        make_array_key("seconds"),
        vm.arena.alloc(Val::Int(dt.second() as i64)),
    );
    map.insert(
        make_array_key("minutes"),
        vm.arena.alloc(Val::Int(dt.minute() as i64)),
    );
    map.insert(
        make_array_key("hours"),
        vm.arena.alloc(Val::Int(dt.hour() as i64)),
    );
    map.insert(
        make_array_key("mday"),
        vm.arena.alloc(Val::Int(dt.day() as i64)),
    );
    map.insert(
        make_array_key("wday"),
        vm.arena
            .alloc(Val::Int(dt.weekday().number_from_sunday() as i64)),
    );
    map.insert(
        make_array_key("mon"),
        vm.arena.alloc(Val::Int(dt.month() as i64)),
    );
    map.insert(
        make_array_key("year"),
        vm.arena.alloc(Val::Int(dt.year() as i64)),
    );
    map.insert(
        make_array_key("yday"),
        vm.arena.alloc(Val::Int(dt.ordinal0() as i64)),
    );

    let weekday = match dt.weekday() {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    };
    map.insert(
        make_array_key("weekday"),
        vm.arena.alloc(Val::String(weekday.as_bytes().to_vec().into())),
    );

    let month = match dt.month() {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "",
    };
    map.insert(
        make_array_key("month"),
        vm.arena.alloc(Val::String(month.as_bytes().to_vec().into())),
    );

    map.insert(make_array_key("0"), vm.arena.alloc(Val::Int(timestamp)));

    Ok(vm.arena.alloc(Val::Array(Rc::new(crate::core::value::ArrayData {
        map,
        next_free: 0,
    }))))
}

/// idate(string $format, ?int $timestamp = null): int|false
pub fn php_idate(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 2 {
        return Err("idate() expects 1 or 2 parameters".into());
    }

    let format = String::from_utf8_lossy(&get_string_arg(vm, args[0])?).to_string();
    if format.len() != 1 {
        return Err("idate() format must be exactly one character".into());
    }

    let timestamp = if args.len() == 2 {
        get_int_arg(vm, args[1])?
    } else {
        Utc::now().timestamp()
    };

    let dt = Local.timestamp_opt(timestamp, 0).unwrap();

    let result = match format.chars().next().unwrap() {
        'B' => {
            let seconds = (dt.hour() * 3600 + dt.minute() * 60 + dt.second()) as f64;
            ((seconds + 3600.0) / 86.4).floor() as i64 % 1000
        }
        'd' => dt.day() as i64,
        'h' => {
            let hour = dt.hour();
            (if hour == 0 || hour == 12 {
                12
            } else {
                hour % 12
            }) as i64
        }
        'H' => dt.hour() as i64,
        'i' => dt.minute() as i64,
        'I' => 0, // Simplified
        'L' => {
            if NaiveDate::from_ymd_opt(dt.year(), 2, 29).is_some() {
                1
            } else {
                0
            }
        }
        'm' => dt.month() as i64,
        's' => dt.second() as i64,
        't' => {
            let days_in_month = NaiveDate::from_ymd_opt(dt.year(), dt.month() + 1, 1)
                .unwrap_or(NaiveDate::from_ymd_opt(dt.year() + 1, 1, 1).unwrap())
                .signed_duration_since(NaiveDate::from_ymd_opt(dt.year(), dt.month(), 1).unwrap())
                .num_days();
            days_in_month
        }
        'U' => timestamp,
        'w' => dt.weekday().number_from_sunday() as i64,
        'W' => dt.iso_week().week() as i64,
        'y' => (dt.year() % 100) as i64,
        'Y' => dt.year() as i64,
        'z' => dt.ordinal0() as i64,
        'Z' => dt.offset().fix().local_minus_utc() as i64,
        _ => return Err("idate(): Invalid format character".into()),
    };

    Ok(vm.arena.alloc(Val::Int(result)))
}

/// gettimeofday(bool $as_float = false): array|float
pub fn php_gettimeofday(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() > 1 {
        return Err("gettimeofday() expects at most 1 parameter".into());
    }

    let as_float = if args.len() == 1 {
        let val = vm.arena.get(args[0]);
        matches!(&val.value, Val::Bool(true) | Val::Int(1))
    } else {
        false
    };

    let now = Utc::now();
    let secs = now.timestamp();
    let usecs = now.timestamp_subsec_micros();

    if as_float {
        let float_time = secs as f64 + (usecs as f64 / 1_000_000.0);
        Ok(vm.arena.alloc(Val::Float(float_time)))
    } else {
        let mut map = IndexMap::new();
        map.insert(make_array_key("sec"), vm.arena.alloc(Val::Int(secs)));
        map.insert(
            make_array_key("usec"),
            vm.arena.alloc(Val::Int(usecs as i64)),
        );
        map.insert(
            make_array_key("minuteswest"),
            vm.arena.alloc(Val::Int(0)),
        );
        map.insert(
            make_array_key("dsttime"),
            vm.arena.alloc(Val::Int(0)),
        );

        Ok(vm.arena.alloc(Val::Array(Rc::new(crate::core::value::ArrayData {
            map,
            next_free: 0,
        }))))
    }
}

/// localtime(?int $timestamp = null, bool $associative = false): array
pub fn php_localtime(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() > 2 {
        return Err("localtime() expects at most 2 parameters".into());
    }

    let timestamp = if !args.is_empty() {
        get_int_arg(vm, args[0])?
    } else {
        Utc::now().timestamp()
    };

    let associative = if args.len() == 2 {
        let val = vm.arena.get(args[1]);
        matches!(&val.value, Val::Bool(true) | Val::Int(1))
    } else {
        false
    };

    let dt = Local.timestamp_opt(timestamp, 0).unwrap();

    let mut map = IndexMap::new();

    if associative {
        map.insert(
            make_array_key("tm_sec"),
            vm.arena.alloc(Val::Int(dt.second() as i64)),
        );
        map.insert(
            make_array_key("tm_min"),
            vm.arena.alloc(Val::Int(dt.minute() as i64)),
        );
        map.insert(
            make_array_key("tm_hour"),
            vm.arena.alloc(Val::Int(dt.hour() as i64)),
        );
        map.insert(
            make_array_key("tm_mday"),
            vm.arena.alloc(Val::Int(dt.day() as i64)),
        );
        map.insert(
            make_array_key("tm_mon"),
            vm.arena.alloc(Val::Int((dt.month() - 1) as i64)),
        );
        map.insert(
            make_array_key("tm_year"),
            vm.arena.alloc(Val::Int((dt.year() - 1900) as i64)),
        );
        map.insert(
            make_array_key("tm_wday"),
            vm.arena
                .alloc(Val::Int(dt.weekday().number_from_sunday() as i64)),
        );
        map.insert(
            make_array_key("tm_yday"),
            vm.arena.alloc(Val::Int(dt.ordinal0() as i64)),
        );
        map.insert(
            make_array_key("tm_isdst"),
            vm.arena.alloc(Val::Int(0)),
        );
    } else {
        map.insert(
            make_array_key("0"),
            vm.arena.alloc(Val::Int(dt.second() as i64)),
        );
        map.insert(
            make_array_key("1"),
            vm.arena.alloc(Val::Int(dt.minute() as i64)),
        );
        map.insert(
            make_array_key("2"),
            vm.arena.alloc(Val::Int(dt.hour() as i64)),
        );
        map.insert(
            make_array_key("3"),
            vm.arena.alloc(Val::Int(dt.day() as i64)),
        );
        map.insert(
            make_array_key("4"),
            vm.arena.alloc(Val::Int((dt.month() - 1) as i64)),
        );
        map.insert(
            make_array_key("5"),
            vm.arena.alloc(Val::Int((dt.year() - 1900) as i64)),
        );
        map.insert(
            make_array_key("6"),
            vm.arena
                .alloc(Val::Int(dt.weekday().number_from_sunday() as i64)),
        );
        map.insert(
            make_array_key("7"),
            vm.arena.alloc(Val::Int(dt.ordinal0() as i64)),
        );
        map.insert(make_array_key("8"), vm.arena.alloc(Val::Int(0)));
    }

    Ok(vm.arena.alloc(Val::Array(Rc::new(crate::core::value::ArrayData {
        map,
        next_free: if associative { 0 } else { 9 },
    }))))
}

// ============================================================================
// Timezone Functions
// ============================================================================

/// date_default_timezone_get(): string
pub fn php_date_default_timezone_get(vm: &mut VM, _args: &[Handle]) -> Result<Handle, String> {
    // In a real implementation, this would read from ini settings
    // For now, return UTC
    Ok(vm.arena.alloc(Val::String("UTC".as_bytes().to_vec().into())))
}

/// date_default_timezone_set(string $timezoneId): bool
pub fn php_date_default_timezone_set(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("date_default_timezone_set() expects exactly 1 parameter".into());
    }

    let tz_str = String::from_utf8_lossy(&get_string_arg(vm, args[0])?).to_string();

    // Validate timezone
    match parse_timezone(&tz_str) {
        Ok(_) => Ok(vm.arena.alloc(Val::Bool(true))),
        Err(_) => Ok(vm.arena.alloc(Val::Bool(false))),
    }
}

// ============================================================================
// Sun Functions (Simplified - deprecated in PHP 8.4)
// ============================================================================

/// date_sunrise(int $timestamp, int $returnFormat = SUNFUNCS_RET_STRING, ?float $latitude = null, ?float $longitude = null, ?float $zenith = null, ?float $utcOffset = null): string|int|float|false
pub fn php_date_sunrise(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 6 {
        return Err("date_sunrise() expects 1 to 6 parameters".into());
    }

    // Simplified implementation - just return a fixed sunrise time
    let return_format = if args.len() > 1 {
        get_int_arg(vm, args[1])?
    } else {
        SUNFUNCS_RET_STRING
    };

    match return_format {
        0 => Ok(vm.arena.alloc(Val::Int(1234567890))), // SUNFUNCS_RET_TIMESTAMP
        1 => Ok(vm
            .arena
            .alloc(Val::String("06:00".as_bytes().to_vec().into()))), // SUNFUNCS_RET_STRING
        2 => Ok(vm.arena.alloc(Val::Float(6.0))),                      // SUNFUNCS_RET_DOUBLE
        _ => Ok(vm.arena.alloc(Val::Bool(false))),
    }
}

/// date_sunset(int $timestamp, int $returnFormat = SUNFUNCS_RET_STRING, ?float $latitude = null, ?float $longitude = null, ?float $zenith = null, ?float $utcOffset = null): string|int|float|false
pub fn php_date_sunset(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.is_empty() || args.len() > 6 {
        return Err("date_sunset() expects 1 to 6 parameters".into());
    }

    // Simplified implementation
    let return_format = if args.len() > 1 {
        get_int_arg(vm, args[1])?
    } else {
        SUNFUNCS_RET_STRING
    };

    match return_format {
        0 => Ok(vm.arena.alloc(Val::Int(1234567890))),
        1 => Ok(vm
            .arena
            .alloc(Val::String("18:00".as_bytes().to_vec().into()))),
        2 => Ok(vm.arena.alloc(Val::Float(18.0))),
        _ => Ok(vm.arena.alloc(Val::Bool(false))),
    }
}

/// date_sun_info(int $timestamp, float $latitude, float $longitude): array
pub fn php_date_sun_info(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 3 {
        return Err("date_sun_info() expects exactly 3 parameters".into());
    }

    let _timestamp = get_int_arg(vm, args[0])?;
    let _latitude = get_float_arg(vm, args[1])?;
    let _longitude = get_float_arg(vm, args[2])?;

    // Simplified implementation - return placeholder data
    let mut map = IndexMap::new();
    map.insert(
        make_array_key("sunrise"),
        vm.arena.alloc(Val::Int(1234567890)),
    );
    map.insert(
        make_array_key("sunset"),
        vm.arena.alloc(Val::Int(1234567890)),
    );
    map.insert(
        make_array_key("transit"),
        vm.arena.alloc(Val::Int(1234567890)),
    );
    map.insert(
        make_array_key("civil_twilight_begin"),
        vm.arena.alloc(Val::Int(1234567890)),
    );
    map.insert(
        make_array_key("civil_twilight_end"),
        vm.arena.alloc(Val::Int(1234567890)),
    );
    map.insert(
        make_array_key("nautical_twilight_begin"),
        vm.arena.alloc(Val::Int(1234567890)),
    );
    map.insert(
        make_array_key("nautical_twilight_end"),
        vm.arena.alloc(Val::Int(1234567890)),
    );
    map.insert(
        make_array_key("astronomical_twilight_begin"),
        vm.arena.alloc(Val::Int(1234567890)),
    );
    map.insert(
        make_array_key("astronomical_twilight_end"),
        vm.arena.alloc(Val::Int(1234567890)),
    );

    Ok(vm.arena.alloc(Val::Array(Rc::new(crate::core::value::ArrayData {
        map,
        next_free: 0,
    }))))
}

// ============================================================================
// Date Parsing Functions
// ============================================================================

/// date_parse(string $datetime): array
pub fn php_date_parse(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 1 {
        return Err("date_parse() expects exactly 1 parameter".into());
    }

    let datetime_str = String::from_utf8_lossy(&get_string_arg(vm, args[0])?).to_string();

    // Simplified parsing - in real PHP this is very complex
    let mut map = IndexMap::new();

    // Try to parse and extract components
    if let Ok(dt) = NaiveDateTime::parse_from_str(&datetime_str, "%Y-%m-%d %H:%M:%S") {
        map.insert(
            make_array_key("year"),
            vm.arena.alloc(Val::Int(dt.year() as i64)),
        );
        map.insert(
            make_array_key("month"),
            vm.arena.alloc(Val::Int(dt.month() as i64)),
        );
        map.insert(
            make_array_key("day"),
            vm.arena.alloc(Val::Int(dt.day() as i64)),
        );
        map.insert(
            make_array_key("hour"),
            vm.arena.alloc(Val::Int(dt.hour() as i64)),
        );
        map.insert(
            make_array_key("minute"),
            vm.arena.alloc(Val::Int(dt.minute() as i64)),
        );
        map.insert(
            make_array_key("second"),
            vm.arena.alloc(Val::Int(dt.second() as i64)),
        );
    } else {
        // Return false values
        map.insert(make_array_key("year"), vm.arena.alloc(Val::Bool(false)));
        map.insert(make_array_key("month"), vm.arena.alloc(Val::Bool(false)));
        map.insert(make_array_key("day"), vm.arena.alloc(Val::Bool(false)));
        map.insert(make_array_key("hour"), vm.arena.alloc(Val::Bool(false)));
        map.insert(make_array_key("minute"), vm.arena.alloc(Val::Bool(false)));
        map.insert(make_array_key("second"), vm.arena.alloc(Val::Bool(false)));
    }

    map.insert(
        make_array_key("fraction"),
        vm.arena.alloc(Val::Float(0.0)),
    );
    map.insert(
        make_array_key("warning_count"),
        vm.arena.alloc(Val::Int(0)),
    );
    map.insert(
        make_array_key("warnings"),
        vm.arena.alloc(Val::Array(Rc::new(crate::core::value::ArrayData {
            map: IndexMap::new(),
            next_free: 0,
        }))),
    );
    map.insert(
        make_array_key("error_count"),
        vm.arena.alloc(Val::Int(0)),
    );
    map.insert(
        make_array_key("errors"),
        vm.arena.alloc(Val::Array(Rc::new(crate::core::value::ArrayData {
            map: IndexMap::new(),
            next_free: 0,
        }))),
    );
    map.insert(
        make_array_key("is_localtime"),
        vm.arena.alloc(Val::Bool(false)),
    );

    Ok(vm.arena.alloc(Val::Array(Rc::new(crate::core::value::ArrayData {
        map,
        next_free: 0,
    }))))
}

/// date_parse_from_format(string $format, string $datetime): array
pub fn php_date_parse_from_format(vm: &mut VM, args: &[Handle]) -> Result<Handle, String> {
    if args.len() != 2 {
        return Err("date_parse_from_format() expects exactly 2 parameters".into());
    }

    let _format = String::from_utf8_lossy(&get_string_arg(vm, args[0])?).to_string();
    let _datetime_str = String::from_utf8_lossy(&get_string_arg(vm, args[1])?).to_string();

    // Simplified implementation - return basic structure
    let mut map = IndexMap::new();
    map.insert(ArrayKey::Str(Rc::new("year".as_bytes().to_vec())), vm.arena.alloc(Val::Bool(false)));
    map.insert(ArrayKey::Str(Rc::new("month".as_bytes().to_vec())), vm.arena.alloc(Val::Bool(false)));
    map.insert(ArrayKey::Str(Rc::new("day".as_bytes().to_vec())), vm.arena.alloc(Val::Bool(false)));
    map.insert(ArrayKey::Str(Rc::new("hour".as_bytes().to_vec())), vm.arena.alloc(Val::Bool(false)));
    map.insert(ArrayKey::Str(Rc::new("minute".as_bytes().to_vec())), vm.arena.alloc(Val::Bool(false)));
    map.insert(ArrayKey::Str(Rc::new("second".as_bytes().to_vec())), vm.arena.alloc(Val::Bool(false)));
    map.insert(
        make_array_key("fraction"),
        vm.arena.alloc(Val::Float(0.0)),
    );
    map.insert(
        make_array_key("warning_count"),
        vm.arena.alloc(Val::Int(0)),
    );
    map.insert(
        make_array_key("warnings"),
        vm.arena.alloc(Val::Array(Rc::new(crate::core::value::ArrayData {
            map: IndexMap::new(),
            next_free: 0,
        }))),
    );
    map.insert(
        make_array_key("error_count"),
        vm.arena.alloc(Val::Int(0)),
    );
    map.insert(
        make_array_key("errors"),
        vm.arena.alloc(Val::Array(Rc::new(crate::core::value::ArrayData {
            map: IndexMap::new(),
            next_free: 0,
        }))),
    );

    Ok(vm.arena.alloc(Val::Array(Rc::new(crate::core::value::ArrayData {
        map,
        next_free: 0,
    }))))
}
