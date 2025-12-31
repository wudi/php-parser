pub fn str_width(input: &str) -> usize {
    input.chars().count()
}

pub fn trim_width(input: &str, start: i64, width: usize, marker: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len() as i64;
    let mut start_idx = if start < 0 { len + start } else { start };
    if start_idx < 0 {
        start_idx = 0;
    }
    if start_idx >= len {
        return String::new();
    }

    let start_idx = start_idx as usize;
    let end_idx = (start_idx + width).min(chars.len());
    let mut result: String = chars[start_idx..end_idx].iter().collect();
    let trimmed = end_idx < chars.len();
    if trimmed && !marker.is_empty() {
        result.pop();
        result.push_str(marker);
    }
    result
}
