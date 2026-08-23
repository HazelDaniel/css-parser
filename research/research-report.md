# Summary Report

A robust CSS parser typically separates **tokenization** from **grammar**. Tokenization produces a stream of CSS tokens (identifiers, numbers, strings, symbols, etc.) as defined by the CSS Syntax specification. The tokens are then assembled into a parse tree (AST) of **component values**, rules, selectors, declarations, and values. In practice, major engines use *handwritten recursive-descent* parsers guided by the W3C CSS Syntax algorithms (e.g. Blink’s CSSParserImpl, Mozilla/Servo’s Rust `cssparser`). Some tools and libraries instead use *PEG (packrat)* grammars, which allow unlimited lookahead and backtracking at the cost of memoization (extra memory). 

In comparing approaches:

- **Recursive descent (LL)**: Easy to hand-write, good error-control, low memory, but may require grammar rewriting (no left-recursion) and can backtrack unexpectedly. Well-suited to CSS’s mostly deterministic syntax.
- **Packrat (PEG)**: Defines grammar in PEG notation, with memoization to guarantee linear parse time. Backtracking is automatic and grammars can be expressed more naturally, but memo tables can consume O(input·grammar) memory. Packrat parsers normally disallow left-recursion unless extended.
- **LR (LALR, GLR)**: Uses parser generators (e.g. Bison). Supports left-recursion and all CFGs, but error messages are harder, and standalone generators may struggle with CSS’s context-sensitive parts (like custom properties and error recovery rules). 

Memory use vs performance: PEG/packrat does extra work to avoid re-parsing (memo tables), whereas recursive descent often re-parses subtrees on backtracking. CSS parsers usually prioritize *throughput* (e.g. Blink measures CSSParserImpl performance) and *error tolerance* (skip bad rules) over formal grammar purity. Performance-critical parsers (e.g. high-speed parsers like Project Wallace’s) use **arena allocators** or fixed buffers to avoid allocations, even at cost of simpler grammar checks.

**Lexer design** also matters. Many CSS parsers integrate lexing and parsing (see Servo’s approach: it builds “component values” on the fly without storing raw tokens). Others separate them cleanly (Tab Atkins’s `parse-css.js` uses `tokenize()` then parse rules). Separate lexers can simplify parser code at the cost of an intermediate token stream, whereas combined scanning can save passes.

**CSS-specific syntax** imposes requirements: 
- **Selectors:** support type, class, ID, attribute selectors, combinators (`>`, `+`, `~`, etc.), and nested selectors. Grammars are recursive (combinators form left-recursive patterns, usually hand-coded as loops).
- **Values:** numbers, dimensions, percentages, colors (with legacy `rgb(...)` and modern `rgb(255 0 0 / 50%)`), URLs, functions (`calc()`, `var()`, `min()`, etc.) and arithmetic. Functions are parsed as an identifier followed by a parenthesized argument list of component values.
- **Lists:** Many properties allow space- or comma-separated lists (e.g. `margin: 10px 20px;` or `font-family: Arial, sans-serif;`). The parser must distinguish whitespace-separated vs comma-separated sequences.
- **At-rules:** `@media`, `@supports`, `@import`, `@keyframes`, `@font-face`, `@container`, `@layer`, `@property`, etc. Each has its own prelude grammar (usually bracketed blocks or specific tokens) and is parsed by a specialized routine.
- **Nesting:** Modern CSS (Nesting Module) allows rule blocks within other rule blocks. This changes the grammar: rule bodies can contain nested **qualified rules** as well as declarations.
- **Custom properties (`--var`) and `var()`:** The parser treats `--foo` like any identifier in a declaration, but the value of a custom property can be any token stream. Parsing `var()` follows `<function> ::= "var(" <ident> ["," <any-value>]? ")"`. 

**Error recovery:** CSS is *forgiving*. According to the spec, encountering invalid tokens or syntax should trigger skipping to the next appropriate delimiting semicolon or brace, not abort the entire parse. Parsers typically implement a panic mode: on error, skip tokens until matching `}` or `;` and resume. This keeps parsing the rest of the stylesheet (e.g. Blink’s CSSParserImpl and lexbor’s parser emphasize error tolerance). 

**AST design:** Many CSS parsers use *generic AST nodes* (stylesheets, rules, declarations, selector nodes, expression trees) rather than property-specific nodes. For example, Tab Atkins’s parser builds generic CSSOM structures from grammar rules. A common approach is to first parse into a tree of “component values” (tokens, functions, blocks), then separately interpret selectors and property values if needed. Generic ASTs enable handling unknown or future CSS features without parser changes.

**Implementation language tradeoffs (C vs Rust):** 
- **C:** High performance and control, but manual memory management is error-prone. Parsers in C (e.g. Lexbor, libcroco) often use arena allocators or pools for AST nodes. Error checking and concurrency must be coded carefully. However, C integrates smoothly into browser engines (Blink/CSSParserImpl).
- **Rust:** Memory-safe, rich type system (enums/ADTs, pattern matching) makes grammar modeling elegant (as seen in Servo’s `cssparser` and `css_parse` crates). Ownership semantics help avoid leaks, and Rust crates (e.g. `cssparser`, `selectors`) exist. Performance can approach C, but lifetime management (e.g. using `bumpalo` for AST) is often needed to avoid excessive allocations. Rust’s tooling (cargo, rustfmt, clippy) and concurrency support are superior. The main drawback is interop: exposing a Rust API to C/C++ can require FFI overhead.

**Roadmap (“80% CSS parser”):** 
1. **Lexer/Tokenizer:** Implement CSS tokenization (ident, number, string, at-keywords, symbols) per the CSS Syntax spec. Return tokens with types and raw data.
2. **Top-level parser:** Parse a **stylesheet** as a list of rules and at-rules (qualified rules vs at-rules) using the spec’s “consume a list of rules” algorithm.
3. **Rule parsing:** For each rule, parse selectors (for qualified rules) or at-rule prelude. Build AST nodes for rules and at-rules (including nested statements after CSS Nesting).
4. **Declaration parser:** In rule bodies and at-rules that allow declarations, parse declarations (`property: value[!important]`), separated by `;`. Support custom-property syntax `--foo: <any-value>`.
5. **Value parsing:** Parse property values via generic grammar: numbers, dimensions, percentages, strings, colors, URLs, and **functions** with arguments (handle nested functions). Support list separators (`,`). A sub-parser for `<calc()>` and arithmetic operators (`+ - * /`) can be implemented either as a grammar or by building an expression tree.
6. **Selectors:** Parse selectors with grammar for type/class/id, attribute selectors (`[attr=val]`), combinators, pseudo-classes, pseudo-elements. Often done via mutual recursion: parse a sequence of simple selector sequences separated by combinators.
7. **At-rules:** Add common at-rules parsing (`@media`, `@supports`, `@keyframes`, `@font-face`, etc.). Each at-rule has a known “top-level block grammar” to parse its contents.
8. **Nesting:** Integrate the CSS Nesting Module: allow nested qualified rules inside rule bodies (e.g. `&:hover { ... }`). This requires that while parsing declarations inside a rule, you also check for `{` that starts a nested rule rather than a declaration.
9. **Error recovery:** Implement sync behavior: on error in declarations, skip to next `;` or closing `}`; in selectors, skip to `,` or end-of-rule. Report errors but continue parsing.
10. **AST and semantic pass:** Once syntax parsing is done, implement utilities to interpret or transform the AST (e.g. resolve `var()` by a separate pass if needed, validate selector specificity, etc.).

**Comparison of Parser Types:**  

| Feature                   | Packrat (PEG)      | Recursive-Descent (LL)         | LR (LALR, etc.)             |
|---------------------------|--------------------|-------------------------------|-----------------------------|
| **Performance** (speed)   | Guaranteed O(n)    | Often O(n), worst O(n^k) if backtracking | Guaranteed O(n)            |
| **Memory**                | High (memo tables, O(n·grammar)) | Low (stack & call overhead)  | Moderate (parse stack)     |
| **Implementation** Ease   | Write PEG grammar; use packrat engine | Write parse functions by hand | Define grammar; use generator |
| **Error Reporting**       | Usually fair (customizable)  | Good (custom errors)      | Poor (generic errors)      |
| **Incremental Parsing**   | Hard (memo invalidation) | Easier (reparse changed subtree) | Complex (no natural API)  |
| **Left-Recursion**        | No (unless special algorithm) | Must be rewritten       | Native support             |
| **CSS Constructs Fit**    | Can express any deterministic grammar easily, but needs left-recursion workaround | Can encode CSS selectors/values; must eliminate indirect left recursion | Supports all, but grammar must be CFG; mixed contexts may be tricky |

For most CSS projects, **recursive-descent** is common (Mozilla’s `css_parse` crate uses it), often with manual backtracking via conditional checks (`peek` or `try_parse`) rather than full PEG memoization. Packrat is attractive for its backtracking convenience, but for a browser or formatter performance, the memory cost can be too high.

### CSS Lexer vs Scanner-Parser

CSS lexing rules are spelled out in the CSS Syntax spec. A separate **lexer/tokenizer** can implement those (e.g. Blink’s `CSSTokenizer` or Servo’s approach). Alternatively, a combined scanner-parser can process characters one-by-one and group them into tokens on the fly. Servo’s `rust-cssparser` actually tokenizes and builds “component values” in one pass. A separate lexer simplifies grammar rules (parser deals only with tokens) and enables easier streaming. The combined approach can reduce data copying and slightly improve speed.

### Selector Parsing Example

A typical recursive-descent snippet for selectors (pseudo-code):

```rust
// Simplified Rust-style pseudocode for selectors
fn parse_selector_list(input: &mut Parser) -> Result<Vec<Selector>, Error> {
    let mut selectors = vec![parse_selector(input)?];
    while input.try_eat(Token::Comma) {
        selectors.push(parse_selector(input)?);
    }
    Ok(selectors)
}

fn parse_selector(input: &mut Parser) -> Result<Selector, Error> {
    let mut parts = Vec::new();
    // parse first simple selector
    parts.push(parse_simple_selector(input)?);
    // parse combinators and subsequent parts
    while let Some(comb) = input.peek_combinator() {
        input.consume(); // e.g. '>', '+', '~', or whitespace
        let next = parse_simple_selector(input)?;
        parts.push((comb, next));
    }
    Ok(Selector(parts))
}
```

In **C**, a similar approach uses functions like `parse_selector()` that call `parse_simple_selector()` and loop on combinator tokens. Error recovery might synchronize on commas or `{`.

### Declaration/Value Parsing Example (calc() and var())

Pseudocode for parsing values with functions (generic parser idea):

```rust
fn parse_value(input: &mut Parser) -> Result<Value, Error> {
    if let Some(number) = input.try_parse_number() {
        return Ok(Value::Number(number));
    }
    if let Some(ident) = input.try_parse_ident() {
        // Could be a keyword or function
        if input.peek_token() == Token::LeftParen {
            input.consume(); // consume '('
            let mut args = Vec::new();
            if input.peek_token() != Token::RightParen {
                args.push(parse_value(input)?);
                while input.eat(Token::Comma) {
                    args.push(parse_value(input)?);
                }
            }
            input.expect(Token::RightParen)?;
            return Ok(Value::Function(ident, args));
        }
        return Ok(Value::Ident(ident));
    }
    // handle other cases: strings, hex colors, etc.
    Err(Error::UnexpectedToken)
}
```

For `calc()`, one could embed a small expression parser inside the function arguments, treating `+,-,*,/` with correct precedence. For example:

```c
// C-style pseudocode (recursive descent for calc)
Value *parse_calc(Parser *p) {
    match_token(p, FUNCTION("calc")); // consumed 'calc('
    Expr *expr = parse_calc_expr(p);
    match_token(p, ')');
    return expr_to_value(expr);  // convert arithmetic expr to CSS value AST
}
```

### At-Rules and Nesting Example

```c
// Pseudocode for parsing at-rules and nested rules
void parse_rule_list(Parser *p) {
    while (!peek(p, EOF)) {
        if (peek(p, AT_KEYWORD)) {
            parse_at_rule(p);
        } else {
            parse_qualified_rule(p);
        }
    }
}

void parse_qualified_rule(Parser *p) {
    SelectorList *sel = parse_selector_list(p);
    match(p, TOKEN_LEFT_BRACE);
    // inside block, allow declarations or nested rules
    while (!peek(p, TOKEN_RIGHT_BRACE) && !peek(p, EOF)) {
        if (peek(p, IDENT) || peek(p, DASH_IDENT) || peek(p, "--")) {
            parse_declaration(p);
        } else {
            // Nested rule
            parse_qualified_rule(p);
        }
    }
    match(p, TOKEN_RIGHT_BRACE);
}
```

Note that in a nesting-aware grammar, when parsing inside a block, encountering an identifier at block start could either be a declaration name or a nested selector (if it is actually an ampersand or similar). Tools like PostCSS implement such logic (though Sass-style nesting is simpler, CSS native nesting is limited to `&` syntax).

### Error Recovery Strategies

On syntax errors, skip tokens until a safe point. For example, in declaration lists: if parsing a declaration fails, consume tokens until `;` or `}`. In selector lists, skip to next comma or block start. Blink’s parser and others adopt a “discard until known delimiter” policy as per the spec’s *Consume* algorithms. This allows continuing parsing of the rest of the stylesheet.

### AST Design (Generic vs Specific)

A generic AST might look like (Mermaid diagram):

```mermaid
graph TB
    Stylesheet --> RuleList
    RuleList --> Rule
    Rule --> SelectorList
    Rule --> DeclarationList
    DeclarationList --> Declaration
    Declaration --> PropertyName
    Declaration --> Value
    Value --> Number | Dimension | Percent | String | Function | Ident | Hash | Color | URL | ValueList
    Function --> FunctionName
    Function --> ArgList
    ArgList --> Value
    SelectorList --> Selector
    Selector --> CompoundSelector
    CompoundSelector --> TypeSelector
    CompoundSelector --> ClassSelector
    CompoundSelector --> IDSelector
    CompoundSelector --> PseudoClass
    CompoundSelector --> PseudoElement
```

Each node carries source positions and possibly raw text for reserialization. Generic nodes (Function, ValueList) let unknown CSS or future syntax slip through without parser changes.


While a specific AST might use this design:

A practical AST schema for CSS might include node types such as:

| Node Type       | Structure / Fields                                  | Example Content                 |
|-----------------|-----------------------------------------------------|---------------------------------|
| **Stylesheet**  | `children: List<RuleOrAtRule>`                      | Top-level list of rules         |
| **AtRule**      | `name: string`, `prelude: List<Component>`, `block: Block?` | e.g. `@media`, with condition tokens and nested stylesheet/block |
| **Rule**        | `selectors: List<Selector>`, `declarations: List<Declaration>`, `nestedRules: List<Rule>` | e.g. a style rule `.foo > a {...}`, possibly with nested rules |
| **Selector**    | e.g. a sequence or tree representing combinators, simple selectors | See below table                 |
| **Declaration** | `property: string`, `value: Value`, `important: bool` | e.g. `color: blue !important`    |
| **Value**       | (abstract) could be subclassed as Number, Dimension, Percentage, String, Color, URL, Function, Identifier, or BinaryExpression (for `calc`) | e.g. `10px`, `"#fff"`, `rgb(1,2,3)`, or an expression tree for `calc` |
| **Function**    | `name: string`, `arguments: List<Value or ValueList>` | e.g. `calc(100% - 2rem)`, `rgb(255,0,0)` |
| **ValueList**   | `items: List<Value or List<Value>>`, `separator: ',' or ' '` | e.g. `10px 5px` (space-list) or `red, green` (comma-list) |
| **SelectorCompound** | `elements: List<SimpleSelector>`, `combinator: string?` | A chain like `.a.bar:hover` or just `*`. |
| **SimpleSelector** | variant of `TypeSelector(name)` / `ClassSelector(name)` / `IDSelector(name)` / `AttrSelector(attr, op, value)` / `PseudoClass(name)` / `PseudoElement(name)` | e.g. `.title`, `#id`, `[foo="bar"]`, `:hover`, `::before` |
| **Combinator** | Usually encoded in structure (e.g. `Parent > Child` has combinator `>` between child and parent selectors) | e.g. child combinator (`>`), descendant (implicit), etc. |

We would represent the AST in a structured way (e.g. enums or classes). For example, a `Selector` node might contain a list of `SelectorCompound` connected by combinators, or a tree where each `SelectorCompound` has an associated combinator. This separation allows the parser to match selectors and retain their structure. 

*Example:* The CSS rule `body .card > a:hover { margin: 0; }` could produce an AST roughly like:

```
Stylesheet
 └── Rule
     ├─ selectors: [
     │     Compound([Type("body")]), 
     │     Compound([Class("card")]), 
     │     Combinator("child"), Compound([Type("a"), Pseudo(":hover")])
     │   ]
     └─ declarations: [
           Declaration(property="margin", value=ValueList([Dimension(0,"px"), Dimension(0,"px")]), important=false)
       ]
```

(Precise AST shapes will depend on implementation; we expect table above to guide design.)

### C vs Rust: Recommendations

- **C Architecture:** Use a recursive-descent style with structs for AST nodes. Allocate nodes from an **arena** or pool allocator for speed and locality. For example, Lexbor’s CSS parser uses a custom pool and state machine. Error-handling macros (`goto error`) can implement recovery. For debugging, use verbose logging or assert checks; unit-test with a CSS test suite (W3C tests). FFI: expose a C API (e.g. return a `Stylesheet*` handle) for integration.

- **Rust Architecture:** Use enums and structs for AST (e.g. `enum Value { Number(f64), Function(Name, Vec<Value>), ... }`). Use Rust’s `Result`/`Option` for error management. For memory, use [`bumpalo`](https://crates.io/crates/bumpalo) or `RefCell<Vec<_>>` for arenas to avoid many small heap allocations (similar to `css_parse` crate’s use of `bumpalo`). Concurrency: parsing itself is serial, but multiple stylesheets can be parsed in parallel with threads. Tooling: use `cargo test` and maybe fuzzers. Use Rust crates (like [`cssparser`](https://docs.rs/cssparser/) for low-level parsing, [`selectors`](https://docs.rs/selectors/) for selector matching).


### Performance and Benchmarking Plan  

To ensure the parser is efficient, we recommend:  

- **Benchmark Metrics:**  
  - *Throughput:* CSS bytes parsed per second.  
  - *Latency:* Time to parse a stylesheet (end-to-end).  
  - *Memory:* Peak allocations during parse (especially important for packrat).  
  - *Incremental Update Cost:* Time to re-parse after a small edit (if supporting incremental).  
  - *Warm vs Cold Parse:* First-parse vs cached-grammar reuse.  

- **Corpus:** Use a diverse set of real-world CSS:  
  - Popular frameworks: Bootstrap, Tailwind, etc.  
  - Large sites’ CSS (e.g. via WPT or Archive.org snapshots).  
  - CSS from GitHub repos (open-source websites).  
  - Edge cases: synthetically generated deep nesting, long selectors, huge calc expressions.  

- **Tools:**  
  - Write benchmarks in the target language (e.g. Cargo bench for Rust, Google Benchmark for C).  
  - Use a profiler (e.g. `perf`, Instruments, or Rust’s `cargo flamegraph`) to find hot spots (lexer vs parser vs AST building).  
  - Measure memory via tools (Massif, or Rust’s allocator tracing).  

- **Comparison Baseline:** If possible, compare against an existing parser (e.g. browser engine or another library) to gauge relative speed. Also test multi-threaded parsing of multiple files.  

- **Continuous Testing:** Integrate performance tests into CI to catch regressions.  


# Implementation Roadmap  

We suggest an incremental roadmap, each with rough effort levels:

1. **Core Lexer and Basic Parser (High effort):**  
   - Implement tokenizer for CSS input (Unicode-aware).  
   - Parse simple rules (selectors + declarations) into AST.  
   - Handle basic values (numbers, lengths, colors, simple functions).  
   - *Outcome:* Able to parse minimal CSS like `body { margin:0; }`.  
2. **Advanced Selectors and Declarations (Medium):**  
   - Extend selector grammar (attribute selectors, pseudo-classes, combinators).  
   - Implement all declaration value types (dimension, percentage, string, identifier, `url()`, `var()`).  
   - Support `!important`.  
   - *Outcome:* Covers most rule-level CSS (excluding at-rules/nesting).  
3. **At-Rules (Medium):**  
   - Implement `@import`, `@media`, `@supports`, `@font-face`, `@keyframes`, `@layer`, `@container`. Each requires parsing conditions and nested blocks.  
   - Parse `@keyframes` inner rules (e.g. `from`, `to`, percentages).  
   - *Outcome:* Can handle full stylesheet with media queries and animations.  
4. **Functions & Expressions (Medium):**  
   - Improve expression parsing for `calc()`, `min()`, `max()`, `clamp()`, color functions (`rgb()`, `hsl()`).  
   - Decide on operator precedence grammar or Pratt parser.  
   - *Outcome:* Correct AST for arithmetic in CSS values.  
5. **CSS Nesting (Low/High):**  
   - If targeting CSS Nesting Module, allow nested rules (`.foo { color:red; & > a { ... } }`).  
   - Adjust grammar so declaration blocks can contain rules.  
   - *Outcome:* Supports nested CSS syntax.  
6. **CSS Custom Properties (Medium):**  
   - Ensure CSS variables (`--name: value`) in declaration grammar.  
   - Parse `var(--name, fallback)` in values.  
   - *Outcome:* Custom properties accepted and stored as values.  
7. **Error Handling and Recovery (Medium):**  
   - Improve error messages (track location, expected tokens).  
   - Implement resynchronization (skip to next semicolon or block) to continue parsing after errors.  
   - *Outcome:* Parser reports useful errors and recovers where possible.  
8. **Testing and Fuzzing (High ongoing):**  
   - Write unit tests for all grammar rules (property names, values, selectors, at-rules).  
   - Use CSSWG test cases (Web Platform Tests – see [39]).  
   - Integrate fuzzing or random grammar generators (e.g. [Jesse Ruderman’s fuzzer][41]) to find crashes.  
   - *Outcome:* High confidence in parser correctness and robustness.  


```mermaid
flowchart LR
    Input[/CSS Source (byte stream)/] --> Preprocess[Preprocess (charset, BOM)] --> Tokenizer
    Tokenizer -->|tokens| ComponentValues
    ComponentValues --> GrammarParser[Grammar Parser (rules, at-rules)]
    GrammarParser --> AST{AST Builder}
    AST -->|Semantic Validation| Validator
    AST -->|Emit| Output(AST)
```

**Sources:** Official specs and implementations guide these choices (the CSS Syntax Module, Blink source, Servo’s parser README), as do parsing theory sources. This report synthesizes those insights into a coherent design for a high-coverage CSS parser. 

