use css_parser_core::lexer::Lexer;
use css_parser_core::reader::{Reader, ReaderError};
use std::env;
use std::fs::File;

fn main() {
    if let Err(error) = run() {
        eprintln!("css-parser-cli: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let path = env::args()
        .nth(1)
        .ok_or_else(|| "missing input file path".to_string())?;

    let file = File::open(&path).map_err(|error| format!("failed to open {path}: {error}"))?;
    let mut reader = Reader::new(file);

    let source = reader.read().map_err(|error| match error {
        ReaderError::Io { kind, message } => {
            format!("failed to read {path} ({kind:?}): {message}")
        },
        ReaderError::InvalidUtf8 {
            valid_up_to,
            error_len,
        } => format!(
            "input {path} is not valid UTF-8 (valid through byte {valid_up_to}, error length {error_len:?})"
        ),
    })?;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.scan();

    for token in tokens {
        println!("{:?} {:?}", token.kind, token.span);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use css_parser_core::token::TokenKind;
    use css_parser_core::types::LexerSpan;
    use std::io::Cursor;

    #[test]
    fn lexer_token_lexemes_reconstruct_the_reader_source() {
        let original = "@media é{color:red;margin:1px}";
        let mut reader = Reader::new(Cursor::new(original.as_bytes()));
        let source = reader.read().unwrap();
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

        assert_eq!(reconstructed, original);
    }
}
