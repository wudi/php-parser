use encoding_rs::Encoding;

use crate::runtime::mb::encoding::canonical_label;

pub fn decode_bytes(input: &[u8], encoding: &str) -> Result<String, String> {
    let label = canonical_label(encoding).unwrap_or(encoding);
    if label.eq_ignore_ascii_case("UTF-8") {
        return std::str::from_utf8(input)
            .map(|value| value.to_string())
            .map_err(|_| "invalid UTF-8 sequence".to_string());
    }

    let encoding = Encoding::for_label(label.to_ascii_lowercase().as_bytes())
        .ok_or_else(|| format!("unknown encoding: {}", encoding))?;
    let (cow, _, _) = encoding.decode(input);
    Ok(cow.to_string())
}

pub fn encode_string(input: &str, encoding: &str) -> Result<Vec<u8>, String> {
    let label = canonical_label(encoding).unwrap_or(encoding);
    if label.eq_ignore_ascii_case("UTF-8") {
        return Ok(input.as_bytes().to_vec());
    }
    if label.eq_ignore_ascii_case("UTF-16LE") {
        let mut out = Vec::with_capacity(input.len() * 2);
        for unit in input.encode_utf16() {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        return Ok(out);
    }
    if label.eq_ignore_ascii_case("UTF-16BE") {
        let mut out = Vec::with_capacity(input.len() * 2);
        for unit in input.encode_utf16() {
            out.extend_from_slice(&unit.to_be_bytes());
        }
        return Ok(out);
    }

    let encoding = Encoding::for_label(label.to_ascii_lowercase().as_bytes())
        .ok_or_else(|| format!("unknown encoding: {}", encoding))?;
    let (cow, _, _) = encoding.encode(input);
    Ok(cow.into_owned())
}

pub fn convert_bytes(input: &[u8], from: &str, to: &str) -> Result<Vec<u8>, String> {
    let decoded = decode_bytes(input, from)?;
    encode_string(&decoded, to)
}
