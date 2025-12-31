const ENCODINGS: &[&str] = &[
    "UTF-8",
    "ISO-8859-1",
    "ISO-8859-2",
    "ISO-8859-3",
    "ISO-8859-4",
    "ISO-8859-5",
    "ISO-8859-6",
    "ISO-8859-7",
    "ISO-8859-8",
    "ISO-8859-9",
    "ISO-8859-10",
    "ISO-8859-13",
    "ISO-8859-14",
    "ISO-8859-15",
    "ISO-8859-16",
    "Windows-1252",
    "CP1251",
    "CP1252",
    "CP1254",
    "CP1257",
    "SJIS",
    "SJIS-win",
    "EUC-JP",
    "JIS",
    "ISO-2022-JP",
    "EUC-KR",
    "UCS-2",
    "UCS-2LE",
    "UCS-2BE",
    "UCS-4",
    "UCS-4LE",
    "UCS-4BE",
    "UTF-16",
    "UTF-16LE",
    "UTF-16BE",
    "UTF-32",
    "UTF-32LE",
    "UTF-32BE",
    "ASCII",
];

pub fn all_encodings() -> &'static [&'static str] {
    ENCODINGS
}

pub fn canonical_label(name: &str) -> Option<&'static str> {
    let normalized = name.trim().to_ascii_uppercase().replace('_', "-");

    for enc in ENCODINGS {
        if normalized == enc.to_ascii_uppercase() {
            return Some(*enc);
        }
    }

    match normalized.as_str() {
        "UTF8" => Some("UTF-8"),
        "ISO8859-1" | "LATIN1" | "L1" => Some("ISO-8859-1"),
        "SHIFT-JIS" | "SJIS" | "SJIS-WIN" | "CP932" | "WINDOWS-31J" => Some("SJIS"),
        "US-ASCII" => Some("ASCII"),
        "UTF16" => Some("UTF-16"),
        "UTF16LE" => Some("UTF-16LE"),
        "UTF16BE" => Some("UTF-16BE"),
        "UTF32" => Some("UTF-32"),
        "UTF32LE" => Some("UTF-32LE"),
        "UTF32BE" => Some("UTF-32BE"),
        _ => None,
    }
}

pub fn aliases_for(name: &str) -> Vec<&'static str> {
    let normalized = name.trim().to_ascii_uppercase().replace('_', "-");

    match normalized.as_str() {
        "UTF-8" => vec!["UTF-8", "UTF8", "UTF_8"],
        "ISO-8859-1" | "LATIN1" | "L1" => vec!["ISO-8859-1", "ISO8859-1", "ISO_8859-1", "LATIN1", "L1"],
        "SJIS" | "SHIFT-JIS" | "SJIS-WIN" | "CP932" | "WINDOWS-31J" => {
            vec!["SJIS", "SHIFT_JIS", "SHIFT-JIS", "SJIS-win", "CP932", "Windows-31J"]
        }
        "ASCII" | "US-ASCII" => vec!["ASCII", "US-ASCII"],
        "UTF-16" => vec!["UTF-16", "UTF16", "UTF_16"],
        "UTF-16LE" => vec!["UTF-16LE", "UTF16LE", "UTF_16LE"],
        "UTF-16BE" => vec!["UTF-16BE", "UTF16BE", "UTF_16BE"],
        "UTF-32" => vec!["UTF-32", "UTF32", "UTF_32"],
        "UTF-32LE" => vec!["UTF-32LE", "UTF32LE", "UTF_32LE"],
        "UTF-32BE" => vec!["UTF-32BE", "UTF32BE", "UTF_32BE"],
        _ => Vec::new(),
    }
}

pub fn is_valid_encoding(input: &[u8], encoding: &str) -> Result<bool, String> {
    let label = canonical_label(encoding).unwrap_or(encoding);
    if label.eq_ignore_ascii_case("UTF-8") {
        return Ok(std::str::from_utf8(input).is_ok());
    }

    let encoding = Encoding::for_label(label.to_ascii_lowercase().as_bytes())
        .ok_or_else(|| format!("unknown encoding: {}", encoding))?;
    let (_, _, had_errors) = encoding.decode(input);
    Ok(!had_errors)
}
use encoding_rs::Encoding;
