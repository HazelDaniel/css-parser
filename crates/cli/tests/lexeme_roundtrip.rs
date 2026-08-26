use css_parser_core::lexer::Lexer;
use css_parser_core::reader::Reader;
use css_parser_core::token::TokenKind;
use css_parser_core::types::LexerSpan;
use std::fs::{self, File};
use std::path::PathBuf;
use std::process::Command;

fn lexeme(source: &str, span: LexerSpan) -> &str {
    let LexerSpan(start, cursor) = span;
    if start == cursor && cursor == source.len() {
        return "";
    }

    let end = cursor
        + source[cursor..]
            .chars()
            .next()
            .expect("token cursor must point to a source code point")
            .len_utf8();
    &source[start..end]
}

fn is_css_whitespace(c: char) -> bool {
    matches!(c, '\u{0009}' | '\u{000A}' | '\u{000C}' | '\u{000D}' | ' ')
}

fn is_css_printable(c: char) -> bool {
    !matches!(c, '\u{0000}'..='\u{0008}' | '\u{000B}' | '\u{000E}'..='\u{001F}' | '\u{007F}'..='\u{009F}')
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic() || !c.is_ascii()
}

fn is_name(c: char) -> bool {
    is_ident_start(c) || c.is_ascii_digit() || c == '-'
}

fn is_hex(c: char) -> bool {
    c.is_ascii_hexdigit()
}

fn is_css_newline(c: char) -> bool {
    matches!(c, '\u{000A}' | '\u{000C}' | '\u{000D}')
}

fn consume_escape(chars: &[char], mut index: usize) -> Option<usize> {
    if chars.get(index) != Some(&'\\') {
        return None;
    }
    index += 1;

    let next = *chars.get(index)?;
    if is_css_newline(next) {
        return None;
    }

    if is_hex(next) {
        let mut count = 0;
        while count < 6 && chars.get(index).is_some_and(|c| is_hex(*c)) {
            index += 1;
            count += 1;
        }
        if chars.get(index).is_some_and(|c| is_css_whitespace(*c)) {
            index += 1;
        }
    } else {
        index += 1;
    }

    Some(index)
}

fn consume_string_escape(chars: &[char], index: usize) -> Option<usize> {
    if chars.get(index) != Some(&'\\') {
        return None;
    }

    match chars.get(index + 1) {
        Some('\r') if chars.get(index + 2) == Some(&'\n') => Some(index + 3),
        Some('\n' | '\r' | '\u{000C}') => Some(index + 2),
        Some(_) => consume_escape(chars, index),
        None => None,
    }
}

fn consume_name(chars: &[char], mut index: usize) -> Option<usize> {
    while index < chars.len() {
        if is_name(chars[index]) {
            index += 1;
        } else if let Some(next) = consume_escape(chars, index) {
            index = next;
        } else {
            break;
        }
    }
    Some(index)
}

fn consume_ident(chars: &[char], mut index: usize) -> Option<usize> {
    match chars.get(index) {
        Some('-') => {
            index += 1;
            match chars.get(index) {
                Some('-') => index += 1,
                Some(c) if is_ident_start(*c) => index += 1,
                Some('\\') => index = consume_escape(chars, index)?,
                _ => return None,
            }
        }
        Some(c) if is_ident_start(*c) => index += 1,
        Some('\\') => index = consume_escape(chars, index)?,
        _ => return None,
    }

    consume_name(chars, index)
}

fn is_ident(lexeme: &str) -> bool {
    let chars: Vec<_> = lexeme.chars().collect();
    consume_ident(&chars, 0) == Some(chars.len())
}

fn is_hash(lexeme: &str, id_hash: bool) -> bool {
    let chars: Vec<_> = lexeme.chars().collect();
    if chars.first() != Some(&'#') {
        return false;
    }

    let name_start = if id_hash {
        consume_ident(&chars, 1)
    } else {
        consume_name(&chars, 1)
    };
    name_start == Some(chars.len())
}

fn consume_number(chars: &[char]) -> Option<usize> {
    let mut index = 0;
    if matches!(chars.get(index), Some('+' | '-')) {
        index += 1;
    }

    let digits_start = index;
    while chars.get(index).is_some_and(|c| c.is_ascii_digit()) {
        index += 1;
    }
    let has_integer = index != digits_start;

    let has_fraction =
        chars.get(index) == Some(&'.') && chars.get(index + 1).is_some_and(|c| c.is_ascii_digit());
    if has_fraction {
        index += 1;
        while chars.get(index).is_some_and(|c| c.is_ascii_digit()) {
            index += 1;
        }
    } else if !has_integer {
        return None;
    }

    if matches!(chars.get(index), Some('e' | 'E')) {
        let exponent_start = index;
        index += 1;
        if matches!(chars.get(index), Some('+' | '-')) {
            index += 1;
        }
        let exponent_digits = index;
        while chars.get(index).is_some_and(|c| c.is_ascii_digit()) {
            index += 1;
        }
        if index == exponent_digits {
            index = exponent_start;
        }
    }

    Some(index)
}

fn is_number(lexeme: &str) -> bool {
    let chars: Vec<_> = lexeme.chars().collect();
    consume_number(&chars) == Some(chars.len())
}

fn is_string(lexeme: &str) -> bool {
    let chars: Vec<_> = lexeme.chars().collect();
    let Some(terminal @ ('"' | '\'')) = chars.first().copied() else {
        return false;
    };

    let mut index = 1;
    while index < chars.len() {
        match chars[index] {
            c if c == terminal => return index + 1 == chars.len(),
            '\\' => {
                index = match consume_string_escape(&chars, index) {
                    Some(next) => next,
                    None => return false,
                }
            }
            c if is_css_whitespace(c) && c != ' ' => return false,
            _ => index += 1,
        }
    }
    false
}

fn is_url(lexeme: &str) -> bool {
    let chars: Vec<_> = lexeme.chars().collect();
    if chars.len() < 5
        || !chars[..3]
            .iter()
            .zip(['u', 'r', 'l'])
            .all(|(actual, expected)| actual.eq_ignore_ascii_case(&expected))
        || chars[3] != '('
        || chars.last() != Some(&')')
    {
        return false;
    }

    let mut index = 4;
    let mut has_content = false;
    let mut trailing_whitespace = false;
    while index < chars.len() - 1 {
        match chars[index] {
            '\\' => {
                index = match consume_escape(&chars, index) {
                    Some(next) => next,
                    None => return false,
                };
                has_content = true;
                trailing_whitespace = false;
            }
            c if is_css_whitespace(c) => {
                trailing_whitespace = has_content;
                index += 1;
            }
            c if c == '"' || c == '\'' || c == '(' || !is_css_printable(c) => return false,
            _ if trailing_whitespace => return false,
            _ => {
                has_content = true;
                index += 1;
            }
        }
    }
    true
}

fn is_hex_token(lexeme: &str) -> bool {
    let chars: Vec<_> = lexeme.chars().collect();
    let digits = chars.iter().take_while(|c| is_hex(**c)).count();
    (1..=6).contains(&digits)
        && digits + usize::from(chars.get(digits).is_some_and(|c| is_css_whitespace(*c)))
            == chars.len()
}

fn is_digit_token(lexeme: &str) -> bool {
    !lexeme.is_empty() && lexeme.chars().all(|c| c.is_ascii_digit())
}

fn lexeme_matches_kind(kind: &TokenKind, lexeme: &str) -> bool {
    use TokenKind::*;

    match kind {
        CURLY_OPEN => lexeme == "{",
        CURLY_CLOSE => lexeme == "}",
        PAREN_OPEN => lexeme == "(",
        PAREN_CLOSE => lexeme == ")",
        BRACKET_OPEN => lexeme == "[",
        BRACKET_CLOSE => lexeme == "]",
        SEMICOLON => lexeme == ";",
        COMMA => lexeme == ",",
        COLON => lexeme == ":",
        DOT => lexeme == ".",
        PLUS => lexeme == "+",
        HYPHEN => lexeme == "-",
        STAR => lexeme == "*",
        SLASH => lexeme == "/",
        EQUALS => lexeme == "=",
        GREATER_THAN => lexeme == ">",
        LESS_THAN => lexeme == "<",
        TILDE => lexeme == "~",
        PIPE => lexeme == "|",
        CARET => lexeme == "^",
        DOLLAR => lexeme == "$",
        AMPERSAND => lexeme == "&",
        BANG => lexeme == "!",
        QUESTION => lexeme == "?",
        AT => lexeme == "@",
        PERCENT => lexeme == "%",
        HYPHEN_DOUBLE => lexeme == "--",
        COLON_DOUBLE => lexeme == "::",
        TILDE_EQUAL => lexeme == "~=",
        PIPE_EQUAL => lexeme == "|=",
        CARET_EQUAL => lexeme == "^=",
        DOLLAR_EQUAL => lexeme == "$=",
        STAR_EQUAL => lexeme == "*=",
        CDO => lexeme == "<!--",
        CDC => lexeme == "-->",
        IDENT => is_ident(lexeme),
        FUNCTION => lexeme.ends_with('(') && is_ident(&lexeme[..lexeme.len() - 1]),
        AT_KEYWORD => lexeme.starts_with('@') && is_ident(&lexeme[1..]),
        HASH_TOKEN => is_hash(lexeme, false),
        ID_HASH => is_hash(lexeme, true),
        GENERIC_HASH => is_hash(lexeme, false),
        STRING => is_string(lexeme),
        NUMBER => is_number(lexeme),
        PERCENTAGE => lexeme.strip_suffix('%').is_some_and(is_number),
        DIMENSION => {
            let chars: Vec<_> = lexeme.chars().collect();
            consume_number(&chars).and_then(|index| consume_ident(&chars, index))
                == Some(chars.len())
        }
        WHITESPACE => !lexeme.is_empty() && lexeme.chars().all(is_css_whitespace),
        HEX_TOKEN => is_hex_token(lexeme),
        ESCAPE => {
            let chars: Vec<_> = lexeme.chars().collect();
            consume_escape(&chars, 0) == Some(chars.len())
        }
        URL => is_url(lexeme),
        IMPORTANT_TOKEN => lexeme.eq_ignore_ascii_case("!important"),
        DIGIT_TOKEN => is_digit_token(lexeme),
        DELIM(c) => lexeme.chars().count() == 1 && lexeme.starts_with(*c),
        EOF => lexeme.is_empty(),
        BAD_STRING | BAD_URL => true,
    }
}

#[test]
fn cli_lexemes_reconstruct_the_input_file() {
    // TEST: no lexeme was incompletely consumed / skipped
    let corpus_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus");
    let mut input_paths: Vec<_> = fs::read_dir(&corpus_path)
        .expect("corpus directory should be readable")
        .map(|entry| entry.expect("corpus entry should be readable").path())
        .filter(|path| path.is_file())
        .collect();
    input_paths.sort();

    assert!(
        !input_paths.is_empty(),
        "corpus should contain at least one file"
    );

    for input_path in input_paths {
        let cli = Command::new(env!("CARGO_BIN_EXE_css-parser-cli"))
            .arg(&input_path)
            .output()
            .expect("CLI binary should be runnable");
        assert!(
            cli.status.success(),
            "CLI failed for {}: {}",
            input_path.display(),
            String::from_utf8_lossy(&cli.stderr)
        );

        let mut reader = Reader::new(File::open(&input_path).expect("corpus file should open"));
        let source = reader.read().expect("corpus file should be valid UTF-8");
        let mut lexer = Lexer::new(source);
        let tokens = lexer.scan();

        let mut reconstructed = String::new();
        for token in tokens {
            if matches!(token.kind, TokenKind::EOF) {
                continue;
            }

            let LexerSpan(start, cursor) = token.span;
            let code_point = source[cursor..]
                .chars()
                .next()
                .expect("non-EOF token must point to a source code point");
            let end = cursor + code_point.len_utf8();

            reconstructed.push_str(&source[start..end]);
        }

        assert_eq!(
            reconstructed,
            source,
            "round-trip failed for {}",
            input_path.display()
        );
    }
}

#[test]
fn test_lexeme_meaning_correctness() {
    let corpus_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus");
    let mut input_paths: Vec<_> = fs::read_dir(&corpus_path)
        .expect("corpus directory should be readable")
        .map(|entry| entry.expect("corpus entry should be readable").path())
        .filter(|path| path.is_file())
        .collect();
    input_paths.sort();

    assert!(
        !input_paths.is_empty(),
        "corpus should contain at least one file"
    );

    for input_path in input_paths {
        let mut reader = Reader::new(File::open(&input_path).expect("corpus file should open"));
        let source = reader.read().expect("corpus file should be valid UTF-8");
        let mut lexer = Lexer::new(source);

        for token in lexer.scan() {
            if matches!(token.kind, TokenKind::BAD_STRING | TokenKind::BAD_URL) {
                continue;
            }

            let token_lexeme = lexeme(source, token.span);
            assert!(
                lexeme_matches_kind(&token.kind, token_lexeme),
                "invalid {:?} lexeme {:?} in {}",
                token.kind,
                token_lexeme,
                input_path.display()
            );
        }
    }
}

#[test]
fn token_meaning_predicates_cover_project_helper_tokens() {
    assert!(lexeme_matches_kind(&TokenKind::HEX_TOKEN, "1a2F "));
    assert!(lexeme_matches_kind(&TokenKind::ESCAPE, "\\31 "));
    assert!(lexeme_matches_kind(&TokenKind::DIGIT_TOKEN, "12345"));
    assert!(lexeme_matches_kind(
        &TokenKind::IMPORTANT_TOKEN,
        "!IMPORTANT"
    ));

    assert!(!lexeme_matches_kind(&TokenKind::HEX_TOKEN, "abcdef0"));
    assert!(!lexeme_matches_kind(&TokenKind::ESCAPE, "\\\n"));
    assert!(!lexeme_matches_kind(&TokenKind::DIGIT_TOKEN, "12.5"));
    assert!(!lexeme_matches_kind(
        &TokenKind::IMPORTANT_TOKEN,
        "important"
    ));
}

#[test]
fn token_meaning_predicates_cover_spec_fixtures() {
    let valid = [
        (TokenKind::IDENT, "--custom-property"),
        (TokenKind::FUNCTION, "calc("),
        (TokenKind::AT_KEYWORD, "@media"),
        (TokenKind::HASH_TOKEN, "#123"),
        (TokenKind::ID_HASH, "#main"),
        (TokenKind::GENERIC_HASH, "#123"),
        (TokenKind::STRING, "\"escaped\\\"value\""),
        (TokenKind::NUMBER, "-1.25e+2"),
        (TokenKind::PERCENTAGE, "50%"),
        (TokenKind::DIMENSION, "1.5rem"),
        (TokenKind::WHITESPACE, " \n\t"),
        (TokenKind::URL, "uRl(foo\\20bar )"),
        (TokenKind::CDO, "<!--"),
        (TokenKind::CDC, "-->"),
        (TokenKind::DELIM('@'), "@"),
        (TokenKind::EOF, ""),
    ];

    for (kind, token_lexeme) in valid {
        assert!(
            lexeme_matches_kind(&kind, token_lexeme),
            "expected {:?} to accept {:?}",
            kind,
            token_lexeme
        );
    }

    let invalid = [
        (TokenKind::IDENT, "123custom-property"),
        (TokenKind::FUNCTION, "calc"),
        (TokenKind::AT_KEYWORD, "@"),
        (TokenKind::STRING, "\"unterminated"),
        (TokenKind::NUMBER, "1e+"),
        (TokenKind::PERCENTAGE, "50"),
        (TokenKind::DIMENSION, "1.5"),
        (TokenKind::WHITESPACE, "\u{00A0}"),
        (TokenKind::URL, "url(\"quoted\")"),
        (TokenKind::CDO, "<!---"),
        (TokenKind::CDC, "-->-"),
        (TokenKind::DELIM('@'), "@@"),
        (TokenKind::EOF, "source"),
    ];

    for (kind, token_lexeme) in invalid {
        assert!(
            !lexeme_matches_kind(&kind, token_lexeme),
            "expected {:?} to reject {:?}",
            kind,
            token_lexeme
        );
    }
}
