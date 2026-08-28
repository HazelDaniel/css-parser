use css_parser_core::lexer::Lexer;
use css_parser_core::parser::{AstPrinter, Parser};
use std::hint::black_box;
use std::time::{Duration, Instant};

const ITERATIONS: usize = 1_000;

const CORPUS: &[(&str, &str)] = &[
    (
        "001_basic-correctness-check.css",
        include_str!("../../../tests/corpus/001_basic-correctness-check.css"),
    ),
    (
        "002_identifiers-and-escapes.css",
        include_str!("../../../tests/corpus/002_identifiers-and-escapes.css"),
    ),
    (
        "003_strings-and-escapes.css",
        include_str!("../../../tests/corpus/003_strings-and-escapes.css"),
    ),
    (
        "004_numeric-tokens.css",
        include_str!("../../../tests/corpus/004_numeric-tokens.css"),
    ),
    (
        "005_url-tokens-and-bad-urls.css",
        include_str!("../../../tests/corpus/005_url-tokens-and-bad-urls.css"),
    ),
    (
        "006_hash-and-at-keywords.css",
        include_str!("../../../tests/corpus/006_hash-and-at-keywords.css"),
    ),
    (
        "007_functions-and-parentheses.css",
        include_str!("../../../tests/corpus/007_functions-and-parentheses.css"),
    ),
    (
        "008_comments-whitespace-and-preprocessing.css",
        include_str!("../../../tests/corpus/008_comments-whitespace-and-preprocessing.css"),
    ),
    (
        "009_delimiters-and-combinators.css",
        include_str!("../../../tests/corpus/009_delimiters-and-combinators.css"),
    ),
    (
        "010_unicode-ranges-and-specials.css",
        include_str!("../../../tests/corpus/010_unicode-ranges-and-specials.css"),
    ),
    (
        "011_edge-cases-and-malformed-tokens.css",
        include_str!("../../../tests/corpus/011_edge-cases-and-malformed-tokens.css"),
    ),
];

fn measure<F>(mut operation: F) -> Duration
where
    F: FnMut(),
{
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        operation();
    }
    start.elapsed()
}

fn lex(source: &str) -> usize {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.scan();
    black_box(tokens.len())
}

fn parse(tokens: &[css_parser_core::token::Token]) -> (usize, usize) {
    let mut parser = Parser::new(tokens);
    let result = parser.parse_stylesheet();
    black_box((result.value.rule_list.len(), result.errors.len()))
}

fn main() {
    println!("iterations per corpus file: {ITERATIONS}");

    for (name, source) in CORPUS {
        let mut lexer = Lexer::new(source);
        let tokens = lexer.scan();
        let token_count = tokens.len();

        let lex_duration = measure(|| {
            black_box(lex(source));
        });
        let parse_duration = measure(|| {
            black_box(parse(tokens));
        });
        let combined_duration = measure(|| {
            let mut lexer = Lexer::new(source);
            let tokens = lexer.scan();
            black_box(parse(tokens));
        });

        let mut parser = Parser::new(tokens);
        let parsed = parser.parse_stylesheet();
        let print_duration = measure(|| {
            black_box(AstPrinter::render(&parsed.value));
        });

        println!(
            "{name}: bytes={} tokens={token_count} lex={:?} parse={:?} combined={:?} print={:?}",
            source.len(),
            lex_duration,
            parse_duration,
            combined_duration,
            print_duration
        );
    }
}
