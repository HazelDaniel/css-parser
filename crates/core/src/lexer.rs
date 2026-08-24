use std::borrow::Cow;
use crate::errors::{LexerError, LexerErrorReason};
use crate::token::{Token, TokenKind};

struct Lexer<'a> {
    input:              &'a str,
    start:              usize,
    current:            usize,
    line:               usize,
    tokens:             Vec<Token<'a>>,

    // optimization
    last_size_memo:     Vec<usize>,
    setbacks:           usize,
}

impl <'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, start: 0, line: 1, current: 0, tokens: Vec::new(), last_size_memo: Vec::new(), setbacks: 0 }
    }

    fn is_newline(&self, cmp: char) -> bool {
        cmp == '\n' || cmp == '\r'
    }

    fn at_end(&self) -> bool {
        let x = self.input[self.current..].chars().next();
        if let Some(_) = x { return false; }

        true
    }

    pub fn scan(&mut self) -> &[Token<'a>] {
        match self.input.chars().next() {
            None => {
                self.add_token(Token::new(TokenKind::EOF, self.line, Cow::Borrowed("")));
                return &self.tokens[..];
            },
            Some(_) => {
                loop {
                    self.run();
                    self.advance();
                    if self.at_end() { break; }
                }
            }
        }
        self.add_token(Token::new(TokenKind::EOF, self.line, Cow::Borrowed("")));

        &self.tokens[..]
    }

    fn add_token(&mut self, token: Token<'a>) {
        self.tokens.push(token);
    }

    fn run(&mut self) -> Result<(), LexerError> {
        self.start = self.current;
        // invariant: all advances in this function eventually stop at the end of a valid lexeme (or errors)
        match self.peek() {
            None =>  { return Err(LexerError::new(LexerErrorReason::INVARIANT_VIOLATION, self.line, Cow::Borrowed(""))); },
            Some(cmp) => {
                match cmp {
                    x if x.is_whitespace() => { self.whitespace(true)?; },
                    ('"' | '\'') => { self.string(true)?; },
                    '{' => { self.add_token(Token::new(TokenKind::CURLY_OPEN, self.line, Cow::Borrowed("{"))); },
                    '}' => { self.add_token(Token::new(TokenKind::CURLY_CLOSE, self.line, Cow::Borrowed("}"))); },
                    '[' => { self.add_token(Token::new(TokenKind::BRACKET_OPEN, self.line, Cow::Borrowed("["))); },
                    ']' => { self.add_token(Token::new(TokenKind::BRACKET_CLOSE, self.line, Cow::Borrowed("]"))); },
                    '(' => { self.add_token(Token::new(TokenKind::PAREN_OPEN, self.line, Cow::Borrowed("("))); },
                    ')' => { self.add_token(Token::new(TokenKind::PAREN_CLOSE, self.line, Cow::Borrowed(")"))); },
                    ';' => { self.add_token(Token::new(TokenKind::SEMICOLON, self.line, Cow::Borrowed(";"))); },
                    ',' => { self.add_token(Token::new(TokenKind::COMMA, self.line, Cow::Borrowed(","))); },
                    '/' => {
                        if let Some(cmp) = self.peek_next('/') {
                            if (cmp == '*')  {
                                self.comment()?;
                                return Ok(());
                            } else {
                                self.add_token(Token::new(TokenKind::SLASH, self.line, Cow::Borrowed("/")));
                            }
                        }
                        self.add_token(Token::new(TokenKind::SLASH, self.line, Cow::Borrowed("/")));
                    },
                    '\\' => {
                        self.escape(true)?;
                    },
                    identifier => {
                        if self.input[self.current..].starts_with("url") {
                            self.current += ('u'.len_utf8() * 3);
                            self.url(true)?;
                            return Ok(());
                        }
                        // handle idents
                    }
                }
            }
        }

        Ok(())
    }

    fn advance(&mut self) {
        let mut len: usize = 0;

        if let Some(pop) = self.input[self.current..].chars().next() {
            len = pop.len_utf8();
            self.current += len;
            if self.setbacks >= 2 {
                // invariant assumption: advance will always be called before any setback-lookback combination
                // that way, the memo grows minimally and the references to last size are valid
                self.setbacks = 0;
                self.last_size_memo.push(len);
            }
        }
    }

    fn step_back(&mut self) {
        if let Some(pop) = self.last_size_memo.pop() { self.setbacks += 1; self.current -= pop; }
    }

    fn peek(&self) -> Option<char> {
        self.input[self.current..].chars().next()
    }

    fn lookback(&mut self) -> Option<char> {
        if let Some(pop) = self.last_size_memo.pop() {
            self.setbacks += 1;
            return self.input[self.current-pop..].chars().next()
        }

        None
    }

    fn catch_match(&mut self, cmp: char) -> bool {
        if self.at_end() { return false; }
        // invariant: if not at end, there should be a next char
        let pop = self.input[self.current..].chars().next().unwrap();
        if pop != cmp { return false; }
        self.advance();

        true
    }

    fn peek_next(&mut self, pop: char) -> Option<char> {
        self.input[self.current+pop.len_utf8()..].chars().next()
    }
    
    fn url(&mut self, collect: bool) -> Result<(), LexerError> {
        // if terminal != '"' && terminal != '\'' { return Err(LexerError::new(LexerErrorReason::INVARIANT_VIOLATION, self.line, Cow::Borrowed(""))); }
        Ok(())
    }
    
    fn ident(&mut self, collect: bool) -> Result<(), LexerError> {
        Ok(())
    }

    fn string(&mut self, collect: bool) -> Result<(), LexerError> {
        /*
            TODO: make this compliant with css spec:
              - An unescaped newline should terminate the string as a bad string.
              - Unterminated strings should produce a diagnostic.
        */
        if self.at_end() { return Err(LexerError::new(LexerErrorReason::INVALID_TOKEN, self.line, Cow::Borrowed(""))); }
        // invariant: if not at end, there should be a next char
        let terminal = self.peek().unwrap();
        if terminal != '"' && terminal != '\'' { return Err(LexerError::new(LexerErrorReason::INVARIANT_VIOLATION, self.line, Cow::Borrowed(""))); }

        loop {
            if self.at_end() { return Err(LexerError::new(LexerErrorReason::UNTERMINATED_TOKEN, self.line, Cow::Borrowed(""))); }
            self.advance();

            match self.peek() {
                None => {
                    return Err(LexerError::new(LexerErrorReason::UNTERMINATED_TOKEN, self.line, Cow::Borrowed("")));
                },
                Some(x) => {
                    match x {
                        c if c == terminal => {
                            if let Some(lb) = self.lookback() {
                                if lb == '\\' {
                                    continue;
                                } else {
                                    if (collect) { self.add_token(Token::new(TokenKind::STRING, self.line, Cow::Borrowed(""))); }
                                    return Ok(());
                                }
                            } else { }
                        },
                        n if self.is_newline(n) => {
                            if let Some(lb) = self.lookback() {
                                if lb == '\\' {
                                    continue;
                                } else {
                                    return Err(LexerError::new(LexerErrorReason::UNTERMINATED_TOKEN, self.line, Cow::Borrowed("")));
                                }
                            } else { }
                            self.line += 1;
                        },
                        _ => {
                            continue;
                        }
                    }
                },
            }
        }
    }

    fn escape(&mut self, collect: bool) -> Result<(), LexerError> {
        match self.peek_next('\\') {
            Some(x) => {
                if x.is_ascii_hexdigit() {
                    self.advance();
                    self.hex_token(false);
                    if (collect) { self.add_token(Token::new(TokenKind::ESCAPE_TOKEN, self.line, Cow::Borrowed(""))); }

                    return Ok(());
                } else if self.is_newline(x) {
                    if (collect) { self.add_token(Token::new(TokenKind::ESCAPE_TOKEN, self.line, Cow::Borrowed(""))); }

                    return Ok(());
                }

                Err(LexerError::new(LexerErrorReason::INVALID_TOKEN, self.line, Cow::Borrowed("")))
            },
            _ => {
                Err(LexerError::new(LexerErrorReason::INVALID_TOKEN, self.line, Cow::Borrowed("")))
            }
        }
    }

    fn hex_token(&mut self, collect: bool) -> Result<(), LexerError> {
        let mut did_match = false;
        // TODO: hex token matches at most 6 digits. apply that rule soon

        while let Some(hx) = self.peek() {
            if !hx.is_ascii_hexdigit() { break; }
            did_match = true;
            self.advance();
        }

        if did_match {
            // stepback to the end of the last matched hex token - preserving the invariant @ self.run
            if let Some(ws) = self.peek() {
                if !ws.is_whitespace() {
                    self.step_back();
                }
            } else {
                self.step_back();
            }
        }

        if (collect) { self.add_token(Token::new(TokenKind::HEX_TOKEN, self.line, Cow::Borrowed(""))); }

        Ok(())
    }

    fn comment(&mut self) -> Result<(), LexerError> {
        loop {
            if self.at_end() { return Err(LexerError::new(LexerErrorReason::UNTERMINATED_TOKEN, self.line, Cow::Borrowed(""))) }
            // invariant: if not at end, there should be a next char
            let x = self.peek().unwrap();

            if x == '*' {
                if let Some(y) = self.peek_next(x) {
                    if y == '/' {
                        self.advance();
                        return Ok(())
                    }

                    self.advance();
                    continue;
                }
                return Err(LexerError::new(LexerErrorReason::UNTERMINATED_TOKEN, self.line, Cow::Borrowed("")));
            } else if self.is_newline(x) {
                self.line += 1;
            }

            self.advance();
        }
    }

    fn whitespace(&mut self, collect: bool) -> Result<(), LexerError> {
        let mut did_match = false;
        
        while let Some(x) = self.peek() {
            if !x.is_whitespace() { break; }
            did_match = true;
            self.newline(x, false);
            self.advance();
        }

        if did_match {
            match self.peek() {
                Some(_) => {
                    // stepback to the end of the last matched whitespace - preserving the invariant @ self.run
                    self.step_back();
                }
                _ => {}
            }
            if (collect) { self.add_token(Token::new(TokenKind::WHITESPACE, self.line, Cow::Borrowed(""))); }
            return Ok(());
        }

        Ok(())
    }

    fn newline(&mut self, x: char, collect: bool) -> bool {
        let mut dual_seq = false;
        
        if x == '\r' {
            if let Some(cmp) = self.peek_next('\r') {
                if cmp == '\n' {
                    self.line += 1;
                    self.advance();
                    dual_seq = true;
                }
                if (collect) { self.add_token(Token::new(TokenKind::WHITESPACE, self.line, Cow::Borrowed(""))); return dual_seq; }
                return dual_seq;
            }
            if (collect) { self.add_token(Token::new(TokenKind::WHITESPACE, self.line, Cow::Borrowed(""))); return dual_seq; }
            return dual_seq;
        } else if x == '\n' && !dual_seq { self.line += 1; }

        if (collect) { self.add_token(Token::new(TokenKind::WHITESPACE, self.line, Cow::Borrowed(""))); }

        dual_seq
    }
}


mod utils {
}