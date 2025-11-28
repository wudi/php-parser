use php_parser_rs::lexer::Lexer;
use php_parser_rs::lexer::token::TokenKind;
use std::process::Command;
use std::str;

fn get_php_tokens(code: &str) -> Vec<(String, String)> {
    // Escape single quotes and backslashes for PHP single-quoted string
    let code_escaped = code.replace("\\", "\\\\").replace("'", "\\'");
    let php_code = format!(
        "echo json_encode(array_map(function($t) {{ return is_array($t) ? [token_name($t[0]), $t[1]] : ['CHAR', $t]; }}, token_get_all('{}')));",
        code_escaped
    );

    let output = Command::new("php")
        .arg("-r")
        .arg(&php_code)
        .output()
        .expect("Failed to execute PHP");

    if !output.status.success() {
        panic!("PHP execution failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    let json = String::from_utf8(output.stdout).unwrap();
    println!("PHP Output: {}", json);
    let tokens: Vec<(String, String)> = serde_json::from_str(&json).expect("Failed to parse JSON");
    tokens
}

fn get_php_tokens_from_file(path: &str) -> Vec<(String, String)> {
    let php_code = format!(
        "echo json_encode(array_map(function($t) {{ return is_array($t) ? [token_name($t[0]), $t[1]] : ['CHAR', $t]; }}, token_get_all(file_get_contents('{}'))));",
        path
    );

    let output = Command::new("php")
        .arg("-r")
        .arg(&php_code)
        .output()
        .expect("Failed to execute PHP");

    if !output.status.success() {
        panic!("PHP execution failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    let json = String::from_utf8(output.stdout).unwrap();
    let tokens: Vec<(String, String)> = serde_json::from_str(&json).expect("Failed to parse JSON");
    tokens
}

fn map_kind_to_php(kind: TokenKind, _text: &str) -> &'static str {
    match kind {
        TokenKind::Function => "T_FUNCTION",
        TokenKind::Class => "T_CLASS",
        TokenKind::Interface => "T_INTERFACE",
        TokenKind::Trait => "T_TRAIT",
        TokenKind::Enum => "T_ENUM",
        TokenKind::Extends => "T_EXTENDS",
        TokenKind::Implements => "T_IMPLEMENTS",
        TokenKind::If => "T_IF",
        TokenKind::Else => "T_ELSE",
        TokenKind::ElseIf => "T_ELSEIF",
        TokenKind::Return => "T_RETURN",
        TokenKind::Echo => "T_ECHO",
        TokenKind::Print => "T_PRINT",
        TokenKind::While => "T_WHILE",
        TokenKind::Do => "T_DO",
        TokenKind::For => "T_FOR",
        TokenKind::Foreach => "T_FOREACH",
        TokenKind::As => "T_AS",
        TokenKind::Switch => "T_SWITCH",
        TokenKind::Case => "T_CASE",
        TokenKind::Default => "T_DEFAULT",
        TokenKind::Break => "T_BREAK",
        TokenKind::Continue => "T_CONTINUE",
        TokenKind::Goto => "T_GOTO",
        TokenKind::Try => "T_TRY",
        TokenKind::Catch => "T_CATCH",
        TokenKind::Finally => "T_FINALLY",
        TokenKind::Throw => "T_THROW",
        TokenKind::Public => "T_PUBLIC",
        TokenKind::Protected => "T_PROTECTED",
        TokenKind::Private => "T_PRIVATE",
        TokenKind::Static => "T_STATIC",
        TokenKind::Abstract => "T_ABSTRACT",
        TokenKind::Final => "T_FINAL",
        TokenKind::Readonly => "T_READONLY",
        TokenKind::Namespace => "T_NAMESPACE",
        TokenKind::Use => "T_USE",
        TokenKind::Global => "T_GLOBAL",
        TokenKind::New => "T_NEW",
        TokenKind::Clone => "T_CLONE",
        TokenKind::InstanceOf => "T_INSTANCEOF",
        TokenKind::Array => "T_ARRAY",
        TokenKind::Const => "T_CONST",
        TokenKind::Include => "T_INCLUDE",
        TokenKind::IncludeOnce => "T_INCLUDE_ONCE",
        TokenKind::Require => "T_REQUIRE",
        TokenKind::RequireOnce => "T_REQUIRE_ONCE",
        TokenKind::Eval => "T_EVAL",
        TokenKind::Exit => "T_EXIT",
        TokenKind::Empty => "T_EMPTY",
        TokenKind::Isset => "T_ISSET",
        TokenKind::Unset => "T_UNSET",
        TokenKind::List => "T_LIST",
        TokenKind::Yield => "T_YIELD",
        TokenKind::YieldFrom => "T_YIELD_FROM",
        TokenKind::Declare => "T_DECLARE",
        TokenKind::Match => "T_MATCH",
        TokenKind::HaltCompiler => "T_HALT_COMPILER",
        TokenKind::Attribute => "T_ATTRIBUTE",
        TokenKind::Line => "T_LINE",
        TokenKind::File => "T_FILE",
        TokenKind::Dir => "T_DIR",
        TokenKind::ClassC => "T_CLASS_C",
        TokenKind::TraitC => "T_TRAIT_C",
        TokenKind::MethodC => "T_METHOD_C",
        TokenKind::FuncC => "T_FUNC_C",
        TokenKind::NsC => "T_NS_C",
        TokenKind::Identifier => "T_STRING",
        TokenKind::LNumber => "T_LNUMBER",
        TokenKind::DNumber => "T_DNUMBER",
        TokenKind::StringLiteral => "T_CONSTANT_ENCAPSED_STRING",
        TokenKind::NumString => "T_NUM_STRING",
        TokenKind::Variable => "T_VARIABLE",
        TokenKind::InlineHtml => "T_INLINE_HTML",
        TokenKind::EncapsedAndWhitespace => "T_ENCAPSED_AND_WHITESPACE",
        TokenKind::DollarOpenCurlyBraces => "T_DOLLAR_OPEN_CURLY_BRACES",
        TokenKind::CurlyOpen => "T_CURLY_OPEN",
        TokenKind::DoubleQuote => "CHAR", // " is a char in PHP token_get_all unless part of string
        TokenKind::Backtick => "CHAR", // ` is a char
        TokenKind::StartHeredoc => "T_START_HEREDOC",
        TokenKind::EndHeredoc => "T_END_HEREDOC",
        TokenKind::Dollar => "CHAR", // $ is char if not variable
        TokenKind::NsSeparator => "T_NS_SEPARATOR",
        TokenKind::Comment => "T_COMMENT",
        TokenKind::DocComment => "T_DOC_COMMENT",
        TokenKind::Arrow => "T_OBJECT_OPERATOR",
        TokenKind::NullSafeArrow => "T_NULLSAFE_OBJECT_OPERATOR",
        TokenKind::DoubleArrow => "T_DOUBLE_ARROW",
        TokenKind::DoubleColon => "T_DOUBLE_COLON",
        TokenKind::Ellipsis => "T_ELLIPSIS",
        TokenKind::Pow => "T_POW",
        TokenKind::Inc => "T_INC",
        TokenKind::Dec => "T_DEC",
        TokenKind::PlusEq => "T_PLUS_EQUAL",
        TokenKind::MinusEq => "T_MINUS_EQUAL",
        TokenKind::MulEq => "T_MUL_EQUAL",
        TokenKind::DivEq => "T_DIV_EQUAL",
        TokenKind::ModEq => "T_MOD_EQUAL",
        TokenKind::ConcatEq => "T_CONCAT_EQUAL",
        TokenKind::PowEq => "T_POW_EQUAL",
        TokenKind::AndEq => "T_AND_EQUAL",
        TokenKind::OrEq => "T_OR_EQUAL",
        TokenKind::XorEq => "T_XOR_EQUAL",
        TokenKind::SlEq => "T_SL_EQUAL",
        TokenKind::SrEq => "T_SR_EQUAL",
        TokenKind::CoalesceEq => "T_COALESCE_EQUAL",
        TokenKind::EqEq => "T_IS_EQUAL",
        TokenKind::EqEqEq => "T_IS_IDENTICAL",
        TokenKind::BangEq => "T_IS_NOT_EQUAL",
        TokenKind::BangEqEq => "T_IS_NOT_IDENTICAL",
        TokenKind::LtEq => "T_IS_SMALLER_OR_EQUAL",
        TokenKind::GtEq => "T_IS_GREATER_OR_EQUAL",
        TokenKind::Spaceship => "T_SPACESHIP",
        TokenKind::Sl => "T_SL",
        TokenKind::Sr => "T_SR",
        TokenKind::AmpersandAmpersand => "T_BOOLEAN_AND",
        TokenKind::PipePipe => "T_BOOLEAN_OR",
        TokenKind::LogicalOr => "T_LOGICAL_OR",
        TokenKind::LogicalXor => "T_LOGICAL_XOR",
        TokenKind::LogicalAnd => "T_LOGICAL_AND",
        TokenKind::Coalesce => "T_COALESCE",
        TokenKind::IntCast => "T_INT_CAST",
        TokenKind::FloatCast => "T_DOUBLE_CAST",
        TokenKind::StringCast => "T_STRING_CAST",
        TokenKind::ArrayCast => "T_ARRAY_CAST",
        TokenKind::ObjectCast => "T_OBJECT_CAST",
        TokenKind::BoolCast => "T_BOOL_CAST",
        TokenKind::UnsetCast => "T_UNSET_CAST",
        TokenKind::At => "CHAR",
        
        // Type hints
        TokenKind::TypeBool => "T_STRING",
        TokenKind::TypeInt => "T_STRING",
        TokenKind::TypeFloat => "T_STRING",
        TokenKind::TypeString => "T_STRING",
        TokenKind::TypeObject => "T_STRING",
        TokenKind::TypeCallable => "T_CALLABLE",
        TokenKind::TypeIterable => "T_ITERABLE", // PHP 7.1+
        TokenKind::TypeVoid => "T_STRING", // void is not a token in PHP lexer?
        // Actually void, bool, int etc are T_STRING in lexer, but parser handles them.
        // But callable IS a token T_CALLABLE.
        
        TokenKind::TypeMixed => "T_STRING",
        TokenKind::TypeNever => "T_STRING",
        TokenKind::TypeNull => "T_STRING",
        TokenKind::TypeFalse => "T_STRING",
        TokenKind::TypeTrue => "T_STRING",
        
        // Single chars
        TokenKind::SemiColon => "CHAR",
        TokenKind::Colon => "CHAR",
        TokenKind::Comma => "CHAR",
        TokenKind::OpenBrace => "CHAR", // { or T_CURLY_OPEN?
        TokenKind::CloseBrace => "CHAR", // }
        TokenKind::OpenParen => "CHAR",
        TokenKind::CloseParen => "CHAR",
        TokenKind::OpenBracket => "CHAR",
        TokenKind::CloseBracket => "CHAR",
        TokenKind::Plus => "CHAR",
        TokenKind::Minus => "CHAR",
        TokenKind::Asterisk => "CHAR",
        TokenKind::Slash => "CHAR",
        TokenKind::Percent => "CHAR",
        TokenKind::Dot => "CHAR",
        TokenKind::Eq => "CHAR",
        TokenKind::Bang => "CHAR",
        TokenKind::Lt => "CHAR",
        TokenKind::Gt => "CHAR",
        TokenKind::Ampersand => "CHAR",
        TokenKind::AmpersandFollowedByVarOrVararg => "T_AMPERSAND_FOLLOWED_BY_VAR_OR_VARARG",
        TokenKind::AmpersandNotFollowedByVarOrVararg => "T_AMPERSAND_NOT_FOLLOWED_BY_VAR_OR_VARARG",
        TokenKind::Pipe => "CHAR",
        TokenKind::Caret => "CHAR",
        TokenKind::BitNot => "CHAR",
        TokenKind::Question => "CHAR",
        
        // Tags
        TokenKind::OpenTag => "T_OPEN_TAG",
        TokenKind::OpenTagEcho => "T_OPEN_TAG_WITH_ECHO",
        TokenKind::CloseTag => "T_CLOSE_TAG",
        
        _ => "UNKNOWN",
    }
}

#[test]
fn test_lexer_compliance() {
    let cases = vec![
        "<?php echo $a;",
        "<?php $a = 1 + 2;",
        "<?php function foo() { return 1; }",
        "<?php \"hello $name\";",
        "<?php <<<EOT\nhello\nEOT;",
        "<?php 0x1A; 0b11; 1.5; 1e10;",
        "<?php // comment\n/* comment */",
        "<?php /** doc */",
        "<?php (int)$a;",
        "<?php $a?->b;",
        "<?php #[Attribute]",
        "<?php match($a) { 1 => 2 };",
        "<?php enum Status { case Draft; }",
        "<?php __halt_compiler(); data",
        "<?php \n    <<<END\n    content\n    END;",
        "<?php    $a = <<<EOT\nThis is a heredoc string.\nEOT.'ending';\necho $a;",
    ];

    for code in cases {
        println!("Testing: {}", code);
        let php_tokens = get_php_tokens(code);
        
        let lexer = Lexer::new(code.as_bytes());
        let mut rust_tokens = Vec::new();
        
        for token in lexer {
            if token.kind == TokenKind::Eof {
                break;
            }
            let text = &code[token.span.start..token.span.end];
            let php_name = map_kind_to_php(token.kind, text);
            
            // Special handling for chars
            let name = if php_name == "CHAR" {
                "CHAR".to_string()
            } else {
                php_name.to_string()
            };
            
            rust_tokens.push((name, text.to_string()));
        }
        
        // Compare
        // Note: PHP token_get_all might return whitespace tokens which we skip?
        // My lexer skips whitespace in `next()` unless it's EncapsedAndWhitespace or InlineHtml.
        // PHP returns T_WHITESPACE.
        
        let php_tokens_filtered: Vec<(String, String)> = php_tokens.into_iter()
            .filter(|(name, _)| name != "T_WHITESPACE")
            .collect();
            
        // Also my lexer might produce different tokens for some things.
        // e.g. OpenBrace vs T_CURLY_OPEN.
        
        // Let's just print them for now to see.
        println!("PHP: {:?}", php_tokens_filtered);
        println!("Rust: {:?}", rust_tokens);
        
        assert_eq!(rust_tokens.len(), php_tokens_filtered.len(), "Token count mismatch for: {}", code);
    }
}

#[test]
fn test_run_tests_php() {
    let path = "/Users/eagle/Sourcecode/php-src/run-tests.php";
    println!("Testing file: {}", path);
    
    let php_tokens = get_php_tokens_from_file(path);
    let code = std::fs::read_to_string(path).expect("Failed to read file");
    
    let lexer = Lexer::new(code.as_bytes());
    let mut rust_tokens = Vec::new();
    
    let mut count = 0;
    for token in lexer {
        count += 1;
        if count % 1000 == 0 {
            // println!("Lexed {} tokens", count);
        }
        if token.kind == TokenKind::Eof {
            break;
        }
        if token.span.end <= token.span.start && token.kind != TokenKind::Eof {
             println!("Last tokens:");
             let start_idx = if rust_tokens.len() > 20 { rust_tokens.len() - 20 } else { 0 };
             for t in &rust_tokens[start_idx..] {
                 println!("{:?}", t);
             }
             panic!("Lexer stuck or empty token at {}: {:?}", token.span.start, token.kind);
        }
        let text = &code[token.span.start..token.span.end];
        let php_name = map_kind_to_php(token.kind, text);
        
        let name = if php_name == "CHAR" {
            "CHAR".to_string()
        } else {
            php_name.to_string()
        };
        
        rust_tokens.push((name, text.to_string()));
    }
    
    let php_tokens_filtered: Vec<(String, String)> = php_tokens.into_iter()
        .filter(|(name, _)| name != "T_WHITESPACE")
        .collect();

    let rust_tokens_filtered: Vec<(String, String)> = rust_tokens.into_iter()
        // Rust lexer skips whitespace, so we don't need to filter it out, 
        // but we should ensure we don't produce any "UNKNOWN" that maps to whitespace?
        // map_kind_to_php doesn't produce T_WHITESPACE.
        .collect();

    // Compare
    let min_len = std::cmp::min(php_tokens_filtered.len(), rust_tokens_filtered.len());
    for i in 0..min_len {
        if php_tokens_filtered[i] != rust_tokens_filtered[i] {
            println!("Mismatch at index {}:", i);
            println!("PHP: {:?}", php_tokens_filtered[i]);
            println!("Rust: {:?}", rust_tokens_filtered[i]);
            
            // Print context
            let start = if i > 5 { i - 5 } else { 0 };
            let end = if i + 5 < min_len { i + 5 } else { min_len };
            println!("Context PHP: {:?}", &php_tokens_filtered[start..end]);
            println!("Context Rust: {:?}", &rust_tokens_filtered[start..end]);
            
            panic!("Token mismatch");
        }
    }
    
    assert_eq!(php_tokens_filtered.len(), rust_tokens_filtered.len(), "Token count mismatch");
}
