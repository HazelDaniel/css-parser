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

    print!("{source}");

    Ok(())
}
