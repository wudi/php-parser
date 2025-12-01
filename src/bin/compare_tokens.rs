use php_parser_rs::lexer::Lexer;
use php_parser_rs::lexer::token::TokenKind;
use serde_json::Value;
use std::env;
use std::fs;
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <file>", args[0]);
        std::process::exit(1);
    }
    let file_path = &args[1];

    // 1. Get PHP Tokens
    let output = Command::new("php")
        .arg("tools/comparator/dump_tokens.php")
        .arg(file_path)
        .output()
        .expect("Failed to run php script");

    if !output.status.success() {
        eprintln!("PHP failed: {}", String::from_utf8_lossy(&output.stderr));
        std::process::exit(1);
    }

    let php_tokens: Vec<Value> =
        serde_json::from_slice(&output.stdout).expect("Failed to parse PHP JSON");

    // 2. Get Rust Tokens
    let code = fs::read_to_string(file_path).expect("Failed to read file");
    let lexer = Lexer::new(code.as_bytes());
    let mut rust_tokens = Vec::new();

    for token in lexer {
        if token.kind == TokenKind::Eof {
            break;
        }
        rust_tokens.push(token);
    }

    // 3. Compare
    let mut p_idx = 0;
    let mut r_idx = 0;
    let mut mismatch = false;

    while p_idx < php_tokens.len() && r_idx < rust_tokens.len() {
        let p_tok = &php_tokens[p_idx];

        let p_kind = p_tok["kind"].as_str().unwrap_or("");
        let p_text = p_tok["text"].as_str().unwrap_or("");

        // Handle PHP Qualified Names (T_NAME_QUALIFIED, etc.)
        if p_kind == "T_NAME_QUALIFIED"
            || p_kind == "T_NAME_FULLY_QUALIFIED"
            || p_kind == "T_NAME_RELATIVE"
        {
            let mut constructed = String::new();
            let mut temp_r_idx = r_idx;
            let mut matched = false;

            while temp_r_idx < rust_tokens.len() {
                let t = rust_tokens[temp_r_idx];
                let t_text = t.span.as_str(code.as_bytes());
                constructed.push_str(&String::from_utf8_lossy(t_text));
                temp_r_idx += 1;

                if constructed == p_text {
                    r_idx = temp_r_idx;
                    p_idx += 1;
                    matched = true;
                    break;
                }
                if constructed.len() > p_text.len() {
                    break;
                }
            }

            if matched {
                continue;
            }
        }

        let r_tok = rust_tokens[r_idx];
        let r_kind = r_tok.kind;
        let r_text = r_tok.span.as_str(code.as_bytes());
        let r_text_str = String::from_utf8_lossy(r_text);

        // Handle T_CLOSE_TAG with newline
        if p_kind == "T_CLOSE_TAG" && r_kind == TokenKind::CloseTag && p_text.len() > r_text.len() {
            // PHP token includes trailing newline(s)
            // Check if next Rust token is InlineHtml and matches the difference
            if r_idx + 1 < rust_tokens.len() {
                let next_r_tok = rust_tokens[r_idx + 1];
                if next_r_tok.kind == TokenKind::InlineHtml {
                    let next_r_text = next_r_tok.span.as_str(code.as_bytes());
                    let next_r_text_str = String::from_utf8_lossy(next_r_text);

                    let combined = format!("{}{}", r_text_str, next_r_text_str);
                    if combined == p_text {
                        r_idx += 1; // Skip next token
                    }
                }
            }
        }

        let r_kind_mapped = map_kind(r_kind);

        if r_kind_mapped != p_kind && !is_acceptable_mismatch(r_kind, p_kind, &r_text_str, p_text) {
            println!(
                "Mismatch at index {}: PHP {} ({}) vs Rust {} ({:?})",
                p_idx, p_kind, p_text, r_kind_mapped, r_kind
            );
            println!("  PHP Text: {:?}", p_text);
            println!("  Rust Text: {:?}", r_text_str);
            mismatch = true;
            break;
        }

        p_idx += 1;
        r_idx += 1;
    }

    if mismatch {
        println!("FAIL: Token mismatch");
    } else if p_idx != php_tokens.len() || r_idx != rust_tokens.len() {
        println!(
            "FAIL: Length mismatch. PHP: {}, Rust: {}",
            php_tokens.len(),
            rust_tokens.len()
        );
    } else {
        println!("OK: Tokens match");
    }
}

fn map_kind(kind: TokenKind) -> String {
    match kind {
        TokenKind::OpenTag => "T_OPEN_TAG".to_string(),
        TokenKind::OpenTagEcho => "T_OPEN_TAG_WITH_ECHO".to_string(),
        TokenKind::CloseTag => "T_CLOSE_TAG".to_string(),
        TokenKind::Exit => "T_EXIT".to_string(),
        TokenKind::Die => "T_EXIT".to_string(),
        TokenKind::Function => "T_FUNCTION".to_string(),
        TokenKind::TypeCallable => "T_CALLABLE".to_string(),
        TokenKind::Array => "T_ARRAY".to_string(),
        TokenKind::Const => "T_CONST".to_string(),
        TokenKind::Return => "T_RETURN".to_string(),
        TokenKind::If => "T_IF".to_string(),
        TokenKind::Else => "T_ELSE".to_string(),
        TokenKind::ElseIf => "T_ELSEIF".to_string(),
        TokenKind::EndIf => "T_ENDIF".to_string(),
        TokenKind::Echo => "T_ECHO".to_string(),
        TokenKind::Print => "T_PRINT".to_string(),
        TokenKind::Do => "T_DO".to_string(),
        TokenKind::While => "T_WHILE".to_string(),
        TokenKind::EndWhile => "T_ENDWHILE".to_string(),
        TokenKind::For => "T_FOR".to_string(),
        TokenKind::EndFor => "T_ENDFOR".to_string(),
        TokenKind::Foreach => "T_FOREACH".to_string(),
        TokenKind::EndForeach => "T_ENDFOREACH".to_string(),
        TokenKind::Declare => "T_DECLARE".to_string(),
        TokenKind::EndDeclare => "T_ENDDECLARE".to_string(),
        TokenKind::As => "T_AS".to_string(),
        TokenKind::Switch => "T_SWITCH".to_string(),
        TokenKind::EndSwitch => "T_ENDSWITCH".to_string(),
        TokenKind::Case => "T_CASE".to_string(),
        TokenKind::Default => "T_DEFAULT".to_string(),
        TokenKind::Break => "T_BREAK".to_string(),
        TokenKind::Continue => "T_CONTINUE".to_string(),
        TokenKind::Goto => "T_GOTO".to_string(),
        TokenKind::Insteadof => "T_INSTEADOF".to_string(),
        TokenKind::New => "T_NEW".to_string(),
        TokenKind::Class => "T_CLASS".to_string(),
        TokenKind::Interface => "T_INTERFACE".to_string(),
        TokenKind::Trait => "T_TRAIT".to_string(),
        TokenKind::Extends => "T_EXTENDS".to_string(),
        TokenKind::Implements => "T_IMPLEMENTS".to_string(),
        TokenKind::Public => "T_PUBLIC".to_string(),
        TokenKind::Protected => "T_PROTECTED".to_string(),
        TokenKind::Private => "T_PRIVATE".to_string(),
        TokenKind::PublicSet => "T_PUBLIC_SET".to_string(),
        TokenKind::ProtectedSet => "T_PROTECTED_SET".to_string(),
        TokenKind::PrivateSet => "T_PRIVATE_SET".to_string(),
        TokenKind::Final => "T_FINAL".to_string(),
        TokenKind::Readonly => "T_READONLY".to_string(),
        TokenKind::Abstract => "T_ABSTRACT".to_string(),
        TokenKind::Static => "T_STATIC".to_string(),
        // TokenKind::Var => "T_VAR".to_string(), // Mapped to Public
        TokenKind::Try => "T_TRY".to_string(),
        TokenKind::Catch => "T_CATCH".to_string(),
        TokenKind::Throw => "T_THROW".to_string(),
        TokenKind::Finally => "T_FINALLY".to_string(),
        TokenKind::List => "T_LIST".to_string(),
        TokenKind::Yield => "T_YIELD".to_string(),
        TokenKind::YieldFrom => "T_YIELD_FROM".to_string(),
        TokenKind::Clone => "T_CLONE".to_string(),
        TokenKind::InstanceOf => "T_INSTANCEOF".to_string(),
        TokenKind::Use => "T_USE".to_string(),
        TokenKind::Global => "T_GLOBAL".to_string(),
        TokenKind::Isset => "T_ISSET".to_string(),
        TokenKind::Empty => "T_EMPTY".to_string(),
        TokenKind::Include => "T_INCLUDE".to_string(),
        TokenKind::IncludeOnce => "T_INCLUDE_ONCE".to_string(),
        TokenKind::Require => "T_REQUIRE".to_string(),
        TokenKind::RequireOnce => "T_REQUIRE_ONCE".to_string(),
        TokenKind::Eval => "T_EVAL".to_string(),
        TokenKind::Unset => "T_UNSET".to_string(),
        TokenKind::Namespace => "T_NAMESPACE".to_string(),
        TokenKind::HaltCompiler => "T_HALT_COMPILER".to_string(),
        TokenKind::Dir => "T_DIR".to_string(),
        TokenKind::File => "T_FILE".to_string(),
        TokenKind::Line => "T_LINE".to_string(),
        TokenKind::FuncC => "T_FUNC_C".to_string(),
        TokenKind::ClassC => "T_CLASS_C".to_string(),
        TokenKind::TraitC => "T_TRAIT_C".to_string(),
        TokenKind::MethodC => "T_METHOD_C".to_string(),
        TokenKind::NsC => "T_NS_C".to_string(),
        TokenKind::LNumber => "T_LNUMBER".to_string(),
        TokenKind::DNumber => "T_DNUMBER".to_string(),
        TokenKind::StringLiteral => "T_CONSTANT_ENCAPSED_STRING".to_string(),
        TokenKind::Variable => "T_VARIABLE".to_string(),
        TokenKind::InlineHtml => "T_INLINE_HTML".to_string(),
        TokenKind::EncapsedAndWhitespace => "T_ENCAPSED_AND_WHITESPACE".to_string(),
        TokenKind::DollarOpenCurlyBraces => "T_DOLLAR_OPEN_CURLY_BRACES".to_string(),
        TokenKind::CurlyOpen => "T_CURLY_OPEN".to_string(),
        TokenKind::StartHeredoc => "T_START_HEREDOC".to_string(),
        TokenKind::EndHeredoc => "T_END_HEREDOC".to_string(),
        TokenKind::NsSeparator => "T_NS_SEPARATOR".to_string(),
        TokenKind::Ellipsis => "T_ELLIPSIS".to_string(),
        TokenKind::Coalesce => "T_COALESCE".to_string(),
        TokenKind::Pow => "T_POW".to_string(),
        TokenKind::Spaceship => "T_SPACESHIP".to_string(),
        TokenKind::DoubleArrow => "T_DOUBLE_ARROW".to_string(),
        TokenKind::DoubleColon => "T_DOUBLE_COLON".to_string(),
        TokenKind::Inc => "T_INC".to_string(),
        TokenKind::Dec => "T_DEC".to_string(),
        TokenKind::PlusEq => "T_PLUS_EQUAL".to_string(),
        TokenKind::MinusEq => "T_MINUS_EQUAL".to_string(),
        TokenKind::MulEq => "T_MUL_EQUAL".to_string(),
        TokenKind::DivEq => "T_DIV_EQUAL".to_string(),
        TokenKind::ConcatEq => "T_CONCAT_EQUAL".to_string(),
        TokenKind::ModEq => "T_MOD_EQUAL".to_string(),
        TokenKind::AndEq => "T_AND_EQUAL".to_string(),
        TokenKind::OrEq => "T_OR_EQUAL".to_string(),
        TokenKind::XorEq => "T_XOR_EQUAL".to_string(),
        TokenKind::SlEq => "T_SL_EQUAL".to_string(),
        TokenKind::SrEq => "T_SR_EQUAL".to_string(),
        TokenKind::PowEq => "T_POW_EQUAL".to_string(),
        TokenKind::CoalesceEq => "T_COALESCE_EQUAL".to_string(),
        TokenKind::AmpersandAmpersand => "T_BOOLEAN_AND".to_string(),
        TokenKind::PipePipe => "T_BOOLEAN_OR".to_string(),
        TokenKind::LogicalAnd => "T_LOGICAL_AND".to_string(),
        TokenKind::LogicalOr => "T_LOGICAL_OR".to_string(),
        TokenKind::LogicalXor => "T_LOGICAL_XOR".to_string(),
        TokenKind::Sl => "T_SL".to_string(),
        TokenKind::Sr => "T_SR".to_string(),
        TokenKind::EqEq => "T_IS_EQUAL".to_string(),
        TokenKind::BangEq => "T_IS_NOT_EQUAL".to_string(),
        TokenKind::EqEqEq => "T_IS_IDENTICAL".to_string(),
        TokenKind::BangEqEq => "T_IS_NOT_IDENTICAL".to_string(),
        TokenKind::LtEq => "T_IS_SMALLER_OR_EQUAL".to_string(),
        TokenKind::GtEq => "T_IS_GREATER_OR_EQUAL".to_string(),
        TokenKind::Arrow => "T_OBJECT_OPERATOR".to_string(),
        TokenKind::NullSafeArrow => "T_NULLSAFE_OBJECT_OPERATOR".to_string(),
        TokenKind::Comment => "T_COMMENT".to_string(),
        TokenKind::DocComment => "T_DOC_COMMENT".to_string(),
        TokenKind::Identifier => "T_STRING".to_string(),
        TokenKind::NumString => "T_NUM_STRING".to_string(),
        TokenKind::StringVarname => "T_STRING_VARNAME".to_string(),
        TokenKind::Attribute => "T_ATTRIBUTE".to_string(),
        TokenKind::Enum => "T_ENUM".to_string(),
        TokenKind::Match => "T_MATCH".to_string(),
        TokenKind::Fn => "T_FN".to_string(),
        TokenKind::AmpersandFollowedByVarOrVararg => {
            "T_AMPERSAND_FOLLOWED_BY_VAR_OR_VARARG".to_string()
        }
        TokenKind::AmpersandNotFollowedByVarOrVararg => {
            "T_AMPERSAND_NOT_FOLLOWED_BY_VAR_OR_VARARG".to_string()
        }

        // Single char tokens
        TokenKind::SemiColon => ";".to_string(),
        TokenKind::Colon => ":".to_string(),
        TokenKind::Comma => ",".to_string(),
        TokenKind::Dot => ".".to_string(),
        TokenKind::OpenBracket => "[".to_string(),
        TokenKind::CloseBracket => "]".to_string(),
        TokenKind::OpenParen => "(".to_string(),
        TokenKind::CloseParen => ")".to_string(),
        TokenKind::Pipe => "|".to_string(),
        TokenKind::Caret => "^".to_string(),
        TokenKind::Ampersand => "&".to_string(),
        TokenKind::Plus => "+".to_string(),
        TokenKind::Minus => "-".to_string(),
        TokenKind::Asterisk => "*".to_string(),
        TokenKind::Slash => "/".to_string(),
        TokenKind::Percent => "%".to_string(),
        TokenKind::Bang => "!".to_string(),
        TokenKind::BitNot => "~".to_string(),
        TokenKind::Eq => "=".to_string(),
        TokenKind::Gt => ">".to_string(),
        TokenKind::Lt => "<".to_string(),
        TokenKind::At => "@".to_string(),
        TokenKind::Question => "?".to_string(),
        TokenKind::Backtick => "`".to_string(),
        TokenKind::OpenBrace => "{".to_string(),
        TokenKind::CloseBrace => "}".to_string(),
        TokenKind::Dollar => "$".to_string(),
        TokenKind::DoubleQuote => "\"".to_string(),

        // Fallback
        _ => format!("{:?}", kind),
    }
}

fn is_acceptable_mismatch(r_kind: TokenKind, p_kind: &str, r_text: &str, p_text: &str) -> bool {
    // PHP 8.0+ treats namespaced names as T_NAME_QUALIFIED etc, but token_get_all might return T_STRING sequence
    // We need to check if we can map them.

    // Identifier vs Keyword: PHP might return T_STRING for "bool" in some contexts?
    // Actually token_get_all returns T_STRING for "bool" if it's not a type hint?

    if r_kind == TokenKind::Identifier
        && (p_kind == "T_STRING"
            || p_kind == "T_NAME_QUALIFIED"
            || p_kind == "T_NAME_FULLY_QUALIFIED"
            || p_kind == "T_NAME_RELATIVE")
    {
        return true;
    }

    // Type hints
    if matches!(
        r_kind,
        TokenKind::TypeBool
            | TokenKind::TypeInt
            | TokenKind::TypeFloat
            | TokenKind::TypeString
            | TokenKind::TypeObject
            | TokenKind::TypeVoid
            | TokenKind::TypeIterable
            | TokenKind::TypeMixed
            | TokenKind::TypeNever
            | TokenKind::TypeNull
            | TokenKind::TypeFalse
            | TokenKind::TypeTrue
    ) && p_kind == "T_STRING"
    {
        return true;
    }

    // Casts
    if matches!(
        r_kind,
        TokenKind::IntCast
            | TokenKind::FloatCast
            | TokenKind::StringCast
            | TokenKind::ArrayCast
            | TokenKind::ObjectCast
            | TokenKind::BoolCast
            | TokenKind::UnsetCast
    ) && (p_kind == "T_INT_CAST"
        || p_kind == "T_DOUBLE_CAST"
        || p_kind == "T_STRING_CAST"
        || p_kind == "T_ARRAY_CAST"
        || p_kind == "T_OBJECT_CAST"
        || p_kind == "T_BOOL_CAST"
        || p_kind == "T_UNSET_CAST")
    {
        return true;
    }

    // Var keyword
    if r_kind == TokenKind::Public && p_kind == "T_VAR" {
        return true;
    }

    // Ampersand variations
    if (r_kind == TokenKind::Ampersand
        || r_kind == TokenKind::AmpersandFollowedByVarOrVararg
        || r_kind == TokenKind::AmpersandNotFollowedByVarOrVararg)
        && (p_kind == "&"
            || p_kind == "T_AMPERSAND_FOLLOWED_BY_VAR_OR_VARARG"
            || p_kind == "T_AMPERSAND_NOT_FOLLOWED_BY_VAR_OR_VARARG")
    {
        // PHP < 8.1 returns "&" always? Or depends on version.
        // Let's allow mismatch here if text matches "&"
        if r_text == "&" && p_text == "&" {
            return true;
        }
    }

    if p_kind == "T_STRING" && r_text == p_text {
        return true;
    }

    if r_kind == TokenKind::DocComment && p_kind == "T_COMMENT" {
        return true;
    }

    if r_kind == TokenKind::LNumber && p_kind == "T_DNUMBER" && r_text == p_text {
        return true;
    }

    false
}
