use std::collections::HashMap;
use crate::errors::{LexerErrorReason, LEX_ERROR_MAP};

pub mod token;
pub mod lexer;
pub mod errors;
pub mod types;

fn main() {
    LEX_ERROR_MAP.get_or_init(|| {
        HashMap::from([
            (LexerErrorReason::INVALID_TOKEN, "invalid token"),
            (LexerErrorReason::UNTERMINATED_TOKEN, "unterminated token"),
            (LexerErrorReason::INVARIANT_VIOLATION, "invariant violation"),
            (LexerErrorReason::EOF, "unexpected end of file"),
        ])
    });
    
    println!("Hello, From Core");
}
