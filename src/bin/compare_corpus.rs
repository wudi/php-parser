use bumpalo::Bump;
use php_parser_rs::lexer::Lexer;
use php_parser_rs::lexer::token::TokenKind;
use php_parser_rs::parser::Parser;
use rayon::prelude::*;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <directory>", args[0]);
        std::process::exit(1);
    }

    let dir = &args[1];
    println!("Scanning directory: {}", dir);

    let files = find_php_files(Path::new(dir));
    println!("Found {} PHP files", files.len());

    let total = files.len();
    let processed = AtomicUsize::new(0);
    let both_ok = AtomicUsize::new(0);
    let both_fail = AtomicUsize::new(0);
    let nikic_ok_rust_fail = AtomicUsize::new(0);
    let nikic_fail_rust_ok = AtomicUsize::new(0);
    let token_match = AtomicUsize::new(0);
    let token_mismatch = AtomicUsize::new(0);

    let start = Instant::now();

    files.par_iter().for_each(|path| {
        let code = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
        };

        // 1. Run PHP Script (Check Parse + Get Tokens)
        let output = Command::new("php")
            .arg("tools/comparator/check_file.php")
            .arg(path)
            .output();

        let (nikic_success, php_tokens) = match output {
            Ok(o) if o.status.success() => {
                let tokens: Option<Vec<Value>> = serde_json::from_slice(&o.stdout).ok();
                (true, tokens)
            }
            _ => (false, None),
        };

        // 2. Run Rust Parser
        let bump = Bump::new();
        let lexer = Lexer::new(code.as_bytes());
        let mut parser = Parser::new(lexer, &bump);
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parser.parse_program()));

        let rust_success = match result {
            Ok(program) => program.errors.is_empty(),
            Err(_) => false,
        };

        // 3. Compare Outcomes
        if nikic_success && rust_success {
            both_ok.fetch_add(1, Ordering::Relaxed);

            // 4. Compare Tokens
            if let Some(p_tokens) = php_tokens {
                let lexer = Lexer::new(code.as_bytes());
                let mut r_tokens = Vec::new();
                for token in lexer {
                    if token.kind == TokenKind::Eof {
                        break;
                    }
                    r_tokens.push(token);
                }

                if compare_tokens(&p_tokens, &r_tokens, &code) {
                    token_match.fetch_add(1, Ordering::Relaxed);
                } else {
                    let count = token_mismatch.fetch_add(1, Ordering::Relaxed);
                    if count < 5 {
                        println!("Token Mismatch: {:?}", path);
                    }
                }
            }
        } else if !nikic_success && !rust_success {
            both_fail.fetch_add(1, Ordering::Relaxed);
        } else if nikic_success && !rust_success {
            nikic_ok_rust_fail.fetch_add(1, Ordering::Relaxed);
            println!("FAIL [Nikic OK, Rust FAIL]: {:?}", path);
        } else {
            nikic_fail_rust_ok.fetch_add(1, Ordering::Relaxed);
        }

        let p = processed.fetch_add(1, Ordering::Relaxed) + 1;
        if p.is_multiple_of(50) {
            print!("\rProcessed {}/{} files...", p, total);
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        }
    });

    println!("\n\n--------------------------------------------------");
    println!("Comparison Complete in {:.2?}", start.elapsed());
    println!("Total Files: {}", total);
    println!("Both OK: {}", both_ok.load(Ordering::Relaxed));
    println!("Both Fail: {}", both_fail.load(Ordering::Relaxed));
    println!(
        "Nikic OK, Rust FAIL: {}",
        nikic_ok_rust_fail.load(Ordering::Relaxed)
    );
    println!(
        "Nikic FAIL, Rust OK: {}",
        nikic_fail_rust_ok.load(Ordering::Relaxed)
    );
    println!("Token Match: {}", token_match.load(Ordering::Relaxed));
    println!("Token Mismatch: {}", token_mismatch.load(Ordering::Relaxed));
}

fn compare_tokens(
    p_tokens: &[Value],
    r_tokens: &[php_parser_rs::lexer::token::Token],
    code: &str,
) -> bool {
    let mut p_idx = 0;
    let mut r_idx = 0;

    while p_idx < p_tokens.len() && r_idx < r_tokens.len() {
        let p_tok = &p_tokens[p_idx];

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

            while temp_r_idx < r_tokens.len() {
                let t = r_tokens[temp_r_idx];
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

        let r_tok = r_tokens[r_idx];
        let r_kind = r_tok.kind;
        let r_text = r_tok.span.as_str(code.as_bytes());
        let r_text_str = String::from_utf8_lossy(r_text);

        // Handle T_CLOSE_TAG with newline
        if p_kind == "T_CLOSE_TAG" && r_kind == TokenKind::CloseTag
            && p_text.len() > r_text.len() {
                // PHP token includes trailing newline(s)
                // Check if next Rust token is InlineHtml and matches the difference
                if r_idx + 1 < r_tokens.len() {
                    let next_r_tok = r_tokens[r_idx + 1];
                    if next_r_tok.kind == TokenKind::InlineHtml {
                        let next_r_text = next_r_tok.span.as_str(code.as_bytes());
                        let next_r_text_str = String::from_utf8_lossy(next_r_text);

                        let combined = format!("{}{}", r_text_str, next_r_text_str);
                        if combined == p_text {
                            r_idx += 1; // Skip next token
                            // Update r_text_str for comparison?
                            // Actually, we can just continue to map_kind check,
                            // but we need to be careful.
                            // If we skip, we are effectively merging.
                            // But we need to make sure the current r_kind (CloseTag) matches p_kind (T_CLOSE_TAG).
                            // map_kind(CloseTag) -> T_CLOSE_TAG.
                            // So it will match.
                            // But is_acceptable_mismatch might check text.
                            // We should probably update r_text_str or just let it pass if we verified the combined text.
                        }
                    }
                }
            }

        let r_kind_mapped = map_kind(r_kind);

        if r_kind_mapped != p_kind
            && !is_acceptable_mismatch(r_kind, p_kind, &r_text_str, p_text) {
                return false;
            }

        p_idx += 1;
        r_idx += 1;
    }

    p_idx == p_tokens.len() && r_idx == r_tokens.len()
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
        // TokenKind::Var => "T_VAR".to_string(),
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
    if r_kind == TokenKind::Identifier
        && (p_kind == "T_STRING"
            || p_kind == "T_NAME_QUALIFIED"
            || p_kind == "T_NAME_FULLY_QUALIFIED"
            || p_kind == "T_NAME_RELATIVE")
    {
        return true;
    }

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

    if r_kind == TokenKind::Public && p_kind == "T_VAR" {
        return true;
    }

    if (r_kind == TokenKind::Ampersand
        || r_kind == TokenKind::AmpersandFollowedByVarOrVararg
        || r_kind == TokenKind::AmpersandNotFollowedByVarOrVararg)
        && (p_kind == "&"
            || p_kind == "T_AMPERSAND_FOLLOWED_BY_VAR_OR_VARARG"
            || p_kind == "T_AMPERSAND_NOT_FOLLOWED_BY_VAR_OR_VARARG")
        && r_text == "&" && p_text == "&" {
            return true;
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

fn find_php_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    if dir.is_dir()
        && let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    files.extend(find_php_files(&path));
                } else if path.extension().is_some_and(|ext| ext == "php") {
                    files.push(path);
                }
            }
        }
    files
}
