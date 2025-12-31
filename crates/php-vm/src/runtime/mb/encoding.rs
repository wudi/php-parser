pub fn all_encodings() -> Vec<String> {
    vec!["UTF-8".to_string()]
}

pub fn aliases_for(name: &str) -> Vec<&'static str> {
    match name.to_ascii_uppercase().as_str() {
        "UTF-8" => vec!["UTF-8", "UTF8", "UTF_8"],
        _ => Vec::new(),
    }
}
