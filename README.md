# CSS Parser

A generic, syntax-oriented CSS parser written in Rust for common CSS syntax.

This project is intended to cover simple, practical CSS cases rather than implement
the entire CSS specification. It parses source into a syntax tree, preserves source
spans, reports errors, and exposes a command-line driver for inspection.

## Workspace layout

The repository is a Cargo workspace with a reusable library crate and a separate CLI:

```text
crates/
├── core/              # css-parser-core library
│   └── src/
│       ├── lexer.rs   # CSS tokenization
│       ├── parser.rs  # stylesheet AST, parser, visitors, and printer
│       ├── reader.rs  # file reading and UTF-8 decoding
│       ├── selector.rs # selector AST and parser
│       ├── token.rs   # token kinds and source spans
│       └── types.rs   # shared span types
└── cli/               # css-parser-cli executable
    └── src/main.rs
```

The core crate does not depend on the CLI. Applications can use the library directly,
while the CLI provides a simple way to run the lexer and parser against a file.

## Current capabilities

- CSS tokenization with source spans and line information.
- Identifiers, escapes, strings, numbers, dimensions, percentages, hashes, URLs,
  functions, comments, whitespace, delimiters, and malformed-token handling.
- Qualified rules, style blocks, declarations, generic at-rules, and component values.
- Structured selector parsing for common cases:
  - type, universal, class, and ID selectors;
  - attribute selectors;
  - descendant, child, sibling, and column combinators;
  - pseudo-classes and pseudo-elements;
  - selector lists and parser diagnostics.
- Recursive-descent parsing with error recovery and source-span diagnostics.
- Visitor-based AST traversal using Rust enums and pattern matching.
- Tree-style AST output through `AstPrinter`.
- UTF-8 file reading through the CLI reader flow.

## Running the CLI

Build and run the parser with a CSS file path:

```sh
cargo run -p css-parser-cli -- path/to/input.css
```

The CLI reads the file, tokenizes it, parses the stylesheet, prints the semantic AST
as an indented tree, and reports parse errors on standard error.

## Library usage

The reusable API is provided by `css-parser-core`. A typical pipeline is:

```rust
use css_parser_core::lexer::Lexer;
use css_parser_core::parser::{AstPrinter, Parser};

let mut lexer = Lexer::new(source);
let tokens = lexer.scan();
let mut parser = Parser::new(tokens);
let result = parser.parse_stylesheet();

println!("{}", AstPrinter::render(&result.value));
```

The parser result contains both the generated AST and a collection of parse errors.
Qualified rules retain their raw prelude for source-oriented consumers, while the
default AST printer displays the structured selector tree without duplicating that
raw prelude.

## Testing and checks

Run the complete workspace test suite:

```sh
cargo test --workspace
```

Run the same quality checks used by the project CI script:

```sh
./scripts/ci-check.sh
```

The repository includes unit tests for lexer consumers, selectors, parser recovery,
reader behavior, AST traversal, and source-span/lexeme round trips. CSS fixtures live
under `tests/corpus` and are used by integration tests.

## Scope and limitations

This is a syntax-oriented parser, not a CSS styling engine. It currently does not:

- evaluate expressions or CSS functions;
- apply the cascade, inheritance, or computed-style rules;
- validate property-specific value grammars;
- resolve URLs, variables, or external resources;
- implement every CSS at-rule or selector feature;
- support CSS nesting as a committed feature;
- provide layout, rendering, or browser-engine behavior.

Unknown or unsupported syntax is generally retained as component values where the
grammar permits it, and diagnostics are collected for malformed input.

## Development direction

Planned work is expected to focus on structured at-rules, broader selector coverage,
structured declaration values, mathematical expressions, stronger recovery behavior,
and expanded corpus/fuzz testing. The README should be updated whenever a feature is
added, removed, or diverges from this stated scope.
