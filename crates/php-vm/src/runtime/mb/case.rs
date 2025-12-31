pub fn to_lowercase(input: &str) -> String {
    input.chars().flat_map(|ch| ch.to_lowercase()).collect()
}

pub fn to_uppercase(input: &str) -> String {
    input.chars().flat_map(|ch| ch.to_uppercase()).collect()
}

pub fn to_titlecase(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut capitalize = true;
    for ch in input.chars() {
        if ch.is_whitespace() {
            out.push(ch);
            capitalize = true;
            continue;
        }
        if capitalize {
            out.extend(ch.to_uppercase());
            capitalize = false;
        } else {
            out.extend(ch.to_lowercase());
        }
    }
    out
}
