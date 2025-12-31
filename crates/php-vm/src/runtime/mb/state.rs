#[derive(Debug, Clone)]
pub struct MbStringState {
    pub internal_encoding: String,
    pub detect_order: Vec<String>,
    pub substitute_char: MbSubstitute,
    pub language: String,
    pub regex_encoding: String,
    pub regex_options: String,
    pub http_input: Option<String>,
    pub http_output: Option<String>,
}

#[derive(Debug, Clone)]
pub enum MbSubstitute {
    Char(char),
    None,
    Long,
}

impl Default for MbStringState {
    fn default() -> Self {
        Self {
            internal_encoding: "UTF-8".to_string(),
            detect_order: vec!["UTF-8".to_string()],
            substitute_char: MbSubstitute::Char('?'),
            language: "neutral".to_string(),
            regex_encoding: "UTF-8".to_string(),
            regex_options: String::new(),
            http_input: None,
            http_output: None,
        }
    }
}
